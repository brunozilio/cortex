//! Who is connected and who each event reaches. Pure bookkeeping: the
//! server feeds it connections, commands heard and events to deliver.

use std::collections::{HashMap, HashSet};

use superfik_core::{Principal, RunId};
use superfik_protocol::{ClientCommand, Recipient};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(u64);

#[derive(Debug)]
struct Link {
    principal: Principal,
    runs: HashSet<RunId>,
}

#[derive(Debug, Default)]
pub struct Hub {
    next: u64,
    links: HashMap<ConnectionId, Link>,
    orphans: HashMap<RunId, Principal>,
    owners: HashMap<RunId, Principal>,
}

impl Hub {
    pub fn join(&mut self, principal: Principal) -> ConnectionId {
        let id = ConnectionId(self.next);
        self.next += 1;
        let adopted: HashSet<RunId> =
            self.orphans.iter().filter(|(_, owner)| **owner == principal).map(|(run, _)| *run).collect();
        self.orphans.retain(|run, _| !adopted.contains(run));
        self.links.insert(id, Link { principal, runs: adopted });
        id
    }

    pub fn leave(&mut self, connection: ConnectionId) {
        if let Some(link) = self.links.remove(&connection) {
            let principal = link.principal;
            self.orphans.extend(link.runs.into_iter().map(|run| (run, principal.clone())));
        }
    }

    pub fn heard(&mut self, connection: ConnectionId, command: &ClientCommand) -> bool {
        let run = match command {
            ClientCommand::SubmitPrompt { run, .. } | ClientCommand::OptimizeMemory { run, .. } => *run,
            _ => return true,
        };
        let Some(link) = self.links.get_mut(&connection) else { return true };
        let owner = self.owners.entry(run).or_insert_with(|| link.principal.clone());
        if *owner != link.principal {
            return false;
        }
        link.runs.insert(run);
        true
    }

    pub fn finished(&mut self, run: RunId) {
        for link in self.links.values_mut() {
            link.runs.remove(&run);
        }
        self.orphans.remove(&run);
        self.owners.remove(&run);
    }

    pub fn reach(&self, to: &Recipient) -> Vec<ConnectionId> {
        let mut reached: Vec<ConnectionId> = self
            .links
            .iter()
            .filter(|(_, link)| match to {
                Recipient::Principal(principal) => link.principal == *principal,
                Recipient::Run(run) => link.runs.contains(run),
                Recipient::Everyone => true,
            })
            .map(|(id, _)| *id)
            .collect();
        reached.sort();
        reached
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Effort, EngineKind, ModelOption, OrgId, PromptRequest, UserId, WorkspaceId};

    use super::*;

    fn model() -> ModelOption {
        ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5")
    }

    fn submit(run: RunId) -> ClientCommand {
        ClientCommand::OptimizeMemory { path: "/repo".into(), run, model: model() }
    }

    fn prompt(run: RunId) -> ClientCommand {
        let request = PromptRequest {
            workspace: WorkspaceId::new(),
            cwd: "/repo".into(),
            prompt: "Build a sky".into(),
            model: model(),
            effort: Effort::High,
            output_style: None,
            safety_check: true,
            repositories: vec![],
            skills: vec![],
            disabled_skills: vec![],
            references: vec![],
            notes: vec![],
            resume: None,
            command: None,
            handoff: None,
            memory: None,
            permission: Default::default(),
            connections: vec![],
            plugins: None,
            browser: None,
            credentials: Default::default(),
        };
        ClientCommand::SubmitPrompt { run, request }
    }

    #[test]
    fn an_answer_reaches_every_connection_of_its_principal() {
        let mut hub = Hub::default();
        let ana = Principal::member(UserId::new(), OrgId::new());
        let laptop = hub.join(ana.clone());
        let _other = hub.join(Principal::member(UserId::new(), OrgId::new()));
        let desktop = hub.join(ana.clone());
        assert_eq!(hub.reach(&Recipient::Principal(ana)), [laptop, desktop]);
    }

    #[test]
    fn a_run_reaches_the_connection_that_started_it() {
        let mut hub = Hub::default();
        let starter = hub.join(Principal::LocalOwner);
        let _watcher = hub.join(Principal::LocalOwner);
        let run = RunId::new();
        hub.heard(starter, &submit(run));
        assert_eq!(hub.reach(&Recipient::Run(run)), [starter]);
        assert_eq!(hub.reach(&Recipient::Run(RunId::new())), []);
    }

    #[test]
    fn a_prompt_is_followed_like_any_run() {
        let mut hub = Hub::default();
        let starter = hub.join(Principal::LocalOwner);
        let run = RunId::new();
        hub.heard(starter, &ClientCommand::CancelRun { run });
        assert_eq!(hub.reach(&Recipient::Run(run)), []);
        hub.heard(starter, &prompt(run));
        assert_eq!(hub.reach(&Recipient::Run(run)), [starter]);
    }

    #[test]
    fn a_run_is_followed_only_by_who_started_it() {
        let mut hub = Hub::default();
        let ana = Principal::member(UserId::new(), OrgId::new());
        let laptop = hub.join(ana.clone());
        let max = hub.join(Principal::member(UserId::new(), OrgId::new()));
        let run = RunId::new();
        assert!(hub.heard(laptop, &prompt(run)));
        assert!(!hub.heard(max, &prompt(run)), "someone else's run");
        let desktop = hub.join(ana);
        assert!(hub.heard(desktop, &submit(run)), "the same principal, again");
        assert_eq!(hub.reach(&Recipient::Run(run)), [laptop, desktop]);
        hub.finished(run);
        assert!(hub.heard(max, &prompt(run)), "an ended run's id is free");
    }

    #[test]
    fn a_finished_run_reaches_nobody() {
        let mut hub = Hub::default();
        let starter = hub.join(Principal::LocalOwner);
        let run = RunId::new();
        hub.heard(starter, &submit(run));
        hub.finished(run);
        assert_eq!(hub.reach(&Recipient::Run(run)), []);
    }

    #[test]
    fn a_reconnect_follows_the_runs_its_dropped_connection_started() {
        let mut hub = Hub::default();
        let ana = Principal::member(UserId::new(), OrgId::new());
        let dropped = hub.join(ana.clone());
        let (run, ended) = (RunId::new(), RunId::new());
        hub.heard(dropped, &submit(run));
        hub.heard(dropped, &submit(ended));
        hub.leave(dropped);
        hub.finished(ended);
        let _someone = hub.join(Principal::member(UserId::new(), OrgId::new()));
        let again = hub.join(ana);
        assert_eq!(hub.reach(&Recipient::Run(run)), [again]);
        assert_eq!(hub.reach(&Recipient::Run(ended)), [], "an ended run is not taken up");
    }

    #[test]
    fn everyone_means_every_connection_still_here() {
        let mut hub = Hub::default();
        let (a, b, c) =
            (hub.join(Principal::LocalOwner), hub.join(Principal::LocalOwner), hub.join(Principal::LocalOwner));
        hub.leave(b);
        assert_eq!(hub.reach(&Recipient::Everyone), [a, c]);
    }
}
