use std::path::PathBuf;

use super::ServerSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archive {
    Gzip,
    Zip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipe {
    Npm { packages: Vec<String> },
    Go { module: String },
    Download { url: String, archive: Archive },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub recipe: Recipe,
    pub program: PathBuf,
}

const CLANGD: &str = "23.1.0";

pub fn install_for(spec: &ServerSpec, os: &str, arch: &str) -> Option<Install> {
    let built_in = super::defaults().into_iter().any(|own| own.name == spec.name && own.command == spec.command);
    if !built_in {
        return None;
    }
    let exe = if os == "windows" { ".exe" } else { "" };
    match spec.name.as_str() {
        "rust-analyzer" => rust_analyzer(os, arch, exe),
        "clangd" => clangd(os, arch, exe),
        "typescript" => npm(&["typescript-language-server", "typescript@6"], &spec.command, os),
        "pyright" => npm(&["pyright"], &spec.command, os),
        "gopls" => Some(Install {
            recipe: Recipe::Go { module: "golang.org/x/tools/gopls@latest".into() },
            program: PathBuf::from("bin").join(format!("gopls{exe}")),
        }),
        _ => None,
    }
}

fn rust_analyzer(os: &str, arch: &str, exe: &str) -> Option<Install> {
    if !matches!(arch, "aarch64" | "x86_64") {
        return None;
    }
    let (target, archive, extension) = match os {
        "macos" => ("apple-darwin", Archive::Gzip, "gz"),
        "linux" => ("unknown-linux-gnu", Archive::Gzip, "gz"),
        "windows" => ("pc-windows-msvc", Archive::Zip, "zip"),
        _ => return None,
    };
    let url = format!(
        "https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-{arch}-{target}.{extension}"
    );
    Some(Install { recipe: Recipe::Download { url, archive }, program: format!("rust-analyzer{exe}").into() })
}

fn clangd(os: &str, arch: &str, exe: &str) -> Option<Install> {
    let platform = match (os, arch) {
        ("macos", "aarch64" | "x86_64") => "mac",
        ("linux", "x86_64") => "linux",
        ("windows", "x86_64") => "windows",
        _ => return None,
    };
    let url = format!("https://github.com/clangd/clangd/releases/download/{CLANGD}/clangd-{platform}-{CLANGD}.zip");
    Some(Install {
        recipe: Recipe::Download { url, archive: Archive::Zip },
        program: PathBuf::from(format!("clangd_{CLANGD}")).join("bin").join(format!("clangd{exe}")),
    })
}

fn npm(packages: &[&str], command: &str, os: &str) -> Option<Install> {
    let script = if os == "windows" { ".cmd" } else { "" };
    Some(Install {
        recipe: Recipe::Npm { packages: packages.iter().map(|package| (*package).to_owned()).collect() },
        program: PathBuf::from("node_modules").join(".bin").join(format!("{command}{script}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::defaults;

    fn install(name: &str, os: &str, arch: &str) -> Option<Install> {
        let spec = defaults().into_iter().find(|spec| spec.name == name).expect("built in");
        install_for(&spec, os, arch)
    }

    fn download(url: &str, archive: Archive, program: &str) -> Option<Install> {
        Some(Install { recipe: Recipe::Download { url: url.into(), archive }, program: program.into() })
    }

    #[test]
    fn rust_analyzer_comes_from_its_github_release_for_the_platform() {
        let latest = "https://github.com/rust-lang/rust-analyzer/releases/latest/download";
        assert_eq!(
            install("rust-analyzer", "macos", "aarch64"),
            download(&format!("{latest}/rust-analyzer-aarch64-apple-darwin.gz"), Archive::Gzip, "rust-analyzer")
        );
        assert_eq!(
            install("rust-analyzer", "linux", "x86_64"),
            download(&format!("{latest}/rust-analyzer-x86_64-unknown-linux-gnu.gz"), Archive::Gzip, "rust-analyzer")
        );
        assert_eq!(
            install("rust-analyzer", "windows", "x86_64"),
            download(&format!("{latest}/rust-analyzer-x86_64-pc-windows-msvc.zip"), Archive::Zip, "rust-analyzer.exe")
        );
        assert_eq!(install("rust-analyzer", "freebsd", "x86_64"), None);
    }

    #[test]
    fn clangd_comes_from_a_pinned_release_where_one_is_built() {
        let release = "https://github.com/clangd/clangd/releases/download/23.1.0";
        assert_eq!(
            install("clangd", "macos", "aarch64"),
            download(&format!("{release}/clangd-mac-23.1.0.zip"), Archive::Zip, "clangd_23.1.0/bin/clangd")
        );
        assert_eq!(
            install("clangd", "linux", "x86_64"),
            download(&format!("{release}/clangd-linux-23.1.0.zip"), Archive::Zip, "clangd_23.1.0/bin/clangd")
        );
        assert_eq!(
            install("clangd", "windows", "x86_64"),
            download(&format!("{release}/clangd-windows-23.1.0.zip"), Archive::Zip, "clangd_23.1.0/bin/clangd.exe")
        );
        assert_eq!(install("clangd", "linux", "aarch64"), None);
    }

    #[test]
    fn node_servers_come_from_npm_and_gopls_from_go_install() {
        let npm = |packages: &[&str]| Recipe::Npm { packages: packages.iter().map(|p| (*p).to_owned()).collect() };
        assert_eq!(
            install("typescript", "macos", "aarch64"),
            Some(Install {
                recipe: npm(&["typescript-language-server", "typescript@6"]),
                program: "node_modules/.bin/typescript-language-server".into(),
            })
        );
        assert_eq!(
            install("pyright", "windows", "x86_64"),
            Some(Install { recipe: npm(&["pyright"]), program: "node_modules/.bin/pyright-langserver.cmd".into() })
        );
        assert_eq!(
            install("gopls", "linux", "aarch64"),
            Some(Install {
                recipe: Recipe::Go { module: "golang.org/x/tools/gopls@latest".into() },
                program: "bin/gopls".into(),
            })
        );
    }

    #[test]
    fn a_server_with_its_own_command_is_never_installed() {
        let mut spec = defaults().into_iter().find(|spec| spec.name == "typescript").expect("built in");
        spec.command = "/opt/node/bin/typescript-language-server".into();
        assert_eq!(install_for(&spec, "macos", "aarch64"), None);
        let mut custom = spec.clone();
        custom.name = "deno".into();
        custom.command = "deno".into();
        assert_eq!(install_for(&custom, "macos", "aarch64"), None);
    }
}
