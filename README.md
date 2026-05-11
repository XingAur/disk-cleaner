# C盘清理助手

一个轻量级 Windows C 盘清理工具，基于 Tauri + Vue + Rust 实现。应用以免安装单文件 EXE 为主要交付目标，启动时请求管理员权限，扫描和清理过程均带进度反馈。

## 下载

请在 GitHub Release 中下载免安装版本：

```text
disk-cleaner-portable.exe
```

下载后直接运行即可。首次启动会触发 Windows UAC 管理员权限确认，以尽可能清理系统临时目录、回收站和 Windows 更新缓存。

## 功能特性

- 以 C 盘空间治理为主，额外聚合所有本地磁盘的回收站，避免 Windows 回收站合并视图漏扫。
- 展示 C 盘总容量、当前剩余空间、可清理空间；清理完成后自动重新扫描刷新剩余空间。
- 支持浅色、深色、跟随系统主题。
- 清理范围按轻度、标准、深度拆成独立卡片，用户自行选择要清理的部分。
- 主界面按系统临时、应用缓存、Windows.old、回收站、系统缓存、大文件、下载内容、系统备份点分组展示。
- 清理前使用应用内统一弹窗二次确认。
- 清理过程永久删除选中项目，完成后会自动重新扫描刷新结果；失败、占用、权限不足和已不存在的文件会分类统计并写入日志。
- 系统备份点单独列出，不参与普通文件清理。
- 内置 Windows Common Controls v6 manifest，避免部分系统上原生弹窗入口点异常。

## 卡片等级

| 等级 | 卡片 | 说明 |
| --- | --- | --- |
| 轻度 | 系统临时 | 默认勾选，主要处理 Temp、崩溃转储、缩略图缓存等低风险项目 |
| 标准 | 应用缓存、Windows.old、回收站、系统缓存 | 不默认勾选，适合确认后释放更多空间 |
| 深度 | 大文件、下载内容、系统备份点 | 不默认勾选或不参与普通清理，需要用户谨慎判断 |

## 技术栈

- Tauri 2
- Vue 3
- TypeScript
- Rust
- MSVC Windows 构建链

## 环境要求

- Windows 10/11 x64
- Node.js 18+
- Rust stable
- Microsoft Visual Studio Build Tools，需包含 MSVC C++ 工具链和 Windows SDK

## 开发

```bash
npm ci
npm run tauri:dev
```

## 构建

```bash
npm ci
npm run tauri:build
```

构建完成后，便携 EXE 位于：

```text
src-tauri/target/release/disk-cleaner-tauri.exe
```

NSIS 安装包位于：

```text
src-tauri/target/release/bundle/nsis/
```

## 验证

```bash
npm ci
npm run verify
```

如果只验证 Rust 清理规则和安全边界：

```bash
npm run test:rust
```

## 项目结构

```text
.
|-- public/
|-- src/
|-- src-tauri/
|   |-- icons/
|   |-- src/
|   |-- build.rs
|   |-- Cargo.toml
|   `-- tauri.conf.json
|-- .github/
|   `-- workflows/
|-- package.json
`-- README.md
```

## 安全说明

本工具会请求管理员权限，并对用户确认的项目执行永久删除。请在执行清理前确认选中的卡片范围，尤其是 Windows.old、回收站、系统缓存、大文件和旧下载项。

管理员权限可以提升系统目录清理成功率，但无法删除正在被系统或应用占用的文件。这类文件会跳过、写入日志，并在清理后的重新扫描中继续显示。

清理命令不会信任前端传回的文件路径。应用会先保存最近一次扫描计划，清理时前端只提交项目 ID，后端只会从扫描计划中的已知项目里解析要删除的文件。

## 发布

推荐使用 GitHub Release 分发免安装 EXE。发布前至少执行：

```bash
npm run verify
npm run tauri:build
```

便携 EXE 使用 `src-tauri/target/release/disk-cleaner-tauri.exe`，上传到 Release 时建议命名为 `disk-cleaner-portable.exe`。

CI 会在 Windows 环境中执行依赖安装、类型检查、前端构建、Rust 测试和 Tauri 构建，并上传便携 EXE 与 NSIS 安装包作为 artifacts。

## 上传仓库前

以下内容属于可再生成内容，已通过 `.gitignore` 排除：

- `node_modules/`
- `dist/`
- `src-tauri/target/`
- `dist-exe/`
- `.worktrees/`
- 安装包和其他临时 EXE
