use axum::http::{StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};

pub const MISSING: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>Console</title></head>\
<body style=\"font-family: system-ui; max-width: 40rem; margin: 4rem auto; line-height: 1.5\">\
<h1>The console app is not built yet</h1>\
<p>This program was compiled without the page. From the repository, with <a href=\"https://bun.sh\">Bun</a> installed, \
build the page and then the program again:</p>\
<pre>cargo run -p xtask -- web\ncargo build --release</pre>\
<p>The first command writes the page to <code>web/dist</code>; the second builds it into the program. \
Then start the program again and open this page from the new link it prints.</p></body></html>";

#[derive(Debug)]
pub struct Asset {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/bundle.rs"));

pub fn embedded() -> &'static [Asset] {
    BUNDLE
}

pub fn has_index(bundle: &[Asset]) -> bool {
    bundle.iter().any(|asset| asset.path == "index.html")
}

pub fn resolve<'a>(bundle: &'a [Asset], uri: &Uri) -> Option<&'a Asset> {
    let wanted = uri.path().trim_start_matches('/');
    let found = bundle.iter().find(|asset| asset.path == wanted);
    found.or_else(|| bundle.iter().find(|asset| asset.path == "index.html"))
}

pub fn content_type(asset: &Asset) -> String {
    mime_guess::from_path(asset.path).first_or_octet_stream().to_string()
}

pub fn serve(bundle: &'static [Asset], uri: &Uri) -> Response {
    let Some(asset) = resolve(bundle, uri) else {
        return (StatusCode::SERVICE_UNAVAILABLE, Html(MISSING)).into_response();
    };
    ([(header::CONTENT_TYPE, content_type(asset))], asset.bytes).into_response()
}
