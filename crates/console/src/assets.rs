use std::path::PathBuf;

use axum::http::{StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};

pub const MISSING: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>Console</title></head>\
<body style=\"font-family: system-ui; max-width: 40rem; margin: 4rem auto; line-height: 1.5\">\
<h1>The console app is not built yet</h1>\
<p>Install <a href=\"https://bun.sh\">Bun</a> and build the CLI again, or build the app once from the repository and reload this page:</p>\
<pre>cd web &amp;&amp; bun install &amp;&amp; bun run build</pre></body></html>";

#[derive(Debug)]
pub struct Asset {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/bundle.rs"));

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
    if bundle.iter().any(|asset| asset.path == "index.html") {
        return Assets::Bundle(bundle);
    }
    locate(folders).map_or(Assets::Missing, Assets::Folder)
}

pub fn discover() -> Assets {
    let chosen = std::env::var_os(brand::env("CONSOLE_DIR")).map(PathBuf::from);
    let installed = platform::data_dir(brand::NAME, brand::COMMAND).map(|data| data.join("console"));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    choose(chosen, BUNDLE, [installed, Some(source)])
}

pub fn serve(bundle: &'static [Asset], uri: &Uri) -> Response {
    let wanted = uri.path().trim_start_matches('/');
    let found = bundle.iter().find(|asset| asset.path == wanted);
    let Some(asset) = found.or_else(|| bundle.iter().find(|asset| asset.path == "index.html")) else {
        return (StatusCode::SERVICE_UNAVAILABLE, Html(MISSING)).into_response();
    };
    let kind = mime_guess::from_path(asset.path).first_or_octet_stream();
    ([(header::CONTENT_TYPE, kind.as_ref())], asset.bytes).into_response()
}
