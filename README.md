# Hellcall Desktop

English | [中文](docs/README_zh.md)

Hellcall Desktop is a cross-platform desktop application built with Tauri, React, and Rust. It provides a seamless way to configure and manage voice-activated keyboard macros—designed to enhance gameplay experiences (such as calling stratagems in Helldivers 2).

![Hellcall Desktop Preview](./preview.png)

## Features

- **Voice-Activated Macros:** Trigger custom key sequences using your microphone.
- **Configurable Recognizer:** Adjust VAD (Voice Activity Detection) silence duration and chunk times for optimal responsiveness.
- **Advanced Key Presser:** Fine-tune key release intervals, inter-key intervals, and initial wait times.
- **Customizable Triggers & Macros:** Define specific hit-words, grammars, and shortcuts to trigger sequences. You can also assign custom audio feedback (e.g., `normal_reply.wav`).
- **Vision-Based OCC:** Utilize an experimental "One-Click Completion" (OCC) vision module to automatically recognize game states via YOLO computer vision and trigger stratagems instantly.
- **Modern UI:** Built with React 19, Tailwind CSS 4, and Radix UI primitives. It features a sleek, game-inspired dark theme and a drag-and-drop interface for sorting your macros.
- **Localization:** Multi-language support (English and Chinese) powered by `react-i18next`.

## Tech Stack

- **Frontend:** React 19, Vite, TypeScript, Tailwind CSS 4, Zustand (State Management), @dnd-kit (Drag and Drop), Radix UI.
- **Backend/Desktop Framework:** Tauri 2.0, Rust.
- **Voice Engine:** Integrated [Vosk](https://alphacephei.com/vosk/) speech recognition engine.
- **Computer Vision:** High-performance YOLO inference powered by [ONNX Runtime (ort)](https://github.com/ort-rs/ort) with CUDA hardware acceleration support.

## Prerequisites

- Node.js (v18+)
- Rust (Latest stable toolchain)
- Bun (or your preferred Node package manager)

## Vosk Setup

This project relies on the Vosk speech recognition engine. Before running or building the application, you must download the appropriate language model and native library into the `src-tauri` directory.

1. **Vosk Library (`src-tauri/lib`)**:
   Download the precompiled `libvosk` library for your operating system (Windows/macOS/Linux) from the [Vosk Releases page](https://github.com/alphacep/vosk-api/releases). Extract the files and place the dynamic library (e.g., `.dll`, `.dylib`, or `.so`) inside the `src-tauri/lib/` directory.

2. **Vosk Model (`src-tauri/model`)**:
   Download a Vosk language model from the [Vosk Models page](https://alphacephei.com/vosk/models).
   - **Important**: You must download a **"small"** model (e.g., `vosk-model-small-en-us-0.15` for English or `vosk-model-small-cn-0.22` for Chinese). Large models are not optimized for this real-time macro use case.
   - Extract the contents of the downloaded model zip file (the internal files like `am`, `conf`, `graph`, `ivector`) directly into the `src-tauri/model/` directory.

## Getting Started

1. **Clone the repository:**
   ```bash
   git clone https://github.com/LyceumHewun/hellcall-desktop.git
   cd hellcall-desktop
   ```

2. **Install frontend dependencies:**
   
   ```bash
   bun install
   ```
   *(Alternatively, use `npm install`, `yarn`, or `pnpm install`)*
   
3. **Run in development mode:**
   ```bash
   bun tauri dev
   ```
   This command starts the Vite development server and launches the Tauri window.

4. **Build for production:**
   ```bash
   bun tauri build
   ```
   The final executable bundle will be compiled into `src-tauri/target/release/`.

## Directory Structure

- `/src` - React frontend code (UI components, Views, Zustand store, Types).
- `/src-tauri` - Rust backend, Tauri configuration, native plugins, and static assets (like acoustic models and audio files).
- `/src/store` - Zustand store (`configStore.ts`) for managing global state and interacting with the Rust backend.
- `/src/app/views` - Core views including Macros, Global Settings, Key Bindings, and Logs.

## Configuration

Settings are managed seamlessly and saved locally via Tauri's application data directory. A `config.toml` file is automatically generated to store:
- Voice Recognition tuning
- Key simulation timings
- Trigger word logic
- Complete list of saved command macros

## License

This project is licensed under the MIT License.
