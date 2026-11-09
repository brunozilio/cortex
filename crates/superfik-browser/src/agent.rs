//! Common agent input: native key names, clipboard edits and result text.
//! Semantic snapshots and element refs use the native accessibility protocol.
//! Pure.

use superfik_core::Locator;

use crate::{KeyInput, KeyPhase, Modifiers};

pub const SNAPSHOT_LIMIT: usize = 20_000;

pub fn point(locator: Locator) -> Option<(f32, f32)> {
    match locator {
        Locator::Point { x, y } => Some((x, y)),
        Locator::Ref(_) => None,
    }
}

pub fn named_key(name: &str) -> Option<KeyInput> {
    let key =
        |code: i32, text: Option<char>| KeyInput { phase: KeyPhase::Down, code, text, modifiers: Modifiers::default() };
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "enter" | "return" => key(0x0D, Some('\r')),
        "tab" => key(0x09, None),
        "escape" | "esc" => key(0x1B, None),
        "backspace" => key(0x08, None),
        "delete" => key(0x2E, None),
        "space" => key(0x20, Some(' ')),
        "arrowleft" | "left" => key(0x25, None),
        "arrowup" | "up" => key(0x26, None),
        "arrowright" | "right" => key(0x27, None),
        "arrowdown" | "down" => key(0x28, None),
        "pageup" => key(0x21, None),
        "pagedown" => key(0x22, None),
        "home" => key(0x24, None),
        "end" => key(0x23, None),
        _ => {
            let mut chars = name.chars();
            let c = chars.next().filter(|_| chars.next().is_none())?;
            let code = if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() as i32 } else { 0 };
            key(code, Some(c))
        }
    })
}

pub fn edit_script(command: &crate::EditCommand) -> String {
    use crate::EditCommand;
    let selection = "(function () { var e = document.activeElement; \
        if (e && typeof e.selectionStart === 'number' && e.type !== 'password') \
        return e.value.slice(e.selectionStart, e.selectionEnd); \
        return String(document.getSelection() || ''); })()";
    match command {
        EditCommand::Copy => selection.to_owned(),
        EditCommand::Cut => {
            format!("(function () {{ var t = {selection}; document.execCommand('delete'); return t; }})()")
        }
        EditCommand::Paste(text) => {
            let text = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned());
            format!("document.execCommand('insertText', false, {text})")
        }
        EditCommand::SelectAll => "document.execCommand('selectAll')".to_owned(),
        EditCommand::Undo => "document.execCommand('undo')".to_owned(),
        EditCommand::Redo => "document.execCommand('redo')".to_owned(),
    }
}

pub fn readable(json: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(serde_json::Value::String(text)) => text,
        _ => json.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_have_codes_and_enter_types_a_return() {
        assert_eq!(named_key("Enter").map(|k| (k.code, k.text)), Some((0x0D, Some('\r'))));
        assert_eq!(named_key("ArrowDown").map(|k| k.code), Some(0x28));
        assert_eq!(named_key("a").map(|k| (k.code, k.text)), Some((0x41, Some('a'))));
        assert!(named_key("Hyper").is_none());
    }

    #[test]
    fn pasted_text_is_quoted_safely() {
        let script = edit_script(&crate::EditCommand::Paste("a\"b');\nalert(1)//".into()));
        assert_eq!(script, r#"document.execCommand('insertText', false, "a\"b');\nalert(1)//")"#);
        assert!(edit_script(&crate::EditCommand::Cut).contains("execCommand('delete')"));
    }

    #[test]
    fn evaluated_json_reads_as_text_without_string_quotes() {
        assert_eq!(readable(r#""hello""#), "hello");
        assert_eq!(readable("[1,2]"), "[1,2]");
    }
}
