use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Component, Path},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub bytes: u64,
    pub files: u64,
    pub fingerprint: u64,
    pub modified_ms: u64,
    pub is_dir: bool,
    pub blocked: Option<String>,
}

pub fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
pub fn safe_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("路径必须是没有 .. 的绝对路径".into());
    }
    let s = path.to_string_lossy();
    if s.starts_with(r"\\") || s.contains('\0') {
        return Err("不处理网络、设备或特殊路径".into());
    }
    #[cfg(windows)]
    {
        use std::path::Prefix;
        if !matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_)))
        {
            return Err("仅处理本地盘符路径".into());
        }
        for part in path.components() {
            if let Component::Normal(name) = part {
                let name = name.to_string_lossy();
                if name.contains(':') || name.ends_with('.') || name.ends_with(' ') {
                    return Err("不处理数据流或含歧义的 Windows 路径".into());
                }
            }
        }
    }
    for ancestor in path.ancestors() {
        let meta = fs::symlink_metadata(ancestor).map_err(|e| format!("无法核对路径：{e}"))?;
        if is_link(&meta) {
            return Err("路径含符号链接或重解析点，禁止处理".into());
        }
    }
    Ok(())
}
fn sensitive(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    matches!(
        name.as_str(),
        ".git"
            | ".ssh"
            | ".aws"
            | ".azure"
            | ".gnupg"
            | ".kube"
            | "auth.json"
            | "credentials"
            | "credentials.json"
            | "tokens.json"
            | "secrets.json"
            | "id_rsa"
            | "id_ed25519"
            | "cookies"
            | "login data"
            | "config.toml"
            | "settings.json"
            | "ntuser.dat"
            | "sam"
            | "system"
            | "security"
            | "$recycle.bin"
            | "system volume information"
    ) || name.starts_with(".env")
        || name.starts_with("state_")
        || [".db-wal", ".db-shm", ".sqlite-wal", ".sqlite-shm"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
        || matches!(
            path.extension()
                .and_then(|s| s.to_str())
                .map(str::to_lowercase)
                .as_deref(),
            Some("db" | "sqlite" | "sqlite3" | "pem" | "key" | "pfx" | "sys")
        )
}
pub fn inspect(path: &Path, cancel: &AtomicBool) -> Result<Snapshot, String> {
    safe_path(path)?;
    let root = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let modified_ms = root
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let blocked = path
        .ancestors()
        .any(sensitive)
        .then(|| "位于凭据、数据库或项目元数据目录，禁止处理".into());
    let mut result = Snapshot {
        bytes: 0,
        files: 0,
        fingerprint: 0,
        modified_ms,
        is_dir: root.is_dir(),
        blocked,
    };
    let mut hash = DefaultHasher::new();
    for (count, entry) in walkdir::WalkDir::new(path)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .enumerate()
    {
        if cancel.load(Ordering::Relaxed) {
            return Err("扫描已停止".into());
        }
        if count >= 200_000 {
            return Err("目录超过 200000 个条目，统计不完整；请缩小扫描范围".into());
        }
        let entry = entry.map_err(|e| format!("目录读取不完整：{e}"))?;
        let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        if is_link(&meta) {
            return Err("包含链接或重解析点，禁止处理".into());
        }
        if sensitive(entry.path()) {
            result.blocked = Some("包含凭据、数据库、运行状态或项目元数据，禁止整体清理".into());
        }
        entry.path().hash(&mut hash);
        meta.len().hash(&mut hash);
        meta.modified().ok().hash(&mut hash);
        meta.created().ok().hash(&mut hash);
        meta.permissions().readonly().hash(&mut hash);
        meta.is_dir().hash(&mut hash);
        if meta.is_file() {
            result.bytes = result.bytes.saturating_add(meta.len());
            result.files += 1;
        }
    }
    result.fingerprint = hash.finish();
    Ok(result)
}

pub fn restricted_root(path: &Path) -> bool {
    if path.parent().is_none() {
        return true;
    }
    let separator = std::path::MAIN_SEPARATOR;
    let key = |p: &Path| super::key(p).trim_end_matches(separator).to_owned();
    if let Ok(exe) = std::env::current_exe() {
        let root = key(path);
        if key(&exe) == root || key(&exe).starts_with(&(root + &separator.to_string())) {
            return true;
        }
    }
    if path.components().any(|c| matches!(c, Component::Normal(n) if n.to_string_lossy().eq_ignore_ascii_case("com.xyj.diskcleaner"))) { return true; }
    let protected: Vec<_> = [
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "SystemRoot",
    ]
    .into_iter()
    .filter_map(std::env::var_os)
    .map(std::path::PathBuf::from)
    .collect();
    #[cfg(target_os = "macos")]
    {
        let system_roots = [
            "/",
            "/Users",
            "/Applications",
            "/System",
            "/Library",
            "/usr",
            "/bin",
            "/sbin",
            "/private",
            "/var",
            "/etc",
            "/Volumes",
        ];
        if system_roots.iter().any(|root| path == Path::new(root)) {
            return true;
        }
        if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
            if path == home
                || path == home.join("Library")
                || path == home.join("Library/Application Support")
                || path == home.join("Library/Caches")
                || path == home.join(".Trash")
            {
                return true;
            }
        }
    }
    protected.iter().any(|p| key(p) == key(path)) || path.components().any(|p| matches!(p, Component::Normal(n) if matches!(n.to_string_lossy().to_lowercase().as_str(), ".git" | ".ssh" | ".aws" | ".azure" | ".gnupg" | "$recycle.bin" | "system32" | "winsxs" | "system volume information")))
}
