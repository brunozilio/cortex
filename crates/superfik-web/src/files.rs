//! The browser's file picker, standing in for the desktop's native dialog:
//! an `<input type=file>` the page clicks, whose files are read and handed
//! back (name, bytes) as `Later::Picked`.

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use winit::event_loop::EventLoopProxy;

use crate::page::{Later, console_error};

pub(crate) fn pick(proxy: EventLoopProxy<Later>) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
    let Ok(input) =
        document.create_element("input").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().map_err(Into::into))
    else {
        return;
    };
    input.set_type("file");
    input.set_multiple(true);
    let chosen = input.clone();
    let on_change = Closure::once_into_js(move || {
        let Some(list) = chosen.files() else { return };
        let files: Vec<web_sys::File> = (0..list.length()).filter_map(|i| list.get(i)).collect();
        wasm_bindgen_futures::spawn_local(async move {
            let mut read = Vec::new();
            for file in files {
                match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                    Ok(buffer) => read.push((file.name(), js_sys::Uint8Array::new(&buffer).to_vec())),
                    Err(_) => console_error(&format!("could not read {}", file.name())),
                }
            }
            if !read.is_empty() {
                let _ = proxy.send_event(Later::Picked(read));
            }
        });
    });
    input.set_onchange(Some(on_change.unchecked_ref()));
    input.click();
}
