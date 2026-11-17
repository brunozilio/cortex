const KEYWORDS: [&str; 7] = ["then", "do", "else", "elif", "if", "while", "until"];

struct Wrapper {
    name: &'static str,
    valued: &'static [&'static str],
    positionals: usize,
}

const fn wrapper(name: &'static str, valued: &'static [&'static str]) -> Wrapper {
    Wrapper { name, valued, positionals: 0 }
}

const WRAPPERS: [Wrapper; 12] = [
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
    wrapper("env", &["-u", "-C", "-S", "-P", "--unset", "--chdir", "--split-string"]),
    wrapper("nice", &["-n", "--adjustment"]),
    Wrapper { name: "timeout", valued: &["-s", "-k", "--signal", "--kill-after"], positionals: 1 },
    wrapper("stdbuf", &["-i", "-o", "-e", "--input", "--output", "--error"]),
    wrapper("ionice", &["-c", "-n", "--class", "--classdata"]),
    Wrapper { name: "chrt", valued: &[], positionals: 1 },
    wrapper("time", &["-f", "-o", "--format", "--output"]),
    wrapper("command", &[]),
    wrapper("builtin", &[]),
    wrapper("nohup", &[]),
];

impl Wrapper {
    fn value_follows(&self, flag: &str) -> bool {
        if flag.starts_with("--") {
            return self.valued.contains(&flag);
        }
        let letters: Vec<char> = flag.chars().skip(1).collect();
        let valued = letters.iter().position(|letter| self.valued.contains(&format!("-{letter}").as_str()));
        valued.is_some_and(|at| at + 1 == letters.len())
    }

    fn end(&self, words: &[&str], mut at: usize) -> usize {
        let mut positionals = self.positionals;
        while let Some(word) = words.get(at) {
            if word.starts_with('-') && word.len() > 1 {
                at += 1 + usize::from(self.value_follows(word));
            } else if positionals > 0 && !assigns(word) {
                positionals -= 1;
                at += 1;
            } else {
                break;
            }
        }
        at
    }
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
        } else if let Some(wrapper) = WRAPPERS.iter().find(|wrapper| wrapper.name == word) {
            at = wrapper.end(words, at + 1);
        } else {
            break;
        }
    }
    at
}

pub fn run(words: &[&str]) -> String {
    let Some((program, arguments)) = words.get(program_at(words)..).and_then(<[&str]>::split_first) else {
        return String::new();
    };
    let program = unquoted(opened(program));
    let program = program.rsplit('/').next().unwrap_or_default().to_owned();
    std::iter::once(program).chain(arguments.iter().map(|word| (*word).to_owned())).collect::<Vec<_>>().join(" ")
}
