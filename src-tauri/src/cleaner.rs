use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    env, fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};
use walkdir::WalkDir;

const RECYCLE_BIN_CATEGORY: &str = "Recycle Bin";
const DOWNLOAD_ITEM_CATEGORY: &str = "Download item suggestion";

#[cfg(target_os = "windows")]
const SHERB_NOCONFIRMATION: u32 = 0x00000001;
#[cfg(target_os = "windows")]
const SHERB_NOPROGRESSUI: u32 = 0x00000002;
#[cfg(target_os = "windows")]
const SHERB_NOSOUND: u32 = 0x00000004;

#[cfg(target_os = "windows")]
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn GetLogicalDrives() -> u32;

    fn GetDiskFreeSpaceExW(
        lpDirectoryName: *const u16,
        lpFreeBytesAvailableToCaller: *mut u64,
        lpTotalNumberOfBytes: *mut u64,
        lpTotalNumberOfFreeBytes: *mut u64,
    ) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "Shell32")]
unsafe extern "system" {
    fn SHEmptyRecycleBinW(
        hwnd: *mut std::ffi::c_void,
        pszRootPath: *const u16,
        dwFlags: u32,
    ) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanStrength {
    Light,
    Standard,
    Deep,
}

impl Default for CleanStrength {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    pub strength: CleanStrength,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    #[serde(default)]
    pub id: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub category: String,
    pub risk: RiskLevel,
    pub default_selected: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub items: Vec<CleanupItem>,
    pub suggestions: Vec<CleanupItem>,
    #[serde(default)]
    pub system_backups: Vec<CleanupItem>,
    pub skipped: Vec<String>,
    pub reclaimable_bytes: u64,
    pub suggested_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub system_drive: String,
    #[serde(default)]
    pub drive_space: Option<DriveSpace>,
    pub plan: CleanupPlan,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveSpace {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub freed_bytes: u64,
    pub deleted_count: usize,
    #[serde(default)]
    pub skipped_count: usize,
    #[serde(default)]
    pub locked_count: usize,
    #[serde(default)]
    pub permission_failed_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
    pub log_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub percent: u8,
    pub phase: String,
    pub current_path: String,
    pub scanned_files: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupProgress {
    pub percent: u8,
    pub phase: String,
    pub current_path: String,
    pub processed_items: u64,
    pub total_items: u64,
}

#[derive(Debug, Clone)]
pub struct Rules {
    pub temp_dirs: Vec<PathBuf>,
    pub enhanced_cache_dirs: Vec<PathBuf>,
    pub windows_old_dirs: Vec<PathBuf>,
    pub recycle_bin_dirs: Vec<PathBuf>,
    pub system_cache_dirs: Vec<PathBuf>,
    pub suggest_dirs: Vec<PathBuf>,
    pub exclude_dirs: Vec<PathBuf>,
    pub old_file_days: u64,
    pub large_file_bytes: u64,
    pub max_suggestion_depth: usize,
    pub enable_third_party_cache: bool,
}

pub fn default_rules() -> Rules {
    rules_for_strength(CleanStrength::Standard)
}

pub fn rules_for_strength(strength: CleanStrength) -> Rules {
    let system_drive = system_drive_root();
    let local_app_data = env_path("LOCALAPPDATA");
    let app_data = env_path("APPDATA");
    let user_profile = env_path("USERPROFILE");
    let program_data = env_path("ProgramData");

    let mut temp_dirs = Vec::new();
    if let Some(temp) = env_path("TEMP") {
        temp_dirs.push(temp);
    }
    push_join(&mut temp_dirs, &local_app_data, "Temp");
    temp_dirs.push(system_drive.join("Windows").join("Temp"));
    push_join(&mut temp_dirs, &local_app_data, "CrashDumps");
    push_join(
        &mut temp_dirs,
        &local_app_data,
        "Microsoft\\Windows\\Explorer",
    );

    let mut enhanced_cache_dirs = Vec::new();
    push_join(
        &mut enhanced_cache_dirs,
        &local_app_data,
        "Google\\Chrome\\User Data\\Default\\Cache",
    );
    push_join(
        &mut enhanced_cache_dirs,
        &local_app_data,
        "Google\\Chrome\\User Data\\Default\\Code Cache",
    );
    push_join(
        &mut enhanced_cache_dirs,
        &local_app_data,
        "Microsoft\\Edge\\User Data\\Default\\Cache",
    );
    push_join(
        &mut enhanced_cache_dirs,
        &local_app_data,
        "Microsoft\\Edge\\User Data\\Default\\Code Cache",
    );
    push_join(&mut enhanced_cache_dirs, &app_data, "Code\\Cache");
    push_join(&mut enhanced_cache_dirs, &app_data, "Code\\CachedData");
    push_join(&mut enhanced_cache_dirs, &app_data, "Code\\GPUCache");
    push_join(&mut enhanced_cache_dirs, &app_data, "npm-cache");
    push_join(&mut enhanced_cache_dirs, &local_app_data, "pnpm-cache");
    push_join(&mut enhanced_cache_dirs, &local_app_data, "Yarn\\Cache");
    push_join(
        &mut enhanced_cache_dirs,
        &app_data,
        "Tencent\\WeChat\\Cache",
    );
    push_join(
        &mut enhanced_cache_dirs,
        &app_data,
        "Tencent\\WeChat\\XPlugin\\Cache",
    );
    push_join(&mut enhanced_cache_dirs, &app_data, "Tencent\\QQ\\Temp");
    push_join(
        &mut enhanced_cache_dirs,
        &app_data,
        "Tencent\\WeMeet\\cache",
    );
    push_join(
        &mut enhanced_cache_dirs,
        &app_data,
        "Kingsoft\\office6\\cache",
    );
    push_join(
        &mut enhanced_cache_dirs,
        &local_app_data,
        "JetBrains\\Transient",
    );

    let windows_old_dirs = vec![system_drive.join("Windows.old")];
    let recycle_bin_dirs = recycle_bin_roots_for_drive_roots(&drive_roots());

    let mut system_cache_dirs = vec![system_drive
        .join("Windows")
        .join("SoftwareDistribution")
        .join("Download")];
    push_join(
        &mut system_cache_dirs,
        &program_data,
        "Microsoft\\Windows\\WER\\ReportArchive",
    );
    push_join(
        &mut system_cache_dirs,
        &program_data,
        "Microsoft\\Windows\\WER\\ReportQueue",
    );
    push_join(
        &mut system_cache_dirs,
        &local_app_data,
        "Microsoft\\Windows\\INetCache",
    );
    push_join(&mut system_cache_dirs, &local_app_data, "D3DSCache");

    let mut suggest_dirs = Vec::new();
    push_join(&mut suggest_dirs, &user_profile, "Downloads");
    push_join(&mut suggest_dirs, &user_profile, "Desktop");
    push_join(&mut suggest_dirs, &user_profile, "Documents");
    push_join(&mut suggest_dirs, &user_profile, "Pictures");
    push_join(&mut suggest_dirs, &user_profile, "Videos");

    let exclude_dirs = vec![
        system_drive.join("Windows").join("System32"),
        system_drive.join("Windows").join("WinSxS"),
        system_drive.join("Program Files"),
        system_drive.join("Program Files (x86)"),
    ];

    Rules {
        temp_dirs,
        enhanced_cache_dirs: match strength {
            CleanStrength::Light => Vec::new(),
            CleanStrength::Standard | CleanStrength::Deep => enhanced_cache_dirs,
        },
        windows_old_dirs: match strength {
            CleanStrength::Light => Vec::new(),
            CleanStrength::Standard | CleanStrength::Deep => windows_old_dirs,
        },
        recycle_bin_dirs: match strength {
            CleanStrength::Light => Vec::new(),
            CleanStrength::Standard | CleanStrength::Deep => recycle_bin_dirs,
        },
        system_cache_dirs: match strength {
            CleanStrength::Light => Vec::new(),
            CleanStrength::Standard | CleanStrength::Deep => system_cache_dirs,
        },
        suggest_dirs: match strength {
            CleanStrength::Light => Vec::new(),
            CleanStrength::Standard => suggest_dirs
                .into_iter()
                .filter(|path| path.ends_with("Downloads"))
                .collect(),
            CleanStrength::Deep => suggest_dirs,
        },
        exclude_dirs,
        old_file_days: match strength {
            CleanStrength::Light => 60,
            CleanStrength::Standard => 30,
            CleanStrength::Deep => 14,
        },
        large_file_bytes: match strength {
            CleanStrength::Light => 1024 * 1024 * 1024,
            CleanStrength::Standard => 500 * 1024 * 1024,
            CleanStrength::Deep => 200 * 1024 * 1024,
        },
        max_suggestion_depth: match strength {
            CleanStrength::Light => 2,
            CleanStrength::Standard => 3,
            CleanStrength::Deep => 4,
        },
        enable_third_party_cache: !matches!(strength, CleanStrength::Light),
    }
}

pub fn scan_system_drive() -> io::Result<ScanResult> {
    scan_system_drive_with_progress(|_| {})
}

pub fn scan_system_drive_with_progress<F>(on_progress: F) -> io::Result<ScanResult>
where
    F: FnMut(ScanProgress),
{
    scan_system_drive_with_options_and_progress(ScanOptions::default(), on_progress)
}

pub fn scan_system_drive_with_options_and_progress<F>(
    options: ScanOptions,
    on_progress: F,
) -> io::Result<ScanResult>
where
    F: FnMut(ScanProgress),
{
    let rules = rules_for_strength(options.strength);
    let mut plan = build_plan_for_roots_with_progress(&rules, on_progress)?;
    append_system_backups(&mut plan);
    Ok(ScanResult {
        system_drive: system_drive_root().display().to_string(),
        drive_space: system_drive_space().ok(),
        plan,
    })
}

pub fn build_plan_for_roots(rules: &Rules) -> io::Result<CleanupPlan> {
    build_plan_for_roots_with_progress(rules, |_| {})
}

pub fn build_plan_for_roots_with_progress<F>(
    rules: &Rules,
    mut on_progress: F,
) -> io::Result<CleanupPlan>
where
    F: FnMut(ScanProgress),
{
    let mut plan = CleanupPlan::default();
    let temp_dirs = dedupe_paths(&rules.temp_dirs);
    let enhanced_cache_dirs = if rules.enable_third_party_cache {
        dedupe_paths(&rules.enhanced_cache_dirs)
    } else {
        Vec::new()
    };
    let windows_old_dirs = dedupe_paths(&rules.windows_old_dirs);
    let recycle_bin_dirs = dedupe_paths(&rules.recycle_bin_dirs);
    let system_cache_dirs = dedupe_paths(&rules.system_cache_dirs);
    let suggest_dirs = dedupe_paths(&rules.suggest_dirs);
    let total_roots = temp_dirs.len()
        + enhanced_cache_dirs.len()
        + windows_old_dirs.len()
        + recycle_bin_dirs.len()
        + system_cache_dirs.len()
        + suggest_dirs.len();
    let mut state = ScanState::new(total_roots);

    state.emit("Preparing scan", "", &mut on_progress);

    for root in &temp_dirs {
        scan_default_dir(
            root,
            rules,
            &mut plan,
            &mut state,
            &mut on_progress,
            "Low risk cache",
            RiskLevel::Low,
            true,
            "Temporary or system cache file. Permanent deletion requires confirmation.",
        );
    }

    for root in &enhanced_cache_dirs {
        scan_default_dir(
            root,
            rules,
            &mut plan,
            &mut state,
            &mut on_progress,
            "Enhanced cache",
            RiskLevel::Medium,
            true,
            "Third-party cache selected by enhanced mode. Permanent deletion requires confirmation.",
        );
    }

    for root in &windows_old_dirs {
        scan_default_dir(
            root,
            rules,
            &mut plan,
            &mut state,
            &mut on_progress,
            "Windows.old",
            RiskLevel::Medium,
            true,
            "Previous Windows installation files. Clean only after confirming rollback is not needed.",
        );
    }

    for root in &recycle_bin_dirs {
        scan_recycle_bin_dir(root, rules, &mut plan, &mut state, &mut on_progress);
    }

    for root in &system_cache_dirs {
        scan_default_dir(
            root,
            rules,
            &mut plan,
            &mut state,
            &mut on_progress,
            "System update cache",
            RiskLevel::Medium,
            false,
            "Windows update, error report, or shader cache file. Permanent deletion requires confirmation.",
        );
    }

    for root in &suggest_dirs {
        scan_suggestion_dir(root, rules, &mut plan, &mut state, &mut on_progress);
    }

    state.complete(&mut on_progress);
    Ok(plan)
}

pub fn execute_cleanup(plan: &CleanupPlan) -> io::Result<CleanupReport> {
    execute_cleanup_with_progress(plan, |_| {})
}

pub fn selected_items_from_plan(
    plan: &CleanupPlan,
    item_ids: &[String],
) -> Result<Vec<CleanupItem>, String> {
    let mut selected = Vec::with_capacity(item_ids.len());

    for id in item_ids {
        if id.trim().is_empty() {
            return Err("cleanup item id cannot be empty".to_string());
        }

        let Some(item) = plan
            .items
            .iter()
            .chain(plan.suggestions.iter())
            .find(|item| item.id == *id)
        else {
            return Err(format!("unknown cleanup item id: {id}"));
        };

        let mut selected_item = item.clone();
        selected_item.default_selected = true;
        selected.push(selected_item);
    }

    Ok(selected)
}

pub fn execute_cleanup_with_progress<F>(
    plan: &CleanupPlan,
    on_progress: F,
) -> io::Result<CleanupReport>
where
    F: FnMut(CleanupProgress),
{
    execute_cleanup_with_recycle_bin_emptier(plan, on_progress, empty_recycle_bin)
}

pub(crate) fn execute_cleanup_with_recycle_bin_emptier<F, E>(
    plan: &CleanupPlan,
    on_progress: F,
    empty_recycle_bin: E,
) -> io::Result<CleanupReport>
where
    F: FnMut(CleanupProgress),
    E: FnMut() -> io::Result<()>,
{
    execute_cleanup_with_recycle_bin_hooks(
        plan,
        on_progress,
        empty_recycle_bin,
        live_recycle_bin_content_size,
    )
}

pub(crate) fn execute_cleanup_with_recycle_bin_hooks<F, E, S>(
    plan: &CleanupPlan,
    mut on_progress: F,
    mut empty_recycle_bin: E,
    recycle_bin_content_size: S,
) -> io::Result<CleanupReport>
where
    F: FnMut(CleanupProgress),
    E: FnMut() -> io::Result<()>,
    S: Fn() -> u64,
{
    let mut report = CleanupReport::default();
    let total_items = plan
        .items
        .iter()
        .filter(|item| item.default_selected)
        .count() as u64;
    let mut processed_items = 0_u64;

    emit_cleanup_progress(
        0,
        "Preparing cleanup",
        "",
        processed_items,
        total_items,
        &mut on_progress,
    );

    let recycle_items: Vec<&CleanupItem> = plan
        .items
        .iter()
        .filter(|item| item.default_selected && is_recycle_bin_item(item))
        .collect();

    if !recycle_items.is_empty() {
        emit_cleanup_progress(
            cleanup_percent(processed_items, total_items),
            "Emptying recycle bin",
            "All Recycle Bins",
            processed_items,
            total_items,
            &mut on_progress,
        );

        let recycle_count = recycle_items.len();
        let live_recycle_bytes = recycle_bin_content_size();
        let outcome_phase = if live_recycle_bytes == 0 {
            report.skipped_count += recycle_count;
            report.errors.push(
                "All Recycle Bins - skipped: Windows Recycle Bin is already empty".to_string(),
            );
            "Skipped"
        } else {
            match empty_recycle_bin() {
                Ok(()) => {
                    report.deleted_count += recycle_count;
                    report.freed_bytes += live_recycle_bytes;
                    "Deleted"
                }
                Err(error) => {
                    report.failed_count += recycle_count;
                    if error.kind() == io::ErrorKind::PermissionDenied {
                        report.permission_failed_count += recycle_count;
                    }
                    report.errors.push(format!(
                        "All Recycle Bins - failed to empty Windows Recycle Bin ({error})"
                    ));
                    "Failed"
                }
            }
        };

        processed_items += recycle_count as u64;
        emit_cleanup_progress(
            cleanup_percent(processed_items, total_items),
            outcome_phase,
            "All Recycle Bins",
            processed_items,
            total_items,
            &mut on_progress,
        );
    }

    for item in &plan.items {
        if !item.default_selected || is_recycle_bin_item(item) {
            continue;
        }

        emit_cleanup_progress(
            cleanup_percent(processed_items, total_items),
            "Deleting",
            &item.path.display().to_string(),
            processed_items,
            total_items,
            &mut on_progress,
        );

        if !item.path.exists() {
            report.skipped_count += 1;
            report.errors.push(format!(
                "{} - skipped: file no longer exists",
                item.path.display()
            ));
            processed_items += 1;
            emit_cleanup_progress(
                cleanup_percent(processed_items, total_items),
                "Skipped",
                &item.path.display().to_string(),
                processed_items,
                total_items,
                &mut on_progress,
            );
            continue;
        }

        let outcome_phase = match delete_path(&item.path) {
            Ok(()) => {
                report.deleted_count += 1;
                report.freed_bytes += item.size_bytes;
                "Deleted"
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                report.skipped_count += 1;
                report.errors.push(format!(
                    "{} - skipped: file no longer exists",
                    item.path.display()
                ));
                "Skipped"
            }
            Err(error) if is_locked_error(&error) => {
                report.skipped_count += 1;
                report.locked_count += 1;
                report.errors.push(format!(
                    "{} - skipped: file is currently in use ({error})",
                    item.path.display()
                ));
                "Skipped"
            }
            Err(error) => {
                report.failed_count += 1;
                if error.kind() == io::ErrorKind::PermissionDenied {
                    report.permission_failed_count += 1;
                }
                report
                    .errors
                    .push(format!("{} - {}", item.path.display(), error));
                "Failed"
            }
        };
        processed_items += 1;
        emit_cleanup_progress(
            cleanup_percent(processed_items, total_items),
            outcome_phase,
            &item.path.display().to_string(),
            processed_items,
            total_items,
            &mut on_progress,
        );
    }

    emit_cleanup_progress(
        100,
        "Complete",
        "",
        processed_items,
        total_items,
        &mut on_progress,
    );
    Ok(report)
}

fn scan_default_dir(
    root: &Path,
    rules: &Rules,
    plan: &mut CleanupPlan,
    state: &mut ScanState,
    on_progress: &mut impl FnMut(ScanProgress),
    category: &str,
    risk: RiskLevel,
    default_selected: bool,
    reason: &str,
) {
    state.begin_root(root, on_progress);
    if !root.exists() || is_excluded(root, rules) {
        state.finish_root(root, on_progress);
        return;
    }

    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                plan.skipped.push(error.to_string());
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() || is_excluded(path, rules) {
            continue;
        }
        if is_runtime_protected_file(path) {
            plan.skipped.push(format!(
                "{} - runtime protected file skipped",
                path.display()
            ));
            continue;
        }

        state.file_seen(path, on_progress);
        let size = file_size(path);
        plan.items.push(cleanup_item(
            path,
            size,
            category,
            risk,
            default_selected,
            reason,
        ));
        plan.reclaimable_bytes += size;
    }
    state.finish_root(root, on_progress);
}

fn scan_recycle_bin_dir(
    root: &Path,
    rules: &Rules,
    plan: &mut CleanupPlan,
    state: &mut ScanState,
    on_progress: &mut impl FnMut(ScanProgress),
) {
    state.begin_root(root, on_progress);
    if !root.exists() || is_excluded(root, rules) {
        state.finish_root(root, on_progress);
        return;
    }

    let mut has_recycle_content = false;
    for entry in WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                plan.skipped.push(error.to_string());
                continue;
            }
        };
        let path = entry.path();
        if !is_countable_recycle_entry(root, path) {
            continue;
        }
        has_recycle_content = true;
        state.file_seen(path, on_progress);
    }

    if has_recycle_content {
        let size = recycle_bin_content_size(root);
        plan.items.push(cleanup_item(
            root,
            size,
            RECYCLE_BIN_CATEGORY,
            RiskLevel::Medium,
            false,
            "Recycle Bin contents. Uses Windows Recycle Bin emptying to keep Explorer in sync.",
        ));
        plan.reclaimable_bytes += size;
    }

    state.finish_root(root, on_progress);
}

fn scan_suggestion_dir(
    root: &Path,
    rules: &Rules,
    plan: &mut CleanupPlan,
    state: &mut ScanState,
    on_progress: &mut impl FnMut(ScanProgress),
) {
    state.begin_root(root, on_progress);
    if !root.exists() || is_excluded(root, rules) {
        state.finish_root(root, on_progress);
        return;
    }

    let is_downloads_root = root.ends_with("Downloads");
    for entry in WalkDir::new(root)
        .follow_links(false)
        .max_depth(rules.max_suggestion_depth)
        .into_iter()
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                plan.skipped.push(error.to_string());
                continue;
            }
        };
        let path = entry.path();
        if path == root
            || !within_suggestion_depth(root, path, rules.max_suggestion_depth)
            || is_excluded(path, rules)
        {
            continue;
        }

        if is_downloads_root {
            if !is_direct_child(root, path) {
                continue;
            }
            if !path.is_file() && !path.is_dir() {
                continue;
            }

            state.file_seen(path, on_progress);
            let size = path_size(path);
            let large = size >= rules.large_file_bytes;
            let old = path.is_file() && is_older_than(path, rules.old_file_days).unwrap_or(false);
            let category = if large {
                "Large file suggestion"
            } else if old {
                "Old download suggestion"
            } else {
                DOWNLOAD_ITEM_CATEGORY
            };
            let risk = if large {
                RiskLevel::Medium
            } else {
                RiskLevel::High
            };
            plan.suggestions.push(cleanup_item(
                path,
                size,
                category,
                risk,
                false,
                "Downloaded file or folder. Review manually before permanent deletion.",
            ));
            plan.suggested_bytes += size;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        state.file_seen(path, on_progress);
        let size = file_size(path);
        let large = size >= rules.large_file_bytes;
        let old = false;
        if !large && !old {
            continue;
        }

        let category = if large {
            "Large file suggestion"
        } else {
            "Old download suggestion"
        };
        let risk = if large {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        };
        let reason = if large {
            "Large file candidate. Review manually before permanent deletion."
        } else {
            "Old download candidate. Review manually before permanent deletion."
        };

        plan.suggestions
            .push(cleanup_item(path, size, category, risk, false, reason));
        plan.suggested_bytes += size;
    }
    state.finish_root(root, on_progress);
}

fn append_system_backups(plan: &mut CleanupPlan) {
    let Ok(output) = Command::new("vssadmin")
        .args([
            "list",
            "shadows",
            &format!("/for={}", system_drive_letter()),
        ])
        .output()
    else {
        return;
    };

    if !output.status.success() {
        return;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let count = parse_restore_point_count(&text);
    if count == 0 {
        return;
    }

    let reason = format!(
            "Detected {count} system restore point or shadow copy record. Review carefully before using Windows Disk Cleanup or System Protection settings."
        );
    plan.system_backups.push(cleanup_item(
        Path::new("system-restore-points"),
        0,
        "System restore points",
        RiskLevel::High,
        false,
        &reason,
    ));
}

pub fn parse_restore_point_count(output: &str) -> usize {
    let english = output
        .lines()
        .filter(|line| line.to_ascii_lowercase().contains("shadow copy id"))
        .count();
    if english > 0 {
        return english;
    }

    output
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.contains("卷影复制 ID")
                || trimmed.contains("卷影副本 ID")
                || trimmed.contains("影子副本 ID")
        })
        .count()
}

fn emit_cleanup_progress(
    percent: u8,
    phase: &str,
    current_path: &str,
    processed_items: u64,
    total_items: u64,
    on_progress: &mut impl FnMut(CleanupProgress),
) {
    on_progress(CleanupProgress {
        percent,
        phase: phase.to_string(),
        current_path: current_path.to_string(),
        processed_items,
        total_items,
    });
}

fn within_suggestion_depth(root: &Path, path: &Path, max_depth: usize) -> bool {
    path.strip_prefix(root)
        .map(|relative| relative.components().count() <= max_depth)
        .unwrap_or(true)
}

fn cleanup_percent(processed_items: u64, total_items: u64) -> u8 {
    if total_items == 0 {
        return 100;
    }
    ((processed_items * 100) / total_items).min(100) as u8
}

struct ScanState {
    total_roots: usize,
    completed_roots: usize,
    scanned_files: u64,
    last_percent: u8,
    next_file_emit: u64,
}

impl ScanState {
    fn new(total_roots: usize) -> Self {
        Self {
            total_roots,
            completed_roots: 0,
            scanned_files: 0,
            last_percent: 0,
            next_file_emit: 1,
        }
    }

    fn begin_root(&mut self, root: &Path, on_progress: &mut impl FnMut(ScanProgress)) {
        self.emit("Scanning", &root.display().to_string(), on_progress);
    }

    fn file_seen(&mut self, path: &Path, on_progress: &mut impl FnMut(ScanProgress)) {
        self.scanned_files += 1;
        if self.scanned_files >= self.next_file_emit {
            self.next_file_emit = self.scanned_files + 250;
            self.emit("Scanning", &path.display().to_string(), on_progress);
        }
    }

    fn finish_root(&mut self, root: &Path, on_progress: &mut impl FnMut(ScanProgress)) {
        self.completed_roots = self.completed_roots.saturating_add(1);
        self.last_percent = self.root_percent(self.completed_roots);
        self.emit("Scanned", &root.display().to_string(), on_progress);
    }

    fn complete(&mut self, on_progress: &mut impl FnMut(ScanProgress)) {
        self.last_percent = 100;
        self.emit("Complete", "", on_progress);
    }

    fn emit(
        &mut self,
        phase: &str,
        current_path: &str,
        on_progress: &mut impl FnMut(ScanProgress),
    ) {
        let percent = self.current_percent().max(self.last_percent);
        self.last_percent = percent;
        on_progress(ScanProgress {
            percent,
            phase: phase.to_string(),
            current_path: current_path.to_string(),
            scanned_files: self.scanned_files,
        });
    }

    fn current_percent(&self) -> u8 {
        if self.total_roots == 0 {
            return 100;
        }
        let start = self.root_percent(self.completed_roots);
        let end = self.root_percent((self.completed_roots + 1).min(self.total_roots));
        let span = end.saturating_sub(start);
        let within_root = ((self.scanned_files % 2500) / 250) as u8;
        start + within_root.min(span.saturating_sub(1))
    }

    fn root_percent(&self, roots: usize) -> u8 {
        if self.total_roots == 0 {
            100
        } else {
            ((roots * 100) / self.total_roots).min(100) as u8
        }
    }
}

fn delete_path(path: &Path) -> io::Result<()> {
    match delete_path_once(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            let _ = clear_readonly(path);
            let _ = grant_delete_permissions(path);
            delete_path_once(path)
        }
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
pub fn system_drive_space() -> io::Result<DriveSpace> {
    let root = wide_null_path(&system_drive_root());
    let mut available_bytes = 0_u64;
    let mut total_bytes = 0_u64;
    let mut free_bytes = 0_u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            root.as_ptr(),
            &mut available_bytes,
            &mut total_bytes,
            &mut free_bytes,
        )
    };

    if ok == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(DriveSpace {
        total_bytes,
        free_bytes,
        available_bytes,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn system_drive_space() -> io::Result<DriveSpace> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "drive space is only supported on Windows",
    ))
}

#[cfg(target_os = "windows")]
fn empty_recycle_bin() -> io::Result<()> {
    let result = unsafe {
        SHEmptyRecycleBinW(
            std::ptr::null_mut(),
            std::ptr::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
    };

    if result >= 0 {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("SHEmptyRecycleBinW failed: 0x{:08x}", result as u32),
        ))
    }
}

#[cfg(not(target_os = "windows"))]
fn empty_recycle_bin() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "emptying Recycle Bin is only supported on Windows",
    ))
}

fn delete_path_once(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(target_os = "windows")]
fn grant_delete_permissions(path: &Path) -> io::Result<()> {
    let path_arg = path.as_os_str();
    let _ = Command::new("takeown")
        .arg("/F")
        .arg(path_arg)
        .arg("/A")
        .status();

    let status = Command::new("icacls")
        .arg(path_arg)
        .arg("/grant")
        .arg("*S-1-5-32-544:F")
        .arg("/C")
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "failed to grant Administrators delete permission",
        ))
    }
}

#[cfg(not(target_os = "windows"))]
fn grant_delete_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn is_locked_error(error: &io::Error) -> bool {
    error.raw_os_error() == Some(32)
}

fn is_runtime_protected_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "node" | "sys"))
        .unwrap_or(false)
}

fn is_recycle_bin_item(item: &CleanupItem) -> bool {
    item.category == RECYCLE_BIN_CATEGORY
}

fn dedupe_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for path in paths {
        let key = normalized_path_key(path);
        if seen.insert(key) {
            deduped.push(path.clone());
        }
    }

    deduped
}

fn normalized_path_key(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

#[cfg(target_os = "windows")]
fn wide_null_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn clear_readonly(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        for entry in WalkDir::new(path).contents_first(true).into_iter() {
            let entry = entry?;
            clear_readonly(entry.path())?;
        }
    }

    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn cleanup_item(
    path: &Path,
    size_bytes: u64,
    category: &str,
    risk: RiskLevel,
    default_selected: bool,
    reason: &str,
) -> CleanupItem {
    CleanupItem {
        id: cleanup_item_id(path, size_bytes, category),
        path: path.to_path_buf(),
        size_bytes,
        category: category.to_string(),
        risk,
        default_selected,
        reason: reason.to_string(),
    }
}

fn cleanup_item_id(path: &Path, size_bytes: u64, category: &str) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy()
        .to_ascii_lowercase()
        .hash(&mut hasher);
    size_bytes.hash(&mut hasher);
    category.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn is_excluded(path: &Path, rules: &Rules) -> bool {
    let Ok(candidate) = path.canonicalize() else {
        return false;
    };
    rules.exclude_dirs.iter().any(|root| {
        root.canonicalize()
            .map(|canonical_root| candidate.starts_with(canonical_root))
            .unwrap_or(false)
    })
}

fn is_older_than(path: &Path, days: u64) -> io::Result<bool> {
    let modified = fs::metadata(path)?.modified()?;
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_else(|_| Duration::from_secs(0));
    Ok(age >= Duration::from_secs(days * 24 * 60 * 60))
}

fn path_size(path: &Path) -> u64 {
    if path.is_dir() {
        directory_size(path)
    } else {
        file_size(path)
    }
}

fn directory_size(path: &Path) -> u64 {
    directory_size_with_filter(path, |_| true)
}

fn directory_size_with_filter(path: &Path, include: impl Fn(&Path) -> bool) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|candidate| candidate.is_file() && include(candidate))
        .map(|candidate| file_size(&candidate))
        .sum()
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn is_desktop_ini(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case("desktop.ini"))
        .unwrap_or(false)
}

fn recycle_bin_content_size(root: &Path) -> u64 {
    directory_size_with_filter(root, |candidate| {
        is_countable_recycle_entry(root, candidate)
    })
}

fn live_recycle_bin_content_size() -> u64 {
    recycle_bin_roots_for_drive_roots(&drive_roots())
        .iter()
        .map(|root| {
            if root.exists() {
                recycle_bin_content_size(root)
            } else {
                0
            }
        })
        .sum()
}

fn is_countable_recycle_entry(root: &Path, path: &Path) -> bool {
    if is_desktop_ini(path)
        || is_recycle_info_file(path)
        || (is_direct_child(root, path) && path.is_dir())
    {
        return false;
    }

    if path.is_file() {
        return true;
    }

    path.is_dir()
        && directory_size_with_filter(path, |candidate| {
            !is_desktop_ini(candidate) && !is_recycle_info_file(candidate)
        }) > 0
}

fn is_recycle_info_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let bytes = name.as_bytes();
            bytes.len() >= 2 && bytes[0] == b'$' && bytes[1].eq_ignore_ascii_case(&b'I')
        })
        .unwrap_or(false)
}

fn is_direct_child(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .map(|relative| relative.components().count() == 1)
        .unwrap_or(false)
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn push_join(paths: &mut Vec<PathBuf>, base: &Option<PathBuf>, suffix: &str) {
    if let Some(base) = base {
        paths.push(base.join(suffix));
    }
}

pub(crate) fn recycle_bin_roots_for_drive_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots.iter().map(|root| root.join("$Recycle.Bin")).collect()
}

#[cfg(target_os = "windows")]
fn drive_roots() -> Vec<PathBuf> {
    let mask = unsafe { GetLogicalDrives() };
    let mut roots = Vec::new();

    for index in 0..26 {
        if mask & (1 << index) == 0 {
            continue;
        }
        let letter = (b'A' + index as u8) as char;
        roots.push(PathBuf::from(format!("{letter}:\\")));
    }

    if roots.is_empty() {
        roots.push(system_drive_root());
    }
    roots
}

#[cfg(not(target_os = "windows"))]
fn drive_roots() -> Vec<PathBuf> {
    vec![system_drive_root()]
}

fn system_drive_root() -> PathBuf {
    env::var_os("SystemDrive")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:"))
        .join("\\")
}

fn system_drive_letter() -> String {
    env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string())
}
