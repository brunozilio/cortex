//! Images attached to the prompt: pasted from the clipboard (Ctrl+V) or
//! dropped on the terminal as a file. Each one is typed into the prompt as
//! `[Image #N]`; a prompt sends the images it still names.

use std::path::{Path, PathBuf};
use std::process::Command;

use superfik_sdk::Image;

use super::app::{App, Msg};

#[derive(Debug, Default)]
pub struct Attachments {
    images: Vec<Image>,
}

impl Attachments {
    /// Keeps `image` and gives the text that names it in the prompt.
    pub fn add(&mut self, image: Image) -> String {
        self.images.push(image);
        placeholder(self.images.len())
    }

    /// The images `text` names, in the order it first names them.
    pub fn named_in(&self, text: &str) -> Vec<Image> {
        let mut named: Vec<usize> = Vec::new();
        let mut rest = text;
        while let Some(at) = rest.find("[Image #") {
            rest = &rest[at + "[Image #".len()..];
            let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            if rest[digits..].starts_with(']')
                && let Ok(number) = rest[..digits].parse::<usize>()
                && (1..=self.images.len()).contains(&number)
                && !named.contains(&number)
            {
                named.push(number);
            }
        }
        named.into_iter().map(|number| self.images[number - 1].clone()).collect()
    }
}

pub fn placeholder(number: usize) -> String {
    format!("[Image #{number}]")
}

/// The file a paste names when it is a single path, as terminals paste a
/// dropped file: plain, quoted, or with its spaces escaped.
pub fn pasted_path(text: &str, cwd: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() || text.contains('\n') {
        return None;
    }
    let quoted = ['\'', '"'].iter().find_map(|q| text.strip_prefix(*q).and_then(|t| t.strip_suffix(*q)));
    let raw = match quoted {
        Some(inner) => inner.to_owned(),
        None => unescape(text),
    };
    let raw = raw.strip_prefix("file://").unwrap_or(&raw);
    let path = match (raw.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => cwd.join(raw),
    };
    path.is_file().then_some(path)
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            c => out.push(c),
        }
    }
    out
}

/// The image in the file at `path`; `None` when it holds none.
pub fn image_file(path: &Path) -> Result<Option<Image>, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > superfik_sdk::MAX_IMAGE_BYTES as u64 {
        return Err("The image is over the 20 MiB limit.".into());
    }
    Image::from_bytes(&std::fs::read(path).map_err(|e| e.to_string())?)
}

/// The image on the system clipboard, if it holds one.
pub fn clipboard() -> Result<Option<Image>, String> {
    let bytes = if cfg!(target_os = "macos") {
        let script = "the clipboard as «class PNGf»";
        run("osascript", &["-e", script]).and_then(|out| apple_data(&String::from_utf8_lossy(&out)))
    } else {
        run("wl-paste", &["--no-newline", "--type", "image/png"])
            .or_else(|| run("xclip", &["-selection", "clipboard", "-t", "image/png", "-o"]))
    };
    match bytes {
        Some(bytes) => Image::from_bytes(&bytes),
        None => Ok(None),
    }
}

fn run(program: &str, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new(program).args(args).output().ok()?;
    (output.status.success() && !output.stdout.is_empty()).then_some(output.stdout)
}

/// The bytes AppleScript prints as `«data PNGf89504E47…»`.
fn apple_data(printed: &str) -> Option<Vec<u8>> {
    let hex = printed.trim().strip_prefix("«data PNGf")?.strip_suffix('»')?;
    (hex.len() % 2 == 0)
        .then(|| (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect())
        .flatten()
}

impl App {
    /// Ctrl+V: attaches the clipboard's image, read on a thread.
    pub fn paste_image(&self) {
        self.spawn(|_| Msg::Clipboard(clipboard()));
    }

    pub(super) fn clipboard(&mut self, found: Result<Option<Image>, String>) {
        match found {
            Ok(Some(image)) => self.attach(image),
            Ok(None) => self.transcript.notice("No image on the clipboard."),
            Err(error) => self.transcript.error(error),
        }
    }

    /// A paste: a dropped image file is attached, anything else typed.
    pub fn paste(&mut self, text: &str) {
        let home = std::env::home_dir();
        if let Some(path) = pasted_path(text, &self.cwd, home.as_deref()) {
            match image_file(&path) {
                Ok(Some(image)) => return self.attach(image),
                Ok(None) => {}
                Err(error) => return self.transcript.error(error),
            }
        }
        self.input.insert_str(&text.replace("\r\n", "\n").replace('\r', "\n"));
    }

    fn attach(&mut self, image: Image) {
        let named = self.attachments.add(image);
        let spaced = self.input.text()[..self.input.cursor()].ends_with(|c: char| !c.is_whitespace());
        self.input.insert_str(&format!("{}{named} ", if spaced { " " } else { "" }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n pixels";

    fn png() -> Image {
        Image::from_bytes(PNG).expect("ok").expect("png")
    }

    #[test]
    fn a_prompt_sends_the_images_it_still_names() {
        let mut attachments = Attachments::default();
        assert_eq!(attachments.add(png()), "[Image #1]");
        let jpeg = Image::from_bytes(b"\xff\xd8\xff jpeg").expect("ok").expect("jpeg");
        assert_eq!(attachments.add(jpeg.clone()), "[Image #2]");
        assert_eq!(attachments.named_in("compare [Image #2] with [Image #1] and [Image #2]"), [jpeg, png()]);
        assert!(attachments.named_in("[Image #3] [Image #] [Image #1").is_empty(), "unknown or broken names");
    }

    #[test]
    fn a_dropped_file_is_found_however_the_terminal_quotes_it() {
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("my shot.png");
        std::fs::write(&file, PNG).expect("write");
        let cwd = dir.path();
        assert_eq!(pasted_path(&format!("'{}'", file.display()), cwd, None), Some(file.clone()));
        assert_eq!(pasted_path(&file.display().to_string().replace(' ', "\\ "), cwd, None), Some(file.clone()));
        assert_eq!(pasted_path("my\\ shot.png ", cwd, None), Some(file.clone()));
        assert_eq!(pasted_path("~/my shot.png", Path::new("/"), Some(cwd)), Some(file.clone()));
        assert_eq!(pasted_path("look at my shot.png", cwd, None), None);
        assert_eq!(pasted_path(&format!("{}\nmore", file.display()), cwd, None), None);
        assert_eq!(image_file(&file), Ok(Some(png())));
    }

    #[test]
    fn applescript_prints_the_clipboard_image_as_hex() {
        assert_eq!(apple_data("«data PNGf89504E47»\n"), Some(vec![0x89, 0x50, 0x4e, 0x47]));
        assert_eq!(apple_data("«data PNGf895»"), None);
        assert_eq!(apple_data("nothing"), None);
    }
}
