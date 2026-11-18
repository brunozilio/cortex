#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub fn of(text: &str) -> Self {
        let crlf = text.matches("\r\n").count();
        let lf = text.matches('\n').count() - crlf;
        if crlf > lf { Self::CrLf } else { Self::Lf }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }

    pub fn apply(self, text: &str) -> String {
        let lf = text.replace("\r\n", "\n");
        match self {
            Self::Lf => lf,
            Self::CrLf => lf.replace('\n', "\r\n"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub old: String,
    pub new: String,
}

pub fn replacement_in(file: &str, old: &str, new: &str) -> Replacement {
    let ending = LineEnding::of(file);
    if ending == LineEnding::CrLf {
        let shaped = Replacement { old: ending.apply(old), new: ending.apply(new) };
        if file.contains(&shaped.old) {
            return shaped;
        }
    }
    Replacement { old: old.to_owned(), new: new.to_owned() }
}

pub fn shaped_like(original: &str, text: &str) -> String {
    let ending = LineEnding::of(original);
    let body = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text);
    let mut shaped = ending.apply(body);
    if original.ends_with('\n') {
        shaped.push_str(ending.as_str());
    }
    shaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ending_most_lines_use_is_the_file_s() {
        assert_eq!(LineEnding::of("a\r\nb\r\nc\n"), LineEnding::CrLf);
        assert_eq!(LineEnding::of("a\nb\nc\r\n"), LineEnding::Lf);
        assert_eq!(LineEnding::of("one line"), LineEnding::Lf);
        assert_eq!(LineEnding::of(""), LineEnding::Lf);
    }

    #[test]
    fn a_replacement_written_with_lf_matches_a_crlf_file_and_keeps_its_endings() {
        let file = "fn a() {\r\n    1\r\n}\r\n";
        let shaped = replacement_in(file, "{\n    1\n}", "{\n    2\n    3\n}");
        assert_eq!(shaped, Replacement { old: "{\r\n    1\r\n}".into(), new: "{\r\n    2\r\n    3\r\n}".into() });
        let single = replacement_in(file, "1", "x\ny");
        assert_eq!(single.new, "x\r\ny", "new lines in a one-line match still take the file's ending");
    }

    #[test]
    fn an_lf_file_and_a_crlf_text_that_matches_only_as_written_are_left_as_given() {
        assert_eq!(
            replacement_in("a\nb\n", "a\nb", "c\r\nd"),
            Replacement { old: "a\nb".into(), new: "c\r\nd".into() }
        );
        let mixed = "a\r\nb\r\nc\nd\n\r\n";
        assert_eq!(replacement_in(mixed, "c\nd", "e").old, "c\nd", "a lone LF in a CRLF file still matches as written");
    }

    #[test]
    fn a_rewritten_file_keeps_the_original_s_ending_and_its_trailing_newline_or_lack_of_one() {
        assert_eq!(shaped_like("{\r\n}\r\n", "{\n  \"a\": 1\n}\n"), "{\r\n  \"a\": 1\r\n}\r\n");
        assert_eq!(shaped_like("{\r\n}", "{\n  \"a\": 1\n}\n"), "{\r\n  \"a\": 1\r\n}");
        assert_eq!(shaped_like("{\n}", "{\n}\n"), "{\n}");
        assert_eq!(shaped_like("{\n}\n", "{\n}"), "{\n}\n");
    }
}
