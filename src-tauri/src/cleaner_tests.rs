use crate::cleaner::{
    build_plan_for_roots, build_plan_for_roots_with_progress, execute_cleanup,
    execute_cleanup_with_progress, execute_cleanup_with_recycle_bin_hooks,
    parse_restore_point_count, recycle_bin_roots_for_drive_roots, rules_for_strength,
    selected_items_from_plan, system_drive_space, CleanStrength, CleanupItem, CleanupPlan,
    RiskLevel, Rules,
};
use std::fs;
use std::io::Write;

fn write_file(path: &std::path::Path, bytes: usize) {
    let mut file = fs::File::create(path).expect("create test file");
    file.write_all(&vec![b'x'; bytes]).expect("write test file");
}

fn empty_rules() -> Rules {
    Rules {
        temp_dirs: Vec::new(),
        enhanced_cache_dirs: Vec::new(),
        windows_old_dirs: Vec::new(),
        recycle_bin_dirs: Vec::new(),
        system_cache_dirs: Vec::new(),
        suggest_dirs: Vec::new(),
        exclude_dirs: Vec::new(),
        old_file_days: 30,
        large_file_bytes: 10_000,
        max_suggestion_depth: 3,
        enable_third_party_cache: false,
    }
}

#[test]
fn low_risk_temp_files_are_selected_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("Temp");
    fs::create_dir_all(&temp_dir).unwrap();
    write_file(&temp_dir.join("cache.tmp"), 1024);

    let rules = Rules {
        temp_dirs: vec![temp_dir.clone()],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].risk, RiskLevel::Low);
    assert!(plan.items[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 1024);
}

#[test]
fn duplicate_scan_roots_do_not_duplicate_cleanup_items() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("Temp");
    fs::create_dir_all(&temp_dir).unwrap();
    write_file(&temp_dir.join("cache.tmp"), 1024);

    let rules = Rules {
        temp_dirs: vec![temp_dir.clone(), temp_dir],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.reclaimable_bytes, 1024);
}

#[test]
fn runtime_driver_files_are_skipped_from_default_temp_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("Temp");
    fs::create_dir_all(&temp_dir).unwrap();
    write_file(&temp_dir.join("active.sys"), 1024);

    let rules = Rules {
        temp_dirs: vec![temp_dir],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert!(plan.items.is_empty());
    assert_eq!(plan.reclaimable_bytes, 0);
    assert!(plan
        .skipped
        .iter()
        .any(|entry| entry.contains("active.sys")));
}

#[test]
fn large_files_are_suggestions_not_selected() {
    let dir = tempfile::tempdir().unwrap();
    let downloads = dir.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    write_file(&downloads.join("large.zip"), 2048);

    let rules = Rules {
        suggest_dirs: vec![downloads],
        old_file_days: 3650,
        large_file_bytes: 1024,
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.suggestions.len(), 1);
    assert_eq!(plan.suggestions[0].risk, RiskLevel::Medium);
    assert!(!plan.suggestions[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 0);
}

#[test]
fn downloads_scan_includes_recent_small_files_and_directories() {
    let dir = tempfile::tempdir().unwrap();
    let downloads = dir.path().join("Downloads");
    let downloaded_folder = downloads.join("bank-statement");
    fs::create_dir_all(&downloaded_folder).unwrap();
    write_file(&downloads.join("recent.png"), 512);
    write_file(&downloaded_folder.join("statement.csv"), 1024);

    let rules = Rules {
        suggest_dirs: vec![downloads],
        old_file_days: 3650,
        large_file_bytes: 10_000,
        max_suggestion_depth: 3,
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.suggestions.len(), 2);
    assert!(plan
        .suggestions
        .iter()
        .any(|item| item.path.ends_with("recent.png")));
    assert!(plan
        .suggestions
        .iter()
        .any(|item| item.path.ends_with("bank-statement")));
    assert_eq!(plan.suggested_bytes, 1536);
    assert!(plan.suggestions.iter().all(|item| !item.default_selected));
}

#[test]
fn enhanced_cache_dirs_are_selected_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let browser_cache = dir.path().join("ChromeCache");
    fs::create_dir_all(&browser_cache).unwrap();
    write_file(&browser_cache.join("data_0"), 4096);

    let rules = Rules {
        enhanced_cache_dirs: vec![browser_cache],
        enable_third_party_cache: true,
        ..empty_rules()
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
        windows_old_dirs: vec![windows_old],
        enable_third_party_cache: true,
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].category, "Windows.old");
    assert_eq!(plan.items[0].risk, RiskLevel::Medium);
    assert!(plan.items[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 8192);
}

#[test]
fn recycle_bin_files_are_counted_as_cleanup_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let recycle_bin = dir.path().join("$Recycle.Bin");
    fs::create_dir_all(&recycle_bin).unwrap();
    write_file(&recycle_bin.join("$R123.tmp"), 2048);

    let rules = Rules {
        recycle_bin_dirs: vec![recycle_bin],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].category, "Recycle Bin");
    assert_eq!(plan.items[0].risk, RiskLevel::Medium);
    assert!(!plan.items[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 2048);
}

#[test]
fn recycle_bin_roots_are_created_for_every_drive_root() {
    let roots = recycle_bin_roots_for_drive_roots(&[
        std::path::PathBuf::from("C:\\"),
        std::path::PathBuf::from("D:\\"),
    ]);

    assert_eq!(
        roots,
        vec![
            std::path::PathBuf::from("C:\\$Recycle.Bin"),
            std::path::PathBuf::from("D:\\$Recycle.Bin"),
        ]
    );
}

#[test]
fn recycle_bin_directories_create_cleanup_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let recycle_bin = dir.path().join("$Recycle.Bin");
    let deleted_folder = recycle_bin.join("S-1-5-21-test").join("$RDELETED");
    fs::create_dir_all(&deleted_folder).unwrap();
    write_file(&deleted_folder.join("nested.txt"), 2048);

    let rules = Rules {
        recycle_bin_dirs: vec![recycle_bin.clone()],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].category, "Recycle Bin");
    assert_eq!(plan.items[0].path, recycle_bin);
    assert_eq!(plan.items[0].size_bytes, 2048);
    assert!(!plan.items[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 2048);
}

#[test]
fn recycle_bin_metadata_only_is_not_counted() {
    let dir = tempfile::tempdir().unwrap();
    let recycle_bin = dir.path().join("$Recycle.Bin");
    let sid_dir = recycle_bin.join("S-1-5-21-test");
    fs::create_dir_all(&sid_dir).unwrap();
    write_file(&sid_dir.join("desktop.ini"), 256);

    let rules = Rules {
        recycle_bin_dirs: vec![recycle_bin],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert!(plan.items.is_empty());
    assert_eq!(plan.reclaimable_bytes, 0);
}

#[test]
fn recycle_bin_orphan_shell_metadata_is_not_counted() {
    let dir = tempfile::tempdir().unwrap();
    let recycle_bin = dir.path().join("$Recycle.Bin");
    let sid_dir = recycle_bin.join("S-1-5-21-test");
    let empty_deleted_folder = sid_dir.join("$REMPTY");
    fs::create_dir_all(&empty_deleted_folder).unwrap();
    write_file(&sid_dir.join("$IABCDE.exe"), 170);
    write_file(&sid_dir.join("$IIMAGE.png"), 130);

    let rules = Rules {
        recycle_bin_dirs: vec![recycle_bin],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert!(plan.items.is_empty());
    assert_eq!(plan.reclaimable_bytes, 0);
}

#[test]
fn recycle_bin_scan_handles_unicode_names_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let recycle_bin = dir.path().join("$Recycle.Bin");
    let sid_dir = recycle_bin.join("S-1-5-21-test");
    fs::create_dir_all(&sid_dir).unwrap();
    write_file(&sid_dir.join("测试文件.tmp"), 512);

    let rules = Rules {
        recycle_bin_dirs: vec![recycle_bin],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].category, "Recycle Bin");
    assert_eq!(plan.reclaimable_bytes, 512);
}

#[test]
fn cleanup_empties_recycle_bin_once_instead_of_deleting_raw_items() {
    let dir = tempfile::tempdir().unwrap();
    let recycle_file = dir.path().join("$Recycle.Bin").join("$R123.tmp");
    let cache_file = dir.path().join("cache.tmp");
    fs::create_dir_all(recycle_file.parent().unwrap()).unwrap();
    write_file(&recycle_file, 1024);
    write_file(&cache_file, 512);

    let plan = CleanupPlan {
        items: vec![
            CleanupItem {
                id: "recycle".to_string(),
                path: recycle_file.clone(),
                size_bytes: 1024,
                category: "Recycle Bin".to_string(),
                risk: RiskLevel::Medium,
                default_selected: true,
                reason: "test recycle bin cleanup".to_string(),
            },
            CleanupItem {
                id: "cache".to_string(),
                path: cache_file.clone(),
                size_bytes: 512,
                category: "Low risk cache".to_string(),
                risk: RiskLevel::Low,
                default_selected: true,
                reason: "test cache cleanup".to_string(),
            },
        ],
        suggestions: Vec::new(),
        system_backups: Vec::new(),
        skipped: Vec::new(),
        reclaimable_bytes: 1536,
        suggested_bytes: 0,
    };
    let mut recycle_empty_calls = 0;

    let report = execute_cleanup_with_recycle_bin_hooks(
        &plan,
        |_| {},
        || {
            recycle_empty_calls += 1;
            Ok(())
        },
        || 1024,
    )
    .expect("cleanup");

    assert_eq!(recycle_empty_calls, 1);
    assert!(
        recycle_file.exists(),
        "test hook should replace raw deletion"
    );
    assert!(!cache_file.exists());
    assert_eq!(report.deleted_count, 2);
    assert_eq!(report.freed_bytes, 1536);
    assert_eq!(report.failed_count, 0);
}

#[cfg(target_os = "windows")]
#[test]
fn system_drive_space_reports_total_and_free_bytes() {
    let space = system_drive_space().expect("system drive space");

    assert!(space.total_bytes > 0);
    assert!(space.free_bytes > 0);
    assert!(space.free_bytes <= space.total_bytes);
    assert!(space.available_bytes <= space.total_bytes);
}

#[test]
fn system_cache_files_are_counted_as_cleanup_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let update_cache = dir.path().join("SoftwareDistribution").join("Download");
    fs::create_dir_all(&update_cache).unwrap();
    write_file(&update_cache.join("update.cab"), 4096);

    let rules = Rules {
        system_cache_dirs: vec![update_cache],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].category, "System update cache");
    assert_eq!(plan.items[0].risk, RiskLevel::Medium);
    assert!(!plan.items[0].default_selected);
    assert_eq!(plan.reclaimable_bytes, 4096);
}

#[test]
fn cleanup_deletes_selected_files_and_reports_failures() {
    let dir = tempfile::tempdir().unwrap();
    let doomed = dir.path().join("delete.log");
    write_file(&doomed, 512);

    let plan = CleanupPlan {
        items: vec![
            CleanupItem {
                id: "doomed".to_string(),
                path: doomed.clone(),
                size_bytes: 512,
                category: "测试".to_string(),
                risk: RiskLevel::Low,
                default_selected: true,
                reason: "测试清理".to_string(),
            },
            CleanupItem {
                id: "missing".to_string(),
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
    assert_eq!(report.failed_count, 0);
    assert_eq!(report.skipped_count, 1);
    assert_eq!(report.freed_bytes, 512);
    assert!(!doomed.exists());
}

#[test]
fn cleanup_does_not_write_log_file_by_default() {
    let report = execute_cleanup(&CleanupPlan::default()).expect("cleanup");

    assert!(
        report.log_path.is_none(),
        "cleanup should not create logs beside the portable exe"
    );
}

#[test]
fn cleanup_does_not_report_stale_recycle_bytes_when_live_recycle_bin_is_empty() {
    let plan = CleanupPlan {
        items: vec![CleanupItem {
            id: "stale-recycle".to_string(),
            path: std::path::PathBuf::from("D:\\$Recycle.Bin"),
            size_bytes: 4096,
            category: "Recycle Bin".to_string(),
            risk: RiskLevel::Medium,
            default_selected: true,
            reason: "stale scan".to_string(),
        }],
        suggestions: Vec::new(),
        system_backups: Vec::new(),
        skipped: Vec::new(),
        reclaimable_bytes: 4096,
        suggested_bytes: 0,
    };
    let mut recycle_empty_calls = 0;

    let report = execute_cleanup_with_recycle_bin_hooks(
        &plan,
        |_| {},
        || {
            recycle_empty_calls += 1;
            Ok(())
        },
        || 0,
    )
    .expect("cleanup");

    assert_eq!(recycle_empty_calls, 0);
    assert_eq!(report.freed_bytes, 0);
    assert_eq!(report.deleted_count, 0);
    assert_eq!(report.skipped_count, 1);
    assert_eq!(report.failed_count, 0);
}

#[test]
fn cleanup_clears_readonly_attribute_before_deleting_file() {
    let dir = tempfile::tempdir().unwrap();
    let readonly = dir.path().join("readonly.tmp");
    write_file(&readonly, 256);
    let mut permissions = fs::metadata(&readonly).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&readonly, permissions).unwrap();

    let plan = CleanupPlan {
        items: vec![CleanupItem {
            id: "readonly".to_string(),
            path: readonly.clone(),
            size_bytes: 256,
            category: "test".to_string(),
            risk: RiskLevel::Low,
            default_selected: true,
            reason: "test cleanup".to_string(),
        }],
        suggestions: Vec::new(),
        system_backups: Vec::new(),
        skipped: Vec::new(),
        reclaimable_bytes: 256,
        suggested_bytes: 0,
    };

    let report = execute_cleanup(&plan).expect("cleanup");

    if readonly.exists() {
        let mut permissions = fs::metadata(&readonly).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&readonly, permissions).unwrap();
    }
    assert_eq!(report.deleted_count, 1);
    assert_eq!(report.failed_count, 0);
    assert!(!readonly.exists());
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
                id: "first".to_string(),
                path: first,
                size_bytes: 128,
                category: "test".to_string(),
                risk: RiskLevel::Low,
                default_selected: true,
                reason: "test cleanup".to_string(),
            },
            CleanupItem {
                id: "second".to_string(),
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
    let documents = dir.path().join("Documents");
    let deep = documents.join("a").join("b").join("c").join("d");
    fs::create_dir_all(&deep).unwrap();
    write_file(&deep.join("too-deep.zip"), 2048);

    let rules = Rules {
        suggest_dirs: vec![documents],
        old_file_days: 3650,
        large_file_bytes: 1024,
        max_suggestion_depth: 3,
        enable_third_party_cache: true,
        ..empty_rules()
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
        suggest_dirs: vec![downloads],
        old_file_days: 3650,
        large_file_bytes: 1024,
        enable_third_party_cache: true,
        ..empty_rules()
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

#[test]
fn scanned_cleanup_items_have_stable_ids() {
    let dir = tempfile::tempdir().unwrap();
    let temp_dir = dir.path().join("Temp");
    fs::create_dir_all(&temp_dir).unwrap();
    write_file(&temp_dir.join("cache.tmp"), 1024);

    let rules = Rules {
        temp_dirs: vec![temp_dir],
        ..empty_rules()
    };

    let plan = build_plan_for_roots(&rules).expect("build plan");

    assert_eq!(plan.items.len(), 1);
    assert!(!plan.items[0].id.is_empty());
}

#[test]
fn selected_items_from_plan_resolves_items_and_suggestions_by_id() {
    let selected_file = CleanupItem {
        id: "cache-1".to_string(),
        path: std::path::PathBuf::from("cache.tmp"),
        size_bytes: 512,
        category: "Low risk cache".to_string(),
        risk: RiskLevel::Low,
        default_selected: false,
        reason: "test cleanup".to_string(),
    };
    let suggested_file = CleanupItem {
        id: "large-1".to_string(),
        path: std::path::PathBuf::from("large.zip"),
        size_bytes: 2048,
        category: "Large file suggestion".to_string(),
        risk: RiskLevel::Medium,
        default_selected: false,
        reason: "review manually".to_string(),
    };
    let plan = CleanupPlan {
        items: vec![selected_file],
        suggestions: vec![suggested_file],
        system_backups: Vec::new(),
        skipped: Vec::new(),
        reclaimable_bytes: 512,
        suggested_bytes: 2048,
    };

    let selected = selected_items_from_plan(&plan, &["large-1".to_string(), "cache-1".to_string()])
        .expect("resolve selected items");

    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].id, "large-1");
    assert_eq!(selected[1].id, "cache-1");
    assert!(selected.iter().all(|item| item.default_selected));
}

#[test]
fn selected_items_from_plan_rejects_unknown_ids() {
    let plan = CleanupPlan {
        items: vec![CleanupItem {
            id: "known".to_string(),
            path: std::path::PathBuf::from("cache.tmp"),
            size_bytes: 512,
            category: "Low risk cache".to_string(),
            risk: RiskLevel::Low,
            default_selected: false,
            reason: "test cleanup".to_string(),
        }],
        suggestions: Vec::new(),
        system_backups: Vec::new(),
        skipped: Vec::new(),
        reclaimable_bytes: 512,
        suggested_bytes: 0,
    };

    let error = selected_items_from_plan(&plan, &["unknown".to_string()])
        .expect_err("unknown ids should be rejected");

    assert!(error.contains("unknown"));
}
