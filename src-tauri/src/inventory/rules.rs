use super::model::*;
use std::{
    env,
    path::{Path, PathBuf},
};

pub struct Roots {
    pub home: PathBuf,
    pub local: PathBuf,
    pub roaming: PathBuf,
    pub windows: PathBuf,
    pub temp: PathBuf,
}
impl Roots {
    #[cfg(not(target_os = "macos"))]
    pub fn current() -> Result<Self, String> {
        let get = |key| {
            env::var_os(key)
                .filter(|x| !x.is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| format!("缺少环境目录：{key}"))
        };
        Ok(Self {
            home: get("USERPROFILE")?,
            local: get("LOCALAPPDATA")?,
            roaming: get("APPDATA")?,
            windows: get("SystemRoot")?,
            temp: get("TEMP")?,
        })
    }
    #[cfg(target_os = "macos")]
    pub fn current() -> Result<Self, String> {
        let home = env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or("缺少用户 HOME 目录")?;
        let home = home.canonicalize().map_err(|e| e.to_string())?;
        let temp = env::temp_dir().canonicalize().map_err(|e| e.to_string())?;
        Ok(Self {
            local: home.join("Library/Caches"),
            roaming: home.join("Library/Application Support"),
            windows: "/System".into(),
            home,
            temp,
        })
    }
}
fn rule(
    path: PathBuf,
    source: &str,
    category: &str,
    protection: Protection,
    impact: &str,
    children: bool,
) -> Rule {
    Rule {
        evidence: format!("已知目录规则 · {source}（仅目录关联，不证明文件作者）"),
        path,
        source: source.into(),
        category: category.into(),
        protection,
        impact: impact.into(),
        children,
    }
}
fn windows_rules(r: &Roots) -> Vec<Rule> {
    use Protection::*;
    let mut out = vec![];
    if let Ok(entries) = std::fs::read_dir(r.home.join(".codex")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("state_") || name == "auth.json" || name == "config.toml" {
                out.push(rule(
                    entry.path(),
                    "Codex",
                    "state",
                    Blocked,
                    "包含登录凭据、配置或运行数据库，禁止直接清理。",
                    false,
                ));
            }
        }
    }
    for (base, rel, source, category, protection, impact) in [
        (
            &r.home,
            ".codex/sessions",
            "Codex",
            "chat",
            Review,
            "会丢失本地会话与历史上下文；建议备份。",
        ),
        (
            &r.home,
            ".codex/archived_sessions",
            "Codex",
            "chat",
            Review,
            "会丢失归档会话。",
        ),
        (
            &r.home,
            ".codex/log",
            "Codex",
            "cache",
            Safe,
            "删除诊断日志；关闭 Codex 后处理，可能影响故障排查。",
        ),
        (
            &r.home,
            ".codex/tmp",
            "Codex",
            "temp",
            Safe,
            "可能包含正在运行的任务文件；退出 Codex 后处理。",
        ),
        (
            &r.home,
            ".claude/projects",
            "Claude Code",
            "chat",
            Review,
            "会丢失项目会话和本地历史记录。",
        ),
        (
            &r.home,
            ".claude/debug",
            "Claude Code",
            "cache",
            Safe,
            "删除调试日志，关闭 Claude Code 后可重建。",
        ),
        (
            &r.roaming,
            "Cursor/Cache",
            "Cursor",
            "cache",
            Safe,
            "关闭 Cursor 后可重建缓存。",
        ),
        (
            &r.roaming,
            "Cursor/Code Cache",
            "Cursor",
            "cache",
            Safe,
            "代码缓存会在启动时重建。",
        ),
        (
            &r.roaming,
            "Cursor/GPUCache",
            "Cursor",
            "cache",
            Safe,
            "图形缓存会在启动时重建。",
        ),
        (
            &r.roaming,
            "Cursor/User/workspaceStorage",
            "Cursor",
            "chat",
            Blocked,
            "包含工作区状态和数据库，请在应用内管理。",
        ),
        (
            &r.roaming,
            "Cursor/User/globalStorage",
            "Cursor",
            "state",
            Blocked,
            "包含数据库、扩展和登录状态，禁止直接清理。",
        ),
        (
            &r.home,
            ".cache/huggingface/hub",
            "Hugging Face",
            "model",
            Review,
            "模型需重新下载；可能包含共享链接，建议使用官方缓存管理命令。",
        ),
        (
            &r.home,
            ".cache/lm-studio/models",
            "LM Studio",
            "model",
            Review,
            "移除本地模型，需要重新下载后才能使用。",
        ),
        (
            &r.home,
            ".lmstudio/models",
            "LM Studio",
            "model",
            Review,
            "移除本地模型，需要重新下载后才能使用。",
        ),
        (
            &r.local,
            "pip/Cache",
            "Python / pip",
            "dependency",
            Safe,
            "已安装环境保留；安装依赖时需重新下载。",
        ),
        (
            &r.local,
            "npm-cache/_cacache",
            "Node.js / npm",
            "dependency",
            Safe,
            "项目保留；安装依赖时需重新下载。",
        ),
        (
            &r.local,
            "Google/Chrome/User Data/Default/Cache",
            "Chrome",
            "cache",
            Safe,
            "关闭浏览器后处理；网页缓存将重新下载。",
        ),
        (
            &r.local,
            "Microsoft/Edge/User Data/Default/Cache",
            "Edge",
            "cache",
            Safe,
            "关闭浏览器后处理；网页缓存将重新下载。",
        ),
        (
            &r.local,
            "CrashDumps",
            "Windows",
            "temp",
            Safe,
            "会移除崩溃诊断文件。",
        ),
    ] {
        out.push(rule(
            base.join(rel),
            source,
            category,
            protection,
            impact,
            false,
        ));
    }
    let ollama = env::var_os("OLLAMA_MODELS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| r.home.join(".ollama/models"));
    out.push(rule(
        ollama,
        "Ollama",
        "model",
        Review,
        "包含模型块与清单；建议使用 ollama rm 按模型卸载，整体移除后需重新下载。",
        false,
    ));
    out.push(rule(
        r.temp.clone(),
        "Windows",
        "temp",
        Safe,
        "临时内容可能正在使用；关闭相关程序后处理。",
        true,
    ));
    out.push(rule(
        r.windows.join("Temp"),
        "Windows",
        "temp",
        Safe,
        "系统临时内容；权限不足或占用时跳过。",
        true,
    ));
    out.push(rule(
        r.windows.join("SoftwareDistribution/Download"),
        "Windows",
        "system",
        Blocked,
        "更新缓存应通过 Windows 存储设置清理，避免干扰更新服务。",
        false,
    ));
    if let Some(root) = r.windows.parent() {
        out.push(rule(
            root.join("Windows.old"),
            "Windows",
            "system",
            Blocked,
            "清理后不能回退旧系统，请通过 Windows 存储设置审核处理。",
            false,
        ));
    }
    out.push(rule(
        r.home.join("Downloads"),
        "未知",
        "download",
        Review,
        "下载文件可能包含安装包和个人资料；逐项确认后处理。",
        true,
    ));
    // ComfyUI can be portable: only known user locations; custom locations require registration.
    for base in [r.home.join("ComfyUI"), r.home.join("Documents/ComfyUI")] {
        out.extend(comfy_rules(&base));
    }
    out
}
pub fn default_rules(r: &Roots) -> Vec<Rule> {
    let all = windows_rules(r);
    #[cfg(not(target_os = "macos"))]
    {
        all
    }
    #[cfg(target_os = "macos")]
    {
        let mut all: Vec<_> = all
            .into_iter()
            .filter(|rule| {
                !["Windows", "Chrome", "Edge", "Python / pip", "Node.js / npm"]
                    .contains(&rule.source.as_str())
            })
            .collect();
        for (path, source, category, children) in [
            (r.local.join("pip"), "Python / pip", "dependency", false),
            (
                r.home.join(".npm/_cacache"),
                "Node.js / npm",
                "dependency",
                false,
            ),
            (
                r.local.join("Google/Chrome/Default/Cache"),
                "Chrome",
                "cache",
                false,
            ),
            (
                r.local.join("Microsoft Edge/Default/Cache"),
                "Edge",
                "cache",
                false,
            ),
            (r.local.join("Cursor"), "Cursor", "cache", false),
            (r.temp.clone(), "macOS", "temp", true),
        ] {
            all.push(rule(
                path,
                source,
                category,
                Protection::Safe,
                "已知缓存或临时内容；请关闭相关应用后审核，重新使用时可能需要重建或下载。",
                children,
            ));
        }
        all
    }
}
fn comfy_rules(base: &Path) -> Vec<Rule> {
    vec![
        rule(
            base.join("output"),
            "ComfyUI",
            "output",
            Protection::Review,
            "包含生成图片、视频等成果；请先备份。",
            false,
        ),
        rule(
            base.join("models"),
            "ComfyUI",
            "model",
            Protection::Review,
            "模型需要重新下载；工作流可能无法运行。",
            false,
        ),
        rule(
            base.join("temp"),
            "ComfyUI",
            "temp",
            Protection::Safe,
            "关闭 ComfyUI 后处理预览与临时内容。",
            true,
        ),
    ]
}
pub fn large_file_rules(roots: &Roots) -> Vec<Rule> {
    [
        "Desktop",
        "Documents",
        if cfg!(target_os = "macos") {
            "Movies"
        } else {
            "Videos"
        },
    ]
    .into_iter()
    .map(|folder| {
        let mut item = rule(
            roots.home.join(folder),
            "未知",
            "large",
            Protection::Review,
            "用户目录中的大文件，可能是项目、素材或个人资料；大小不代表可删除，请先核对。",
            false,
        );
        item.evidence = "用户目录大小筛选 · 来源未知".into();
        item
    })
    .collect()
}
pub fn registered_rules(registrations: &[Registration]) -> Result<Vec<Rule>, String> {
    if registrations.len() > 32 {
        return Err("最多登记 32 个目录".into());
    }
    let mut out = vec![];
    for entry in registrations {
        if !entry.path.is_absolute()
            || entry.path.parent().is_none()
            || entry.label.trim().is_empty()
            || entry.label.len() > 100
        {
            return Err("请输入绝对目录路径和 1–100 字的来源名称".into());
        }
        if entry
            .path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err("目录不能包含 ..".into());
        }
        match entry.kind.as_str() {
            "comfyui" => out.extend(comfy_rules(&entry.path)),
            "output" | "unknown" => {
                let mut item = rule(
                    entry.path.clone(),
                    &entry.label,
                    if entry.kind == "output" {
                        "output"
                    } else {
                        "unknown"
                    },
                    Protection::Review,
                    "人工登记仅代表用户声明；可能包含项目或个人成果，需逐项审核。",
                    true,
                );
                item.evidence = format!("人工登记 · {}（未经独立验证）", entry.label);
                out.push(item);
            }
            _ => return Err("不支持的目录类型".into()),
        }
    }
    Ok(out)
}
