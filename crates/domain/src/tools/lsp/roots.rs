use std::path::{Path, PathBuf};

use super::ServerSpec;

pub fn server_for<'a>(file: &Path, specs: &'a [ServerSpec]) -> Option<(&'a ServerSpec, &'a str)> {
    let extension = format!(".{}", file.extension()?.to_str()?);
    specs.iter().find_map(|spec| spec.extension_to_language.get(&extension).map(|id| (spec, id.as_str())))
}

pub fn root_for(file: &Path, markers: &[String], cwd: &Path, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let inside = file.starts_with(cwd);
    let folder = file.parent().unwrap_or(file);
    let found = folder
        .ancestors()
        .take_while(|dir| !inside || dir.starts_with(cwd))
        .find(|dir| markers.iter().any(|marker| exists(&dir.join(marker))));
    match found {
        Some(dir) => dir.to_path_buf(),
        None if inside => cwd.to_path_buf(),
        None => folder.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::defaults;

    #[test]
    fn a_file_goes_to_the_server_of_its_extension() {
        let specs = defaults();
        let (server, language) = server_for(Path::new("/w/web/App.tsx"), &specs).expect("typescript");
        assert_eq!((server.name.as_str(), language), ("typescript", "typescriptreact"));
        assert!(server_for(Path::new("/w/notes.txt"), &specs).is_none());
        assert!(server_for(Path::new("/w/Makefile"), &specs).is_none());
    }

    #[test]
    fn the_root_is_the_closest_marker_but_never_above_the_working_folder() {
        let present = ["/w/Cargo.toml", "/w/crates/a/Cargo.toml", "/Cargo.toml", "/elsewhere/lib/Cargo.toml"];
        let exists = |path: &Path| present.iter().any(|p| Path::new(p) == path);
        let markers = ["Cargo.toml".to_owned()];
        let cwd = Path::new("/w");
        assert_eq!(root_for(Path::new("/w/crates/a/src/lib.rs"), &markers, cwd, exists), Path::new("/w/crates/a"));
        assert_eq!(root_for(Path::new("/w/src/main.rs"), &markers, cwd, exists), Path::new("/w"));
        assert_eq!(root_for(Path::new("/w/tools/x.rs"), &[], cwd, exists), Path::new("/w"));
        assert_eq!(root_for(Path::new("/elsewhere/lib/src/x.rs"), &markers, cwd, exists), Path::new("/elsewhere/lib"));
        assert_eq!(root_for(Path::new("/tmp/x.rs"), &[], cwd, exists), Path::new("/tmp"));
    }
}
