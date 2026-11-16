use std::path::PathBuf;

pub const MISSING: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>Console</title></head>\
<body style=\"font-family: system-ui; max-width: 40rem; margin: 4rem auto; line-height: 1.5\">\
<h1>The console app is not built yet</h1>\
<p>Build it once from the repository, then reload this page:</p>\
<pre>cd web &amp;&amp; bun install &amp;&amp; bun run build</pre></body></html>";

pub fn locate(candidates: impl IntoIterator<Item = Option<PathBuf>>) -> Option<PathBuf> {
    candidates.into_iter().flatten().find(|folder| folder.join("index.html").is_file())
}

pub fn places() -> [Option<PathBuf>; 3] {
    let chosen = std::env::var_os(brand::env("CONSOLE_DIR")).map(PathBuf::from);
    let installed = platform::data_dir(brand::NAME, brand::COMMAND).map(|data| data.join("console"));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    [chosen, installed, Some(source)]
}
