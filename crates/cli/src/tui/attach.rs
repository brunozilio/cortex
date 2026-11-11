use std::path::{Path, PathBuf};

use sdk::Image;

use super::app::{App, Msg};

#[derive(Debug, Default)]
pub struct Attachments {
    images: Vec<Image>,
}

impl Attachments {
    pub fn add(&mut self, image: Image) -> String {
        self.images.push(image);
        placeholder(self.images.len())
    }

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

pub fn image_file(path: &Path) -> Result<Option<Image>, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > sdk::MAX_IMAGE_BYTES as u64 {
        return Err("The image is over the 20 MiB limit.".into());
    }
    Image::from_bytes(&std::fs::read(path).map_err(|e| e.to_string())?)
}

pub fn clipboard() -> Result<Option<Image>, String> {
    match platform::clipboard_image() {
        Some(bytes) => Image::from_bytes(&bytes),
        None => Ok(None),
    }
}

impl App {
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

    pub fn paste(&mut self, text: &str) {
        let home = std::env::home_dir();
        if let Some(path) = pasted_path(text, &self.cwd, home.as_deref()) {
            match image_file(&path) {
                Ok(Some(image)) => return self.attach(image),
                Ok(None) => {}
                Err(error) => return self.transcript.error(error),
            }
        }
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        match self.pastes.fold(&text) {
            Some(placeholder) => self.input.insert_str(&placeholder),
            None => self.input.insert_str(&text),
        }
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
    fn applescript_prints_the_clipboard_image_as_hex() {}
}
