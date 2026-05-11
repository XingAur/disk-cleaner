# Three Stage Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the disk cleaner reproducible to build, safer to run with administrator rights, and easier to maintain and release.

**Architecture:** Keep the existing Tauri 2 + Vue 3 shape, but move trust boundaries into Rust. The frontend should select opaque item ids, while the backend resolves those ids from the latest scan plan and performs deletion only for known scanned items.

**Tech Stack:** Vue 3, TypeScript, Vite, Tauri 2, Rust, Cargo tests, GitHub Actions.

---

### Task 1: Restore Reproducible Dependency Installation

**Files:**
- Modify: `package-lock.json`
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Prove the current baseline fails**

Run: `npm ci --registry=https://registry.npmjs.org`

Expected: FAIL with `E401` because `package-lock.json` contains private `packages.aliyun.com/.../repo-rhbkx/` tarball URLs.

Run: `cd src-tauri; cargo test --lib --test cleaner_tests`

Expected: FAIL with `frontendDist` missing because the Tauri bin is compiled during tests.

- [ ] **Step 2: Rewrite lockfile tarball URLs**

Replace every `resolved` URL prefix:

```text
https://packages.aliyun.com/64cc7343a0c93ee7446892d5/npm/repo-rhbkx/
```

with:

```text
https://registry.npmjs.org/
```

Keep integrity hashes unchanged.

- [ ] **Step 3: Add stable verification scripts**

In `package.json`, add:

```json
"check": "vue-tsc --noEmit",
"test:rust": "cd src-tauri && cargo test --lib --test cleaner_tests",
"verify": "npm run check && npm run build && npm run test:rust"
```

- [ ] **Step 4: Stop test builds from requiring frontend dist**

In `src-tauri/Cargo.toml`, set the bin target to:

```toml
[[bin]]
name = "disk-cleaner-tauri"
path = "src/main.rs"
test = false
```

If Cargo still compiles the Tauri bin for integration tests, move `src-tauri/tests/cleaner_tests.rs` to `src-tauri/src/cleaner_tests.rs`, register it from `src-tauri/src/lib.rs`, and run `cargo test --lib`.

- [ ] **Step 5: Verify dependency and Rust baseline**

Run: `npm ci --registry=https://registry.npmjs.org`

Expected: PASS.

Run: `npm run test:rust`

Expected: PASS.

### Task 2: Enforce Backend Cleanup Selection

**Files:**
- Modify: `src-tauri/src/cleaner.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tests/cleaner_tests.rs`
- Modify: `src/App.vue`

- [ ] **Step 1: Write failing tests for item ids and selection**

Add tests that assert scanned cleanup items have non-empty ids, selected ids resolve to files from `items` and `suggestions`, and unknown ids are rejected.

- [ ] **Step 2: Run tests to verify RED**

Run: `npm run test:rust`

Expected: FAIL because `CleanupItem` has no `id` and no selection helper exists.

- [ ] **Step 3: Add ids and selection helper**

Add an `id: String` field to `CleanupItem` with serde default compatibility. Generate ids when scanner creates cleanup items. Add:

```rust
pub fn selected_items_from_plan(
    plan: &CleanupPlan,
    item_ids: &[String],
) -> Result<Vec<CleanupItem>, String>
```

This helper should search only `plan.items` and `plan.suggestions`, preserve requested order, set `default_selected = true`, and reject unknown ids.

- [ ] **Step 4: Store last scan plan in Tauri state**

Add a `Mutex<Option<CleanupPlan>>` state in `main.rs`. Store the latest scan result after `scan_system_drive`. Change `cleanup_selected` to accept `item_ids: Vec<String>` and resolve them through `selected_items_from_plan`.

- [ ] **Step 5: Change frontend cleanup payload**

Keep displaying full items, but send only:

```ts
itemIds: selectedItems.value.map(item => item.id)
```

to `cleanup_selected`.

- [ ] **Step 6: Verify GREEN**

Run: `npm run test:rust`

Expected: PASS.

Run: `npm run check`

Expected: PASS.

### Task 3: Productize Maintenance And Release

**Files:**
- Create: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `.gitignore`

- [ ] **Step 1: Add CI workflow**

Create a Windows CI workflow that runs `npm ci`, `npm run check`, `npm run build`, `npm run test:rust`, and `npm run tauri:build`. Upload the release exe as an artifact.

- [ ] **Step 2: Ignore local worktrees and generated schema churn**

Add `.worktrees/` to `.gitignore`. Leave generated schemas tracked, but do not treat CRLF-only changes as feature work.

- [ ] **Step 3: Document reproducible setup and release flow**

Update README with a short "Verification" section:

```bash
npm ci
npm run verify
```

Update release notes to say GitHub Actions artifacts are preferred for distribution, while `dist-exe/` is only a convenience copy.

- [ ] **Step 4: Final verification**

Run: `npm run verify`

Expected: PASS.

Run: `npm run tauri:build`

Expected: PASS and produce `src-tauri/target/release/disk-cleaner-tauri.exe`.
