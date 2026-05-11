use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};
use walkdir::WalkDir;

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
    pub plan: CleanupPlan,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub freed_bytes: u64,
    pub deleted_count: usize,
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
    let total_roots = root_count(rules);
    let mut state = ScanState::new(total_roots);

    state.emit("Preparing scan", "", &mut on_progress);

    for root in &rules.temp_dirs {
        scan_default_dir(
            root,
            rules,
            &mut plan,
            &mut state,
            &mut on_progress,
            "Low risk cache",
            RiskLevel::Low,
            "Temporary or system cache file. Permanent deletion requires confirmation.",
        );
    }

    if rules.enable_third_party_cache {
        for root in &rules.enhanced_cache_dirs {
            scan_default_dir(
                root,
                rules,
                &mut plan,
                &mut state,
                &mut on_progress,
                "Enhanced cache",
                RiskLevel::Medium,
                "Third-party cache selected by enhanced mode. Permanent deletion requires confirmation.",
            );
        }
    }

    for root in &rules.windows_old_dirs {
        scan_default_dir(
            root,
            rules,
            &mut plan,
            &mut state,
            &mut on_progress,
            "Windows.old",
            RiskLevel::Medium,
            "Previous Windows installation files. Clean only after confirming rollback is not needed.",
        );
    }

    for root in &rules.suggest_dirs {
        scan_suggestion_dir(root, rules, &mut plan, &mut state, &mut on_progress);
    }

    state.complete(&mut on_progress);
    Ok(plan)
}

pub fn execute_cleanup(plan: &CleanupPlan) -> io::Result<CleanupReport> {
    execute_cleanup_with_progress(plan, |_| {})
}

pub fn execute_cleanup_with_progress<F>(
    plan: &CleanupPlan,
    mut on_progress: F,
) -> io::Result<CleanupReport>
where
    F: FnMut(CleanupProgress),
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

    for item in &plan.items {
        if !item.default_selected {
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
            report.failed_count += 1;
            report.errors.push(format!(
                "{} - file missing or inaccessible",
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

        match delete_path(&item.path) {
            Ok(()) => {
                report.deleted_count += 1;
                report.freed_bytes += item.size_bytes;
            }
            Err(error) => {
                report.failed_count += 1;
                report
                    .errors
                    .push(format!("{} - {}", item.path.display(), error));
            }
        }
        processed_items += 1;
        emit_cleanup_progress(
            cleanup_percent(processed_items, total_items),
            "Deleted",
            &item.path.display().to_string(),
            processed_items,
            total_items,
            &mut on_progress,
        );
    }

    report.log_path = write_report(&report).ok();
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

        state.file_seen(path, on_progress);
        let size = file_size(path);
        plan.items.push(CleanupItem {
            path: path.to_path_buf(),
            size_bytes: size,
            category: category.to_string(),
            risk,
            default_selected: true,
            reason: reason.to_string(),
        });
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
        if !path.is_file()
            || !within_suggestion_depth(root, path, rules.max_suggestion_depth)
            || is_excluded(path, rules)
        {
            continue;
        }

        state.file_seen(path, on_progress);
        let size = file_size(path);
        let large = size >= rules.large_file_bytes;
        let old = root.ends_with("Downloads")
            && is_older_than(path, rules.old_file_days).unwrap_or(false);
        if !large && !old {
            continue;
        }

        plan.suggestions.push(CleanupItem {
            path: path.to_path_buf(),
            size_bytes: size,
            category: if large {
                "Large file suggestion"
            } else {
                "Old download suggestion"
            }
            .to_string(),
            risk: if large {
                RiskLevel::Medium
            } else {
                RiskLevel::High
            },
            default_selected: false,
            reason: if large {
                "Large file candidate. Review manually before permanent deletion."
            } else {
                "Old download candidate. Review manually before permanent deletion."
            }
            .to_string(),
        });
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

    plan.system_backups.push(CleanupItem {
        path: PathBuf::from("system-restore-points"),
        size_bytes: 0,
        category: "System restore points".to_string(),
        risk: RiskLevel::High,
        default_selected: false,
        reason: format!(
            "Detected {count} system restore point or shadow copy record. Review carefully before using Windows Disk Cleanup or System Protection settings."
        ),
    });
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

fn root_count(rules: &Rules) -> usize {
    rules.temp_dirs.len()
        + if rules.enable_third_party_cache {
            rules.enhanced_cache_dirs.len()
        } else {
            0
        }
        + rules.windows_old_dirs.len()
        + rules.suggest_dirs.len()
}

fn delete_path(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn write_report(report: &CleanupReport) -> io::Result<PathBuf> {
    let dir = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("logs");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("cleanup-{}.log", timestamp_compact()));
    let mut content = String::new();
    content.push_str("C drive cleanup report\n");
    content.push_str(&format!(
        "Freed space: {}\n",
        format_bytes(report.freed_bytes)
    ));
    content.push_str(&format!("Deleted: {}\n", report.deleted_count));
    content.push_str(&format!("Failed: {}\n", report.failed_count));
    for error in &report.errors {
        content.push_str(&format!("- {error}\n"));
    }
    fs::write(&path, content)?;
    Ok(path)
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

fn file_size(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
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

fn system_drive_root() -> PathBuf {
    env::var_os("SystemDrive")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:"))
        .join("\\")
}

fn system_drive_letter() -> String {
    env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string())
}

pub fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut index = 0;
    while value >= 1024.0 && index < units.len() - 1 {
        value /= 1024.0;
        index += 1;
    }
    if index == 0 {
        format!("{} {}", bytes, units[index])
    } else {
        format!("{value:.1} {}", units[index])
    }
}

fn timestamp_compact() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    now.as_secs().to_string()
}
