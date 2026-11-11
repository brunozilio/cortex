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

pub fn hides_more(part: &str) -> bool {
    const MARKS: [&str; 7] = ["$(", "`", "<(", ">(", ">", "$'", "$\""];
    const RUNNERS: [&str; 11] = ["bash", "sh", "zsh", "dash", "ksh", "fish", "eval", "xargs", "source", ".", "exec"];
    MARKS.iter().any(|mark| part.contains(mark))
        || normalized(part).split(' ').next().is_some_and(|first| RUNNERS.contains(&first))
}

pub fn collapsed(part: &str) -> String {
    part.split_whitespace().collect::<Vec<_>>().join(" ")
}

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

pub fn command_matches(specifier: &str, command: &str) -> bool {
    if let Some(prefix) = specifier.strip_suffix(":*") {
        let prefix = prefix.trim_end();
        return command == prefix || command.strip_prefix(prefix).is_some_and(|rest| rest.starts_with(' '));
    }
    if specifier.contains('*') { wildcard(specifier, command, false) } else { command == specifier }
}

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

pub fn path_matches(specifier: &str, path: &str, cwd: &str, home: Option<&str>, fold_case: bool) -> bool {
    let Some((pattern, _)) = anchored(specifier, cwd, home) else { return false };
    let (pattern, path) = (fold(pattern, fold_case), split(path, fold_case));
    let pattern: Vec<&str> = pattern.iter().map(String::as_str).collect();
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    (1..=path.len()).any(|end| segments(&pattern, &path[..end]))
}

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
