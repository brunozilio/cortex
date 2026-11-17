use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use domain::Folder;

const MOST_RECORDS: usize = 50_000;
const DEEPEST: usize = 8;
const HASHED_AT_MOST: u64 = 64 * 1024;
const BRIEF_FOLDERS: [&str; 9] =
    ["skills", "commands", "agents", "rules", "hooks", "plugins", "output-styles", "styles", "tools"];
const NEVER_WALKED: [&str; 3] = [".git", "node_modules", "target"];
const INSTRUCTION_NAMES: [&str; 3] = [brand::INSTRUCTIONS, "AGENTS.md", brand::LOCAL_INSTRUCTIONS];

struct TooMany;

#[derive(Default)]
struct Seen {
    records: Vec<(PathBuf, u128, u64, u64)>,
}

pub(in crate::engine) fn fingerprint(folder: Folder<'_>) -> Option<u64> {
    let gate = crate::trust::Gate::of(folder);
    let mut seen = Seen::default();
    seen.everything(folder, gate.home()).ok()?;
    let mut hasher = DefaultHasher::new();
    folder.cwd.hash(&mut hasher);
    serde_json::to_string(&folder.extensions).unwrap_or_default().hash(&mut hasher);
    format!("{:?}", gate.trust()).hash(&mut hasher);
    seen.records.sort();
    seen.records.hash(&mut hasher);
    Some(hasher.finish())
}

impl Seen {
    fn everything(&mut self, folder: Folder<'_>, home: Option<&Path>) -> Result<(), TooMany> {
        let homes = home.map(|home| home.join(brand::DIR)).into_iter();
        let installed = content::folder().into_iter();
        for root in homes.chain(installed).chain(folder.cwd.ancestors().map(|dir| dir.join(brand::DIR))) {
            self.brand_folder(&root)?;
        }
        for directory in folder.cwd.ancestors() {
            for name in INSTRUCTION_NAMES {
                self.file(&directory.join(name))?;
            }
        }
        self.nested_instructions(folder.cwd)?;
        let extensions = folder.extensions.into_iter();
        for root in extensions.flat_map(|extensions| extensions.plugins.iter().chain(&extensions.dirs)) {
            self.tree(root, 0)?;
        }
        Ok(())
    }

    fn brand_folder(&mut self, root: &Path) -> Result<(), TooMany> {
        for path in sorted(root) {
            let briefed = path.extension().is_some_and(|extension| extension == "json" || extension == "md");
            let hidden = path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.'));
            if path.is_file() && briefed && !hidden {
                self.file(&path)?;
            }
        }
        for folder in BRIEF_FOLDERS {
            self.tree(&root.join(folder), 0)?;
        }
        Ok(())
    }

    fn tree(&mut self, root: &Path, depth: usize) -> Result<(), TooMany> {
        if depth > DEEPEST {
            return Ok(());
        }
        for path in sorted(root) {
            let skipped =
                path.file_name().and_then(|name| name.to_str()).is_some_and(|name| NEVER_WALKED.contains(&name));
            if skipped {
                continue;
            }
            if path.is_dir() {
                self.tree(&path, depth + 1)?;
            } else {
                self.file(&path)?;
            }
        }
        Ok(())
    }

    fn nested_instructions(&mut self, cwd: &Path) -> Result<(), TooMany> {
        let walk = ignore::WalkBuilder::new(cwd)
            .hidden(false)
            .max_depth(Some(32))
            .filter_entry(|entry| {
                !entry.file_type().is_some_and(|kind| kind.is_dir())
                    || !entry.file_name().to_str().is_some_and(|name| NEVER_WALKED.contains(&name))
            })
            .build();
        for entry in walk.filter_map(Result::ok) {
            let named = entry.file_name().to_str().is_some_and(|name| INSTRUCTION_NAMES.contains(&name));
            if named && entry.file_type().is_some_and(|kind| kind.is_file()) {
                self.file(entry.path())?;
            }
        }
        Ok(())
    }

    fn file(&mut self, path: &Path) -> Result<(), TooMany> {
        if self.records.len() >= MOST_RECORDS {
            return Err(TooMany);
        }
        let Ok(meta) = std::fs::metadata(path) else { return Ok(()) };
        let modified = meta.modified().ok().and_then(|at| at.duration_since(UNIX_EPOCH).ok());
        let content = if meta.len() <= HASHED_AT_MOST { content_hash(path) } else { 0 };
        self.records.push((path.to_path_buf(), modified.map_or(0, |at| at.as_nanos()), meta.len(), content));
        Ok(())
    }
}

fn sorted(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    paths
}

fn content_hash(path: &Path) -> u64 {
    let mut bytes = Vec::new();
    let read = std::fs::File::open(path).and_then(|file| file.take(HASHED_AT_MOST).read_to_end(&mut bytes));
    if read.is_err() {
        return 0;
    }
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}
