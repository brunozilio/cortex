use serde::{Deserialize, Serialize};

const RECENT_SECS: u64 = 10 * 60;
const KEPT_SECS: u64 = 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub pid: u32,
    pub session: String,
    pub path: String,
    pub at_secs: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims(pub Vec<Claim>);

impl Claims {
    pub fn claim(&mut self, claim: Claim, alive: &dyn Fn(u32) -> bool) -> Option<Claim> {
        let now = claim.at_secs;
        self.0.retain(|kept| alive(kept.pid) && now.saturating_sub(kept.at_secs) < KEPT_SECS);
        let other = self
            .0
            .iter()
            .filter(|kept| kept.path == claim.path && kept.session != claim.session)
            .filter(|kept| now.saturating_sub(kept.at_secs) < RECENT_SECS)
            .max_by_key(|kept| kept.at_secs)
            .cloned();
        self.0.retain(|kept| !(kept.path == claim.path && kept.session == claim.session));
        self.0.push(claim);
        other
    }
}

pub fn warning(other: &Claim, now_secs: u64) -> String {
    let minutes = now_secs.saturating_sub(other.at_secs) / 60;
    let when = if minutes == 0 { "less than a minute ago".to_owned() } else { format!("{minutes} min ago") };
    format!(
        "\n\n<other-session>Another session (process {}) changed this file {when}. Its work and yours may \
         overwrite each other: re-read the file before editing it again, and tell the user if you both need it.</other-session>",
        other.pid
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(pid: u32, session: &str, path: &str, at_secs: u64) -> Claim {
        Claim { pid, session: session.into(), path: path.into(), at_secs }
    }

    #[test]
    fn another_live_session_s_recent_edit_of_the_same_file_is_reported() {
        let mut claims = Claims::default();
        let alive = |_: u32| true;
        assert_eq!(claims.claim(claim(1, "a", "src/x.rs", 1_000), &alive), None);
        assert_eq!(claims.claim(claim(1, "a", "src/x.rs", 1_100), &alive), None, "your own edits are not a clash");
        assert_eq!(claims.claim(claim(2, "b", "src/y.rs", 1_150), &alive), None);

        let other = claims.claim(claim(2, "b", "src/x.rs", 1_200), &alive);

        assert_eq!(other, Some(claim(1, "a", "src/x.rs", 1_100)));
        assert_eq!(claims.0.len(), 3);
    }

    #[test]
    fn dead_sessions_and_old_edits_are_forgotten() {
        let mut claims = Claims::default();
        claims.claim(claim(1, "a", "src/x.rs", 0), &|_| true);
        assert_eq!(claims.claim(claim(2, "b", "src/x.rs", 700), &|_| true), None, "ten minutes is no longer recent");

        let mut claims = Claims::default();
        claims.claim(claim(1, "a", "src/x.rs", 0), &|_| true);
        assert_eq!(claims.claim(claim(2, "b", "src/x.rs", 10), &|pid| pid != 1), None, "a session that ended");
        assert_eq!(claims.0.len(), 1);
    }

    #[test]
    fn the_warning_says_who_and_when() {
        let other = claim(42, "a", "x", 1_000);
        assert!(warning(&other, 1_030).contains("process 42") && warning(&other, 1_030).contains("less than a minute"));
        assert!(warning(&other, 1_300).contains("5 min ago"));
    }
}
