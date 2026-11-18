use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use domain::team::{LEAD, TeammateAsk};
use platform::sync::Locked;

use super::Team;

const POLL: Duration = Duration::from_millis(500);

type Waiting = HashMap<u64, Vec<(TeammateAsk, Team)>>;

fn asks() -> MutexGuard<'static, Waiting> {
    static ASKS: OnceLock<Mutex<Waiting>> = OnceLock::new();
    ASKS.get_or_init(Mutex::default).locked()
}

pub fn deliver(team: &Team, agent: &str, client: u64) -> usize {
    let mails = team.unread(agent);
    let count = mails.len();
    for mail in &mails {
        match TeammateAsk::of(&team.name, mail) {
            Some(ask) => asks().entry(client).or_default().push((ask, team.clone())),
            None => crate::tools::inbox::push(client, domain::inbox::teammate_notice(mail)),
        }
    }
    count
}

pub fn take_asks(client: u64) -> Vec<TeammateAsk> {
    asks().get(&client).map(|waiting| waiting.iter().map(|(ask, _)| ask.clone()).collect()).unwrap_or_default()
}

pub fn answer(client: u64, ask: &TeammateAsk, decision: &domain::Decision) -> Result<(), String> {
    let team = {
        let mut asks = asks();
        let waiting = asks.get_mut(&client).ok_or("no teammate is waiting for an answer")?;
        let at = waiting
            .iter()
            .position(|(waiting, _)| waiting == ask)
            .ok_or("that teammate is no longer waiting for an answer")?;
        waiting.remove(at).1
    };
    team.send(&ask.teammate, LEAD, &ask.answer(decision).text(), Some("permission answer")).map_err(String::from)
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
    use domain::team::Protocol;

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

    #[test]
    fn a_teammate_s_permission_request_waits_for_the_user_and_the_answer_reaches_only_that_teammate() {
        let home = tempfile::tempdir().expect("home");
        let team = Team::named(home.path(), "t2");
        team.open("s").expect("open");
        let request = Protocol::PermissionRequest {
            request_id: "p1".into(),
            from: "tester".into(),
            tool_name: "Bash".into(),
            description: "npm run deploy".into(),
        };
        team.send(LEAD, "tester", &request.text(), None).expect("send");
        team.send("tester", LEAD, "unrelated work", None).expect("send");

        deliver(&team, LEAD, 96_002);

        assert!(crate::tools::inbox::take(96_002).is_empty(), "the model never sees it");
        let waiting = take_asks(96_002);
        assert_eq!(waiting.len(), 1);
        assert_eq!(team.take_answer("tester", "p1"), None, "nobody answered yet");
        answer(96_002, &waiting[0], &domain::Decision::Allow).expect("answered");
        assert!(take_asks(96_002).is_empty());
        assert_eq!(team.take_answer("tester", "p1"), Some(true));
        assert_eq!(team.unread("tester").len(), 1, "the teammate's other mail stays");
        assert!(answer(96_002, &waiting[0], &domain::Decision::Deny).is_err(), "answered once");
    }
}
