pub mod model;
pub mod platform;
#[cfg(windows)]
mod recycle_sink;
pub mod rules;
pub mod snapshot;
#[cfg(test)]
mod tests;

use model::*;
use snapshot::Snapshot;
use std::{
    collections::{hash_map::DefaultHasher, BTreeSet, HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct Entry {
    pub item: Item,
    pub snapshot: Option<Snapshot>,
}
#[derive(Default, Clone)]
pub struct Plan {
    pub scan_id: String,
    pub entries: HashMap<String, Entry>,
}
fn key(path: &Path) -> String {
    #[cfg(windows)]
    {
        path.to_string_lossy().replace('/', "\\").to_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().into_owned()
    }
}
fn within(child: &Path, parent: &Path) -> bool {
    let (c, p) = (key(child), key(parent));
    let separator = std::path::MAIN_SEPARATOR;
    c == p || c.starts_with(&(p.trim_end_matches(separator).to_owned() + &separator.to_string()))
}
fn overlaps(paths: &BTreeSet<String>, path: &Path) -> bool {
    if path
        .ancestors()
        .any(|ancestor| paths.contains(&key(ancestor)))
    {
        return true;
    }
    let separator = std::path::MAIN_SEPARATOR;
    let prefix = key(path).trim_end_matches(separator).to_owned() + &separator.to_string();
    paths
        .range(prefix.clone()..)
        .next()
        .is_some_and(|next| next.starts_with(&prefix))
}

fn make_entry(rule: &Rule, path: PathBuf, cancel: &AtomicBool) -> Entry {
    let inspection = snapshot::inspect(&path, cancel);
    let mut protection = rule.protection;
    let mut reason = match protection {
        Protection::Safe => "已知缓存或临时目录；使用中的内容仍可能无法清理",
        Protection::Review => "默认保护，请先核对内容与删除影响",
        Protection::Blocked => "请通过对应应用或 Windows 设置管理",
    }
    .to_string();
    let mut complete = true;
    let snap = match inspection {
        Ok(snap) => {
            if let Some(ref why) = snap.blocked {
                protection = Protection::Blocked;
                reason = why.clone();
            }
            Some(snap)
        }
        Err(error) => {
            protection = Protection::Blocked;
            reason = error;
            complete = false;
            None
        }
    };
    if snapshot::restricted_root(&path) {
        protection = Protection::Blocked;
        reason = "系统、用户或应用根目录禁止整体清理".into();
    }
    let mut hasher = DefaultHasher::new();
    key(&path).hash(&mut hasher);
    snap.as_ref().map(|s| s.fingerprint).hash(&mut hasher);
    let item = Item {
        id: format!("{:016x}", hasher.finish()),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        path,
        source: rule.source.clone(),
        category: rule.category.clone(),
        evidence: rule.evidence.clone(),
        impact: rule.impact.clone(),
        protection,
        protection_reason: reason,
        size_bytes: snap.as_ref().map_or(0, |s| s.bytes),
        file_count: snap.as_ref().map_or(0, |s| s.files),
        is_dir: snap.as_ref().is_some_and(|s| s.is_dir),
        modified_ms: snap.as_ref().map_or(0, |s| s.modified_ms),
        permanent_allowed: rule.category == "temp" && protection == Protection::Safe,
        complete,
    };
    Entry {
        item,
        snapshot: snap,
    }
}

pub fn scan(
    rules: Vec<Rule>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> (Inventory, Plan) {
    let scan_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string();
    let mut plan = Plan {
        scan_id: scan_id.clone(),
        ..Default::default()
    };
    let mut items = vec![];
    let mut warnings = vec![];
    let mut paths = BTreeSet::new();
    let total = rules.len();
    for (i, rule) in rules.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        progress(Progress {
            current: rule.path.display().to_string(),
            completed: i,
            total,
        });
        if fs::symlink_metadata(&rule.path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            continue;
        }
        let targets = if rule.category == "large" && snapshot::safe_path(&rule.path).is_ok() {
            let mut list = vec![];
            let walker = walkdir::WalkDir::new(&rule.path)
                .follow_links(false)
                .max_depth(4)
                .into_iter()
                .filter_entry(|entry| {
                    fs::symlink_metadata(entry.path()).is_ok_and(|meta| !snapshot::is_link(&meta))
                        && !snapshot::restricted_root(entry.path())
                });
            for (count, entry) in walker.enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                if count >= 200_000 {
                    warnings.push(format!(
                        "{}：大文件搜索达到 20 万条目上限",
                        rule.path.display()
                    ));
                    break;
                }
                match entry {
                    Ok(entry) => {
                        if let Ok(meta) = entry.metadata() {
                            if meta.is_file() && meta.len() >= 512 * 1024 * 1024 {
                                list.push(entry.path().to_path_buf());
                            }
                        }
                    }
                    Err(e) => warnings.push(format!("大文件搜索：{e}")),
                }
            }
            list
        } else if rule.children
            && snapshot::safe_path(&rule.path).is_ok()
            && !snapshot::restricted_root(&rule.path)
        {
            match fs::read_dir(&rule.path) {
                Ok(children) => {
                    let mut list = vec![];
                    for child in children.take(10_001) {
                        match child {
                            Ok(child) => list.push(child.path()),
                            Err(e) => warnings.push(format!("{}：{e}", rule.path.display())),
                        }
                    }
                    if list.len() > 10_000 {
                        warnings.push(format!(
                            "{}：只列出前 10000 项，请缩小范围",
                            rule.path.display()
                        ));
                        list.truncate(10_000);
                    }
                    list.sort();
                    list
                }
                Err(e) => {
                    warnings.push(format!("{}：{e}", rule.path.display()));
                    vec![rule.path.clone()]
                }
            }
        } else {
            vec![rule.path.clone()]
        };
        for path in targets {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if overlaps(&paths, &path) {
                continue;
            }
            let entry = make_entry(&rule, path.clone(), cancel);
            paths.insert(key(&path));
            items.push(entry.item.clone());
            plan.entries.insert(entry.item.id.clone(), entry);
        }
    }
    let cancelled = cancel.load(Ordering::Relaxed);
    // Partial inventories are useful to view, but never actionable until a complete scan.
    if cancelled {
        for item in &mut items {
            item.protection = Protection::Blocked;
            item.protection_reason = "扫描已停止，请完成扫描后处理".into();
            item.permanent_allowed = false;
            if let Some(e) = plan.entries.get_mut(&item.id) {
                e.item = item.clone();
            }
        }
    }
    items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then(a.path.cmp(&b.path)));
    progress(Progress {
        current: "扫描完成".into(),
        completed: total,
        total,
    });
    (
        Inventory {
            scan_id,
            items,
            warnings,
            cancelled,
            drives: platform::drives(),
            recycle_bytes: platform::recycle_bytes(),
        },
        plan,
    )
}

pub fn prepare(plan: &Plan, request: &CleanupRequest) -> Result<Vec<Entry>, String> {
    if request.scan_id != plan.scan_id {
        return Err("扫描已更新，请重新核对清理清单".into());
    }
    if request.ids.is_empty() || request.ids.len() > 10_000 {
        return Err("请选择 1–10000 个条目".into());
    }
    let reviewed: HashSet<_> = request.reviewed_ids.iter().collect();
    let mut selected: Vec<Entry> = vec![];
    for id in &request.ids {
        let entry = plan.entries.get(id).ok_or("条目不属于本次扫描")?;
        if entry.item.protection == Protection::Blocked || !entry.item.complete {
            return Err(format!("{}：禁止处理", entry.item.name));
        }
        if entry.item.protection == Protection::Review && !reviewed.contains(id) {
            return Err(format!("{}：尚未审核保护", entry.item.name));
        }
        if request.mode == DeleteMode::Permanent && !entry.item.permanent_allowed {
            return Err("永久删除仅限已知临时文件".into());
        }
        if selected
            .iter()
            .any(|e| within(&entry.item.path, &e.item.path))
        {
            continue;
        }
        selected.retain(|e| !within(&e.item.path, &entry.item.path));
        selected.push(entry.clone());
    }
    let cancel = AtomicBool::new(false);
    for entry in &selected {
        let fresh = snapshot::inspect(&entry.item.path, &cancel)?;
        if Some(&fresh) != entry.snapshot.as_ref()
            || fresh.blocked.is_some()
            || snapshot::restricted_root(&entry.item.path)
        {
            return Err(format!("{}：内容或权限已变化，请重新扫描", entry.item.name));
        }
    }
    Ok(selected)
}

pub fn cleanup(
    plan: &Plan,
    request: &CleanupRequest,
    mut progress: impl FnMut(Progress),
) -> Result<CleanupReport, String> {
    let selected = prepare(plan, request)?;
    let mut report = CleanupReport::default();
    for (i, entry) in selected.iter().enumerate() {
        progress(Progress {
            current: entry.item.name.clone(),
            completed: i,
            total: selected.len(),
        });
        let result = (|| {
            let fresh = snapshot::inspect(&entry.item.path, &AtomicBool::new(false))?;
            if Some(&fresh) != entry.snapshot.as_ref() {
                return Err("内容已变化，请重新扫描".into());
            }
            match request.mode {
                DeleteMode::Recycle => platform::recycle(&entry.item.path),
                DeleteMode::Permanent => if entry.item.is_dir {
                    fs::remove_dir_all(&entry.item.path)
                } else {
                    fs::remove_file(&entry.item.path)
                }
                .map_err(|e| e.to_string()),
            }
        })();
        match result {
            Ok(()) => {
                report.completed_ids.push(entry.item.id.clone());
                if request.mode == DeleteMode::Recycle {
                    report.recycled_bytes += entry.item.size_bytes;
                } else {
                    report.deleted_bytes += entry.item.size_bytes;
                }
            }
            Err(e) => report
                .errors
                .push(format!("{}：{e}", entry.item.path.display())),
        }
    }
    progress(Progress {
        current: "处理完成".into(),
        completed: selected.len(),
        total: selected.len(),
    });
    Ok(report)
}

pub fn browse(plan: &mut Plan, id: &str) -> Result<Vec<Item>, String> {
    let parent = plan.entries.get(id).ok_or("未知条目")?.clone();
    snapshot::safe_path(&parent.item.path)?;
    if !parent.item.is_dir {
        return Err("此条目不是目录".into());
    }
    let rule = Rule {
        path: parent.item.path.clone(),
        source: parent.item.source.clone(),
        category: parent.item.category.clone(),
        evidence: parent.item.evidence.clone(),
        impact: parent.item.impact.clone(),
        protection: parent.item.protection,
        children: false,
    };
    let mut items = vec![];
    let paths = fs::read_dir(&parent.item.path)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if paths.len() > 2000 {
        return Err("目录超过 2000 项，请在资源管理器中查看".into());
    }
    for path in paths {
        let entry = make_entry(&rule, path.path(), &AtomicBool::new(false));
        items.push(entry.item.clone());
        plan.entries.insert(entry.item.id.clone(), entry);
    }
    items.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(b.size_bytes.cmp(&a.size_bytes))
    });
    Ok(items)
}
pub fn preview(plan: &Plan, id: &str) -> Result<String, String> {
    let item = &plan.entries.get(id).ok_or("未知条目")?.item;
    if item.protection == Protection::Blocked {
        return Err("受限制条目不提供内容预览".into());
    }
    snapshot::safe_path(&item.path)?;
    let extension = item
        .path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if !matches!(
        extension.as_str(),
        "txt" | "md" | "log" | "json" | "jsonl" | "csv" | "yaml" | "yml" | "py" | "js" | "ts"
    ) {
        return Err("仅预览文本、日志与源码文件，请使用资源管理器查看其他格式".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(&item.path)
        .map_err(|e| e.to_string())?
        .take(65536)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.contains(&0) {
        return Err("文件可能是二进制内容，已停止预览".into());
    }
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if bytes.len() == 65536 {
        text.push_str("\n…仅显示前 64 KiB");
    }
    Ok(text)
}
