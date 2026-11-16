use std::path::Path;

use serde::{Deserialize, Serialize};

pub const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;

const RUNNABLE: [&str; 14] =
    ["exe", "msi", "bat", "cmd", "com", "scr", "dll", "so", "dylib", "app", "dmg", "pkg", "deb", "rpm"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Image,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refused {
    TooLarge,
    NotAllowed,
    Empty,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "the file is over the size limit",
            Self::NotAllowed => "this kind of file is not accepted",
            Self::Empty => "the file is empty",
        })
    }
}

pub fn kind(head: &[u8]) -> Kind {
    if crate::image::media_type(head).is_some() { Kind::Image } else { Kind::File }
}

pub fn limit(kind: Kind) -> u64 {
    match kind {
        Kind::Image => crate::MAX_IMAGE_BYTES as u64,
        Kind::File => MAX_FILE_BYTES,
    }
}

pub fn accepted(name: &str, size: u64, kind: Kind) -> Result<(), Refused> {
    let extension = Path::new(name).extension().map(|ext| ext.to_string_lossy().to_ascii_lowercase());
    if extension.is_some_and(|ext| RUNNABLE.contains(&ext.as_str())) {
        return Err(Refused::NotAllowed);
    }
    match size {
        0 => Err(Refused::Empty),
        size if size > limit(kind) => Err(Refused::TooLarge),
        _ => Ok(()),
    }
}

pub fn safe_name(given: &str) -> String {
    let last = given.rsplit(['/', '\\']).next().unwrap_or_default();
    let kept: String = last.chars().filter(|c| !c.is_control()).collect();
    let kept = kept.trim().trim_start_matches('.').trim();
    if kept.is_empty() { "file".to_owned() } else { kept.chars().take(120).collect() }
}

const ATTACHED: &str = "\n\nThe user attached these files; read them as needed:\n";

pub fn told(prompt: &str, files: &[&Path]) -> String {
    if files.is_empty() {
        return prompt.to_owned();
    }
    let listed: Vec<String> = files.iter().map(|path| format!("- {}", path.display())).collect();
    format!("{prompt}{ATTACHED}{}", listed.join("\n"))
}

pub fn written(told: &str) -> (String, Vec<String>) {
    let Some((prompt, listed)) = told.rsplit_once(ATTACHED) else { return (told.to_owned(), Vec::new()) };
    let name = |line: &str| {
        let path = Path::new(line.strip_prefix("- ")?);
        Some(path.file_name()?.to_string_lossy().into_owned())
    };
    match listed.lines().map(name).collect::<Option<Vec<String>>>() {
        Some(names) if !names.is_empty() => (prompt.to_owned(), names),
        _ => (told.to_owned(), Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n....";

    #[test]
    fn a_file_is_an_image_only_when_its_bytes_say_so() {
        assert_eq!(kind(PNG), Kind::Image);
        assert_eq!(kind(b"name,total\n"), Kind::File);
        assert_eq!(kind(b""), Kind::File);
    }

    #[test]
    fn each_kind_has_its_own_size_limit() {
        assert_eq!(accepted("a.png", 20 * 1024 * 1024, Kind::Image), Ok(()));
        assert_eq!(accepted("a.png", 20 * 1024 * 1024 + 1, Kind::Image), Err(Refused::TooLarge));
        assert_eq!(accepted("a.csv", 20 * 1024 * 1024 + 1, Kind::File), Ok(()));
        assert_eq!(accepted("a.csv", MAX_FILE_BYTES + 1, Kind::File), Err(Refused::TooLarge));
        assert_eq!(accepted("a.csv", 0, Kind::File), Err(Refused::Empty));
    }

    #[test]
    fn programs_are_not_accepted_whatever_the_case_of_their_ending() {
        for name in ["setup.exe", "Setup.EXE", "tool.dmg", "lib.so"] {
            assert_eq!(accepted(name, 10, Kind::File), Err(Refused::NotAllowed), "{name}");
        }
        assert_eq!(accepted("notes.so.txt", 10, Kind::File), Ok(()));
        assert_eq!(accepted("Makefile", 10, Kind::File), Ok(()));
    }

    #[test]
    fn a_name_is_kept_as_a_plain_file_name_inside_its_folder() {
        assert_eq!(safe_name("report.pdf"), "report.pdf");
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("C:\\Users\\me\\q1.xlsx"), "q1.xlsx");
        assert_eq!(safe_name(".env"), "env");
        assert_eq!(safe_name("a\u{0}b\n.txt"), "ab.txt");
        assert_eq!(safe_name("   "), "file");
        assert_eq!(safe_name(&"x".repeat(300)).len(), 120);
    }

    #[test]
    fn attached_files_are_named_to_the_agent_after_what_the_user_wrote() {
        assert_eq!(told("sum it", &[]), "sum it");
        assert_eq!(
            told("sum it", &[Path::new("/u/1/sales.csv"), Path::new("/u/2/q1.pdf")]),
            "sum it\n\nThe user attached these files; read them as needed:\n- /u/1/sales.csv\n- /u/2/q1.pdf"
        );
    }

    #[test]
    fn what_was_told_is_read_back_as_what_the_user_wrote_and_the_names_attached() {
        let sent = told("sum it\n\nby month", &[Path::new("/u/1/sales.csv"), Path::new("/u/2/q1.pdf")]);
        assert_eq!(
            written(&sent),
            ("sum it\n\nby month".to_owned(), vec!["sales.csv".to_owned(), "q1.pdf".to_owned()])
        );
        assert_eq!(written("just this\n- not a file"), ("just this\n- not a file".to_owned(), Vec::new()));
        let bare = told("quote it", &[Path::new("/u/1/a.txt")]).replace("- /u/1/a.txt", "");
        assert_eq!(written(&bare), (bare.clone(), Vec::new()), "the sentence alone names no file");
    }

    #[test]
    fn a_prompt_that_quotes_the_sentence_and_a_list_reads_as_files_attached() {
        let quoted = told("as the agent is told", &[Path::new("x")]);
        assert_eq!(written(&quoted), ("as the agent is told".to_owned(), vec!["x".to_owned()]));
    }
}
