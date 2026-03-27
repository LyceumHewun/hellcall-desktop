# Style and conventions
- Frontend uses TypeScript with simple exported interfaces for shared config shapes.
- Backend uses Rust with Tauri commands in `src-tauri/src/lib.rs` and engine code in `src-tauri/src/hellcall.rs`.
- Existing Rust style favors small helper functions, `Result<_, String>` at the Tauri boundary, and `utils::format_and_log_error` for user-facing/backend logging errors.
- Keep Windows path handling in mind; the code already strips `\\?\` prefixes before passing paths into libraries that may reject them.
- Preserve current behavior where possible; avoid reverting unrelated local changes.