use super::Drive;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

pub fn drives() -> Vec<Drive> {
    let Ok(output) = Command::new("/bin/df").args(["-kP", "/"]).output() else {
        return vec![];
    };
    if !output.status.success() {
        return vec![];
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(line) = text.lines().nth(1) else {
        return vec![];
    };
    let columns: Vec<_> = line.split_whitespace().collect();
    let parse = |index: usize| {
        columns
            .get(index)
            .and_then(|s| s.parse::<u64>().ok())
            .map(|v| v.saturating_mul(1024))
    };
    match (parse(1), parse(3)) {
        (Some(total_bytes), Some(free_bytes)) => vec![Drive {
            path: "/".into(),
            total_bytes,
            free_bytes,
        }],
        _ => vec![],
    }
}
fn trash_path() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".Trash"))
        .ok_or_else(|| "缺少用户 HOME 目录".into())
}
pub fn recycle_bytes() -> u64 {
    let Ok(path) = trash_path() else {
        return 0;
    };
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::symlink_metadata(entry.path()).ok())
        .filter(|meta| meta.is_file() && !super::super::snapshot::is_link(meta))
        .fold(0u64, |sum, meta| sum.saturating_add(meta.len()))
}
pub fn recycle(path: &Path) -> Result<(), String> {
    // trash-rs uses NSFileManager's native trash API on macOS, with no rm fallback.
    trash::delete(path).map_err(|e| e.to_string())
}
pub fn empty_recycle_bin() -> Result<(), String> {
    Err("请在 Finder 废纸篓中检查内容并执行清倒".into())
}
pub fn reveal(path: &Path) -> Result<(), String> {
    Command::new("/usr/bin/open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn open_system(target: &str) -> Result<(), String> {
    let argument = match target {
        "recycle" => trash_path()?,
        "storage" => PathBuf::from("x-apple.systempreferences:com.apple.settings.Storage"),
        _ => return Err("不支持的入口".into()),
    };
    Command::new("/usr/bin/open")
        .arg(argument)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
