# sherpa-onnx-sys 2.0.1, patched for Superfik

Upstream: https://crates.io/crates/sherpa-onnx-sys (2.0.1), wired in through
`[patch.crates-io]` in the workspace `Cargo.toml`.

sherpa-onnx's static archive bundles its own ONNX Runtime (1.28), and memory's
embeddings (fastembed → ort → `ort-sys`) link another (1.31). Apple's linker
takes the first definition of each symbol; GNU ld refuses the duplicates, so
the app did not link on Linux.

sherpa-onnx reaches ONNX Runtime only through `OrtGetApiBase` (its C++ API is
header-only over the versioned C API, which newer runtimes keep serving), so on
Linux one runtime serves both:

- `build.rs`: the static link leaves out sherpa's `libonnxruntime.a` on Linux.
  It also reruns when the extracted libraries are gone, since a restored
  build cache keeps its output but not `target/sherpa-onnx-prebuilt`.
- `Cargo.toml`: on Linux it depends on `ort-sys` at fastembed's exact version
  (`links = "onnxruntime"` keeps a second one from creeping in).
- `src/lib.rs`: `extern crate ort_sys` on Linux, so ort-sys is linked after this
  crate and resolves `OrtGetApiBase` even in builds without embeddings.

On Windows the prebuilt archive's CRT now follows Rust's `crt-static` target
feature: MD by default, MT only for an explicitly static CRT. Always selecting
MT mixed two CRTs and left SQLite's `__imp_strspn` unresolved with lld-link.
Both choices use the Release native archive, since Rust does not use MSVC's
debug CRT even in a debug Cargo profile. macOS is unchanged.

When fastembed moves to another `ort`, bump the `ort-sys` pin here to match.
