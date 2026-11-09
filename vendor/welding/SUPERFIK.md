# Superfik integration patch

Vendored from crates.io `welding` 0.15.0, upstream commit
`1836887569367fe8476b014ed8563d302a5c829e` (package subdirectory `welding`).
Upstream: https://github.com/merely-made/wgpu-weld. Source remains MPL-2.0;
the full licence text is in `LICENSE`.

## Modified and added files

Modified from upstream 0.15.0 (each keeps its original MPL-2.0 notice):

- `src/auth.rs`
- `src/downloads.rs`
- `src/lib.rs`
- `src/macos_cef/cef_backed.rs`
- `src/macos_cef/mod.rs`
- `src/native_frame/mod.rs`
- `src/surface.rs`
- `src/view.rs`

Added by Superfik, also MPL-2.0:

- `src/macos_cef/managed.rs` (upstream surface setup extracted from
  `cef_backed.rs`, keeping its upstream notice)
- `src/macos_cef/native_hardware.rs`
- `src/macos_cef/native_network.rs`

`LICENSE` and this file are also additions. Everything else is unchanged from
the crates.io release.

## Changes

Local changes add an opt-in native hardware preference configuration to the
macOS producer, applied to its ready RequestContext before browser creation.
An ordinary CEF distribution rejects these preferences; callers get an error
instead of silently running an experiment without its native overrides.

The mutable error output passed to SetPreference must be an allocated empty
CEF string, not CefString::default() (which supplies a null pointer). The real
native acceptance harness in Superfik covers that call path.

The adapter also applies native CEF screen metrics and fixed HTTP(S) proxy
preferences before creating each browser. Screen scale is shared by rendering
and pointer coordinate conversion. Proxy authentication is restricted to the
configured host and port, caps retries, and never answers origin authentication
with proxy credentials. Configured proxies disable non-proxied WebRTC UDP and
remove Chromium's implicit loopback bypass. Passwords are redacted from Debug.

Managed popups are opt-in. Each native popup has its own render/event/client
state, retains its real opener and RequestContext, and is exposed to the host
as a surface. Only user-gesture popups are adopted; at most 16 may be live per
pool. Extracted upstream surface setup remains MPL-2.0 in `managed.rs`.
Downloads reserve unique destinations atomically with private file permissions
so duplicate suggested names cannot overwrite an existing download.

The macOS producer (`macos_cef`) is also compiled on Linux: only the
accelerated paint differs (an IOSurface on macOS, DMABUF planes dup'ed from
the callback on Linux, as upstream's `linux_cef` does). Frames are imported
as `Bgra8Unorm`/`Rgba8Unorm` on both, the formats Superfik's renderer
expects. With `cpu-paint-fallback`, `OnPaint` stores a `CpuBitmap` (BGRA,
copied out of the callback) for CEF without a shared texture: no GPU
process, or a software GPU.

On Linux, DevTools-enabled OSR surfaces retain JavaScript dialog callbacks
instead of invoking CEF's parent-window-dependent native runner. The Page
domain must explicitly accept/dismiss dialogs and supply prompt text. Closing
or resetting a dialog drops the retained callback outside its mutex. The real
`native_smoke` DIALOG acceptance covers dismiss, accept and prompt text.
