use std::path::{Path, PathBuf};

const WINDOWS_EXTENSIONS: [&str; 3] = ["exe", "cmd", "bat"];

pub fn file_names(os: &str, name: &str) -> Vec<String> {
    if os != "windows" {
        return vec![name.to_owned()];
    }
    let given = Path::new(name).extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase);
    if given.is_some_and(|ext| WINDOWS_EXTENSIONS.contains(&ext.as_str())) {
        return vec![name.to_owned()];
    }
    WINDOWS_EXTENSIONS.iter().map(|ext| format!("{name}.{ext}")).collect()
}

pub fn find(os: &str, names: &[&str], dirs: &[PathBuf], is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    names.iter().find_map(|name| {
        let spellings = file_names(os, name);
        dirs.iter().find_map(|dir| spellings.iter().map(|file| dir.join(file)).find(|candidate| is_file(candidate)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_spells_a_tool_with_the_extensions_it_starts() {
        assert_eq!(file_names("windows", "claude"), ["claude.exe", "claude.cmd", "claude.bat"]);
        assert_eq!(file_names("windows", "npx.cmd"), ["npx.cmd"]);
        assert_eq!(file_names("macos", "claude"), ["claude"]);
    }

    #[test]
    fn npms_extensionless_script_is_never_chosen_on_windows() {
        let npm = PathBuf::from(r"C:\Users\ana\AppData\Roaming").join("npm");
        let script = npm.join("claude");
        let shim = npm.join("claude.cmd");
        let found = find("windows", &["claude"], std::slice::from_ref(&npm), |path| path == script || path == shim);
        assert_eq!(found, Some(shim));
    }

    #[test]
    fn an_exe_beats_a_shim_in_the_same_folder() {
        let dir = PathBuf::from(r"C:\tools");
        let found = find("windows", &["gh"], std::slice::from_ref(&dir), |_| true);
        assert_eq!(found, Some(dir.join("gh.exe")));
    }

    #[test]
    fn earlier_folders_win_and_names_keep_their_order() {
        let (a, b) = (PathBuf::from("/a"), PathBuf::from("/b"));
        let in_b = |path: &Path| path == Path::new("/b/kiro-cli") || path == Path::new("/a/kiro");
        assert_eq!(find("macos", &["kiro-cli", "kiro"], &[a, b], in_b), Some(PathBuf::from("/b/kiro-cli")));
    }
}
