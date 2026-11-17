use std::time::Duration;

use serde_json::Value;

use crate::features::{CODE_MODE, CODE_MODE_ONLY, Features};

pub const TOOL: &str = "REPL";
pub const SEARCH: &str = "searchTools";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Beside,
    Only,
}

impl Mode {
    pub fn of(features: &Features) -> Self {
        match (features.on(CODE_MODE), features.on(CODE_MODE_ONLY)) {
            (false, _) => Self::Off,
            (true, false) => Self::Beside,
            (true, true) => Self::Only,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Declared<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub schema: &'a Value,
}

pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

pub fn declarations(tools: &[Declared<'_>]) -> String {
    let mut sorted: Vec<&Declared<'_>> = tools.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(b.name));
    sorted.iter().map(|tool| declaration(tool)).collect::<Vec<_>>().join("\n\n")
}

pub fn declaration(tool: &Declared<'_>) -> String {
    let callee = if is_identifier(tool.name) {
        format!("function {}", tool.name)
    } else {
        format!("function tools[{:?}]", tool.name)
    };
    format!("{}declare {callee}(input: {}): Promise<string>;", doc(tool.description, ""), type_of(tool.schema, ""))
}

fn doc(text: &str, indent: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    let lines: Vec<String> =
        text.replace("*/", "*\\/").lines().map(|line| format!("{indent} * {line}").trim_end().to_owned()).collect();
    format!("{indent}/**\n{}\n{indent} */\n", lines.join("\n"))
}

fn type_of(schema: &Value, indent: &str) -> String {
    if let Some(choices) = schema.get("enum").and_then(Value::as_array) {
        return choices.iter().map(Value::to_string).collect::<Vec<_>>().join(" | ");
    }
    if let Some(constant) = schema.get("const") {
        return constant.to_string();
    }
    if let Some(options) = schema.get("anyOf").or_else(|| schema.get("oneOf")).and_then(Value::as_array) {
        return options.iter().map(|option| type_of(option, indent)).collect::<Vec<_>>().join(" | ");
    }
    match schema.get("type") {
        Some(Value::Array(types)) => {
            types.iter().map(|kind| type_of(&with_type(schema, kind), indent)).collect::<Vec<_>>().join(" | ")
        }
        Some(Value::String(kind)) => match kind.as_str() {
            "string" => "string".to_owned(),
            "integer" | "number" => "number".to_owned(),
            "boolean" => "boolean".to_owned(),
            "null" => "null".to_owned(),
            "array" => array_of(schema, indent),
            "object" => object_of(schema, indent),
            _ => "unknown".to_owned(),
        },
        _ if schema.get("properties").is_some() => object_of(schema, indent),
        _ => "unknown".to_owned(),
    }
}

fn with_type(schema: &Value, kind: &Value) -> Value {
    let mut single = schema.clone();
    if let Some(object) = single.as_object_mut() {
        object.insert("type".to_owned(), kind.clone());
    }
    single
}

fn array_of(schema: &Value, indent: &str) -> String {
    let item = schema.get("items").map_or_else(|| "unknown".to_owned(), |items| type_of(items, indent));
    if item.contains(' ') && !item.starts_with('{') { format!("({item})[]") } else { format!("{item}[]") }
}

fn object_of(schema: &Value, indent: &str) -> String {
    let Some(properties) = schema.get("properties").and_then(Value::as_object).filter(|map| !map.is_empty()) else {
        return "Record<string, unknown>".to_owned();
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |list| list.iter().filter_map(Value::as_str).collect());
    let inner = format!("{indent}  ");
    let fields: Vec<String> = properties
        .iter()
        .map(|(name, property)| {
            let key = if is_identifier(name) { name.clone() } else { format!("{name:?}") };
            let optional = if required.contains(&name.as_str()) { "" } else { "?" };
            let described = property.get("description").and_then(Value::as_str).unwrap_or_default();
            format!("{}{inner}{key}{optional}: {};", doc(described, &inner), type_of(property, &inner))
        })
        .collect();
    format!("{{\n{}\n{indent}}}", fields.join("\n"))
}
pub const PART: usize = 20_000;
pub const SHORTEST: Duration = Duration::from_secs(1);
pub const LONGEST: Duration = Duration::from_secs(600);

pub fn total_limit(requested_ms: Option<u64>) -> Duration {
    requested_ms.map_or(LONGEST, |ms| Duration::from_millis(ms).clamp(SHORTEST, LONGEST))
}

pub fn cut_middle(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let head = limit / 2;
    let tail = limit - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(count - tail).collect();
    format!("{start}\n[... {} characters cut from the middle ...]\n{end}", count - limit)
}

pub fn shown(logs: &[String], outcome: &Result<String, String>, limit: usize) -> String {
    let mut parts = Vec::new();
    if !logs.is_empty() {
        let lines: Vec<String> = logs.iter().map(|line| cut_middle(line, limit)).collect();
        parts.push(format!("<log>\n{}\n</log>", lines.join("\n")));
    }
    parts.push(match outcome {
        Ok(result) => format!("<result>\n{}\n</result>", cut_middle(result, limit)),
        Err(error) => format!("<error>\n{}\n</error>", cut_middle(error, limit)),
    });
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_short_text_is_kept_whole_and_a_long_one_keeps_its_head_and_tail() {
        assert_eq!(cut_middle("short", 10), "short");
        assert_eq!(cut_middle("abcdefghij", 4), "ab\n[... 6 characters cut from the middle ...]\nij");
        assert_eq!(cut_middle("ééééé", 2), "é\n[... 3 characters cut from the middle ...]\né");
    }

    #[test]
    fn the_output_holds_the_logs_then_the_result_or_the_error() {
        let logs = vec!["first".to_owned(), "second".to_owned()];

        assert_eq!(shown(&logs, &Ok("42".into()), PART), "<log>\nfirst\nsecond\n</log>\n<result>\n42\n</result>");
        assert_eq!(shown(&[], &Err("TypeError: x".into()), PART), "<error>\nTypeError: x\n</error>");
    }

    #[test]
    fn each_part_is_cut_on_its_own() {
        let logs = vec!["0123456789".to_owned()];

        let text = shown(&logs, &Ok("abcdefghij".into()), 4);

        assert!(text.contains("01\n[... 6 characters cut from the middle ...]\n89"));
        assert!(text.contains("ab\n[... 6 characters cut from the middle ...]\nij"));
    }

    #[test]
    fn the_flags_choose_off_beside_or_only() {
        let flags = |value: serde_json::Value| Features::default().layered(&value, true).expect("flags");

        assert_eq!(Mode::of(&Features::default()), Mode::Beside);
        assert_eq!(Mode::of(&flags(serde_json::json!({ CODE_MODE_ONLY: true }))), Mode::Only);
        assert_eq!(Mode::of(&flags(serde_json::json!({ CODE_MODE: false, CODE_MODE_ONLY: true }))), Mode::Off);
    }

    #[test]
    fn a_tool_is_declared_as_a_typed_async_function_with_its_documentation() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "The regular expression." },
                "output_mode": { "enum": ["content", "count"] },
                "-i": { "type": "boolean" },
                "paths": { "type": "array", "items": { "type": "string" } },
                "limit": { "type": ["integer", "null"] }
            },
            "required": ["pattern"]
        });

        let text = declaration(&Declared { name: "Grep", description: "Search files.\nFast.", schema: &schema });

        assert_eq!(
            text,
            "/**\n * Search files.\n * Fast.\n */\ndeclare function Grep(input: {\n  \"-i\"?: boolean;\n  limit?: number | null;\n  \
             output_mode?: \"content\" | \"count\";\n  paths?: string[];\n  /**\n   * The regular expression.\n   */\n  pattern: string;\n\
             }): Promise<string>;"
        );
    }

    #[test]
    fn a_name_that_is_no_identifier_is_declared_through_tools_and_the_list_is_sorted() {
        let empty = serde_json::json!({ "type": "object" });
        let tools = [
            Declared { name: "Zed", description: "", schema: &empty },
            Declared { name: "has-dash", description: "", schema: &empty },
            Declared { name: "Alpha", description: "", schema: &empty },
        ];

        let text = declarations(&tools);

        assert_eq!(
            text,
            "declare function Alpha(input: Record<string, unknown>): Promise<string>;\n\n\
             declare function Zed(input: Record<string, unknown>): Promise<string>;\n\n\
             declare function tools[\"has-dash\"](input: Record<string, unknown>): Promise<string>;"
        );
        assert!(is_identifier("mcp__notes__find") && is_identifier("$") && !is_identifier("9a") && !is_identifier(""));
    }

    #[test]
    fn a_description_cannot_close_its_comment_early() {
        let empty = serde_json::json!({});

        let text = declaration(&Declared { name: "X", description: "ends */ here", schema: &empty });

        assert!(
            text.contains("ends *\\/ here") && text.ends_with("declare function X(input: unknown): Promise<string>;")
        );
    }

    #[test]
    fn a_script_runs_ten_minutes_at_most_and_a_second_at_least() {
        assert_eq!(total_limit(None), Duration::from_secs(600));
        assert_eq!(total_limit(Some(5_000)), Duration::from_secs(5));
        assert_eq!(total_limit(Some(10)), Duration::from_secs(1));
        assert_eq!(total_limit(Some(9_000_000)), Duration::from_secs(600));
    }
}
