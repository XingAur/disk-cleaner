//! Ollama 按模型盘点：读取 models/manifests 下的清单，算出每个模型引用的数据块。
//! 卸载一个模型 = 删除它的清单 + 只被它引用的数据块（与 `ollama rm` 一致），
//! 与其他模型共用的数据块保留。处理时会重新读取全部清单，扫描后新拉取的模型共用的数据块也不会被删。
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// 一个模型可以单独处理的部分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts {
    /// Ollama 的 models 目录（含 manifests 与 blobs）
    pub root: PathBuf,
    /// 扫描时只被这个模型引用的数据块
    pub blobs: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub manifest: PathBuf,
    /// 只被这个模型引用的数据块
    pub own: Vec<PathBuf>,
    /// 与其他模型共用的数据块数量
    pub shared: usize,
    pub own_bytes: u64,
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
fn manifests(root: &Path) -> Vec<(PathBuf, Vec<String>, Vec<PathBuf>)> {
    let base = root.join("manifests");
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(&base)
        .follow_links(false)
        .min_depth(1)
        .max_depth(6)
        .sort_by_file_name()
        .into_iter()
        .flatten()
    {
        let path = entry.path().to_path_buf();
        if !is_plain_file(&path) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let blobs: Vec<PathBuf> = digests(&text)
            .into_iter()
            .map(|d| root.join("blobs").join(d))
            .collect();
        if blobs.is_empty() {
            continue;
        }
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
    out
}

/// 列出 models 目录下的所有模型；不是标准布局（没有 manifests / blobs）时返回空。
pub fn models(root: &Path) -> Vec<Model> {
    if !root.join("manifests").is_dir() || !root.join("blobs").is_dir() {
        return Vec::new();
    }
    let all = manifests(root);
    let mut refs: HashMap<PathBuf, usize> = HashMap::new();
    for (_, _, blobs) in &all {
        for blob in blobs {
            *refs.entry(blob.clone()).or_default() += 1;
        }
    }
    all.into_iter()
        .map(|(manifest, parts, blobs)| {
            let size = |p: &PathBuf| fs::symlink_metadata(p).map(|m| m.len()).unwrap_or(0);
            let existing: Vec<PathBuf> = blobs.into_iter().filter(|b| is_plain_file(b)).collect();
            let (own, shared): (Vec<_>, Vec<_>) = existing
                .iter()
                .cloned()
                .partition(|b| refs.get(b) == Some(&1));
            Model {
                name: model_name(&parts),
                own_bytes: own.iter().map(size).sum::<u64>() + size(&manifest),
                total_bytes: existing.iter().map(size).sum::<u64>() + size(&manifest),
                shared: shared.len(),
                own,
                manifest,
            }
        })
        .collect()
}

/// 处理前重新核对：只保留现在仍然只被这个清单引用、且仍是普通文件的数据块。
pub fn still_unshared(root: &Path, manifest: &Path, blobs: &[PathBuf]) -> Vec<PathBuf> {
    let others: HashSet<PathBuf> = manifests(root)
        .into_iter()
        .filter(|(path, _, _)| super::key(path) != super::key(manifest))
        .flat_map(|(_, _, blobs)| blobs)
        .collect();
    blobs
        .iter()
        .filter(|b| !others.contains(*b) && is_plain_file(b))
        .cloned()
        .collect()
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
