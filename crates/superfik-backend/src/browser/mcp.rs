//! The browser tools as MCP sees them: their definitions and arguments,
//! and answers as tool results. Pure.

use serde_json::{Value, json};
use superfik_core::browser::*;
use superfik_core::{BrowserAnswer, BrowserOp, Locator};

fn target_schema() -> Value {
    json!({
        "ref": { "type": "integer", "description": "An element's [ref=N] from the last browser_snapshot." },
        "x": { "type": "number", "description": "Or a point: CSS pixels from the page's left edge." },
        "y": { "type": "number", "description": "And from its top edge." }
    })
}

pub fn tools() -> Vec<Value> {
    let none = json!({ "type": "object", "properties": {} });
    let mut type_properties = target_schema();
    type_properties["text"] = json!({ "type": "string", "description": "What to type." });
    type_properties["submit"] = json!({ "type": "boolean", "description": "Press Enter after typing." });
    let mut tools = vec![
        json!({
            "name": NAVIGATE_TOOL,
            "description": "Open an address in the browser card beside this conversation and wait for it to load. \
                            `localhost:3000` and bare domains work.",
            "inputSchema": { "type": "object", "properties": { "url": { "type": "string" } }, "required": ["url"] }
        }),
        json!({ "name": BACK_TOOL, "description": "Go back in the card's history.", "inputSchema": none }),
        json!({ "name": FORWARD_TOOL, "description": "Go forward in the card's history.", "inputSchema": none }),
        json!({ "name": RELOAD_TOOL, "description": "Reload the page.", "inputSchema": none }),
        json!({
            "name": SNAPSHOT_TOOL,
            "description": "Read the page: its address, title and visible text, with every link, button and \
                            field numbered as [ref=N] for browser_click and browser_type.",
            "inputSchema": {"type":"object","properties":{"offset":{"type":"integer","minimum":0,"maximum":10000000,"description":"Continue a truncated native snapshot at its returned offset."}}}
        }),
        json!({
            "name": CLICK_TOOL,
            "description": "Click an element by ref (scrolled into view first) or a point, as a user would.",
            "inputSchema": { "type": "object", "properties": target_schema() }
        }),
        json!({
            "name": TYPE_TOOL,
            "description": "Type text, into the element with `ref` (clicked first) or wherever the focus is.",
            "inputSchema": { "type": "object", "properties": type_properties, "required": ["text"] }
        }),
        json!({
            "name": KEY_TOOL,
            "description": "Press a key: Enter, Tab, Escape, Backspace, Delete, ArrowUp/Down/Left/Right, \
                            PageUp/Down, Home, End, or a single character.",
            "inputSchema": { "type": "object", "properties": { "key": { "type": "string" } }, "required": ["key"] }
        }),
        json!({
            "name": SCROLL_TOOL,
            "description": "Scroll the page by CSS pixels (positive y scrolls down).",
            "inputSchema": { "type": "object", "properties": {
                "x": { "type": "integer" }, "y": { "type": "integer" } } }
        }),
        json!({ "name": SCREENSHOT_TOOL, "description": "A PNG of what the page shows.", "inputSchema": none }),
        json!({
            "name": EVALUATE_TOOL,
            "description": "Run a JavaScript expression in the page and get its value back as JSON (promises are \
                            awaited).",
            "inputSchema": { "type": "object", "properties": { "expression": { "type": "string" } },
                             "required": ["expression"] }
        }),
        json!({
            "name": FILL_LOGIN_TOOL,
            "description": "Sign in on the page with a login from the session's vault: its username and \
                            password go into the page's sign-in form without you seeing them, once the user \
                            allows it. `item` is the login's name (mcp__superfik_vault__list_secrets lists them); \
                            it must be saved for this page's address. Submit the form yourself after.",
            "inputSchema": { "type": "object", "properties": { "item": { "type": "string" } },
                             "required": ["item"] }
        }),
        json!({
            "name": AUTOFILL_TOOL,
            "description": "Fill the page with a credential approved through request_credentials, on the first \
                            page where the site offers to sign in, whatever it looks like; do not pick a provider's \
                            button or a method first. Only the approval's status reaches you, never the values. \
                            Answers no_match when nothing approved fits the page: then go one step further and call \
                            it again. Submit the form yourself after.",
            "inputSchema": { "type": "object", "properties": {
                "website": { "type": "string", "description": "The approved login's site, when several are approved." } } }
        }),
    ];
    tools.extend(super::mission_tools::tools());
    tools
}

pub fn op(name: &str, arguments: &Value) -> Result<BrowserOp, String> {
    if let Some(op) = super::mission_tools::op(name, arguments) {
        return op;
    }
    Ok(match name {
        NAVIGATE_TOOL => BrowserOp::Navigate { url: required(arguments, "url")? },
        BACK_TOOL => BrowserOp::Back,
        FORWARD_TOOL => BrowserOp::Forward,
        RELOAD_TOOL => BrowserOp::Reload,
        SNAPSHOT_TOOL => match arguments.get("offset") {
            None => BrowserOp::Snapshot,
            Some(offset) => BrowserOp::SnapshotPage {
                offset: offset
                    .as_u64()
                    .and_then(|o| u32::try_from(o).ok())
                    .filter(|o| *o <= 10_000_000)
                    .ok_or("offset must be between 0 and 10000000")?,
            },
        },
        CLICK_TOOL => BrowserOp::Click {
            target: locator(arguments).ok_or("Give a `ref` from browser_snapshot, or `x` and `y`.")?,
        },
        TYPE_TOOL => BrowserOp::Type {
            target: locator(arguments),
            text: arguments.get("text").and_then(Value::as_str).unwrap_or_default().to_owned(),
            submit: arguments.get("submit").and_then(Value::as_bool).unwrap_or(false),
        },
        KEY_TOOL => BrowserOp::Key { key: required(arguments, "key")? },
        FILL_LOGIN_TOOL => BrowserOp::FillLogin { item: required(arguments, "item")? },
        SCROLL_TOOL => {
            let by = |key| arguments.get(key).and_then(Value::as_i64).unwrap_or(0).clamp(-100_000, 100_000) as i32;
            BrowserOp::Scroll { dx: by("x"), dy: by("y") }
        }
        SCREENSHOT_TOOL => BrowserOp::Screenshot,
        EVALUATE_TOOL => BrowserOp::Evaluate { script: required(arguments, "expression")? },
        _ => return Err(format!("unknown tool: {name}")),
    })
}

fn required(arguments: &Value, key: &str) -> Result<String, String> {
    match arguments.get(key).and_then(Value::as_str).map(str::trim) {
        Some(value) if !value.is_empty() => Ok(value.to_owned()),
        _ => Err(format!("Give `{key}`.")),
    }
}

fn locator(arguments: &Value) -> Option<Locator> {
    if let Some(r) = arguments.get("ref").and_then(Value::as_u64) {
        return u32::try_from(r).ok().map(Locator::Ref);
    }
    let x = arguments.get("x").and_then(Value::as_f64)?;
    let y = arguments.get("y").and_then(Value::as_f64)?;
    Some(Locator::Point { x: x as f32, y: y as f32 })
}

pub fn result(answer: Result<BrowserAnswer, String>) -> Value {
    match answer {
        Ok(BrowserAnswer { text, png: Some(png) }) => json!({ "content": [
            { "type": "image", "data": png, "mimeType": "image/png" },
            { "type": "text", "text": text },
        ] }),
        Ok(BrowserAnswer { text, png: None }) => crate::loopback::text(text, false),
        Err(error) => crate::loopback::text(error, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_become_operations() {
        assert_eq!(
            op(NAVIGATE_TOOL, &json!({ "url": " localhost:3000 " })),
            Ok(BrowserOp::Navigate { url: "localhost:3000".into() })
        );
        assert_eq!(op(CLICK_TOOL, &json!({ "ref": 4 })), Ok(BrowserOp::Click { target: Locator::Ref(4) }));
        assert_eq!(
            op(CLICK_TOOL, &json!({ "x": 10, "y": 20.5 })),
            Ok(BrowserOp::Click { target: Locator::Point { x: 10.0, y: 20.5 } })
        );
        assert!(op(CLICK_TOOL, &json!({})).is_err());
        assert!(op(NAVIGATE_TOOL, &json!({ "url": "" })).is_err());
        assert_eq!(
            op(TYPE_TOOL, &json!({ "text": "hi", "submit": true })),
            Ok(BrowserOp::Type { target: None, text: "hi".into(), submit: true })
        );
        assert_eq!(op(SCROLL_TOOL, &json!({ "y": 400 })), Ok(BrowserOp::Scroll { dx: 0, dy: 400 }));
        assert_eq!(
            op(FILL_LOGIN_TOOL, &json!({ "item": "GitHub" })),
            Ok(BrowserOp::FillLogin { item: "GitHub".into() })
        );
        assert!(op(FILL_LOGIN_TOOL, &json!({})).is_err());
        assert!(op("browser_fly", &json!({})).is_err());
    }

    #[test]
    fn every_tool_is_offered_and_screenshots_carry_an_image() {
        let names: Vec<String> = tools().iter().filter_map(|t| t["name"].as_str().map(str::to_owned)).collect();
        assert_eq!(names, BROWSER_TOOLS);
        let shot = result(Ok(BrowserAnswer { text: "page".into(), png: Some("AAAA".into()) }));
        assert_eq!(shot["content"][0]["type"], "image");
        assert_eq!(result(Err("no card".into()))["isError"], true);
    }
}
