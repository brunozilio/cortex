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
    Download { url: String, archive: Archive, sha256: String },
}

impl Recipe {
    pub const fn tool(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Npm { .. } => Some(("npm", "Node.js")),
            Self::Go { .. } => Some(("go", "Go")),
            Self::Download { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub recipe: Recipe,
    pub program: PathBuf,
}

const CLANGD: &str = "23.1.0";
const RUST_ANALYZER: &str = "2026-11-09";

pub fn install_for(spec: &ServerSpec, os: &str, arch: &str) -> Option<Install> {
    let built_in = super::defaults().into_iter().any(|own| own.name == spec.name && own.command == spec.command);
    if !built_in {
        return None;
    }
    let exe = if os == "windows" { ".exe" } else { "" };
    match spec.name.as_str() {
        "rust-analyzer" => rust_analyzer(os, arch, exe),
        "clangd" => clangd(os, arch, exe),
        "typescript" => npm(&["typescript-language-server@6.0.3", "typescript@6.0.3"], &spec.command, os),
        "pyright" => npm(&["pyright@1.1.414"], &spec.command, os),
        "gopls" => Some(Install {
            recipe: Recipe::Go { module: "golang.org/x/tools/gopls@v0.24.0".into() },
            program: PathBuf::from("bin").join(format!("gopls{exe}")),
        }),
        _ => None,
    }
}

fn rust_analyzer(os: &str, arch: &str, exe: &str) -> Option<Install> {
    let (target, archive, extension, sha256) = match (os, arch) {
        ("macos", "aarch64") => {
            ("apple-darwin", Archive::Gzip, "gz", "e43a65008f8f4291320f33c0545dd577ad7623a7407b584a27155833d3722dfb")
        }
        ("macos", "x86_64") => {
            ("apple-darwin", Archive::Gzip, "gz", "abf410a54e33e9f8b2d37f76dde8884db86a030d6a9cc70c0e3f87a77c084c5c")
        }
        ("linux", "aarch64") => (
            "unknown-linux-gnu",
            Archive::Gzip,
            "gz",
            "813ca3cec78ac6dab99c7bc9d51e63578f6d401b994ecf29cc0dcad86f9b2cd4",
        ),
        ("linux", "x86_64") => (
            "unknown-linux-gnu",
            Archive::Gzip,
            "gz",
            "8c039b25cb338b3970c731646677a4c9d4e9dd7a92c9ab9ae459c71467fbf4a9",
        ),
        ("windows", "aarch64") => {
            ("pc-windows-msvc", Archive::Zip, "zip", "ca0770b76d3881642221d0437f7535e324f22a9499bcd00bbebeb0228617249e")
        }
        ("windows", "x86_64") => {
            ("pc-windows-msvc", Archive::Zip, "zip", "973ab11e84955a7f8e3cf01ccd98a33f3ecbd6d4a14bf59bb164aa9e831c6fef")
        }
        _ => return None,
    };
    let url = format!(
        "https://github.com/rust-lang/rust-analyzer/releases/download/{RUST_ANALYZER}/rust-analyzer-{arch}-{target}.{extension}"
    );
    let recipe = Recipe::Download { url, archive, sha256: sha256.into() };
    Some(Install { recipe, program: format!("rust-analyzer{exe}").into() })
}

fn clangd(os: &str, arch: &str, exe: &str) -> Option<Install> {
    let (platform, sha256) = match (os, arch) {
        ("macos", "aarch64" | "x86_64") => ("mac", "1082e6638223b785ca2daf0939f13afcd0bb95c84ee9a4bbaff4745365159253"),
        ("linux", "x86_64") => ("linux", "e53b1a96196095faedb7642cf64964f7fb9ad4a0c1f00dd2c172a3d9dcbafdfd"),
        ("windows", "x86_64") => ("windows", "23412a240756a162e7b98a282f36aa2a23a88db5ce16a0cbc4fef7253768c810"),
        _ => return None,
    };
    let url = format!("https://github.com/clangd/clangd/releases/download/{CLANGD}/clangd-{platform}-{CLANGD}.zip");
    Some(Install {
        recipe: Recipe::Download { url, archive: Archive::Zip, sha256: sha256.into() },
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

    fn download(url: &str, archive: Archive, sha256: &str, program: &str) -> Option<Install> {
        let recipe = Recipe::Download { url: url.into(), archive, sha256: sha256.into() };
        Some(Install { recipe, program: program.into() })
    }

    #[test]
    fn rust_analyzer_comes_from_a_pinned_github_release_with_its_checksum() {
        let release = "https://github.com/rust-lang/rust-analyzer/releases/download/2026-11-09";
        assert_eq!(
            install("rust-analyzer", "macos", "aarch64"),
            download(
                &format!("{release}/rust-analyzer-aarch64-apple-darwin.gz"),
                Archive::Gzip,
                "e43a65008f8f4291320f33c0545dd577ad7623a7407b584a27155833d3722dfb",
                "rust-analyzer"
            )
        );
        assert_eq!(
            install("rust-analyzer", "linux", "x86_64"),
            download(
                &format!("{release}/rust-analyzer-x86_64-unknown-linux-gnu.gz"),
                Archive::Gzip,
                "8c039b25cb338b3970c731646677a4c9d4e9dd7a92c9ab9ae459c71467fbf4a9",
                "rust-analyzer"
            )
        );
        assert_eq!(
            install("rust-analyzer", "windows", "x86_64"),
            download(
                &format!("{release}/rust-analyzer-x86_64-pc-windows-msvc.zip"),
                Archive::Zip,
                "973ab11e84955a7f8e3cf01ccd98a33f3ecbd6d4a14bf59bb164aa9e831c6fef",
                "rust-analyzer.exe"
            )
        );
        assert_eq!(install("rust-analyzer", "freebsd", "x86_64"), None);
    }

    #[test]
    fn clangd_comes_from_a_pinned_release_where_one_is_built() {
        let release = "https://github.com/clangd/clangd/releases/download/23.1.0";
        assert_eq!(
            install("clangd", "macos", "aarch64"),
            download(
                &format!("{release}/clangd-mac-23.1.0.zip"),
                Archive::Zip,
                "1082e6638223b785ca2daf0939f13afcd0bb95c84ee9a4bbaff4745365159253",
                "clangd_23.1.0/bin/clangd"
            )
        );
        assert_eq!(
            install("clangd", "linux", "x86_64"),
            download(
                &format!("{release}/clangd-linux-23.1.0.zip"),
                Archive::Zip,
                "e53b1a96196095faedb7642cf64964f7fb9ad4a0c1f00dd2c172a3d9dcbafdfd",
                "clangd_23.1.0/bin/clangd"
            )
        );
        assert_eq!(
            install("clangd", "windows", "x86_64"),
            download(
                &format!("{release}/clangd-windows-23.1.0.zip"),
                Archive::Zip,
                "23412a240756a162e7b98a282f36aa2a23a88db5ce16a0cbc4fef7253768c810",
                "clangd_23.1.0/bin/clangd.exe"
            )
        );
        assert_eq!(install("clangd", "linux", "aarch64"), None);
    }

    #[test]
    fn node_servers_come_from_npm_and_gopls_from_go_install() {
        let npm = |packages: &[&str]| Recipe::Npm { packages: packages.iter().map(|p| (*p).to_owned()).collect() };
        assert_eq!(
            install("typescript", "macos", "aarch64"),
            Some(Install {
                recipe: npm(&["typescript-language-server@6.0.3", "typescript@6.0.3"]),
                program: "node_modules/.bin/typescript-language-server".into(),
            })
        );
        assert_eq!(
            install("pyright", "windows", "x86_64"),
            Some(Install {
                recipe: npm(&["pyright@1.1.414"]),
                program: "node_modules/.bin/pyright-langserver.cmd".into()
            })
        );
        assert_eq!(
            install("gopls", "linux", "aarch64"),
            Some(Install {
                recipe: Recipe::Go { module: "golang.org/x/tools/gopls@v0.24.0".into() },
                program: "bin/gopls".into(),
            })
        );
    }

    #[test]
    fn a_recipe_names_the_tool_it_runs_and_a_download_needs_none() {
        assert_eq!(install("pyright", "linux", "x86_64").expect("npm").recipe.tool(), Some(("npm", "Node.js")));
        assert_eq!(install("gopls", "linux", "x86_64").expect("go").recipe.tool(), Some(("go", "Go")));
        assert_eq!(install("clangd", "linux", "x86_64").expect("download").recipe.tool(), None);
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
