//! Faces for the scripts the bundled ones (Inter, the monospace face) cannot
//! draw: Chinese, Japanese and Korean. They are loaded only once some text
//! needs them, so text without them shapes exactly as before.
//!
//! cosmic-text falls back to any face in the database that has a glyph,
//! so loading a face is all a script needs. A desktop takes its system's
//! faces (`SystemFaces`); a browser has none (`NoFaces`) and hands faces
//! in as it fetches them (`TextSystem::add_face`).

use std::path::Path;

use cosmic_text::fontdb;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Script {
    Han,
    Hangul,
}

impl Script {
    pub fn of(c: char) -> Option<Self> {
        match u32::from(c) {
            0x1100..=0x11FF | 0x3130..=0x318F | 0xA960..=0xA97F | 0xAC00..=0xD7FF => Some(Self::Hangul),
            0x2E80..=0x2FDF
            | 0x3000..=0x30FF
            | 0x31C0..=0x31FF
            | 0x3200..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFFEF
            | 0x20000..=0x3FFFF => Some(Self::Han),
            _ => None,
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Han => 1,
            Self::Hangul => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Scripts(u8);

impl Scripts {
    pub fn in_text(text: &str) -> Self {
        if text.is_ascii() {
            return Self::default();
        }
        let mut scripts = Self::default();
        for script in text.chars().filter_map(Script::of) {
            scripts.0 |= script.bit();
        }
        scripts
    }

    pub fn contains(self, script: Script) -> bool {
        self.0 & script.bit() != 0
    }

    pub fn insert(&mut self, script: Script) {
        self.0 |= script.bit();
    }

    pub fn iter(self) -> impl Iterator<Item = Script> {
        [Script::Han, Script::Hangul].into_iter().filter(move |&s| self.contains(s))
    }
}

pub trait FallbackFaces: Send {
    fn load(&mut self, script: Script, db: &mut fontdb::Database) -> bool;
}

#[derive(Debug, Default)]
pub struct NoFaces;

impl FallbackFaces for NoFaces {
    fn load(&mut self, _script: Script, _db: &mut fontdb::Database) -> bool {
        false
    }
}

#[derive(Debug, Default)]
pub struct SystemFaces;

const SYSTEM_FILES: &[(Script, &str)] = &[
    (Script::Han, "/System/Library/PrivateFrameworks/FontServices.framework/Resources/Reserved/PingFangUI.ttc"),
    (Script::Han, "/System/Library/Fonts/PingFang.ttc"),
    (Script::Han, "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc"),
    (Script::Han, "/System/Library/Fonts/Hiragino Sans GB.ttc"),
    (Script::Hangul, "/System/Library/Fonts/AppleSDGothicNeo.ttc"),
    (Script::Han, "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
    (Script::Han, "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
    (Script::Han, "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc"),
    (Script::Hangul, "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
    (Script::Hangul, "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"),
    (Script::Hangul, "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc"),
];

const SYSTEM_FAMILIES: &[(Script, &str)] = &[
    (Script::Han, "PingFang SC"),
    (Script::Han, "Hiragino Sans"),
    (Script::Han, "Noto Sans CJK SC"),
    (Script::Han, "Source Han Sans SC"),
    (Script::Han, "Microsoft YaHei"),
    (Script::Han, "Yu Gothic"),
    (Script::Han, "WenQuanYi Micro Hei"),
    (Script::Han, "Droid Sans Fallback"),
    (Script::Hangul, "Apple SD Gothic Neo"),
    (Script::Hangul, "Noto Sans CJK KR"),
    (Script::Hangul, "Source Han Sans KR"),
    (Script::Hangul, "Malgun Gothic"),
    (Script::Hangul, "NanumGothic"),
];

impl FallbackFaces for SystemFaces {
    fn load(&mut self, script: Script, db: &mut fontdb::Database) -> bool {
        let files = SYSTEM_FILES.iter().filter(|(s, _)| *s == script).map(|(_, path)| Path::new(path));
        if files.filter(|path| path.is_file()).any(|path| load_file(db, path)) {
            return true;
        }
        if let Some(root) = super::windows_fonts_dir() {
            let names: &[&str] = match script {
                Script::Han => &["msyh.ttc", "YuGothR.ttc"],
                Script::Hangul => &["malgun.ttf"],
            };
            if names.iter().map(|name| root.join(name)).filter(|path| path.is_file()).any(|path| load_file(db, &path)) {
                return true;
            }
        }
        let mut installed = fontdb::Database::new();
        installed.load_system_fonts();
        let families = SYSTEM_FAMILIES.iter().filter(|(s, _)| *s == script).map(|(_, family)| *family);
        families.filter_map(|family| file_of(&installed, family)).any(|path| load_file(db, &path))
    }
}

fn load_file(db: &mut fontdb::Database, path: &Path) -> bool {
    let before: Vec<_> = db.faces().map(|face| face.id).collect();
    if let Err(error) = db.load_font_file(path) {
        tracing::debug!(%error, path = %path.display(), "fallback font not loaded");
        return false;
    }
    let added: Vec<_> = db.faces().map(|face| face.id).filter(|id| !before.contains(id)).collect();
    let mut kept = false;
    for id in added {
        if db.with_face_data(id, has_outlines).unwrap_or(false) {
            kept = true;
        } else {
            db.remove_face(id);
        }
    }
    if !kept {
        tracing::debug!(path = %path.display(), "fallback font has no outlines we can draw");
    }
    kept
}

pub(super) fn has_outlines(data: &[u8], index: u32) -> bool {
    let u32_at = |at: usize| data.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let offset = if data.starts_with(b"ttcf") { u32_at(12 + 4 * index as usize) } else { Some(0) };
    let Some(offset) = offset.map(|o| o as usize) else { return false };
    let Some(tables) = data.get(offset + 4..offset + 6).map(|b| usize::from(u16::from_be_bytes([b[0], b[1]]))) else {
        return false;
    };
    (0..tables)
        .filter_map(|i| data.get(offset + 12 + 16 * i..offset + 16 + 16 * i))
        .any(|tag| matches!(tag, b"glyf" | b"CFF " | b"CFF2"))
}

fn file_of(installed: &fontdb::Database, family: &str) -> Option<std::path::PathBuf> {
    installed.faces().filter(|face| face.families.iter().any(|(name, _)| name == family)).find_map(|face| {
        match &face.source {
            fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => Some(path.clone()),
            fontdb::Source::Binary(_) => None,
        }
    })
}

#[cfg(test)]
#[path = "fallback_tests.rs"]
mod tests;
