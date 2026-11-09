# Platforms

Building, packaging and checking Superfik per platform, and the browser
engine they all ship. Server installation is in
[self-hosting.md](self-hosting.md).

## Browser engine (CEF)

Superfik ships a patched Chromium/CEF built in
[superfik-browser](https://github.com/brunozilio/superfik-browser), which
owns the engine's sources, patches, build and
[profile protocol](https://github.com/brunozilio/superfik-browser/blob/main/native/cef/PROFILE_PROTOCOL.md).
Engine changes and Chromium builds happen there; this repository only
downloads a published build. App, adapter and Settings changes reuse it.

- **Pins.** `scripts/cef-release.lock.json` (macOS) names the repository,
  tag, asset, archive SHA-256 and provenance SHA-256. Linux and Windows
  read `scripts/cef-release.linux-x86_64.lock.json` and
  `scripts/cef-release.windows-x86_64.lock.json`; **neither exists yet**.
  Until a pin lands: the Windows MSI ships without the browser (the app
  already reports it unavailable there), CI's Linux job builds against
  stock CEF and skips the browser acceptance, and the Linux packages and
  the server archive are not released (`SUPERFIK_LINUX_RELEASE_ENABLED`
  stays unset).
- **Install.** `python3 scripts/fetch_engine.py [--lock …] [--archive file]`
  checks the archive's checksum, extracts it safely and validates manifest,
  file hashes, symlinks, CEF ABI and provenance; the result is cached in
  `target/browser-engine/<tag>` and revalidated before use. An invalid
  artifact fails instead of falling back to stock CEF.
- **Bundle.** Use either `CEF_PATH=<distribution>` or
  `SUPERFIK_BROWSER_RELEASE_LOCK=<lock>`, never both. Bundling probes the
  native ABI marker (private preference ABI 1) before Cargo runs and before
  signing changes the bytes. Stock CEF is for local development only.
- **New pin.** Publish a candidate in `superfik-browser`, then update the
  lock with its asset and SHA-256. `provenance_sha256` is the SHA-256 of the
  candidate's `superfik-engine.json` `{patches, build}` as compact JSON with
  sorted keys and patch order kept (`scripts/cef_artifact.py::provenance_digest`).
  Run `python3 -m unittest discover -s scripts -p 'test_cef_*.py'` first.
  Only consume artifacts from a trusted builder.
- **Touch ID group.** Keep engines built team-less: the bundler rewrites
  the framework's Keychain group to `F3BSQ3FZTF.superfik.webauthn.v1`
  (`bundle::passkeys`). An engine with the team compiled in defeats that
  rewrite and moves existing passkeys to another group. iCloud Keychain
  passkeys need Apple's restricted
  `com.apple.developer.web-browser.public-key-credential` entitlement
  (not granted yet).

### Browser profiles

Settings › Browser keeps named profiles, assigned to a conversation
explicitly; real values are the default and are never rotated or derived
from GeoIP. Every override is native Chromium code, never an injected
JavaScript shim: CPU and memory, WebGL/WebGPU identity (the real adapter
still executes), seeded canvas and audio readbacks (exports only, 8-bit
grid: use real values when pixel-exact or HDR reads matter), local font
restrictions (web fonts and fallbacks untouched), screen metrics, WebRTC
policy and HTTP(S) proxies (no DIRECT fallback, credentials only to that
proxy, passwords in the Keychain). Changing a profile in use needs a
restart. Google Sync, domain reliability, Optimization Hints, Media Router
and Autofill server traffic are switched off; security protections and
component updates stay on, with no FPS cap or tab throttling.

### Native acceptance

Compiling proves nothing about native behaviour. The
`native_smoke` example (see its README) runs the real engine against
loopback fixtures; `bash scripts/check-browser-native.sh` with `CEF_PATH`
set runs the whole matrix. `SUPERFIK_ADHOC=1` signs ad hoc, as CI does.
Not covered: arbitrary sites' logins or bot detection, popup chains without
a user gesture, uploads outside the staging folder, automatic dialog consent.

## macOS

`scripts/bundle-macos.sh` builds `target/release/bundle/Superfik.app` and
the DMG (`scripts/create-dmg.sh` rebuilds only the DMG, from a logged-in GUI
session). See the README for signing and notarization.

## Linux

Release target: x86-64, Ubuntu 24.04 / Debian 13 or newer, X11 or Wayland,
Vulkan or OpenGL (Mesa software rendering works for automated checks).
ARM64 works for development but is not published until its engine is pinned.

**Install.** Check the `.sha256` from the same release, then
`sudo apt install ./superfik_<version>_amd64.deb` (keep the recommended
desktop portal, `xdg-utils` and a sound server). The portable `.tar.gz`
unpacks anywhere the user owns (`superfik-<version>/usr/bin/superfik`);
keep the whole directory, since CEF's files sit in `usr/lib/superfik`.
Only the portable install updates itself; a `/usr` install updates through
its package. Removing the package keeps user data. The desktop entry is
`superfik.desktop`; the Wayland app id and X11 class are `superfik`.

**Build.** On Ubuntu 24.04 (oldest glibc baseline) with Rust, a C/C++
toolchain, CMake, Ninja, Python 3 and the libraries in
`deploy/linux/Dockerfile.qa`:

```sh
SUPERFIK_RELEASE=1 scripts/bundle-linux.sh
```

It fetches and validates the Linux pin, then writes the `.deb`, the
portable `.tar.gz` and a SHA-256 manifest to
`${CARGO_TARGET_DIR:-target}/release/bundle`. Without `SUPERFIK_RELEASE=1`
a full `CEF_PATH` may be used for local development.

**Checks.**

```sh
CEF_PATH=/path/to/engine SUPERFIK_BROWSER_CHECK_LOGS=target/linux-browser-checks \
  xvfb-run -a bash scripts/check-browser-native.sh
bash .github/workflows/scripts/check-linux-package.sh app.deb app.tar.gz target/linux-package-checks
```

The release installs the `.deb` in a clean Ubuntu container, renders it,
checks both layouts, dependencies, resources and notices, and verifies
removal keeps user data before uploading. By hand, also check a repository,
Ctrl shortcuts, page input, clipboard, a portal file dialog, restart
persistence, audio and voice under both X11 and Wayland.

## Windows

Target `x86_64-pc-windows-msvc`. Not shippable yet: browser cards need the
Windows CEF integration (welding's `windows_cef` producer with Superfik's
patches, DX12 frames, Windows keys) and a pinned Windows engine, whose build
scripts in `superfik-browser` are macOS-only today. Login import on Windows
(DPAPI; Chrome/Edge `v20` cookies cannot be imported) is also missing.

**Build and test** on Windows with Git for Windows, Rust (MSVC), VS 2022
Build Tools (C++ and SDK), Python 3 and .NET 8, from a Developer PowerShell:

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python -m unittest discover -s scripts -p 'test_windows_*.py'
cargo build --target x86_64-pc-windows-msvc -p superfik-app
./scripts/smoke-windows.ps1 -Executable target/x86_64-pc-windows-msvc/debug/superfik.exe
```

From the Mac, `cargo xwin clippy --target x86_64-pc-windows-msvc` checks
Windows code (WinRT, resources) but runs nothing.

- The executable embeds icon, version and a manifest (PerMonitorV2, long
  paths, UTF-8), starts without a console and logs to
  `%LOCALAPPDATA%\Superfik\logs`. The first window is clamped to the
  monitor's work area (`app::window::fit_monitor`);
  `scripts/check-windows-window.ps1` reports its rect from the interactive
  desktop.
- Agent commands, hooks and vault commands run in Git for Windows' Bash
  when installed, else Windows PowerShell (UTF-16 input, UTF-8 output); the
  agent's tool says which. Processes end with their tree (Job Object).
- A fresh Windows needs the Visual C++ runtime (`STATUS_DLL_NOT_FOUND`
  otherwise); the MSI carries the app-local CRT, which must be checked on a
  clean machine.
- Windows Hello must be exercised with consent given, cancelled and
  unavailable.

**VM.** UTM on Apple silicon runs Windows 11 ARM64, which runs the x64 app
emulated: good for GUI, paths, processes, installer and input. Final QA
(DX12/CEF, Hello) needs native x64. With the guest agent, `utmctl exec` and
`utmctl file pull` reach the guest without SSH; GUI tests need the
interactive desktop.

**MSI.**

```powershell
dotnet tool install --global wix --version 5.0.2
./scripts/bundle-windows.ps1 -EngineLock scripts/cef-release.windows-x86_64.lock.json
```

It validates the engine and an x64 GUI executable importing CEF, then
assembles CEF, the app, the x64 CRT (`-VCRuntimeDir` if not found) and
notices. WiX installs per machine in `%ProgramFiles%\Superfik` with a Start
menu shortcut and a fixed UpgradeCode; uninstall keeps user data. The icon
comes from `cargo run -p xtask -- windows-icon assets/windows/superfik.ico`.
`release.yml`'s Windows job stays off until `SUPERFIK_WINDOWS_RELEASE_ENABLED=true`;
the MSI is unsigned for now.
