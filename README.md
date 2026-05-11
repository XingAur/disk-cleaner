# C盘清理助手

一个轻量级 Windows C 盘清理工具，基于 Tauri + Vue + Rust 实现。应用以免安装单文件 EXE 为主要交付目标，启动时请求管理员权限，扫描和清理过程均带进度反馈。

## 下载

仓库内已包含当前便携免安装版本：

```text
dist-exe/C盘清理助手.exe
```

下载后直接运行即可。首次启动会触发 Windows UAC 管理员权限确认。

## 功能特性

- 只扫描系统盘，降低误扫其他磁盘的风险。
- 支持浅色、深色、跟随系统主题。
- 清理范围按轻度、标准、深度拆成独立卡片，用户自行选择要清理的部分。
- 主界面按系统临时、应用缓存、Windows.old、大文件、旧下载、系统备份点分组展示。
- 清理前使用应用内统一弹窗二次确认。
- 清理过程永久删除选中项目，失败或占用文件会跳过并写入日志。
- 系统备份点单独列出，不参与普通文件清理。
- 内置 Windows Common Controls v6 manifest，避免部分系统上原生弹窗入口点异常。

## 卡片等级

| 等级 | 卡片 | 说明 |
| --- | --- | --- |
| 轻度 | 系统临时 | 默认勾选，主要处理 Temp、崩溃转储、缩略图缓存等低风险项目 |
| 标准 | 应用缓存、Windows.old | 不默认勾选，适合确认后释放更多空间 |
| 深度 | 大文件、旧下载、系统备份点 | 不默认勾选或不参与普通清理，需要用户谨慎判断 |

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
npm install
npm run tauri:dev
```

## 构建

```bash
npm install
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

## 测试

```bash
cd src-tauri
cargo test --lib --test cleaner_tests
```

## 项目结构

```text
.
|-- dist-exe/
|   `-- C盘清理助手.exe
|-- public/
|-- src/
|-- src-tauri/
|   |-- icons/
|   |-- src/
|   |-- tests/
|   |-- build.rs
|   |-- Cargo.toml
|   `-- tauri.conf.json
|-- package.json
`-- README.md
```

## 安全说明

本工具会请求管理员权限，并对用户确认的项目执行永久删除。请在执行清理前确认选中的卡片范围，尤其是 Windows.old、大文件和旧下载项。

## 上传仓库前

以下内容属于可再生成内容，已通过 `.gitignore` 排除：

- `node_modules/`
- `dist/`
- `src-tauri/target/`
- 安装包和其他临时 EXE

当前仅保留 `dist-exe/C盘清理助手.exe` 作为仓库内便携发布文件。
