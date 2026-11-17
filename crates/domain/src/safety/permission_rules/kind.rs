use super::pattern::{collapsed, hides_more, split_command, without_harmless_redirects};

const NESTED: [&str; 12] =
    ["gh", "docker", "podman", "kubectl", "npm", "pnpm", "yarn", "bun", "aws", "gcloud", "az", "glab"];
const INTERPRETERS: [&str; 7] = ["python", "python3", "node", "ruby", "perl", "deno", "php"];
const NESTED_DEPTH: usize = 3;

pub fn command_kind(line: &str) -> Option<String> {
    let (parts, balanced) = split_command(line);
    if !balanced {
        return None;
    }
    let significant: Vec<&str> = parts.iter().copied().filter(|part| !goes_along(part)).collect();
    let significant = if significant.is_empty() { parts } else { significant };
    let [command] = significant.as_slice() else { return None };
    if hides_more(command) {
        return None;
    }
    let written = collapsed(&without_harmless_redirects(command));
    let words: Vec<&str> = written.split(' ').collect();
    let (program, rest) = words.split_first()?;
    if !program.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) || program.starts_with('.') {
        return None;
    }
    if INTERPRETERS.contains(program) {
        let script = rest.first().filter(|word| !word.starts_with('-'))?;
        return Some(format!("Bash({program} {script}:*)"));
    }
    let depth = if NESTED.contains(program) { NESTED_DEPTH - 1 } else { 1 };
    let subcommands: Vec<&str> = rest.iter().take(depth).take_while(|word| subcommand(word)).copied().collect();
    Some(match (subcommands.is_empty(), rest.is_empty()) {
        (_, true) => format!("Bash({program})"),
        (true, false) => format!("Bash({program}:*)"),
        (false, false) => format!("Bash({program} {}:*)", subcommands.join(" ")),
    })
}

pub fn goes_along(part: &str) -> bool {
    let words: Vec<&str> = part.split_whitespace().collect();
    match words.as_slice() {
        ["cd", path] => inside(path),
        _ => crate::command_policy::only_reads(part),
    }
}

fn inside(path: &str) -> bool {
    !path.starts_with(['/', '~', '-', '$']) && !path.split('/').any(|segment| segment == "..")
}

fn subcommand(word: &str) -> bool {
    !word.starts_with('-') && word.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
}
