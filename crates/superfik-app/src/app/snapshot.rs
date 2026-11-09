//! Headless rendering of a scripted scenario to a PNG, for design review
//! and visual regression checks (`superfik --snapshot out.png`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use superfik_render::{Renderer, TextSystem, gpu::GpuContext};

use crate::state::{Language, Theme};
use crate::view::motions::Motions;
use crate::view::{LoadedImage, screen};
use superfik_client::scenarios;

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotArgs {
    pub output: PathBuf,
    pub scenario: String,
    pub size: (u32, u32),
    pub scale: f32,
    pub at: f64,
    pub pan: (f32, f32),
    pub hover: Option<(f32, f32)>,
    pub theme: Option<Theme>,
    pub language: Option<Language>,
}

pub fn parse(args: &[String]) -> Option<SnapshotArgs> {
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let output = PathBuf::from(value("--snapshot")?);
    let size = value("--size")
        .and_then(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
        .unwrap_or((1838, 1080));
    Some(SnapshotArgs {
        output,
        scenario: value("--scenario").unwrap_or_else(|| "reference".into()),
        size,
        scale: value("--scale").and_then(|s| s.parse().ok()).unwrap_or(2.0),
        at: value("--at").and_then(|s| s.parse().ok()).unwrap_or(3.0),
        pan: value("--pan")
            .and_then(|s| s.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))))
            .unwrap_or((0.0, 0.0)),
        hover: value("--hover")
            .and_then(|s| s.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))),
        theme: value("--theme")
            .and_then(|t| Theme::ALL.into_iter().find(|theme| theme.label().eq_ignore_ascii_case(&t))),
        language: value("--lang").and_then(|l| Language::from_locale(&l)),
    })
}

pub fn run(args: &SnapshotArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (mut session, synthetic) = scenarios::by_name(&args.scenario).ok_or("unknown scenario")?;
    if let Some(language) = args.language {
        session.preferences.language = language;
    }
    if let Some(theme) = args.theme {
        session.preferences.theme = theme;
    }
    let (w, h) = args.size;
    let device = ((w as f32 * args.scale) as u32, (h as f32 * args.scale) as u32);
    let mut renderer = Renderer::new(GpuContext::headless(device.0, device.1)?);
    let mut text = TextSystem::new();
    let images: HashMap<PathBuf, LoadedImage> = synthetic
        .iter()
        .map(|(path, rgba, size)| {
            (path.clone(), LoadedImage { id: renderer.load_image(rgba, *size, *size), aspect: 1.0 })
        })
        .collect();
    let icons = superfik_backend::local_data_dir().unwrap_or_else(std::env::temp_dir).join("browser-icons");
    let browser_icons = super::browser_icons::BrowserIcons::load(session.logins.sources.clone(), icons, &mut renderer);

    let viewport = (w as f32, h as f32);
    let mut width = superfik_ui::theme::metrics::SIDEBAR_MIN_WIDTH;
    let mut motions = Motions::default();
    let steps = (args.at * 60.0) as usize;
    let (pages, full) = (HashMap::new(), HashMap::new());
    let assets =
        screen::Assets { images: &images, full_images: &full, pages: &pages, browser_icons: browser_icons.ids() };
    let mut hovered = None;
    let mut painted = None;
    for step in 0..=steps {
        let now = step as f64 / 60.0;
        motions.step(1.0 / 60.0);
        let moment = screen::Moment {
            viewport,
            scale: args.scale,
            pan: args.pan,
            hovered,
            caret_visible: true,
            now,
            graph: Default::default(),
            input_scroll: Default::default(),
        };
        let frame = screen::paint(&session, assets, moment, width, &mut motions, &mut text);
        hovered = args.hover.and_then(|(x, y)| {
            let at = superfik_render::Point::new(x, y);
            frame.hits.at(at, superfik_render::Point::new(x - args.pan.0, y - args.pan.1))
        });
        width = frame.sidebar_width;
        painted = Some(frame);
    }
    let painted = painted.ok_or("no frame painted")?;
    let pixels = renderer.capture_passes(
        &[(&painted.canvas, args.scale), (&painted.sidebar, args.scale), (&painted.overlay, args.scale)],
        &mut text,
        args.at as f32,
    )?;
    save_png(&args.output, &pixels, device)?;
    tracing::info!(path = %args.output.display(), "snapshot written");
    Ok(())
}

fn save_png(path: &Path, rgba: &[u8], (w, h): (u32, u32)) -> Result<(), Box<dyn std::error::Error>> {
    let image = image::RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or("pixel buffer size mismatch")?;
    image.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_defaults() {
        let parsed = parse(&args(&["superfik", "--snapshot", "out.png"])).expect("args");
        assert_eq!(parsed.scenario, "reference");
        assert_eq!(parsed.size, (1838, 1080));
        assert_eq!(parsed.scale, 2.0);
        assert_eq!(parsed.theme, None);
    }

    #[test]
    fn parses_the_theme_by_its_settings_label() {
        let theme = |t| parse(&args(&["--snapshot", "a.png", "--theme", t])).expect("args").theme;
        assert_eq!(theme("light"), Some(Theme::Light));
        assert_eq!(theme("Dark"), Some(Theme::Dark));
        assert_eq!(theme("system"), Some(Theme::System));
        assert_eq!(theme("sepia"), None);
        let lang = |l| parse(&args(&["--snapshot", "a.png", "--lang", l])).expect("args").language;
        assert_eq!(lang("pt-BR"), Some(Language::Portuguese));
        assert_eq!(lang("en"), Some(Language::English));
        assert_eq!(lang("fr"), None);
    }

    #[test]
    fn parses_overrides() {
        let parsed = parse(&args(&[
            "--snapshot",
            "a.png",
            "--size",
            "800x600",
            "--scale",
            "1",
            "--scenario",
            "empty",
            "--pan",
            "0,600",
        ]))
        .expect("args");
        assert_eq!((parsed.size, parsed.scale, parsed.scenario.as_str()), ((800, 600), 1.0, "empty"));
        assert_eq!(parsed.pan, (0.0, 600.0));
    }

    #[test]
    fn absent_flag_means_no_snapshot() {
        assert!(parse(&args(&["superfik", "/repo"])).is_none());
    }
}
