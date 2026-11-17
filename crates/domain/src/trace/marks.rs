use std::fmt;

use serde::{Serialize, Serializer};

const REFUSED: &str = "refused: ";
const EDITED: &str = "edited ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    Repeated,
    Refused(String),
    Edited(String),
}

impl Mark {
    pub fn of(flag: &str) -> Option<Self> {
        if flag == "repeated" {
            return Some(Self::Repeated);
        }
        if let Some(reason) = flag.strip_prefix(REFUSED) {
            return Some(Self::Refused(reason.to_owned()));
        }
        flag.strip_prefix(EDITED).map(|path| Self::Edited(path.to_owned()))
    }

    pub fn title(&self) -> Title {
        match self {
            Self::Repeated => Title::Repeated,
            Self::Refused(_) => Title::Refused,
            Self::Edited(_) => Title::Edited,
        }
    }
}

impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repeated => f.write_str("repeated"),
            Self::Refused(reason) => write!(f, "{REFUSED}{reason}"),
            Self::Edited(path) => write!(f, "{EDITED}{path}"),
        }
    }
}

impl Serialize for Mark {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Title {
    Asked,
    Partial,
    Diagnosed,
    Mode,
    Moved,
    Verified,
    Reviewed,
    Notified,
    Notice,
    Repeated,
    Refused,
    Edited,
}

impl Title {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asked => "asked",
            Self::Partial => "partial",
            Self::Diagnosed => "diagnosed",
            Self::Mode => "mode",
            Self::Moved => "moved",
            Self::Verified => "verified",
            Self::Reviewed => "reviewed",
            Self::Notified => "notified",
            Self::Notice => "notice",
            Self::Repeated => "repeated",
            Self::Refused => "refused",
            Self::Edited => "edited",
        }
    }
}

impl fmt::Display for Title {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{Mark, Title};

    #[test]
    fn a_mark_is_written_as_the_flag_the_page_reads_and_read_back_from_it() {
        let marks = [Mark::Repeated, Mark::Refused("outside the project".into()), Mark::Edited("src/a.rs".into())];
        let flags: Vec<String> = marks.iter().map(ToString::to_string).collect();
        assert_eq!(flags, ["repeated", "refused: outside the project", "edited src/a.rs"]);
        for (mark, flag) in marks.iter().zip(&flags) {
            assert_eq!(Mark::of(flag).as_ref(), Some(mark));
            assert_eq!(serde_json::to_value(mark).expect("written"), serde_json::json!(flag));
        }
        assert_eq!(Mark::of("something else"), None);
        assert_eq!(Mark::Refused("x".into()).title(), Title::Refused);
        assert_eq!(serde_json::to_value(Title::Verified).expect("written"), serde_json::json!("verified"));
    }
}
