//! macOS: ONNX Runtime's CoreML code (memory embeddings, via fastembed)
//! calls `___isPlatformVersionAtLeast`, which lives in clang's runtime
//! library; rustc's link line does not include it.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let Ok(output) = Command::new("clang").arg("--print-file-name=libclang_rt.osx.a").output() else { return };
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if path.is_absolute() && path.exists() {
        println!("cargo:rustc-link-arg={}", path.display());
    }
}
