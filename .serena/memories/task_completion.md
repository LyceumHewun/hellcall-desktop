# When a task is completed
- Format Rust changes with `cargo fmt` from `src-tauri/`.
- Run at least `cargo check` for backend-impacting changes.
- If frontend types or views are changed, run the relevant frontend formatter/checker if available; otherwise keep edits minimal and consistent.
- Summarize any verification gaps, especially when network-restricted tooling prevents additional checks.