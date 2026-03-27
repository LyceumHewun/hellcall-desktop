pub mod string;

pub use string::*;

pub fn format_and_log_error<E: std::fmt::Display>(prefix: &str, e: E) -> String {
    let e_msg = format!("{}: {}", prefix, e);
    log::error!("{}", e_msg);
    e_msg
}

pub fn normalize_runtime_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace("\\\\?\\", "")
}
