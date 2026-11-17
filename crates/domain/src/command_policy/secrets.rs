use super::wildcards;
use crate::permission_rules::normalized;

const CONTENT_READERS: [&str; 32] = [
    "cat", "head", "tail", "less", "more", "bat", "grep", "egrep", "fgrep", "rg", "base64", "xxd", "od", "hexdump",
    "strings", "sed", "awk", "cut", "sort", "nl", "tac", "cp", "scp", "curl", "wget", "nc", "jq", "diff", "uniq",
    "openssl", "source", ".",
];

const KEY_FILES: [&str; 10] = [
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    "id_dsa",
    ".npmrc",
    ".netrc",
    ".pypirc",
    ".git-credentials",
    "credentials",
    "secrets.json",
];

const KEY_ENDINGS: [&str; 4] = [".pem", ".key", ".p12", ".pfx"];

const ENV_EXAMPLES: [&str; 4] = ["example", "sample", "template", "dist"];

pub fn exposes(part: &str) -> Option<String> {
    let bare: Vec<&str> = part.split_whitespace().collect();
    if matches!(bare.as_slice(), ["env"] | ["set"] | ["export", "-p"] | ["export"]) || first(part) == "printenv" {
        return Some("it prints the environment, which holds keys and tokens".into());
    }
    let words = crate::destructive::words(&normalized(part));
    let (program, rest) = words.split_first()?;
    if !CONTENT_READERS.contains(&program.as_str()) {
        return None;
    }
    let file = rest.iter().map(|word| word.trim_start_matches('@')).find(|word| names_a_secret(word))?;
    Some(format!("it reads {file}, which holds secrets or could match a file that does"))
}

fn names_a_secret(word: &str) -> bool {
    let unescaped: String = word.chars().filter(|c| *c != '\\').collect();
    wildcards::alternatives(&unescaped).iter().any(|word| {
        let name = word.rsplit('/').next().unwrap_or(word);
        if wildcards::has_wildcards(name) { could_be_secret(name) } else { secret_file(name) }
    })
}

fn could_be_secret(name: &str) -> bool {
    let fixed = KEY_FILES.iter().chain([".env", ".env.*"].iter()).any(|secret| wildcards::could_match(name, secret));
    fixed || KEY_ENDINGS.iter().any(|ending| wildcards::could_match(name, &format!("*{ending}")))
}

fn first(part: &str) -> String {
    normalized(part).split(' ').next().unwrap_or_default().to_owned()
}

fn secret_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let env = name == ".env" || name.strip_prefix(".env.").is_some_and(|variant| !ENV_EXAMPLES.contains(&variant));
    env || KEY_FILES.contains(&name) || KEY_ENDINGS.iter().any(|ending| name.ends_with(ending))
}
