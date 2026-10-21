# 磁盘管家 · Disk Cleaner

基于 Tauri 2、Vue 3、Rust 与 [Coolove UI](https://github.com/XingAur/coolove-ui) 的中文 Windows / macOS 桌面磁盘盘点工具。将 AI 工具内容、系统临时文件、应用缓存与下载内容放在同一个工作区，查看来源和删除影响，再人工决定如何处理。

## 使用

Windows 运行 `disk-cleaner-windows-x64-portable.exe`，或使用 `disk-cleaner-windows-x64-setup.exe` 安装；macOS 下载 Universal DMG，拖入 Applications，兼容 Intel 与 Apple Silicon。默认以普通用户权限启动，1280×820 完整窗口，最小 960×640。支持 Windows 10/11 x64 与 macOS 10.15+（Apple Silicon 需 macOS 11+）。Windows 需 Microsoft Edge WebView2 Runtime，不需要 Python。macOS 包采用 ad-hoc 签名，尚未配置 Apple Developer ID 签名或公证。

1. 点击「开始扫描」，盘点已知目录。没有自动选择任何条目。
2. 按分类、来源、大小筛选，支持空格分隔的多关键词及 `*` 通配符搜索。
3. 单击查看来源证据与删除影响；双击目录浏览子项，文本文件可预览前 64 KiB。
4. 用复选框、Ctrl 或 Shift 多选，将内容加入跨页清理清单。聊天、模型、成果与未知来源需要逐项审核。
5. 核对清单、路径与影响，默认移入 Windows 回收站或 macOS Finder 废纸篓。只有已知临时内容可以选择永久删除。
6. 在 Windows 回收站 / Finder 废纸篓中还原或清空。**移入回收站不会立即释放空间**，展示的逻辑大小也不是实际磁盘释放量。

## 系统毛玻璃（实验）

「外观主题」里可以请求系统毛玻璃：由系统模糊窗口后面的桌面，界面按「色调浓度」叠一层主题颜色。默认关闭，关闭时页面恢复不透明背景；透明原生窗口的边角和拖动表现仍需实机确认。

- 自动：Windows 11 用 Acrylic，Windows 10 用 Blur，macOS 用系统毛玻璃；也可以手动选 Acrylic、Mica（仅 Windows 11，取桌面壁纸颜色）或 Blur。
- Windows 10 与 Windows 11 早期版本上 Acrylic 拖动、缩放窗口时可能卡顿，Blur 在 Windows 11 22H2 起有同样问题；遇到卡顿换一种效果或关闭即可。
- 明确不支持的平台或接口调用报错时恢复普通外观；接口成功不保证系统实际应用了效果。窗口已改为透明窗口，macOS 打开了 `macOSPrivateApi` 及配套 Rust feature。
- 快速开启后立即关闭存在旧异步请求覆盖新状态的已知问题。Windows / macOS 原生视觉效果尚未验收，新配置尚需在 macOS 完成构建验证；暂建议保持默认关闭。

## 图标与托盘

应用、安装器、macOS Dock、系统托盘和界面品牌均使用提供的蓝色章鱼图片，保留透明背景与原图比例。标题栏的「隐藏到托盘」可保留进程；点击托盘图标恢复窗口，右键菜单提供打开、隐藏、退出。关闭窗口仍按原有退出行为处理。

## 扫描范围

| 来源 | 内容 | 处理边界 |
| --- | --- | --- |
| Codex | sessions、archived_sessions、log、tmp，以及顶层配置/状态文件 | 会话需审核；凭据、配置、状态数据库禁止处理 |
| Claude Code | projects、debug、shell-snapshots | 会话需审核；调试日志与终端快照列为候选 |
| Cursor | Cache、Code Cache、GPUCache、workspaceStorage、globalStorage | 缓存列为候选；应用状态禁止整体处理 |
| Windsurf | Cache、Code Cache、GPUCache、workspaceStorage、globalStorage | 同 Cursor：缓存列为候选；聊天与应用状态禁止整体处理 |
| VS Code | Cache、CachedData、Code Cache、GPUCache，以及 `.vscode/extensions` | 缓存列为候选；扩展（含 AI 插件）需审核 |
| Ollama | `.ollama/models`，支持 `OLLAMA_MODELS`；按清单逐个列出模型 | 默认保护；按模型清单分析共享数据块，执行时复查引用；存在下述已知限制，当前建议使用 `ollama rm` 管理模型；不是标准布局时整个目录作为一项 |
| ComfyUI | 用户目录/文档目录下的 ComfyUI，或手动登记安装根目录 | output、models 默认保护；temp 为临时内容 |
| Hugging Face / LM Studio | 常见模型目录 | 默认保护；含链接的目录禁止整体处理 |
| ModelScope / PyTorch | `.cache/modelscope/hub`、`.cache/torch/hub` | 默认保护，需要时重新下载 |
| Python / Node.js | pip 下载缓存、uv 缓存、conda 包缓存（miniconda3 / anaconda3 的 pkgs）、npm 内容缓存、pnpm store、Yarn 缓存 | pip、uv、npm、Yarn 缓存列为候选；conda pkgs 与 pnpm store 需审核（建议优先用 conda clean / pnpm store prune）；不扫描项目环境、node_modules 或虚拟环境 |
| Chrome / Edge | 默认用户配置的常见缓存目录 | 关闭浏览器后审核 |
| Windows | 用户临时目录、Windows Temp、CrashDumps、Windows.old、更新下载目录 | 临时内容可审核；Windows.old、更新缓存通过 Windows 存储设置管理 |
| 下载与自定义目录 | Downloads 直接子项，或人工登记目录的直接子项 | 默认保护，可用大小筛选找大文件 |
| 大文件 | Desktop、Documents、Videos 中至少 512 MiB 的文件，最多向下 4 层 | 保留未知来源与默认保护；不进入链接或项目元数据目录 |

macOS 使用 HOME、Library/Caches、Library/Application Support、用户临时目录、Downloads、Movies 等路径；不扫描 Windows 系统目录。废纸篓容量仅统计当前用户的 `~/.Trash`，不含外置卷，权限不足时可能无法统计；清倒操作交由 Finder 确认。

目录规则只表示关联，不证明是谁创建了文件。不解析聊天正文推断作者；没有证据的内容保持未知或人工登记标记。不承诺覆盖所有安装渠道与版本。

「登记目录」支持 API / 自写脚本输出、普通未知目录、ComfyUI 安装根目录，最多 32 个。登记和外观偏好保存在本机 WebView 存储中；扫描内容与清理清单仅保存在内存，不写清理日志，不上传文件。

### Ollama 按模型清理的已知限制

当前实现跳过读取失败的模型清单，可能将共享数据块错误判断为独占；在修复前请使用 `ollama rm` 卸载模型。共享数据块也不会计入每个模型的独占体积，同时删除所有引用模型时可能遗留共享块，所以列表总量不能代表完整模型目录大小或最终可释放空间。

## 安全边界

- 清理命令仅接受本次扫描的条目 ID，客户端不能提交任意删除路径。
- 后端重复验证保护状态、人工审核、允许的删除模式；过期扫描或伪造 ID 会拒绝执行。
- 执行前复查路径及递归目录快照（名称、大小、修改/创建时间、类型、只读属性）。发生变化要求重新扫描；这不是文件内容的密码学校验，也不能代替关闭正在使用文件的程序。
- 不跟随符号链接、junction 或其他重解析点；不接受 UNC、设备路径、路径穿越或数据流。包含凭据、数据库、项目元数据的条目禁止整体清理。
- 扫描取消后，部分结果只供查看；读取不完整或超限的条目禁止处理。每个候选最多 20 万项；单个登记目录最多列出 1 万个直接子项；目录浏览最多 2000 项。
- Windows 默认通过 `IFileOperation` 回收，并通过删除前回调拒绝永久删除兜底。失败保留明确错误，不更改 ACL、不夺取所有权、不自动提权。macOS 使用系统 NSFileManager 废纸篓接口，不退回永久删除。
- 清空回收站独立于文件清单，要求输入确认文本并保留 Windows 原生确认。作用于全部本地磁盘的回收站。
- 权限不足、文件占用和扫描遗漏会导致处理失败。系统还原点、Windows.old 和更新缓存请通过 Windows 管理工具处理。

## 外观与组件库

内置晴空白、曜石黑、薄荷庭院、奶油拿铁、樱花粉、午夜蓝、紫藤夜色、深林绿及跟随系统。所有组件使用 Coolove UI token。

Coolove UI 0.5.0 新增桌面侧栏、导航、工具栏、文件表格、详情栏与清单操作栏；0.7.0 新增系统毛玻璃（`nativeGlass.ts`）。源码在相邻的 `D:\Code\coolove-ui`，应用内的 `src/coolove-ui` 是同步副本，已纳入 Git，CI 无需相邻仓库。

修改组件库后同步：

```powershell
npm run sync:ui
```

## 开发和构建

要求 Node.js 22.18+（推荐 24）、Rust stable、Visual Studio MSVC C++ 工具链及 Windows SDK。

```powershell
npm ci
npm run tauri:dev
```

仅前端预览：`npm run dev`。浏览器始终明确显示演示模式，使用模拟数据，所有实际清理操作禁用。桌面版首次启动为空状态，等待用户主动扫描。

```powershell
npm run verify
npm run tauri:build -- --no-bundle
```

Windows 便携文件输出：`src-tauri/target/release/disk-cleaner-tauri.exe`。分发时命名为 `disk-cleaner-portable.exe`。需要 NSIS 安装包时运行 `npm run tauri:build -- --bundles nsis`。macOS 需在 Mac 上添加两种 Rust target 后执行 `npm run tauri:build -- --target universal-apple-darwin --bundles app,dmg`。

## 验证

`npm run verify` 执行 TypeScript 检查、前端交互逻辑测试、前端构建与 Rust 测试。

新增测试覆盖保护审核、伪造/过期 ID、快照变化、目录去重、临时文件永久删除限制、凭据与数据库保护、目录浏览/文本预览、取消扫描、路径穿越、Windows junction 以及拒绝永久删除兜底。原 1.0 清理器仅在测试构建中保留回归参考，不再暴露桌面命令。

Windows 回收站集成测试单独运行，只将自动生成的微小测试文件移入回收站，**不清空回收站**：

```powershell
cd src-tauri
cargo test --lib native_recycle_of_disposable_fixture -- --ignored --nocapture
```

## 设计参考

功能设计参考 [scarlimith/ai-disk-manager](https://github.com/scarlimith/ai-disk-manager) 的公开介绍。该项目仅发布发行资料，不公开应用源码；本项目独立实现，使用 Coolove UI，不复制参考项目的代码或角色素材。


## GitHub Actions 自动安装包

推送 `main`、提交 PR 或手动运行 [Desktop installers](https://github.com/XingAur/disk-cleaner/actions/workflows/ci.yml) 会执行类型检查、前端逻辑测试、Rust 回归测试并打包。

- `disk-cleaner-windows-x64`：NSIS 安装器、Windows 便携 EXE 与 SHA256 校验文件。
- `disk-cleaner-macos-universal`：同时包含 arm64 / x86_64 的 DMG、保留应用资源的 `.app.zip` 与 SHA256 校验文件。CI 检查两种架构及 ad-hoc 签名。

打开对应成功运行的 Artifacts 下载，保留 30 天。将来推送 `v*` 版本标签时，在两个系统构建成功后自动创建 GitHub Release 并上传安装包；普通 main 推送仅生成 Artifacts。
