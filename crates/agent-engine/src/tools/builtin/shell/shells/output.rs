pub(super) const KEPT: usize = 1 << 20;

pub(super) fn capped(kept: &mut Vec<u8>) -> usize {
    if kept.len() <= 2 * KEPT {
        return 0;
    }
    let drop = kept.len() - KEPT;
    kept.drain(..drop);
    drop
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Unread {
    pub(super) text: String,
    pub(super) dropped: Option<String>,
}

impl Unread {
    pub(super) fn of(kept: &[u8], dropped: usize, read: usize) -> Self {
        let lost = dropped.saturating_sub(read);
        let from = read.saturating_sub(dropped).min(kept.len());
        let shown = &kept[from..];
        let dropped = (lost > 0).then(|| {
            format!("[earlier output dropped: showing the last {} of {} bytes]", shown.len(), lost + shown.len())
        });
        Self { text: String::from_utf8_lossy(shown).into_owned(), dropped }
    }

    pub(super) fn whole(self) -> String {
        let text = self.text.trim_end();
        match self.dropped {
            Some(dropped) => format!("{dropped}\n{text}"),
            None => text.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unread_output_keeps_only_its_newest_part() {
        let mut kept = vec![b'a'; 2 * KEPT];
        assert_eq!(capped(&mut kept), 0, "at the limit nothing goes");
        kept.extend_from_slice(b"newest");
        assert_eq!(capped(&mut kept), KEPT + 6);
        assert_eq!(kept.len(), KEPT);
        assert!(kept.ends_with(b"newest"));
    }

    #[test]
    fn what_was_dropped_before_it_was_read_is_counted_and_what_was_read_is_not_shown_twice() {
        let all = Unread::of(b"abcdef", 0, 0);
        assert_eq!(all, Unread { text: "abcdef".into(), dropped: None });
        assert_eq!(Unread::of(b"abcdef", 0, 4).text, "ef", "only what is new");
        assert_eq!(Unread::of(b"cdef", 2, 4), Unread { text: "ef".into(), dropped: None }, "read before it went");
        let lost = Unread::of(b"cdef", 10, 4);
        assert_eq!(lost.dropped.as_deref(), Some("[earlier output dropped: showing the last 4 of 10 bytes]"));
        assert_eq!(lost.whole(), "[earlier output dropped: showing the last 4 of 10 bytes]\ncdef");
        assert_eq!(Unread::of(b"cdef\n\n", 0, 0).whole(), "cdef");
    }
}
