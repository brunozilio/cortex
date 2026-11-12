use serde_json::Value;

pub const MAX_PROBLEMS: usize = 20;

pub fn installing(server: &str) -> String {
    format!("<diagnostics>\n{server} is still installing, so this file was not checked; later edits will be.\n</diagnostics>")
}

pub fn report(file: &str, diagnostics: &[Value]) -> Option<String> {
    let mut problems: Vec<(u64, u64, u64, String)> = diagnostics
        .iter()
        .filter_map(|found| {
            let severity = found["severity"].as_u64().unwrap_or(1);
            let start = &found["range"]["start"];
            let message = found["message"].as_str()?.split_whitespace().collect::<Vec<_>>().join(" ");
            let (line, character) = (start["line"].as_u64()?, start["character"].as_u64()?);
            (severity <= 2).then_some((severity, line, character, message))
        })
        .collect();
    if problems.is_empty() {
        return None;
    }
    problems.sort();
    let more = problems.len().saturating_sub(MAX_PROBLEMS);
    let mut lines: Vec<String> = problems
        .into_iter()
        .take(MAX_PROBLEMS)
        .map(|(severity, line, character, message)| {
            let kind = if severity == 1 { "error" } else { "warning" };
            format!("{file}:{}:{} {kind}: {message}", line + 1, character + 1)
        })
        .collect();
    if more > 0 {
        lines.push(format!("… and {more} more"));
    }
    Some(format!("<diagnostics>\n{}\n</diagnostics>", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn problem(severity: u64, line: u64, message: &str) -> Value {
        json!({
            "severity": severity, "message": message, "source": "rustc",
            "range": {"start": {"line": line, "character": 4}, "end": {"line": line, "character": 9}}
        })
    }

    #[test]
    fn errors_come_first_and_hints_are_left_out() {
        let found = [
            problem(2, 1, "unused variable `x`"),
            problem(4, 0, "consider this"),
            problem(1, 7, "mismatched types\nexpected `u32`"),
        ];
        assert_eq!(
            report("src/a.rs", &found).as_deref(),
            Some(
                "<diagnostics>\nsrc/a.rs:8:5 error: mismatched types expected `u32`\nsrc/a.rs:2:5 warning: unused \
                 variable `x`\n</diagnostics>"
            )
        );
        assert_eq!(report("src/a.rs", &[problem(3, 0, "info")]), None);
        assert_eq!(report("src/a.rs", &[]), None);
    }

    #[test]
    fn a_long_list_is_cut_and_says_how_many_more() {
        let found: Vec<Value> = (0..25).map(|line| problem(1, line, "bad")).collect();
        let text = report("a.rs", &found).expect("report");
        assert_eq!(text.lines().count(), MAX_PROBLEMS + 3);
        assert!(text.ends_with("… and 5 more\n</diagnostics>"));
    }
}
