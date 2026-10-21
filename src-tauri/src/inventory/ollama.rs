//! Ollama 按模型盘点：读取 models/manifests 下的清单，算出每个模型引用的数据块。
//! 仅删除清单和扫描时独占、处理时仍未共享的数据块；共享块保守保留。
//! 清单读取失败、损坏、含链接或读取期间变化时拒绝处理，不推断为“没有其他引用”。
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// 一个模型可以单独处理的部分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts {
    /// Ollama 的 models 目录（含 manifests 与 blobs）
    pub root: PathBuf,
    /// 扫描时只被这个模型引用的数据块
    pub blobs: Vec<PathBuf>,
    /// 数据块的扫描快照；不能仅凭文件名在稍后删除发生变化的内容。
    pub snapshots: HashMap<PathBuf, super::snapshot::Snapshot>,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub manifest: PathBuf,
    /// 只被这个模型引用的数据块
    pub own: Vec<PathBuf>,
    /// 与其他模型共用的数据块数量
    pub shared: usize,
    pub total_bytes: u64,
}

/// `registry.ollama.ai/library/qwen2.5/14b` → `qwen2.5:14b`；其他命名空间、仓库保留前缀。
pub fn model_name(parts: &[String]) -> String {
    match parts {
        [registry, namespace, model, tag]
            if registry == "registry.ollama.ai" && namespace == "library" =>
        {
            format!("{model}:{tag}")
        }
        [registry, namespace, model, tag] if registry == "registry.ollama.ai" => {
            format!("{namespace}/{model}:{tag}")
        }
        [registry, namespace, model, tag] => format!("{registry}/{namespace}/{model}:{tag}"),
        _ => parts.join("/"),
    }
}

/// 从清单文本里取出所有 `sha256:<64 位十六进制>` 摘要（去重，保持顺序）。
pub fn digests(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find("sha256:") {
        let hex: String = rest[pos + 7..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if hex.len() == 64 {
            let digest = format!("sha256-{}", hex.to_ascii_lowercase());
            if !out.contains(&digest) {
                out.push(digest);
            }
        }
        rest = &rest[pos + 7..];
    }
    out
}

fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !super::snapshot::is_link(&m))
}

/// 所有清单：(清单路径, 相对 manifests 的各级名称, 引用的数据块路径)
type Manifest = (PathBuf, Vec<String>, Vec<PathBuf>);

fn manifests(root: &Path) -> Result<Vec<Manifest>, String> {
    let base = root.join("manifests");
    super::snapshot::safe_path(&base)?;
    super::snapshot::safe_path(&root.join("blobs"))?;
    let cancel = AtomicBool::new(false);
    let before = super::snapshot::inspect(&base, &cancel)?;
    if before.blocked.is_some() {
        return Err("模型清单目录包含受保护内容，禁止处理".into());
    }
    let mut out = Vec::new();
    for (count, entry) in walkdir::WalkDir::new(&base)
        .follow_links(false)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .enumerate()
    {
        if count >= 10_000 {
            return Err("模型清单超过 10000 项，无法完整核对引用".into());
        }
        let entry = entry.map_err(|e| format!("模型清单目录读取不完整：{e}"))?;
        let path = entry.path().to_path_buf();
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if super::snapshot::is_link(&meta) {
            return Err("模型清单包含链接，无法完整核对引用".into());
        }
        if meta.is_dir() {
            continue;
        }
        if !meta.is_file() || meta.len() > 1024 * 1024 {
            return Err(format!("模型清单类型或大小异常：{}", path.display()));
        }
        let mut text = String::new();
        fs::File::open(&path)
            .and_then(|f| f.take(1024 * 1024 + 1).read_to_string(&mut text))
            .map_err(|e| format!("无法读取模型清单 {}：{e}", path.display()))?;
        if text.len() > 1024 * 1024 {
            return Err("模型清单超过 1 MiB，无法完整核对引用".into());
        }
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("模型清单损坏 {}：{e}", path.display()))?;
        let layers = json
            .get("layers")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("模型清单缺少 layers：{}", path.display()))?;
        let mut descriptors = layers.iter().collect::<Vec<_>>();
        if let Some(config) = json.get("config") {
            descriptors.push(config);
        }
        let mut names = Vec::new();
        for descriptor in descriptors {
            let digest = descriptor
                .get("digest")
                .and_then(|v| v.as_str())
                .ok_or("模型清单缺少数据块摘要")?;
            if digest.len() != 71
                || !digest.starts_with("sha256:")
                || !digest[7..].bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err("模型清单含不支持的数据块摘要，无法核对引用".into());
            }
            names.extend(digests(digest));
        }
        names.sort();
        names.dedup();
        if names.is_empty() {
            return Err("模型清单没有有效的数据块引用".into());
        }
        let blobs: Vec<PathBuf> = names
            .into_iter()
            .map(|d| root.join("blobs").join(d))
            .collect();
        let parts = path
            .strip_prefix(&base)
            .map(|rel| {
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        out.push((path, parts, blobs));
    }
    if super::snapshot::inspect(&base, &cancel)? != before {
        return Err("读取期间模型清单发生变化，请退出 Ollama 后重新扫描".into());
    }
    Ok(out)
}

/// 列出 models 目录下的所有模型；不是标准布局（没有 manifests / blobs）时返回空。
pub fn models(root: &Path) -> Result<Vec<Model>, String> {
    if !root.join("manifests").is_dir() || !root.join("blobs").is_dir() {
        return Ok(Vec::new());
    }
    let all = manifests(root)?;
    let mut refs: HashMap<PathBuf, usize> = HashMap::new();
    for (_, _, blobs) in &all {
        for blob in blobs {
            *refs.entry(blob.clone()).or_default() += 1;
        }
    }
    Ok(all
        .into_iter()
        .map(|(manifest, parts, blobs)| {
            let size = |p: &PathBuf| fs::symlink_metadata(p).map(|m| m.len()).unwrap_or(0);
            let existing: Vec<PathBuf> = blobs.into_iter().filter(|b| is_plain_file(b)).collect();
            let (own, shared): (Vec<_>, Vec<_>) = existing
                .iter()
                .cloned()
                .partition(|b| refs.get(b) == Some(&1));
            Model {
                name: model_name(&parts),
                total_bytes: existing.iter().map(size).sum::<u64>() + size(&manifest),
                shared: shared.len(),
                own,
                manifest,
            }
        })
        .collect())
}

/// 处理前重新核对：只保留现在仍然只被这个清单引用、且仍是普通文件的数据块。
pub fn still_unshared(
    root: &Path,
    manifest: &Path,
    blobs: &[PathBuf],
) -> Result<Vec<PathBuf>, String> {
    let others: HashSet<PathBuf> = manifests(root)?
        .into_iter()
        .filter(|(path, _, _)| super::key(path) != super::key(manifest))
        .flat_map(|(_, _, blobs)| blobs)
        .collect();
    Ok(blobs
        .iter()
        .filter(|b| !others.contains(*b) && is_plain_file(b))
        .cloned()
        .collect())
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
