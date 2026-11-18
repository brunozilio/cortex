use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use domain::team::LEAD;
use platform::sync::Locked;

use super::Team;

const POLL: Duration = Duration::from_millis(500);

pub fn deliver(team: &Team, agent: &str, client: u64) -> usize {
    let mails = team.unread(agent);
    let count = mails.len();
    for mail in &mails {
        crate::tools::inbox::push(client, domain::inbox::teammate_notice(mail));
    }
    count
}

pub fn watch(team: &Team, client: u64) {
    static WATCHED: OnceLock<Mutex<HashSet<(String, u64)>>> = OnceLock::new();
    let first = WATCHED.get_or_init(Mutex::default).locked().insert((team.name.clone(), client));
    if !first {
        return;
    }
    let team = team.clone();
    let started = std::thread::Builder::new().name(format!("lead-inbox-{}", team.name)).spawn(move || {
        while team.exists() {
            deliver(&team, LEAD, client);
            std::thread::sleep(POLL);
        }
    });
    if let Err(error) = started {
        tracing::warn!(%error, "the lead's mailbox cannot be watched; teammates' messages will not arrive");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lead_s_mail_reaches_the_lead_s_inbox_as_teammate_messages() {
        let home = tempfile::tempdir().expect("home");
        let team = Team::named(home.path(), "t");
        team.open("s").expect("open");
        team.send(LEAD, "tester", "done", Some("finished")).expect("send");

        assert_eq!(deliver(&team, LEAD, 96_001), 1);

        let notices = crate::tools::inbox::take(96_001);
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].source, "teammate tester");
        assert_eq!(notices[0].meta("summary"), Some("finished"));
        assert_eq!(deliver(&team, LEAD, 96_001), 0, "each message once");
    }
}
