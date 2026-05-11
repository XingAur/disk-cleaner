use disk_cleaner_tauri::cleaner::{
    build_plan_for_roots, build_plan_for_roots_with_progress, execute_cleanup,
    execute_cleanup_with_progress, parse_restore_point_count, rules_for_strength, CleanStrength,
    CleanupItem, CleanupPlan, RiskLevel, Rules,
};
use std::fs;
use std::io::Write;

fn write_file(path: &std::path::Path, bytes: usize) {
    let mut file = fs::File::create(path).expect("create test file");
    file.write_all(&vec![b'x'; bytes]).expect("write test file");
}

#[test]
fn low_risk_temp_files_are_selected_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("Temp");
    fs::create_dir_all(&temp_dir).unwrap();
    write_file(&temp_dir.join("cache.tmp"), 1024);

    let rules = Rules {
        temp_dirs: vec![temp_dir.clone()],
        enhanced_cache_dirs: Vec::new(),
        windows_old_dirs: Vec::new(),
        suggest_dirs: Vec::new(),
        exclude_dirs: Vec::new(),
        old_file_days: 30,
        large_file_bytes: 10_000,
        max_suggestion_depth: 3,
        enable_third_party_cache: false,
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].risk, RiskLevel::Low);
    assert!(plan.items[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 1024);
}

#[test]
fn large_files_are_suggestions_not_selected() {
    let dir = tempfile::tempdir().unwrap();
    let downloads = dir.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    write_file(&downloads.join("large.zip"), 2048);

    let rules = Rules {
        temp_dirs: Vec::new(),
        enhanced_cache_dirs: Vec::new(),
        windows_old_dirs: Vec::new(),
        suggest_dirs: vec![downloads],
        exclude_dirs: Vec::new(),
        old_file_days: 3650,
        large_file_bytes: 1024,
        max_suggestion_depth: 3,
        enable_third_party_cache: false,
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.suggestions.len(), 1);
    assert_eq!(plan.suggestions[0].risk, RiskLevel::Medium);
    assert!(!plan.suggestions[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 0);
}

#[test]
fn enhanced_cache_dirs_are_selected_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let browser_cache = dir.path().join("ChromeCache");
    fs::create_dir_all(&browser_cache).unwrap();
    write_file(&browser_cache.join("data_0"), 4096);

    let rules = Rules {
        temp_dirs: Vec::new(),
        enhanced_cache_dirs: vec![browser_cache],
        windows_old_dirs: Vec::new(),
        suggest_dirs: Vec::new(),
        exclude_dirs: Vec::new(),
        old_file_days: 30,
        large_file_bytes: 10_000,
        max_suggestion_depth: 3,
        enable_third_party_cache: true,
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].risk, RiskLevel::Medium);
    assert!(plan.items[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 4096);
}

#[test]
fn windows_old_files_are_selected_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let windows_old = dir.path().join("Windows.old");
    fs::create_dir_all(&windows_old).unwrap();
    write_file(&windows_old.join("setup.log"), 8192);

    let rules = Rules {
        temp_dirs: Vec::new(),
        enhanced_cache_dirs: Vec::new(),
        windows_old_dirs: vec![windows_old],
        suggest_dirs: Vec::new(),
        exclude_dirs: Vec::new(),
        old_file_days: 30,
        large_file_bytes: 10_000,
        max_suggestion_depth: 3,
        enable_third_party_cache: true,
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].category, "Windows.old");
    assert_eq!(plan.items[0].risk, RiskLevel::Medium);
    assert!(plan.items[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 8192);
}

#[test]
fn cleanup_deletes_selected_files_and_reports_failures() {
    let dir = tempfile::tempdir().unwrap();
    let doomed = dir.path().join("delete.log");
    write_file(&doomed, 512);

    let plan = CleanupPlan {
        items: vec![
            CleanupItem {
                path: doomed.clone(),
                size_bytes: 512,
                category: "测试".to_string(),
                risk: RiskLevel::Low,
                default_selected: true,
                reason: "测试清理".to_string(),
            },
            CleanupItem {
                path: dir.path().join("missing.tmp"),
                size_bytes: 128,
                category: "测试".to_string(),
                risk: RiskLevel::Low,
                default_selected: true,
                reason: "测试缺失文件".to_string(),
            },
        ],
        suggestions: Vec::new(),
        system_backups: Vec::new(),
        skipped: Vec::new(),
        reclaimable_bytes: 640,
        suggested_bytes: 0,
    };

    let report = execute_cleanup(&plan).expect("cleanup");

    assert_eq!(report.deleted_count, 1);
    assert_eq!(report.failed_count, 1);
    assert_eq!(report.freed_bytes, 512);
    assert!(!doomed.exists());
}

#[test]
fn cleanup_reports_progress_until_complete() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.tmp");
    let second = dir.path().join("second.tmp");
    write_file(&first, 128);
    write_file(&second, 256);

    let plan = CleanupPlan {
        items: vec![
            CleanupItem {
                path: first,
                size_bytes: 128,
                category: "test".to_string(),
                risk: RiskLevel::Low,
                default_selected: true,
                reason: "test cleanup".to_string(),
            },
            CleanupItem {
                path: second,
                size_bytes: 256,
                category: "test".to_string(),
                risk: RiskLevel::Low,
                default_selected: true,
                reason: "test cleanup".to_string(),
            },
        ],
        suggestions: Vec::new(),
        system_backups: Vec::new(),
        skipped: Vec::new(),
        reclaimable_bytes: 384,
        suggested_bytes: 0,
    };
    let mut percents = Vec::new();

    let report = execute_cleanup_with_progress(&plan, |progress| {
        percents.push(progress.percent);
    })
    .expect("cleanup");

    assert_eq!(report.deleted_count, 2);
    assert_eq!(percents.first().copied(), Some(0));
    assert_eq!(percents.last().copied(), Some(100));
    assert!(percents.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn strength_rules_scale_scan_scope() {
    let light = rules_for_strength(CleanStrength::Light);
    let standard = rules_for_strength(CleanStrength::Standard);
    let deep = rules_for_strength(CleanStrength::Deep);

    assert!(light.enhanced_cache_dirs.is_empty());
    assert!(light.windows_old_dirs.is_empty());
    assert!(light.suggest_dirs.is_empty());
    assert!(!standard.windows_old_dirs.is_empty());
    assert!(standard.suggest_dirs.len() <= deep.suggest_dirs.len());
    assert!(deep.large_file_bytes < standard.large_file_bytes);
}

#[test]
fn restore_point_parser_counts_english_and_chinese_output() {
    let english = r#"
Shadow Copy ID: {11111111-1111-1111-1111-111111111111}
Shadow Copy ID: {22222222-2222-2222-2222-222222222222}
"#;
    let chinese = r#"
卷影复制 ID: {aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa}
卷影复制 ID: {bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb}
"#;

    assert_eq!(parse_restore_point_count(english), 2);
    assert_eq!(parse_restore_point_count(chinese), 2);
}

#[test]
fn suggestion_scan_uses_fast_depth_limit() {
    let dir = tempfile::tempdir().unwrap();
    let downloads = dir.path().join("Downloads");
    let deep = downloads.join("a").join("b").join("c").join("d");
    fs::create_dir_all(&deep).unwrap();
    write_file(&deep.join("too-deep.zip"), 2048);

    let rules = Rules {
        temp_dirs: Vec::new(),
        enhanced_cache_dirs: Vec::new(),
        windows_old_dirs: Vec::new(),
        suggest_dirs: vec![downloads],
        exclude_dirs: Vec::new(),
        old_file_days: 3650,
        large_file_bytes: 1024,
        max_suggestion_depth: 3,
        enable_third_party_cache: true,
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert!(plan.suggestions.is_empty());
}

#[test]
fn scan_reports_progress_until_complete() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("Temp");
    let downloads = dir.path().join("Downloads");
    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(&downloads).unwrap();
    write_file(&temp_dir.join("cache.tmp"), 1024);
    write_file(&downloads.join("large.zip"), 2048);

    let rules = Rules {
        temp_dirs: vec![temp_dir],
        enhanced_cache_dirs: Vec::new(),
        windows_old_dirs: Vec::new(),
        suggest_dirs: vec![downloads],
        exclude_dirs: Vec::new(),
        old_file_days: 3650,
        large_file_bytes: 1024,
        max_suggestion_depth: 3,
        enable_third_party_cache: true,
    };
    let mut percents = Vec::new();

    let plan = build_plan_for_roots_with_progress(&rules, |progress| {
        percents.push(progress.percent);
    })
    .expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.suggestions.len(), 1);
    assert!(percents.len() >= 3);
    assert_eq!(percents.first().copied(), Some(0));
    assert_eq!(percents.last().copied(), Some(100));
    assert!(percents.windows(2).all(|pair| pair[0] <= pair[1]));
}
