//! macOS: ONNX Runtime's CoreML code calls `___isPlatformVersionAtLeast`,
//! which lives in clang's runtime library; rustc's link line does not
//! include it.
//!
//! Linux: the binary links CEF's `libcef.so`, which sits beside it (the
//! build copies CEF's files there, and so does the package), so the loader
//! looks in the binary's own folder first.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resources();
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let output = Command::new("clang").arg("--print-file-name=libclang_rt.osx.a").output();
    let Ok(output) = output else { return };
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if path.is_absolute() && path.exists() {
        println!("cargo:rustc-link-arg={}", path.display());
    }
}

fn windows_resources() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/windows");
    for name in ["superfik.ico", "superfik.manifest", "superfik.rc"] {
        println!("cargo:rerun-if-changed={}", assets.join(name).display());
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    let quote = |name: &str| assets.join(name).to_string_lossy().replace('\\', "/");
    let version = env!("CARGO_PKG_VERSION");
    let numeric = version.split(['-', '+']).next().expect("package version");
    let source = include_str!("../../assets/windows/superfik.rc")
        .replace("@ICON@", &quote("superfik.ico"))
        .replace("@MANIFEST@", &quote("superfik.manifest"))
        .replace("@VERSION@", version)
        .replace("@VERSION_NUMERIC@", &format!("{},0", numeric.replace('.', ",")));
    let resource = out.join("superfik.rc");
    std::fs::write(&resource, source).expect("write Windows resources");
    embed_resource::compile_for(resource, ["superfik"], embed_resource::NONE)
        .manifest_required()
        .expect("compile required Windows manifest, icon and version information");
}
