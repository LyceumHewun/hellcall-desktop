# Hellcall Desktop

Hellcall Desktop 是一款基于 Tauri、React 和 Rust 构建的跨平台桌面应用程序。它提供了一种无缝配置和管理语音激活键盘宏的方式——旨在增强游戏体验（例如在《绝地潜兵 2》(Helldivers 2) 中呼叫战略配备）。

[English](../README.md) | 中文

![Hellcall Desktop Preview](../preview.png)

## 功能特性

- **语音激活宏：** 使用麦克风触发自定义按键序列。
- **可配置的识别器：** 调整 VAD（语音活动检测）静音持续时间和音频块(chunk)时间，以获得最佳响应速度。
- **高级按键模拟：** 微调按键释放间隔、按键间间隔以及初始等待时间。
- **自定义触发器和宏：** 定义特定的命中词、语法和后备快捷键来触发序列。您还可以分配自定义的音频反馈（例如，`normal_reply.wav`）。
- **基于视觉的 OCC：** 使用实验性的“一键完成”(One-Click Completion) 视觉模块，通过 YOLO 计算机视觉自动识别游戏状态并瞬间触发战略配备。
- **现代化 UI：** 使用 React 19、Tailwind CSS 4 和 Radix UI 构建。它具有时尚的、受游戏启发的黑暗主题，并支持拖拽界面来对宏进行排序。
- **多语言支持：** 由 `react-i18next` 提供支持的多语言界面（英文和中文）。

## 技术栈

- **前端：** React 19, Vite, TypeScript, Tailwind CSS 4, Zustand (状态管理), @dnd-kit (拖拽), Radix UI。
- **后端/桌面框架：** Tauri 2.0, Rust。
- **语音引擎：** 集成 [Vosk](https://alphacephei.com/vosk/) 语音识别引擎。
- **计算机视觉：** 由 [ONNX Runtime (ort)](https://github.com/ort-rs/ort) 提供支持的高性能 YOLO 推理引擎，支持 CUDA 硬件加速。

## 先决条件

- Node.js (v18+)
- Rust (最新的 stable 工具链)
- Bun (或您喜欢的 Node 包管理器)

## Vosk 配置

该项目依赖于 Vosk 语音识别引擎。在运行或构建应用程序之前，您必须下载相应的语言模型和原生库，并将其放入 `src-tauri` 目录中。

1. **Vosk 原生库 (`src-tauri/lib`)**:
   从 [Vosk Releases 页面](https://github.com/alphacep/vosk-api/releases) 下载适用于您操作系统（Windows/macOS/Linux）的预编译 `libvosk` 库。解压缩文件并将动态库（例如 `.dll`、`.dylib` 或 `.so`）放在 `src-tauri/lib/` 目录中。

2. **Vosk 模型 (`src-tauri/model`)**:
   从 [Vosk Models 页面](https://alphacephei.com/vosk/models) 下载 Vosk 语言模型。
   - **注意**：您必须下载 **"small" (小型)** 模型（例如，英语使用 `vosk-model-small-en-us-0.15`，中文使用 `vosk-model-small-cn-0.22`）。大型模型没有针对这种实时宏触发用例进行优化。
   - 将下载的模型 zip 文件的内容（内部文件，如 `am`, `conf`, `graph`, `ivector`）直接解压到 `src-tauri/model/` 目录中。

## 快速开始

1. **克隆仓库：**
   ```bash
   git clone https://github.com/LyceumHewun/hellcall-desktop.git
   cd hellcall-desktop
   ```

2. **安装前端依赖：**
   ```bash
   bun install
   ```
   *(或者，使用 `npm install`, `yarn`, 或 `pnpm install`)*

3. **运行开发模式：**
   ```bash
   bun tauri dev
   ```
   此命令启动 Vite 开发服务器并启动 Tauri 窗口。

4. **构建生产版本：**
   ```bash
   bun tauri build
   ```
   最终的执行文件将被编译到 `src-tauri/target/release/` 目录中。

## 目录结构

- `/src` - React 前端代码 (UI 组件, 视图, Zustand store, 类型声明)。
- `/src-tauri` - Rust 后端, Tauri 配置, 原生插件，以及静态资产 (如声学模型和音频文件)。
- `/src/store` - Zustand store (`configStore.ts`) 用于管理全局状态并与 Rust 后端进行交互。
- `/src/app/views` - 核心视图页面，包括 宏配置 (Macros)、全局设置 (Global Settings)、按键绑定 (Key Bindings) 和 日志 (Logs)。

## 配置数据

设置会被无缝管理，并保存在 Tauri 的应用程序数据目录中。将自动生成一个 `config.toml` 文件来存储：
- 语音识别的微调参数
- 按键模拟的时序参数
- 触发词逻辑
- 完整的已保存宏命令列表

## 许可证

本项目采用 MIT 许可证
