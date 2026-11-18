use std::path::PathBuf;

use domain::team::{LEAD, Member, coordination, sent_message};
use serde_json::Value;

use super::Agent;
use crate::team::launch::{Launching, launch};
use crate::team::{Team, lead_inbox};
use crate::tools::Outcome;

impl Agent<'_> {
    fn home(&self) -> Option<PathBuf> {
        self.workspace.home.clone()
    }

    fn sender(&self) -> Option<(Team, String)> {
        crate::team::current(
            self.workspace.home.as_deref(),
            &self.workspace.session_id,
            self.workspace.teammate.as_ref(),
        )
        .map(|me| (me.team, me.name))
    }

    pub(super) fn teammate(&self, input: &Value) -> Option<Outcome> {
        let wanted = input["name"].as_str().map(str::trim).filter(|name| !name.is_empty())?;
        Some(self.start_teammate(wanted, input).unwrap_or_else(Outcome::error))
    }

    fn start_teammate(&self, wanted: &str, input: &Value) -> Result<Outcome, String> {
        let home = self.home().ok_or("teammates need a home folder to keep their team in")?;
        if self.depth > 0 || self.workspace.teammate.is_some() {
            return Err(
                "Only the lead starts teammates; ask the lead (SendMessage to team-lead) if you need help.".into()
            );
        }
        let prompt = input["prompt"].as_str().map(str::trim).filter(|prompt| !prompt.is_empty());
        let prompt = prompt.ok_or("Missing required parameter `prompt`: the teammate's task.")?;
        let kind = input["subagent_type"].as_str().unwrap_or("general-purpose");
        let found = self.agents.iter().find(|agent| agent.name == kind);
        let session = &self.workspace.session_id;
        let team = Team::of_session(&home, session);
        team.open(session)?;
        let provider = self.workspace.models.model.provider;
        let model = input["model"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| found.and_then(|agent| agent.config.model(&self.request.model, provider).ok()))
            .unwrap_or_else(|| self.request.model.clone());
        let cwd = self.workspace.cwd.clone();
        let member = Member {
            agent_type: kind.to_owned(),
            model: Some(model.clone()),
            cwd: cwd.display().to_string(),
            ..Member::default()
        };
        let name = team.join(wanted, member)?;
        let mut instructions = coordination(&name, &team.name, &team.config_path().display().to_string());
        if let Some(agent) = found.filter(|agent| !agent.instructions.trim().is_empty()) {
            instructions.push_str(&format!("\n\n# Your instructions ({})\n{}", agent.name, agent.instructions));
        }
        let launching = Launching {
            team: &team,
            name: &name,
            cwd: &cwd,
            prompt,
            instructions: &instructions,
            model: &model,
            mode: self.control.mode(),
        };
        if let Err(error) = launch(&launching) {
            team.left(&name);
            return Err(error);
        }
        lead_inbox::watch(&team, self.workspace.credentials.client);
        Ok(Outcome::ok(format!(
            "Started teammate {name} in team {team}. It works on its own, in its own process, and reports to you with \
             SendMessage; its messages reach you between your steps, and it tells you when it goes idle. Send it more \
             work with SendMessage (to: \"{name}\"), and end it with SendMessage (to: \"{name}\", message: {{\"type\": \
             \"shutdown_request\"}}) when the work is done.",
            team = team.name
        )))
    }

    pub(super) fn team_message(&self, input: &Value) -> Option<Outcome> {
        let to = input["to"].as_str().map(str::trim)?;
        let (team, me) = self.sender()?;
        let to = if to == "main" { LEAD } else { to };
        if !team.knows(to) {
            return None;
        }
        let summary = input["summary"].as_str().map(str::trim).filter(|summary| !summary.is_empty());
        let sent = sent_message(&me, &input["message"], || uuid::Uuid::new_v4().to_string())
            .and_then(|text| team.send(to, &me, &text, summary).map_err(String::from));
        Some(match sent {
            Ok(()) => Outcome::ok(format!("Sent to {to}.")),
            Err(problem) => Outcome::error(format!("The message was not sent: {problem}.")),
        })
    }
}
