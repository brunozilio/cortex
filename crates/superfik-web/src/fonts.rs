//! CJK faces for a browser tab, which has no system fonts to fall back on.
//! None ships in the wasm: when text first needs one
//! (`TextSystem::take_missing`), the page fetches it from these addresses
//! in order and hands it to `TextSystem::add_face`. A server may serve its
//! own copy beside the bundle (`fonts/` in `SUPERFIK_WEB`, for networks
//! that cannot reach the CDN); otherwise Noto Sans CJK (OFL) comes from
//! jsDelivr, pinned to a release.

use superfik_render::Script;

const RELEASE: &str = "https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@Sans2.004/Sans/SubsetOTF";

fn subset(script: Script) -> (&'static str, &'static str) {
    match script {
        Script::Han => ("SC", "NotoSansSC-Regular.otf"),
        Script::Hangul => ("KR", "NotoSansKR-Regular.otf"),
    }
}

pub fn sources(script: Script) -> [String; 2] {
    let (region, file) = subset(script);
    [format!("fonts/{file}"), format!("{RELEASE}/{region}/{file}")]
}

#[cfg(target_arch = "wasm32")]
pub(crate) use fetch::fetch;

#[cfg(target_arch = "wasm32")]
mod fetch {
    use wasm_bindgen::JsCast;
    use winit::event_loop::EventLoopProxy;

    use super::{Script, sources};
    use crate::page::{Later, console_error};

    pub(crate) fn fetch(script: Script, proxy: EventLoopProxy<Later>) {
        wasm_bindgen_futures::spawn_local(async move {
            for url in sources(script) {
                if let Some(bytes) = get(&url).await {
                    let _ = proxy.send_event(Later::Face(script, bytes));
                    return;
                }
            }
            console_error(&format!("no font for {script:?} could be fetched"));
        });
    }

    async fn get(url: &str) -> Option<Vec<u8>> {
        let window = web_sys::window()?;
        let answer = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url)).await.ok()?;
        let response = answer.dyn_into::<web_sys::Response>().ok().filter(web_sys::Response::ok)?;
        let buffer = wasm_bindgen_futures::JsFuture::from(response.array_buffer().ok()?).await.ok()?;
        Some(js_sys::Uint8Array::new(&buffer).to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_servers_own_copy_comes_before_the_cdn() {
        let [own, cdn] = sources(Script::Han);
        assert_eq!(own, "fonts/NotoSansSC-Regular.otf");
        assert!(cdn.starts_with("https://cdn.jsdelivr.net/") && cdn.ends_with("/SC/NotoSansSC-Regular.otf"));
        assert!(sources(Script::Hangul)[1].ends_with("/KR/NotoSansKR-Regular.otf"));
    }
}
