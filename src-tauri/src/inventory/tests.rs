use super::*;
use std::sync::atomic::AtomicBool;

fn rule(path: &Path, protection: Protection, category: &str) -> Rule {
    Rule {
        path: path.into(),
        source: "Test".into(),
        category: category.into(),
        evidence: "test fixture".into(),
        impact: "test only".into(),
        protection,
        children: false,
    }
}
fn fixture(protection: Protection, category: &str) -> (tempfile::TempDir, Inventory, Plan) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("example.txt"), "fixture data").unwrap();
    let (inventory, plan) = scan(
        vec![rule(&path, protection, category)],
        &AtomicBool::new(false),
        |_| {},
    );
    (dir, inventory, plan)
}
fn request(inventory: &Inventory) -> CleanupRequest {
    CleanupRequest {
        scan_id: inventory.scan_id.clone(),
        ids: inventory.items.iter().map(|i| i.id.clone()).collect(),
        reviewed_ids: vec![],
        mode: DeleteMode::Recycle,
    }
}
#[test]
fn rules_keep_chats_models_and_outputs_protected() {
    let roots = rules::Roots {
        home: "C:/test".into(),
        local: "C:/local".into(),
        roaming: "C:/roaming".into(),
        windows: "C:/Windows".into(),
        temp: "C:/temp".into(),
    };
    let all = rules::default_rules(&roots);
    for source in ["Codex", "Claude Code", "Cursor", "Ollama", "ComfyUI"] {
        assert!(all.iter().any(|r| r.source == source));
    }
    assert!(all
        .iter()
        .filter(|r| ["chat", "model", "output"].contains(&r.category.as_str()))
        .all(|r| r.protection != Protection::Safe));
}
#[test]
fn ai_tool_and_package_cache_rules_follow_protection_levels() {
    let roots = rules::Roots {
        home: "C:/test".into(),
        local: "C:/local".into(),
        roaming: "C:/roaming".into(),
        windows: "C:/Windows".into(),
        temp: "C:/temp".into(),
    };
    let all = rules::default_rules(&roots);
    let find = |source: &str, tail: &str| {
        all.iter()
            .find(|r| r.source == source && r.path.ends_with(tail))
            .unwrap_or_else(|| panic!("missing rule {source} {tail}"))
    };
    for source in [
        "Windsurf",
        "VS Code",
        "ModelScope",
        "PyTorch",
        "Python / uv",
        "Python / conda",
        "Node.js / Yarn",
    ] {
        assert!(all.iter().any(|r| r.source == source), "{source}");
    }
    // 聊天与应用状态禁止整体处理，模型与扩展需要审核，缓存可直接处理
    assert_eq!(
        find("Windsurf", "workspaceStorage").protection,
        Protection::Blocked
    );
    assert_eq!(
        find("Windsurf", "globalStorage").protection,
        Protection::Blocked
    );
    assert_eq!(find("VS Code", "extensions").protection, Protection::Review);
    assert_eq!(find("ModelScope", "hub").protection, Protection::Review);
    assert_eq!(find("PyTorch", "hub").protection, Protection::Review);
    assert_eq!(
        find("Python / conda", "pkgs").protection,
        Protection::Review
    );
    assert_eq!(find("VS Code", "CachedData").protection, Protection::Safe);
    assert_eq!(find("Claude Code", "shell-snapshots").category, "cache");
}
#[cfg(target_os = "macos")]
#[test]
fn macos_rules_use_library_caches_and_exclude_windows_system_rules() {
    let r = rules::Roots::current().unwrap();
    let all = rules::default_rules(&r);
    assert!(r.local.ends_with("Library/Caches"));
    assert!(all.iter().all(|rule| rule.source != "Windows"));
    assert!(all
        .iter()
        .any(|rule| rule.source == "macOS" && rule.category == "temp"));
    assert!(all
        .iter()
        .any(|rule| rule.path == r.home.join(".npm/_cacache")));
    assert!(all.iter().any(|rule| rule.path == r.home.join(".cache/uv")));
    assert!(!all.iter().any(|rule| rule.path == r.local.join("uv/cache")));
}
#[cfg(target_os = "macos")]
#[test]
fn macos_paths_do_not_collapse_case_sensitive_siblings() {
    assert!(!within(
        Path::new("/Users/me/cache"),
        Path::new("/Users/me/Cache")
    ));
}
#[cfg(target_os = "macos")]
#[test]
fn native_macos_recycle_only_uses_disposable_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("disk-cleaner-macos-trash-test.txt");
    fs::write(&path, "disposable macOS trash integration fixture").unwrap();
    platform::recycle(&path).unwrap();
    assert!(!path.exists());
}
#[test]
fn scanned_items_are_deduplicated_and_count_logical_bytes() {
    let (_dir, inv, _plan) = fixture(Protection::Safe, "cache");
    let path = &inv.items[0].path;
    let (inv, _) = scan(
        vec![
            rule(path, Protection::Safe, "cache"),
            rule(path, Protection::Safe, "cache"),
        ],
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(inv.items.len(), 1);
    assert_eq!(inv.items[0].size_bytes, 12);
    assert_eq!(inv.items[0].file_count, 1);
}
#[test]
fn indexed_dedup_distinguishes_siblings_and_ancestors() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("cache");
    let mut paths = BTreeSet::new();
    paths.insert(key(&parent.join("nested")));
    assert!(overlaps(&paths, &parent));
    assert!(!overlaps(&paths, &dir.path().join("cache2")));
    paths.clear();
    paths.insert(key(&parent));
    assert!(overlaps(&paths, &parent.join("nested")));
    assert!(overlaps(&paths, &parent));
}
#[test]
fn protected_items_require_explicit_review() {
    let (_dir, inv, plan) = fixture(Protection::Review, "output");
    let mut req = request(&inv);
    assert!(prepare(&plan, &req).is_err());
    req.reviewed_ids = req.ids.clone();
    assert!(prepare(&plan, &req).is_ok());
}
#[test]
fn blocked_items_cannot_be_unlocked_by_frontend() {
    let (_dir, inv, plan) = fixture(Protection::Blocked, "state");
    let mut req = request(&inv);
    req.reviewed_ids = req.ids.clone();
    assert!(prepare(&plan, &req).is_err());
}
#[test]
fn unknown_ids_and_old_scans_are_rejected() {
    let (_dir, inv, plan) = fixture(Protection::Safe, "cache");
    let mut req = request(&inv);
    req.ids.push("forged".into());
    assert!(prepare(&plan, &req).is_err());
    req = request(&inv);
    req.scan_id = "old".into();
    assert!(prepare(&plan, &req).is_err());
}
#[test]
fn changing_a_file_or_adding_a_child_invalidates_snapshot() {
    let (_dir, inv, plan) = fixture(Protection::Safe, "temp");
    fs::write(inv.items[0].path.join("new.txt"), "new").unwrap();
    assert!(prepare(&plan, &request(&inv)).is_err());
}
#[test]
fn permanent_mode_is_only_allowed_for_known_temp_items() {
    let (_dir, inv, plan) = fixture(Protection::Safe, "cache");
    let mut req = request(&inv);
    req.mode = DeleteMode::Permanent;
    assert!(prepare(&plan, &req).is_err());
    let (_dir, inv, plan) = fixture(Protection::Safe, "temp");
    let mut req = request(&inv);
    req.mode = DeleteMode::Permanent;
    let result = cleanup(&plan, &req, |_| {}).unwrap();
    assert_eq!(result.deleted_bytes, 12);
    assert_eq!(result.recycled_bytes, 0);
    assert!(!inv.items[0].path.exists());
}
#[test]
fn credentials_and_databases_block_entire_container() {
    for name in ["auth.json", "state_5.sqlite", ".env", "key.pem"] {
        let (_dir, inv, _) = fixture(Protection::Safe, "temp");
        fs::write(inv.items[0].path.join(name), "secret fixture").unwrap();
        let (inv, plan) = scan(
            vec![rule(&inv.items[0].path, Protection::Safe, "temp")],
            &AtomicBool::new(false),
            |_| {},
        );
        assert_eq!(inv.items[0].protection, Protection::Blocked);
        assert!(prepare(&plan, &request(&inv)).is_err());
    }
}
#[test]
fn browse_returns_trusted_child_ids_and_preview_is_bounded() {
    let (_dir, inv, mut plan) = fixture(Protection::Review, "output");
    let items = browse(&mut plan, &inv.items[0].id).unwrap();
    assert_eq!(preview(&plan, &items[0].id).unwrap(), "fixture data");
    assert!(preview(&plan, "C:/Windows/System32/config/SAM").is_err());
    fs::write(&items[0].path, vec![b'a'; 100_000]).unwrap();
    assert!(preview(&plan, &items[0].id).unwrap().len() < 66000);
}
#[test]
fn parent_child_selection_is_executed_only_once() {
    let (_dir, inv, mut plan) = fixture(Protection::Safe, "temp");
    let items = browse(&mut plan, &inv.items[0].id).unwrap();
    let mut req = request(&inv);
    req.ids.insert(0, items[0].id.clone());
    assert_eq!(prepare(&plan, &req).unwrap().len(), 1);
}
#[test]
fn registrations_reject_relative_paths_and_unknown_kinds() {
    assert!(rules::registered_rules(&[Registration {
        path: "relative".into(),
        label: "test".into(),
        kind: "output".into()
    }])
    .is_err());
    let dir = tempfile::tempdir().unwrap();
    assert!(rules::registered_rules(&[Registration {
        path: dir.path().into(),
        label: "test".into(),
        kind: "cache".into()
    }])
    .is_err());
}
#[test]
fn cancellation_produces_no_actionable_items() {
    let (_dir, inv, _) = fixture(Protection::Safe, "cache");
    let cancel = AtomicBool::new(false);
    let (result, _) = scan(
        vec![rule(&inv.items[0].path, Protection::Safe, "cache")],
        &cancel,
        |_| {
            cancel.store(true, Ordering::Relaxed);
        },
    );
    assert!(result.cancelled);
    assert!(result
        .items
        .iter()
        .all(|i| i.protection == Protection::Blocked));
}
#[test]
fn path_validation_rejects_parent_traversal() {
    let dir = tempfile::tempdir().unwrap();
    assert!(snapshot::safe_path(&dir.path().join("../escape")).is_err());
}
#[test]
fn registered_credential_directory_cannot_bypass_protection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".aws");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("config"), "fixture credentials").unwrap();
    let (inv, _) = scan(
        vec![Rule {
            children: true,
            ..rule(&path, Protection::Review, "unknown")
        }],
        &AtomicBool::new(false),
        |_| {},
    );
    assert!(inv
        .items
        .iter()
        .all(|i| i.protection == Protection::Blocked));
}
#[test]
fn large_file_scan_keeps_only_large_files_as_protected_unknowns() {
    let dir = tempfile::tempdir().unwrap();
    let large = dir.path().join("large.bin");
    // Sparse fixture: reserves logical size without writing a 512 MiB buffer.
    fs::File::create(&large)
        .unwrap()
        .set_len(512 * 1024 * 1024)
        .unwrap();
    fs::write(dir.path().join("small.txt"), "small").unwrap();
    let (inv, _) = scan(
        vec![rule(dir.path(), Protection::Review, "large")],
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(inv.items.len(), 1);
    assert_eq!(inv.items[0].path, large);
    assert_eq!(inv.items[0].protection, Protection::Review);
    assert!(!inv.items[0].permanent_allowed);
}
#[cfg(windows)]
#[test]
#[ignore = "manual Windows integration: recycles one disposable fixture, never empties the bin"]
fn native_recycle_of_disposable_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("disk-cleaner-recycle-test.txt");
    fs::write(&path, "disk-cleaner disposable recycle integration fixture").unwrap();
    platform::recycle(&path).unwrap();
    assert!(!path.exists());
}
#[cfg(windows)]
#[test]
fn junctions_never_allow_outside_tree_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("keep.txt"), "keep").unwrap();
    // Junctions do not require Developer Mode; mklink only affects this disposable fixture.
    let link = dir.path().join("linked");
    let result = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(snapshot::inspect(&link, &AtomicBool::new(false)).is_err());
    assert!(snapshot::inspect(&link.join("keep.txt"), &AtomicBool::new(false)).is_err());
    // Remove the junction itself; never recursively delete its target.
    fs::remove_dir(&link).unwrap();
    assert!(outside.path().join("keep.txt").exists());
}
fn ollama_fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("models");
    let lib = root.join("manifests/registry.ollama.ai/library");
    fs::create_dir_all(lib.join("qwen2.5")).unwrap();
    fs::create_dir_all(lib.join("llama3.2")).unwrap();
    fs::create_dir_all(root.join("blobs")).unwrap();
    let digest = |c: char| std::iter::repeat(c).take(64).collect::<String>();
    for (c, size) in [('a', 100), ('b', 40), ('c', 7)] {
        fs::write(
            root.join(format!("blobs/sha256-{}", digest(c))),
            vec![0u8; size],
        )
        .unwrap();
    }
    // a 只属于 qwen2.5，b 只属于 llama3.2，c（许可证之类）两者共用
    let manifest = |own: char| {
        format!(
            r#"{{"config":{{"digest":"sha256:{}"}},"layers":[{{"digest":"sha256:{}"}}]}}"#,
            digest('c'),
            digest(own)
        )
    };
    fs::write(lib.join("qwen2.5/14b"), manifest('a')).unwrap();
    fs::write(lib.join("llama3.2/3b"), manifest('b')).unwrap();
    (dir, root)
}
fn ollama_rule(root: &Path) -> Rule {
    Rule {
        source: "Ollama".into(),
        ..rule(root, Protection::Review, "model")
    }
}
#[test]
fn ollama_helpers_parse_digests_and_names() {
    let hex = "AB".repeat(32);
    assert_eq!(
        ollama::digests(&format!("x sha256:{hex} sha256:{hex} sha256:123")),
        vec![format!("sha256-{}", hex.to_lowercase())]
    );
    let parts = |s: &str| s.split('/').map(String::from).collect::<Vec<_>>();
    assert_eq!(
        ollama::model_name(&parts("registry.ollama.ai/library/qwen2.5/14b")),
        "qwen2.5:14b"
    );
    assert_eq!(
        ollama::model_name(&parts("registry.ollama.ai/me/tool/latest")),
        "me/tool:latest"
    );
    assert_eq!(
        ollama::model_name(&parts("hf.co/org/repo/q4")),
        "hf.co/org/repo:q4"
    );
}
#[test]
fn ollama_models_are_listed_per_model_without_shared_blobs() {
    let (_dir, root) = ollama_fixture();
    let (inv, plan) = scan(vec![ollama_rule(&root)], &AtomicBool::new(false), |_| {});
    let names: Vec<_> = inv.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names.len(), 2);
    let qwen = inv.items.iter().find(|i| i.name == "qwen2.5:14b").unwrap();
    let manifest_len = fs::metadata(&qwen.path).unwrap().len();
    assert_eq!(qwen.size_bytes, 100 + manifest_len);
    assert_eq!(qwen.protection, Protection::Review);
    assert!(!qwen.permanent_allowed);
    assert!(qwen.impact.contains("共用的 1 个数据块会保留"));
    let parts = plan.entries[&qwen.id].ollama.clone().unwrap();
    assert_eq!(parts.blobs.len(), 1);
    assert!(parts.blobs[0].ends_with(format!("sha256-{}", "a".repeat(64))));
    // 整个 models 目录不会再作为一项出现
    assert!(inv.items.iter().all(|i| i.path != root));
}
#[test]
fn removing_an_ollama_model_keeps_blobs_other_models_use() {
    let (_dir, root) = ollama_fixture();
    let (inv, plan) = scan(vec![ollama_rule(&root)], &AtomicBool::new(false), |_| {});
    let qwen = inv.items.iter().find(|i| i.name == "qwen2.5:14b").unwrap();
    let entry = &plan.entries[&qwen.id];
    // 扫描之后又拉取了一个也用到 a 的模型：a 不能再删
    let a = format!("sha256:{}", "a".repeat(64));
    let lib = root.join("manifests/registry.ollama.ai/library");
    fs::create_dir_all(lib.join("qwen-new")).unwrap();
    fs::write(
        lib.join("qwen-new/latest"),
        format!(r#"{{"layers":[{{"digest":"{a}"}}]}}"#),
    )
    .unwrap();
    let bytes =
        remove_ollama_model(entry, entry.ollama.as_ref().unwrap(), DeleteMode::Permanent).unwrap();
    assert!(!qwen.path.exists());
    assert_eq!(bytes, fs::metadata(lib.join("llama3.2/3b")).unwrap().len());
    for c in ['a', 'b', 'c'] {
        assert!(root
            .join(format!("blobs/sha256-{}", c.to_string().repeat(64)))
            .exists());
    }
    // 再卸载 llama3.2：b 和（现在只剩它在用的）c 一起删，qwen-new 用的 a 保留
    let (inv, plan) = scan(vec![ollama_rule(&root)], &AtomicBool::new(false), |_| {});
    let llama = inv.items.iter().find(|i| i.name == "llama3.2:3b").unwrap();
    let entry = &plan.entries[&llama.id];
    remove_ollama_model(entry, entry.ollama.as_ref().unwrap(), DeleteMode::Permanent).unwrap();
    let blob = |c: char| root.join(format!("blobs/sha256-{}", c.to_string().repeat(64)));
    assert!(blob('a').exists());
    assert!(!blob('b').exists());
    assert!(!blob('c').exists());
}
#[test]
fn ollama_directory_without_manifests_is_kept_as_one_item() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("models");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("model.gguf"), "weights").unwrap();
    let (inv, plan) = scan(vec![ollama_rule(&root)], &AtomicBool::new(false), |_| {});
    assert_eq!(inv.items.len(), 1);
    assert_eq!(inv.items[0].path, root);
    assert!(plan.entries[&inv.items[0].id].ollama.is_none());
}
