use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    tauri_build::build();

    // --- 智能拷贝整个 lib 文件夹到可执行文件所在目录 ---

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    // 向上寻找真正的 target 目录 (如 target/debug 或 target/release)
    // 根据 Tauri 的构建树深度，通常 OUT_DIR 在 target/debug/build/xxx/out
    let mut target_dir = PathBuf::from(&out_dir);
    for _ in 0..3 {
        if let Some(parent) = target_dir.parent() {
            target_dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    let lib_dir = PathBuf::from(&manifest_dir).join("lib");

    if lib_dir.exists() && lib_dir.is_dir() {
        // 调用递归拷贝函数
        if let Err(e) = copy_dir_all(&lib_dir, &target_dir) {
            println!("cargo:warning=Failed to copy lib directory: {}", e);
        } else {
            // 告诉 Cargo，如果整个 lib 文件夹发生变化，重新触发 build.rs
            println!("cargo:rerun-if-changed={}", lib_dir.display());
        }
    } else {
        println!(
            "cargo:warning=The 'lib' directory was not found at {}!",
            lib_dir.display()
        );
    }
}

/// 递归拷贝一个目录下的所有动态链接库到目标目录
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    // 立即将隐式的泛型转换为显式的路径引用，打断无限嵌套
    let src = src.as_ref();
    let dst = dst.as_ref();

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();

        if ty.is_dir() {
            // 注意这里：如果你想保持文件夹结构，应该把子目录名拼接到目标路径上。
            // 但对于 DLL 来说，系统在 EXE 旁边找库是不会去子目录里找的。
            // 为了安全起见，我们把子目录里的 DLL 也直接平铺 (Flatten) 到 target_dir。
            // 传入 path 和 dst，而不是 &dst
            copy_dir_all(&path, dst)?;
        } else {
            // 只拷贝特定的动态库文件
            if let Some(ext) = path.extension() {
                if ext == "dll" || ext == "so" || ext == "dylib" {
                    let file_name = entry.file_name();
                    fs::copy(&path, dst.join(file_name))?;
                }
            }
        }
    }
    Ok(())
}
