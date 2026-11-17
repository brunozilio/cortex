const KEYWORDS: [&str; 7] = ["then", "do", "else", "elif", "if", "while", "until"];

struct Wrapper {
    name: &'static str,
    valued: &'static [&'static str],
    positionals: usize,
    code: &'static [&'static str],
    rest_is_code: bool,
}

const fn wrapper(name: &'static str, valued: &'static [&'static str]) -> Wrapper {
    Wrapper { name, valued, positionals: 0, code: &[], rest_is_code: false }
}

const fn after(name: &'static str, valued: &'static [&'static str], positionals: usize) -> Wrapper {
    Wrapper { name, valued, positionals, code: &[], rest_is_code: false }
}

const SWITCHING_USER: &[&str] =
    &["-s", "--shell", "-g", "--group", "-G", "--supp-group", "-w", "--whitelist-environment", "-u", "--user"];
const SWITCHING_USER_RUNS: &[&str] = &["-c", "--command", "--session-command"];

static MORE_WRAPPERS: [Wrapper; 15] = [
    wrapper("setsid", &[]),
    after("chroot", &["--userspec", "--groups"], 1),
    Wrapper {
        name: "flock",
        valued: &["-w", "-E", "--timeout", "--wait", "--conflict-exit-code"],
        positionals: 1,
        code: &["-c", "--command"],
        rest_is_code: false,
    },
    Wrapper {
        name: "watch",
        valued: &["-n", "--interval", "-q", "--equexit"],
        positionals: 0,
        code: &[],
        rest_is_code: true,
    },
    Wrapper {
        name: "script",
        valued: &["-F", "-t", "-T", "-I", "-O", "-B", "-E", "-m", "--log-timing", "--log-in", "--log-out", "--log-io"],
        positionals: 1,
        code: &["-c", "--command"],
        rest_is_code: false,
    },
    wrapper("caffeinate", &["-t", "-w"]),
    wrapper("xcrun", &["--sdk", "-sdk", "--toolchain", "-toolchain"]),
    after("taskset", &[], 1),
    Wrapper { name: "su", valued: SWITCHING_USER, positionals: 1, code: SWITCHING_USER_RUNS, rest_is_code: false },
    Wrapper { name: "runuser", valued: SWITCHING_USER, positionals: 1, code: SWITCHING_USER_RUNS, rest_is_code: false },
    Wrapper { name: "sg", valued: &[], positionals: 1, code: &["-c"], rest_is_code: true },
    wrapper("pkexec", &["--user"]),
    wrapper("unbuffer", &[]),
    wrapper("arch", &["-arch"]),
    wrapper("strace", &["-o", "-e", "-p", "-s", "-u", "-E", "-a", "-b", "-I", "-O", "-P", "-S", "-X"]),
];

static WRAPPERS: [Wrapper; 15] = [
    wrapper("exec", &["-a"]),
    wrapper(
        "xargs",
        &[
            "-I",
            "-n",
            "-P",
            "-d",
            "-E",
            "-L",
            "-s",
            "-a",
            "--max-args",
            "--max-procs",
            "--delimiter",
            "--eof",
            "--max-lines",
            "--max-chars",
            "--arg-file",
        ],
    ),
    wrapper("busybox", &[]),
    wrapper(
        "sudo",
        &[
            "-u",
            "-g",
            "-h",
            "-p",
            "-C",
            "-D",
            "-r",
            "-t",
            "-T",
            "-U",
            "-R",
            "--user",
            "--group",
            "--host",
            "--prompt",
            "--chdir",
            "--role",
            "--type",
            "--other-user",
            "--close-from",
            "--chroot",
        ],
    ),
    wrapper("doas", &["-u", "-C"]),
    Wrapper {
        name: "env",
        valued: &["-u", "-C", "-P", "--unset", "--chdir"],
        positionals: 0,
        code: &["-S", "--split-string"],
        rest_is_code: false,
    },
    wrapper("nice", &["-n", "--adjustment"]),
    after("timeout", &["-s", "-k", "--signal", "--kill-after"], 1),
    wrapper("stdbuf", &["-i", "-o", "-e", "--input", "--output", "--error"]),
    wrapper("ionice", &["-c", "-n", "--class", "--classdata"]),
    after("chrt", &[], 1),
    wrapper("time", &["-f", "-o", "--format", "--output"]),
    wrapper("command", &[]),
    wrapper("builtin", &[]),
    wrapper("nohup", &[]),
];

fn followed_by_a_value(listed: &[&str], flag: &str) -> bool {
    if listed.contains(&flag) {
        return true;
    }
    if flag.starts_with("--") {
        return false;
    }
    let letters: Vec<char> = flag.chars().skip(1).collect();
    let valued = letters.iter().position(|letter| listed.contains(&format!("-{letter}").as_str()));
    valued.is_some_and(|at| at + 1 == letters.len())
}

impl Wrapper {
    fn named(word: &str) -> Option<&'static Wrapper> {
        WRAPPERS.iter().chain(&MORE_WRAPPERS).find(|wrapper| wrapper.name == word)
    }

    fn scan(&self, words: &[&str], mut at: usize) -> (usize, Vec<String>) {
        let mut positionals = self.positionals;
        let mut code = Vec::new();
        while let Some(word) = words.get(at) {
            let flag = unquoted(word);
            if flag == "--" {
                at += 1;
                break;
            }
            if flag == "-" {
                at += 1;
            } else if flag.starts_with('-') {
                let (name, attached) =
                    flag.split_once('=').map_or((flag.as_str(), None), |(name, value)| (name, Some(value)));
                let glued = (!name.starts_with("--") && name.len() > 2)
                    .then(|| name.get(..2).filter(|short| self.code.contains(short)))
                    .flatten();
                if glued.is_some() {
                    code.push(dequoted(word).get(2..).unwrap_or_default().to_owned());
                    at += 1;
                } else if followed_by_a_value(self.code, name) {
                    code.extend(attached.map(str::to_owned).or_else(|| words.get(at + 1).map(|value| dequoted(value))));
                    at += 1 + usize::from(attached.is_none());
                } else {
                    at += 1 + usize::from(attached.is_none() && followed_by_a_value(self.valued, name));
                }
            } else if positionals > 0 && !assigns(word) {
                positionals -= 1;
                at += 1;
            } else {
                break;
            }
        }
        if self.rest_is_code && at < words.len() {
            code.push(words[at..].iter().map(|word| dequoted(word)).collect::<Vec<_>>().join(" "));
        }
        (at, code)
    }
}

pub fn wrapped_code(words: &[&str]) -> Vec<String> {
    let mut code = Vec::new();
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let word = unquoted(opened(word));
        if word.is_empty() || assigns(&word) || KEYWORDS.contains(&word.as_str()) {
            at += 1;
        } else if let Some(wrapper) = Wrapper::named(&word) {
            let (end, found) = wrapper.scan(words, at + 1);
            code.extend(found);
            at = end;
        } else {
            break;
        }
    }
    code
}

pub fn unquoted(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '"' | '\'' | '\\')).collect()
}

fn opened(word: &str) -> &str {
    word.trim_start_matches(['(', '{', '!'])
}

fn assigns(word: &str) -> bool {
    word.split_once('=')
        .is_some_and(|(name, _)| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

pub fn program_at(words: &[&str]) -> usize {
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let word = unquoted(opened(word));
        if word.is_empty() || assigns(&word) || KEYWORDS.contains(&word.as_str()) {
            at += 1;
        } else if let Some(wrapper) = Wrapper::named(&word) {
            at = wrapper.scan(words, at + 1).0;
        } else {
            break;
        }
    }
    at
}

pub fn shell_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in text.chars() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }
        match (quote, c) {
            (None | Some('"'), '\\') => escaped = true,
            (None, '\'' | '"') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            (None, _) if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
                continue;
            }
            _ => {}
        }
        current.push(c);
        started = true;
    }
    if started {
        words.push(current);
    }
    words
}

pub fn dequoted(word: &str) -> String {
    let mut value = String::new();
    let mut quote: Option<char> = None;
    let mut chars = word.chars().peekable();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (None, '\\') => value.extend(chars.next()),
            (Some('"'), '\\') => match chars.peek() {
                Some('"' | '\\' | '$' | '`') => value.extend(chars.next()),
                _ => value.push('\\'),
            },
            (None, '\'' | '"') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            _ => value.push(c),
        }
    }
    value
}

pub fn program_word(words: &[String]) -> Option<(usize, &str)> {
    let borrowed: Vec<&str> = words.iter().map(String::as_str).collect();
    let at = program_at(&borrowed);
    words.get(at).map(|word| (at, opened(word)))
}

pub fn program_name(word: &str) -> String {
    let program = unquoted(opened(word));
    program.rsplit('/').next().unwrap_or_default().to_lowercase()
}

pub fn run(words: &[&str]) -> String {
    let Some((program, arguments)) = words.get(program_at(words)..).and_then(<[&str]>::split_first) else {
        return String::new();
    };
    let program = unquoted(opened(program));
    let program = program.rsplit('/').next().unwrap_or_default().to_owned();
    std::iter::once(program).chain(arguments.iter().map(|word| (*word).to_owned())).collect::<Vec<_>>().join(" ")
}
