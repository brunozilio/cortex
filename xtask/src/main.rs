//! Developer tasks: `lint` (repo rules clippy cannot check),
//! `cargo run -p xtask -- cef-bundle <app> <executable>`, which completes a
//! release `.app` with CEF's framework and helpers, and
//! `cargo run -p xtask -- iconset <out.iconset>`, which scales
//! `assets/icon.png` (1024×1024, transparent, rendered by Blender from
//! `assets/icon3d.py`) into the PNG sizes macOS `iconutil` expects, and
//! `windows-icon <out.ico>` for the multi-resolution Windows icon,
//! `notices <out>`, which writes the third-party notices the app ships
//! with, and `update-keygen` /
//! `sign-update` (see `update`), the key release archives are signed with.

mod lint;
mod notices;
mod update;

use std::path::{Path, PathBuf};

use image::imageops::FilterType;

const SIZES: [(u32, &str); 10] = [
    (16, "icon_16x16.png"),
    (32, "icon_16x16@2x.png"),
    (32, "icon_32x32.png"),
    (64, "icon_32x32@2x.png"),
    (128, "icon_128x128.png"),
    (256, "icon_128x128@2x.png"),
    (256, "icon_256x256.png"),
    (512, "icon_256x256@2x.png"),
    (512, "icon_512x512.png"),
    (1024, "icon_512x512@2x.png"),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [task, out] if task == "iconset" => iconset(Path::new(out)),
        [task, out] if task == "windows-icon" => windows_icon(Path::new(out)),
        [task] if task == "lint" => lint::run(&root(), false),
        [task, flag] if task == "lint" && flag == "--update-baseline" => lint::run(&root(), true),
        [task, app, executable] if task == "cef-bundle" => cef_bundle(Path::new(app), executable),
        [task, out] if task == "notices" => notices::run(&root(), Path::new(out)),
        [task, out] if task == "update-keygen" => update::keygen(Path::new(out)),
        [task, rest @ ..] if task == "sign-update" => update::sign(rest),
        _ => Err("usage: xtask iconset <out.iconset> | windows-icon <out.ico> | lint [--update-baseline] | cef-bundle <app> <executable> \
             | notices <out.md> | update-keygen <out-file> | sign-update <archive> --version X.Y.Z --platform P --url URL"
            .into()),
    }
}

fn windows_icon(out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    let master = image::open(root().join("assets/icon.png"))?.into_rgba8();
    let mut frames = Vec::new();
    for size in [16, 24, 32, 48, 64, 128, 256] {
        let pixels = image::imageops::resize(&master, size, size, FilterType::Lanczos3);
        frames.push(IcoFrame::as_png(pixels.as_raw(), size, size, image::ExtendedColorType::Rgba8)?);
    }
    IcoEncoder::new(std::fs::File::create(out)?).encode_images(&frames)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn cef_bundle(app: &Path, executable: &str) -> Result<(), Box<dyn std::error::Error>> {
    let layout = superfik_browser::bundle::Layout::of(app, executable);
    let plist = std::fs::read_to_string(root().join("assets/macos/Info.plist"))?;
    let identifier = plist
        .split("<key>CFBundleIdentifier</key><string>")
        .nth(1)
        .and_then(|rest| rest.split('<').next())
        .ok_or("no CFBundleIdentifier in assets/macos/Info.plist")?;
    superfik_browser::bundle::assemble_release(&layout, identifier)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn cef_bundle(_app: &Path, _executable: &str) -> Result<(), Box<dyn std::error::Error>> {
    Err("cef-bundle is macOS-only".into())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn iconset(out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let root = root();
    let master = image::open(root.join("assets/icon.png"))?.into_rgba8();
    std::fs::create_dir_all(out)?;
    for (size, name) in SIZES {
        image::imageops::resize(&master, size, size, FilterType::Lanczos3).save(out.join(name))?;
    }
    Ok(())
}
