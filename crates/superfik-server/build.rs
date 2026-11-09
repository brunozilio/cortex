//! Linux: the server links CEF's `libcef.so` (pages, ADR 0010), which sits
//! beside the binary (the build copies CEF's files there, and so does the
//! image), so the loader looks in the binary's own folder first.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    }
}
