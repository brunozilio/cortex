//! How a rule's specifier matches: the parts of a compound command and the
//! spellings that hide what they run, command prefixes and patterns, and
//! gitignore-like path globs.

/// The commands a shell line runs, split at `&&`, `||`, `;`, `|`, `&` and
/// new lines outside quotes, and whether its quotes all close (when they do
/// not, the split cannot be trusted).
pub fn split_command(line: &str) -> (Vec<&str>, bool) {
    let bytes = line.as_bytes();
    let (mut parts, mut start, mut i) = (Vec::new(), 0, 0);
    let (mut single, mut double) = (false, false);
    while i < bytes.len() {
        let byte = bytes[i];
        match byte {
            b'\\' if !single => i += 1,
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            b'&' | b'|' | b';' | b'\n' if !single && !double => {
                parts.push(&line[start..i]);
                let doubled = matches!(byte, b'&' | b'|') && bytes.get(i + 1) == Some(&byte);
                i += usize::from(doubled);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&line[start.min(line.len())..]);
    let parts = parts.into_iter().map(str::trim).filter(|part| !part.is_empty()).collect();
    (parts, !single && !double)
}

/// Whether a command runs another one inside it, redirects into a file or
/// quotes in a way the splitter does not follow (to err on asking).
pub fn hides_more(part: &str) -> bool {
    const MARKS: [&str; 7] = ["$(", "`", "<(", ">(", ">", "$'", "$\""];
    const RUNNERS: [&str; 11] = ["bash", "sh", "zsh", "dash", "ksh", "fish", "eval", "xargs", "source", ".", "exec"];
    MARKS.iter().any(|mark| part.contains(mark))
        || normalized(part).split(' ').next().is_some_and(|first| RUNNERS.contains(&first))
}

/// The command with each run of whitespace made one space.
pub fn collapsed(part: &str) -> String {
    part.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The command as the shell runs it, for deny and ask rules: without
/// leading `(`, `{` or `!`, variable assignments and wrappers (`env`,
/// `sudo`, `nohup`…), the program unquoted and by its name, not its path.
pub fn normalized(part: &str) -> String {
    const WRAPPERS: [&str; 8] = ["env", "command", "builtin", "nohup", "time", "sudo", "nice", "doas"];
    let collapsed = collapsed(part);
    let mut words = collapsed.trim_start_matches(['(', '{', '!', ' ']).split(' ').peekable();
    let mut wrapped = false;
    while let Some(word) = words.peek() {
        let assignment = word
            .split_once('=')
            .is_some_and(|(name, _)| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        if assignment || WRAPPERS.contains(word) || (wrapped && word.starts_with('-')) {
            wrapped |= WRAPPERS.contains(word);
            words.next();
        } else {
            break;
        }
    }
    let Some(program) = words.next() else { return String::new() };
    let program: String = program.chars().filter(|c| !matches!(c, '"' | '\'' | '\\')).collect();
    let program = program.rsplit('/').next().unwrap_or_default().to_owned();
    std::iter::once(program).chain(words.map(str::to_owned)).collect::<Vec<_>>().join(" ")
}

/// `prefix:*` matches the prefix as whole words, a `*` anything, and
/// anything else the exact command.
pub fn command_matches(specifier: &str, command: &str) -> bool {
    if let Some(prefix) = specifier.strip_suffix(":*") {
        let prefix = prefix.trim_end();
        return command == prefix || command.strip_prefix(prefix).is_some_and(|rest| rest.starts_with(' '));
    }
    if specifier.contains('*') { wildcard(specifier, command, false) } else { command == specifier }
}

/// Where a path specifier starts: `//` at the root, `~/` at home, `./` or a
/// slash inside at the project, and a bare name at any depth below it
/// (`true`).
fn anchored(specifier: &str, cwd: &str, home: Option<&str>) -> Option<(Vec<String>, bool)> {
    let cwd = cwd.trim_end_matches('/');
    let (text, anywhere) = if let Some(rest) = specifier.strip_prefix("//") {
        (format!("/{rest}"), false)
    } else if let Some(rest) = specifier.strip_prefix("~/") {
        (format!("{}/{rest}", home?.trim_end_matches('/')), false)
    } else if let Some(rest) = specifier.strip_prefix("./") {
        (format!("{cwd}/{rest}"), false)
    } else if specifier.trim_end_matches('/').contains('/') {
        (format!("{cwd}/{}", specifier.trim_start_matches('/')), false)
    } else {
        (format!("{cwd}/**/{specifier}"), true)
    };
    Some((text.split('/').filter(|s| !s.is_empty()).map(str::to_owned).collect(), anywhere))
}

fn split(path: &str, fold_case: bool) -> Vec<String> {
    let path = if fold_case { path.to_lowercase() } else { path.to_owned() };
    path.split('/').filter(|s| !s.is_empty()).map(str::to_owned).collect()
}

fn fold(pattern: Vec<String>, fold_case: bool) -> Vec<String> {
    if fold_case { pattern.into_iter().map(|s| s.to_lowercase()).collect() } else { pattern }
}

/// Whether absolute `path` is covered by `specifier` (see [`anchored`]); a
/// folder covers what is inside it. `fold_case` ignores case, as deny and
/// ask rules do (macOS folders mostly do).
pub fn path_matches(specifier: &str, path: &str, cwd: &str, home: Option<&str>, fold_case: bool) -> bool {
    let Some((pattern, _)) = anchored(specifier, cwd, home) else { return false };
    let (pattern, path) = (fold(pattern, fold_case), split(path, fold_case));
    let pattern: Vec<&str> = pattern.iter().map(String::as_str).collect();
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    (1..=path.len()).any(|end| segments(&pattern, &path[..end]))
}

/// Whether a search of folder `tree` meets what `specifier` covers: the
/// folder is covered, or the specifier's fixed folder lies inside it. A
/// bare name, which could be anywhere, only counts when the folder is
/// covered (best effort).
pub fn tree_meets(specifier: &str, tree: &str, cwd: &str, home: Option<&str>) -> bool {
    if path_matches(specifier, tree, cwd, home, true) {
        return true;
    }
    let Some((pattern, anywhere)) = anchored(specifier, cwd, home) else { return false };
    let fixed: Vec<String> =
        fold(pattern, true).into_iter().take_while(|segment| !segment.contains(['*', '?'])).collect();
    let tree = split(tree, true);
    !anywhere && fixed.len() > tree.len() && fixed.starts_with(&tree)
}

fn segments(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => segments(rest, path) || (!path.is_empty() && segments(pattern, &path[1..])),
        Some((first, rest)) => {
            path.first().is_some_and(|name| wildcard(first, name, true)) && segments(rest, &path[1..])
        }
    }
}

/// `*` matches any run of characters (and `?` one, when `question`).
fn wildcard(pattern: &str, text: &str, question: bool) -> bool {
    let (pattern, text): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut p, mut t) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                backtrack = Some((p, t));
                p += 1;
            }
            Some(&c) if c == text[t] || (question && c == '?') => {
                p += 1;
                t += 1;
            }
            _ => match backtrack {
                Some((star, at)) => {
                    p = star + 1;
                    t = at + 1;
                    backtrack = Some((star, at + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}
