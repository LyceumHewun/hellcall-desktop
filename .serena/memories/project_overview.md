# Hellcall Desktop
- Purpose: Cross-platform Tauri desktop app for voice-activated keyboard macros, primarily for Helldivers 2 workflows.
- Stack: React 19 + TypeScript + Vite frontend; Rust + Tauri 2 backend; Vosk speech recognition; ONNX Runtime / YOLO-based vision support.
- Structure: `src/` contains frontend UI, state, i18n, and views. `src-tauri/` contains Rust backend commands, engine logic, native assets, and Tauri config. `src-tauri/src/lib.rs` wires Tauri commands and app lifecycle. `src-tauri/src/hellcall/` contains config, audio, keypress, command matching, speaker, and vision modules.
- Runtime config is stored in the app config directory as `config.toml` and loaded/saved through Rust commands.