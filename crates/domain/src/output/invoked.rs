use crate::permission_rules::{normalized, split_command};

pub fn program(command: &str) -> String {
    let (parts, _) = split_command(command);
    parts.first().and_then(|part| peeled(&normalized(part)).into_iter().next()).unwrap_or_else(|| "shell".to_owned())
}

pub(super) fn invocations(command: &str) -> Option<Vec<Vec<String>>> {
    if piped(command) {
        return None;
    }
    let (parts, _) = split_command(command);
    Some(parts.into_iter().map(|part| peeled(&normalized(part))).filter(|words| !words.is_empty()).collect())
}

fn peeled(normalized: &str) -> Vec<String> {
    let words: Vec<&str> = normalized.split(' ').filter(|word| !word.is_empty()).collect();
    let rest = match words.as_slice() {
        ["npx" | "bunx", rest @ ..]
        | ["pnpm", "exec" | "dlx", rest @ ..]
        | ["uv" | "poetry" | "pdm", "run", rest @ ..]
        | ["bundle", "exec", rest @ ..]
        | ["python" | "python3", "-m", rest @ ..] => rest,
        _ => return words.into_iter().map(str::to_owned).collect(),
    };
    rest.iter().skip_while(|word| word.starts_with('-')).map(|word| (*word).to_owned()).collect()
}

fn piped(command: &str) -> bool {
    let bytes = command.as_bytes();
    let (mut single, mut double, mut i) = (false, false, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if !single => i += 1,
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            b'|' if !single && !double => {
                if bytes.get(i + 1) == Some(&b'|') {
                    i += 1;
                } else if i == 0 || bytes[i - 1] != b'>' {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(command: &str) -> Vec<Vec<String>> {
        invocations(command).unwrap_or_default()
    }

    #[test]
    fn each_part_of_a_command_names_its_program_without_wrappers() {
        assert_eq!(
            words("cd crates && RUST_LOG=1 cargo test -p x"),
            [vec!["cd", "crates"], vec!["cargo", "test", "-p", "x"]]
        );
        assert_eq!(words("npx --yes jest src"), [vec!["jest", "src"]]);
        assert_eq!(words("python3 -m pytest -q"), [vec!["pytest", "-q"]]);
        assert_eq!(words("uv run pytest || echo 'a | b'"), [vec!["pytest"], vec!["echo", "'a", "|", "b'"]]);
        assert_eq!(words("/usr/bin/git status"), [vec!["git", "status"]]);
    }

    #[test]
    fn a_piped_command_already_chose_its_output() {
        assert!(invocations("go test -v ./... | grep -- '--- PASS'").is_none());
        assert!(invocations("cargo test |& tail").is_none());
        assert!(invocations("echo 'a|b' >| out.txt").is_some());
    }

    #[test]
    fn the_program_names_the_command_for_the_savings() {
        assert_eq!(program("FOO=1 cargo test | tail -5"), "cargo");
        assert_eq!(program("npx vitest run"), "vitest");
        assert_eq!(program("   "), "shell");
    }
}
