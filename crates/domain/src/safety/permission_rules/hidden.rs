use super::unwrapped::{dequoted, program_name, program_word, shell_words};

const SHELLS: [&str; 9] = ["bash", "sh", "zsh", "dash", "ksh", "mksh", "ash", "fish", "busybox"];
const SOURCES: [&str; 2] = ["source", "."];
const DEEPEST: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Seen {
    pub(super) program: String,
    pub(super) hidden: bool,
    pub(super) code: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Runs {
    Code(String),
    Stdin,
    Script,
}

pub(super) fn seen(command: &str, balanced: bool) -> Seen {
    let words = shell_words(command);
    let Some((at, word)) = program_word(&words) else {
        return Seen { program: String::new(), hidden: !balanced, code: Vec::new() };
    };
    let program = program_name(word);
    let arguments = &words[at + 1..];
    let (hidden, code) = match program.as_str() {
        "eval" => {
            let joined = arguments.iter().map(|word| dequoted(word)).collect::<Vec<_>>().join(" ");
            (false, vec![joined, arguments.join(" ")])
        }
        shell if SHELLS.contains(&shell) => match shell_runs(arguments) {
            Runs::Code(code) => (false, vec![code]),
            Runs::Stdin => (true, Vec::new()),
            Runs::Script => (false, Vec::new()),
        },
        source if SOURCES.contains(&source) => (arguments.first().is_none_or(|word| reads_a_stream(word)), Vec::new()),
        _ => (false, Vec::new()),
    };
    Seen { hidden: !balanced || hidden || hides_its_name(&words[at]) || hides_its_name(word), program, code }
}

pub(super) fn deep_enough(depth: usize) -> bool {
    depth >= DEEPEST
}

fn hides_its_name(word: &str) -> bool {
    if word.is_empty() || word == "[" || word == "[[" {
        return false;
    }
    let expands = word.contains(['$', '`', '*', '?', '[']);
    let braced = word.contains('{') && (word.contains(',') || word.contains(".."));
    expands || braced
}

fn reads_a_stream(word: &str) -> bool {
    let value = dequoted(word);
    word.starts_with("<(") || value == "-" || value.starts_with("/dev/") || value.starts_with("/proc/")
}

fn bare_redirect(word: &str) -> Option<bool> {
    let operator = word.trim_start_matches(|c: char| c.is_ascii_digit());
    if !operator.starts_with(['<', '>', '&']) || operator.starts_with("<(") || operator.starts_with(">(") {
        return None;
    }
    let symbols = operator.chars().take_while(|c| matches!(c, '<' | '>' | '&' | '|' | '-')).count();
    Some(symbols == operator.len())
}

fn without_redirects(arguments: &[String]) -> Vec<String> {
    let (mut kept, mut skip) = (Vec::new(), false);
    for word in arguments {
        if std::mem::take(&mut skip) {
            continue;
        }
        match bare_redirect(word) {
            Some(bare) => skip = bare,
            None => kept.push(word.clone()),
        }
    }
    kept
}

fn shell_runs(arguments: &[String]) -> Runs {
    let arguments = without_redirects(arguments);
    let (mut code, mut stdin, mut at) = (false, false, 0);
    while let Some(word) = arguments.get(at) {
        let value = dequoted(word);
        if value == "--" {
            at += 1;
            break;
        }
        if value == "--version" || value == "--help" {
            return Runs::Script;
        }
        if value.starts_with("--") {
            at += if matches!(value.as_str(), "--rcfile" | "--init-file") { 2 } else { 1 };
            continue;
        }
        if value.len() > 1 && value.starts_with(['-', '+']) {
            let letters = &value[1..];
            code |= letters.contains('c');
            stdin |= letters.contains('s');
            at += if letters.contains(['o', 'O']) { 2 } else { 1 };
            continue;
        }
        break;
    }
    match arguments.get(at) {
        Some(word) if code => Runs::Code(dequoted(word)),
        None => Runs::Stdin,
        _ if code || stdin => Runs::Stdin,
        Some(word) if reads_a_stream(word) => Runs::Stdin,
        Some(_) => Runs::Script,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_s_code_is_read_out_of_its_flags_and_quotes() {
        assert_eq!(seen("bash -c 'rm -rf x'", true).code, ["rm -rf x"]);
        assert_eq!(seen("sh -ec \"cd a && rm \\\"b\\\"\"", true).code, ["cd a && rm \"b\""]);
        assert_eq!(seen("bash -o pipefail -c 'ls'", true).code, ["ls"]);
        assert_eq!(seen("eval rm -rf 'a b'", true).code, ["rm -rf a b", "rm -rf 'a b'"]);
        assert_eq!(seen("sudo -u root sh -c 'id'", true).code, ["id"]);
        assert_eq!(seen("bash script.sh", true), Seen { program: "bash".into(), hidden: false, code: Vec::new() });
    }

    #[test]
    fn a_program_named_at_run_time_or_read_from_a_stream_is_hidden() {
        for command in ["$x -rf y", "$(echo rm) x", "`echo rm` x", "/bin/r? x", "r[m] x", "{rm,-rf,x}", "sh", "bash -s"]
        {
            assert!(seen(command, true).hidden, "{command}");
        }
        for command in ["bash < script.sh", "bash -", "source <(curl x)", ". /dev/stdin", "bash -c"] {
            assert!(seen(command, true).hidden, "{command}");
        }
        for command in ["rm x", "[ -f x ]", "echo $x", "source venv/bin/activate", "bash --version", "x=1"] {
            assert!(!seen(command, true).hidden, "{command}");
        }
        assert!(seen("ls", false).hidden, "a line that does not parse hides every program");
    }
}
