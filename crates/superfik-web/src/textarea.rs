//! The page's hidden text field. A canvas is not editable, so the browser
//! never offers it an input method (accents on some systems, Japanese,
//! Chinese, a phone's keyboard). The keys go to an invisible `<textarea>`
//! that keeps the focus instead: plain keys become the desktop's key
//! commands (`dom_keys`, `input::keys::map_key`), and what an input method
//! composes arrives whole on `compositionend`.

use superfik_client::input::keys::map_key;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use winit::event_loop::EventLoopProxy;

use crate::dom_keys;
use crate::page::Later;

pub(crate) fn attach(canvas: &web_sys::HtmlCanvasElement, proxy: &EventLoopProxy<Later>) -> Option<()> {
    let document = web_sys::window()?.document()?;
    let field: web_sys::HtmlTextAreaElement = document.create_element("textarea").ok()?.dyn_into().ok()?;
    for (name, value) in
        [("autocapitalize", "off"), ("autocomplete", "off"), ("spellcheck", "false"), ("aria-hidden", "true")]
    {
        let _ = field.set_attribute(name, value);
    }
    let style = field.style();
    for (name, value) in [
        ("position", "fixed"),
        ("left", "0"),
        ("top", "0"),
        ("width", "1px"),
        ("height", "1px"),
        ("opacity", "0"),
        ("border", "0"),
        ("padding", "0"),
        ("resize", "none"),
        ("pointer-events", "none"),
    ] {
        let _ = style.set_property(name, value);
    }
    document.body()?.append_child(&field).ok()?;
    listen(&field, "keydown", key_down(proxy.clone()));
    listen(&field, "keyup", modifiers(proxy.clone()));
    listen(&field, "compositionend", composed(field.clone(), proxy.clone()));
    listen(&field, "input", inserted(field.clone(), proxy.clone()));
    let refocus = field.clone();
    listen(canvas, "focus", move |_| {
        let _ = refocus.focus();
    });
    let _ = field.focus();
    Some(())
}

fn listen(target: &web_sys::EventTarget, kind: &str, handler: impl FnMut(web_sys::Event) + 'static) {
    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(handler);
    let _ = target.add_event_listener_with_callback(kind, handler.as_ref().unchecked_ref());
    handler.forget();
}

fn held(event: &web_sys::KeyboardEvent) -> winit::keyboard::ModifiersState {
    dom_keys::modifiers(event.shift_key(), event.ctrl_key(), event.alt_key(), event.meta_key())
}

fn key_down(proxy: EventLoopProxy<Later>) -> impl FnMut(web_sys::Event) {
    move |event| {
        let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() else { return };
        let mods = held(event);
        let _ = proxy.send_event(Later::Modifiers(mods));
        if event.is_composing() {
            return;
        }
        let page = dom_keys::page_key(&event.code(), &event.key(), true, mods);
        let Some((key, text)) = dom_keys::key(&event.key(), mods) else {
            if let Some(page) = page {
                let _ = proxy.send_event(Later::PageKey(page));
            }
            return;
        };
        let command = map_key(&key, text.as_deref(), mods);
        if !command.kept_by_browser() {
            event.prevent_default();
        }
        let _ = proxy.send_event(Later::Key(command, page));
    }
}

fn modifiers(proxy: EventLoopProxy<Later>) -> impl FnMut(web_sys::Event) {
    move |event| {
        if let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() {
            let mods = held(event);
            let _ = proxy.send_event(Later::Modifiers(mods));
            if let Some(page) = dom_keys::page_key(&event.code(), &event.key(), false, mods) {
                let _ = proxy.send_event(Later::PageKey(page));
            }
        }
    }
}

fn inserted(field: web_sys::HtmlTextAreaElement, proxy: EventLoopProxy<Later>) -> impl FnMut(web_sys::Event) {
    move |event| {
        let Some(event) = event.dyn_ref::<web_sys::InputEvent>() else { return };
        if event.is_composing() || event.input_type() != "insertText" {
            return;
        }
        field.set_value("");
        if let Some(text) = event.data().filter(|text| !text.is_empty()) {
            let _ = proxy.send_event(Later::Text(text));
        }
    }
}

fn composed(field: web_sys::HtmlTextAreaElement, proxy: EventLoopProxy<Later>) -> impl FnMut(web_sys::Event) {
    move |event| {
        let text = event.dyn_ref::<web_sys::CompositionEvent>().and_then(web_sys::CompositionEvent::data);
        field.set_value("");
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            let _ = proxy.send_event(Later::Text(text));
        }
    }
}
