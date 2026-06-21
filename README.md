<!-- markdownlint-disable MD033 MD041 -->
<p align="center">
  <img style="height:200px;width:200px" src="./.github/assets/Irena720.png" alt="jlucourse" />
</p>

<div align="center">

# 🚀 jlucourse · FunkyLesson

**基于 Rust 生态的吉林大学智能选课助手 — Apple 风格界面**

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Leptos](https://img.shields.io/badge/frontend-Leptos-red.svg)](https://leptos.dev/)
[![Tauri](https://img.shields.io/badge/framework-Tauri%202-blue.svg)](https://tauri.app/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

*现代化、跨平台、纯 Rust 实现的选课工具*

[✨ 特性](#-特性) • [📥 快速开始](#-快速开始) • [🚀 本地开发](#-本地开发) • [🛠️ 技术架构](#️-技术架构) • [📖 使用指南](#-使用指南) • [⚖️ 免责声明](#️-免责声明) • [💖 致谢](#-致谢)

</div>

---

## ✨ 特性

- **🍎 Apple 风格界面** — SF 字体、`#f5f5f7` 背景、Apple 蓝主色、柔和圆角与阴影、清爽的蓝色聚焦环；登录 / 批次选择 / 抢课控制台三屏统一设计语言。
- **⚡ 多线程并发抢课** — 12 路 worker 共享游标 + 每门课 `done` 标记，对收藏课程并发轮询；一门成功只停该门、不影响其它，致命错误（未登录）才整体停止。
- **🛟 抢课脚本「兜底」** — 自动脚本之外的两层手动保险：
  - **手动补刀**：每门课「选一次」、列表「全部各选一次」，复用当前登录态直接打一发（桌面 / Web / Android 通用）。
  - **内嵌官方网站**：桌面端一键在内嵌窗口打开官方选课页，并**自动带入登录态**（同一会话、同一设备，无需二次登录、不会把自己挤下线）。
- **🌐 自建 CORS 代理** — 内置 actix 代理桥接官网无 CORS 的接口，连接池复用 + 超时保护 + SSRF 白名单 + 凭据日志脱敏。
- **🖥️ 跨平台** — Windows / macOS / Linux 桌面端，以及 Android。
- **🦀 纯 Rust** — 前端 Leptos（WASM），外壳 Tauri 2，核心逻辑来自 [`funky_lesson_core`](https://github.com/Islatri/funky_lesson_core)。

## 📥 快速开始

### 桌面端

前往 [Releases](https://github.com/sweetcornna/jlucourse/releases) 下载对应平台的安装包（Windows `.msi/.exe`、macOS `.dmg`、Linux `.deb/.AppImage`），双击安装运行。

> 若 Release 暂无你平台的预编译包，可按下方[本地开发](#-本地开发)自行构建。

## 🚀 本地开发

### 环境要求

- **Rust** 1.85+（edition 2024）
- **Trunk**（前端打包）：`cargo install trunk` 或包管理器安装
- **Tauri CLI** 2.x：`cargo install tauri-cli` 或 `cargo binstall tauri-cli`
- `wasm32-unknown-unknown` target：`rustup target add wasm32-unknown-unknown`

### 运行

```bash
git clone https://github.com/sweetcornna/jlucourse.git
cd jlucourse

# 原生桌面（推荐）：自动起 trunk + 内置代理
cargo tauri dev

# 纯 Web 调试（快速改 UI）：分别启动前端与代理
trunk serve                                            # 前端，端口 1420
cargo run --manifest-path src-proxy/Cargo.toml         # CORS 代理，端口 3030
```

> ⚠️ **CORS 注意**：Web 调试流请在浏览器打开 **http://localhost:1420**，不要用 `127.0.0.1`——代理只放行 `localhost:1420` / `tauri.localhost` 来源，用 `127.0.0.1` 会触发 “Failed to fetch”。

### 构建 / 打包

```bash
cargo tauri build              # 当前平台安装包
cargo tauri android build      # Android APK / AAB
```

> 仓库自带 GitHub Actions（`.github/workflows/ci.yml`）：push/PR 跑 fmt + clippy + test；打 `v*` tag 会构建多平台产物。Android 签名构建需在仓库 Secrets 配置 `ANDROID_KEYSTORE_*`，未配置时该任务会失败，可忽略或自行补齐。

## 🛠️ 技术架构

```text
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  Leptos 前端     │ ⇄  │  Tauri 2 外壳    │ ⇄  │ funky_lesson_core │
│  (Rust / WASM)   │    │  + 内嵌代理      │    │  核心选课逻辑     │
└──────────────────┘    └──────────────────┘    └──────────────────┘
         │  HTTP                                   ↑ Authorization
         └────────────►  actix 代理 (127.0.0.1:3030)  ──►  icourses.jlu.edu.cn
```

```text
jlucourse/
├── src/                 # Leptos 前端（app.rs 视图与抢课逻辑 / design.css 设计系统）
├── src-tauri/           # Tauri 外壳（含「内嵌官网兜底」命令）
├── src-proxy/           # actix CORS 代理（库 + 二进制薄包装）
├── design/              # Apple 设计预览（同步至 Claude Design 的设计稿来源）
└── .github/             # CI 与素材
```

## 📖 使用指南

1. **收藏课程**：先到[吉大本科生选课网站](https://icourses.jlu.edu.cn/xsxk/profile/index.html)收藏想抢的课。
2. **登录**：输入学号 + 密码 + 验证码。
3. **选批次** → 进入**抢课控制台**。
4. **开始抢课**：自动多线程轮询收藏课程。
5. **兜底**：脚本抢不到时——点某门课的「选一次」手动补刀，或点「官方选课网站」在内嵌窗口手动操作（桌面端自动带入登录态）。

> 临近开抢、服务器 503 时**不要退出**，等网络恢复程序会自动继续。

## ⚖️ 免责声明

- 📚 本软件仅供学习与研究，请勿用于任何违反学校规定或法律法规的行为。
- 🛡️ 使用本软件产生的一切后果由使用者自行承担。
- 🏫 本软件**未经吉林大学官方授权**，与吉林大学无任何关联。

> 使用本程序即代表你已理解并同意以上声明。

## 💖 致谢

本项目基于 [**Islatri/funky-lesson**](https://github.com/Islatri/funky-lesson)（MIT）二次开发，并在其上完成了一轮稳定性 / 性能优化、Apple 风格重设计与「抢课兜底」功能。

- [funky-lesson](https://github.com/Islatri/funky-lesson) — 上游项目
- [funky_lesson_core](https://github.com/Islatri/funky_lesson_core) — 核心选课逻辑库
- [Fuck-Lesson](https://github.com/H4ckF0rFun/Fuck-Lesson) — 最初的 Python 选课脚本

## 📄 开源协议

[MIT License](LICENSE)。
