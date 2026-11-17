use std::path::PathBuf;

pub use page::{Asset, MISSING, serve};

#[derive(Debug, Clone)]
pub enum Assets {
    Folder(PathBuf),
    Bundle(&'static [Asset]),
    Missing,
}

pub fn locate(candidates: impl IntoIterator<Item = Option<PathBuf>>) -> Option<PathBuf> {
    candidates.into_iter().flatten().find(|folder| folder.join("index.html").is_file())
}

pub fn choose(
    chosen: Option<PathBuf>,
    bundle: &'static [Asset],
    folders: impl IntoIterator<Item = Option<PathBuf>>,
) -> Assets {
    if let Some(folder) = locate([chosen]) {
        return Assets::Folder(folder);
    }
    if page::has_index(bundle) {
        return Assets::Bundle(bundle);
    }
    locate(folders).map_or(Assets::Missing, Assets::Folder)
}

pub fn discover() -> Assets {
    let chosen = std::env::var_os(brand::env("CONSOLE_DIR")).map(PathBuf::from);
    let installed = platform::data_dir(brand::NAME, brand::COMMAND).map(|data| data.join("console"));
    choose(chosen, page::embedded(), [installed])
}

pub fn embedded() -> &'static [Asset] {
    page::embedded()
}
