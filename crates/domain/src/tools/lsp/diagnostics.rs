use serde_json::Value;

pub const MAX_PROBLEMS: usize = 20;

pub fn installing(server: &str) -> String {
    format!(
        "<diagnostics>\n{server} is still installing, so this file was not checked; later edits will be.\n</diagnostics>"
    )
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

pub const MAX_LISTED: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    File,
    Workspace,
}

impl Scope {
    pub const NAMES: [(&'static str, Self); 2] =
        [("diagnostics", Self::File), ("workspaceDiagnostics", Self::Workspace)];

    pub fn named(name: &str) -> Option<Self> {
        Self::NAMES.iter().find(|(known, _)| *known == name).map(|(_, scope)| *scope)
    }
}

fn counted(count: usize, one: &str) -> String {
    if count == 1 { format!("1 {one}") } else { format!("{count} {one}s") }
}

pub fn listed(files: &[(String, Vec<Value>)]) -> Option<String> {
    let mut problems: Vec<(u64, &str, u64, u64, String)> = files
        .iter()
        .flat_map(|(file, found)| found.iter().map(move |found| (file.as_str(), found)))
        .filter_map(|(file, found)| {
            let severity = found["severity"].as_u64().unwrap_or(1);
            let start = &found["range"]["start"];
            let message = found["message"].as_str()?.split_whitespace().collect::<Vec<_>>().join(" ");
            let (line, character) = (start["line"].as_u64()?, start["character"].as_u64()?);
            (severity <= 2).then_some((severity, file, line, character, message))
        })
        .collect();
    if problems.is_empty() {
        return None;
    }
    problems.sort();
    let errors = problems.iter().filter(|problem| problem.0 == 1).count();
    let mut touched: Vec<&str> = problems.iter().map(|problem| problem.1).collect();
    touched.sort_unstable();
    touched.dedup();
    let mut lines = vec![format!(
        "{} and {} in {}",
        counted(errors, "error"),
        counted(problems.len() - errors, "warning"),
        counted(touched.len(), "file")
    )];
    let more = problems.len().saturating_sub(MAX_LISTED);
    lines.extend(problems.into_iter().take(MAX_LISTED).map(|(severity, file, line, character, message)| {
        let kind = if severity == 1 { "error" } else { "warning" };
        format!("{file}:{}:{} {kind}: {message}", line + 1, character + 1)
    }));
    if more > 0 {
        lines.push(format!("… and {more} more"));
    }
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn diagnostics_asked_for_are_counted_listed_errors_first_and_cut_with_a_count() {
        let files = vec![
            ("src/b.rs".to_owned(), vec![problem(2, 3, "unused"), problem(3, 0, "a hint")]),
            ("src/a.rs".to_owned(), vec![problem(1, 9, "mismatched\n types")]),
        ];
        assert_eq!(
            listed(&files).as_deref(),
            Some(
                "1 error and 1 warning in 2 files\nsrc/a.rs:10:5 error: mismatched types\nsrc/b.rs:4:5 warning: unused"
            )
        );
        assert_eq!(listed(&[("a.rs".to_owned(), vec![problem(4, 0, "hint")])]), None);
        assert_eq!(listed(&[]), None);
        let many: Vec<Value> = (0..MAX_LISTED as u64 + 7).map(|line| problem(1, line, "bad")).collect();
        let text = listed(&[("a.rs".to_owned(), many)]).expect("listed");
        assert!(text.starts_with("57 errors and 0 warnings in 1 file\n"), "{text}");
        assert_eq!(text.lines().count(), MAX_LISTED + 2);
        assert!(text.ends_with("\n… and 7 more"));
    }

    #[test]
    fn the_diagnostics_operations_are_named_by_scope() {
        assert_eq!(Scope::named("diagnostics"), Some(Scope::File));
        assert_eq!(Scope::named("workspaceDiagnostics"), Some(Scope::Workspace));
        assert_eq!(Scope::named("hover"), None);
    }

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
