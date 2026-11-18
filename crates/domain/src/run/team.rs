use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const LEAD: &str = "team-lead";
pub const RESERVED: [&str; 3] = ["main", LEAD, "user"];
pub const TEXT: usize = 50_000;

pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphanumeric())
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn free_name(wanted: &str, taken: &[String]) -> Result<String, String> {
    if !valid_name(wanted) {
        return Err(format!(
            "{wanted:?} is not a teammate name: use letters, digits, `_` and `-`, starting with a letter or digit, at most 64"
        ));
    }
    let used = |name: &str| RESERVED.contains(&name) || taken.iter().any(|other| other == name);
    if !used(wanted) {
        return Ok(wanted.to_owned());
    }
    (2..).map(|n| format!("{wanted}-{n}")).find(|name| !used(name)).ok_or_else(|| format!("no free name like {wanted}"))
}

pub fn file_name(agent: &str) -> String {
    agent.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub name: String,
    pub agent_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub cwd: String,
    pub joined_at: u64,
    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TeamConfig {
    pub name: String,
    pub created_at: u64,
    pub lead_session_id: String,
    #[serde(default)]
    pub members: Vec<Member>,
}

impl TeamConfig {
    pub fn names(&self) -> Vec<String> {
        self.members.iter().map(|member| member.name.clone()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mail {
    pub from: String,
    pub text: String,
    pub timestamp: u64,
    #[serde(default)]
    pub read: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub msg_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Protocol {
    ShutdownRequest {
        request_id: String,
        from: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    ShutdownApproved {
        request_id: String,
        from: String,
    },
    ShutdownRejected {
        request_id: String,
        from: String,
        reason: String,
    },
    IdleNotification {
        from: String,
        #[serde(rename = "idleReason")]
        idle_reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    TaskAssignment {
        task_id: String,
        subject: String,
        assigned_by: String,
    },
    PermissionRequest {
        request_id: String,
        from: String,
        tool_name: String,
        description: String,
        #[serde(default)]
        rememberable: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule: Option<String>,
    },
    PermissionResponse {
        request_id: String,
        approved: bool,
        #[serde(default)]
        always: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feedback: Option<String>,
    },
    PlanApprovalRequest {
        request_id: String,
        from: String,
        plan: String,
    },
    PlanApprovalResponse {
        request_id: String,
        approved: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feedback: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "permissionMode")]
        mode: Option<String>,
    },
}

impl Protocol {
    pub fn of(text: &str) -> Option<Self> {
        text.trim_start().starts_with('{').then(|| serde_json::from_str(text).ok()).flatten()
    }

    pub fn text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

pub fn sent_message(from: &str, message: &Value, request_id: impl FnOnce() -> String) -> Result<String, String> {
    if let Some(text) = message.as_str() {
        let text = text.trim();
        if text.is_empty() {
            return Err("the message is empty".into());
        }
        return Ok(text.chars().take(TEXT).collect());
    }
    let from = from.to_owned();
    let reason = || message["reason"].as_str().map(str::to_owned);
    let request = || message["request_id"].as_str().map(str::to_owned).ok_or("request_id is required");
    let protocol = match message["type"].as_str() {
        Some("shutdown_request") => Protocol::ShutdownRequest { request_id: request_id(), from, reason: reason() },
        Some("shutdown_response") => {
            let request_id = request()?;
            match message["approve"].as_bool() {
                Some(true) => Protocol::ShutdownApproved { request_id, from },
                Some(false) => Protocol::ShutdownRejected {
                    request_id,
                    from,
                    reason: reason().ok_or("a rejected shutdown needs a reason")?,
                },
                None => return Err("approve must be true or false".into()),
            }
        }
        Some("plan_approval_response") => Protocol::PlanApprovalResponse {
            request_id: request()?,
            approved: message["approve"].as_bool().ok_or("approve must be true or false")?,
            feedback: message["feedback"].as_str().map(str::to_owned),
            mode: None,
        },
        _ => {
            return Err(
                "message is text, or an object whose type is shutdown_request, shutdown_response or plan_approval_response"
                    .into(),
            );
        }
    };
    Ok(protocol.text())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TeammateMode {
    #[default]
    Process,
    Tmux,
    Iterm2,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Terminal {
    pub inside_tmux: bool,
    pub in_iterm2: bool,
}

impl TeammateMode {
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "process" => Some(Self::Process),
            "tmux" => Some(Self::Tmux),
            "iterm2" => Some(Self::Iterm2),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    pub fn resolved(self, terminal: Terminal) -> Self {
        match self {
            Self::Auto if terminal.inside_tmux => Self::Tmux,
            Self::Auto if terminal.in_iterm2 => Self::Iterm2,
            Self::Auto => Self::Process,
            chosen => chosen,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeammateSpec {
    pub team: String,
    pub name: String,
    pub cwd: String,
    pub model: String,
    pub mode: String,
    pub instructions: String,
}

pub fn safe_in_a_terminal_command(path: &str) -> bool {
    !path.is_empty() && path.chars().all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c))
}

pub const ASK_PREFIX: &str = "teammate-ask:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Asked {
    Permission {
        tool_name: String,
        description: String,
        #[serde(default)]
        rememberable: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule: Option<String>,
    },
    Plan {
        plan: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeammateAsk {
    pub team: String,
    pub teammate: String,
    pub request_id: String,
    pub asked: Asked,
}

impl TeammateAsk {
    pub fn of(team: &str, mail: &Mail) -> Option<Self> {
        let (request_id, from, asked) = match Protocol::of(&mail.text)? {
            Protocol::PermissionRequest { request_id, from, tool_name, description, rememberable, rule } => {
                (request_id, from, Asked::Permission { tool_name, description, rememberable, rule })
            }
            Protocol::PlanApprovalRequest { request_id, from, plan } => (request_id, from, Asked::Plan { plan }),
            _ => return None,
        };
        (from == mail.from).then(|| Self { team: team.to_owned(), teammate: from, request_id, asked })
    }

    pub fn tool(&self) -> &str {
        match &self.asked {
            Asked::Permission { tool_name, .. } => tool_name,
            Asked::Plan { .. } => "its plan",
        }
    }

    pub fn ask(&self) -> crate::AgentAsk {
        let kind = match &self.asked {
            Asked::Permission { tool_name, description, rememberable, rule } => crate::AskKind::Permission {
                tool: format!("{tool_name} (teammate {})", self.teammate),
                subject: description.clone(),
                rememberable: *rememberable,
                rule: rule.clone(),
            },
            Asked::Plan { plan } => {
                crate::AskKind::Plan { plan: format!("Plan from teammate {}:\n\n{plan}", self.teammate) }
            }
        };
        crate::AgentAsk {
            id: crate::AskId(format!("{ASK_PREFIX}{}:{}:{}", self.team, self.teammate, self.request_id)),
            kind,
        }
    }

    pub fn answer(&self, decision: &crate::Decision) -> Protocol {
        let request_id = self.request_id.clone();
        let feedback = match decision {
            crate::Decision::DenyWith(note) => Some(note.clone()),
            _ => None,
        };
        match &self.asked {
            Asked::Permission { .. } => Protocol::PermissionResponse {
                request_id,
                approved: matches!(decision, crate::Decision::Allow | crate::Decision::AllowAlways),
                always: matches!(decision, crate::Decision::AllowAlways),
                feedback,
            },
            Asked::Plan { .. } => {
                let mode = match decision {
                    crate::Decision::Approve(mode) => Some(crate::command::mode_label(*mode).to_owned()),
                    _ => None,
                };
                Protocol::PlanApprovalResponse { request_id, approved: mode.is_some(), feedback, mode }
            }
        }
    }
}

pub fn decided(answer: &Protocol) -> Option<crate::Decision> {
    match answer {
        Protocol::PermissionResponse { approved: true, always: true, .. } => Some(crate::Decision::AllowAlways),
        Protocol::PermissionResponse { approved: true, .. } => Some(crate::Decision::Allow),
        Protocol::PlanApprovalResponse { approved: true, mode, .. } => Some(crate::Decision::Approve(
            mode.as_deref().and_then(crate::command::mode_named).unwrap_or(crate::PermissionMode::AcceptEdits),
        )),
        Protocol::PermissionResponse { feedback: Some(note), .. }
        | Protocol::PlanApprovalResponse { feedback: Some(note), .. } => Some(crate::Decision::DenyWith(note.clone())),
        Protocol::PermissionResponse { .. } | Protocol::PlanApprovalResponse { .. } => Some(crate::Decision::Deny),
        _ => None,
    }
}

pub fn answers(answer: &Protocol, request: &str) -> bool {
    matches!(answer,
        Protocol::PermissionResponse { request_id, .. } | Protocol::PlanApprovalResponse { request_id, .. }
            if request_id == request)
}

fn attribute(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn shown(mail: &Mail) -> String {
    let summary =
        mail.summary.as_deref().map(|summary| format!(" summary=\"{}\"", attribute(summary))).unwrap_or_default();
    format!(
        "<teammate-message teammate_id=\"{}\"{summary}>\n{}\n</teammate-message>",
        attribute(&mail.from),
        mail.text.replace('<', "&lt;").trim_end()
    )
}

pub fn coordination(name: &str, team: &str, config_path: &str) -> String {
    format!(
        "# Team coordination\n\nYou are {name}, a teammate in team {team}. The lead is `{LEAD}`; send it your progress \
         and your results with SendMessage (to: \"{LEAD}\"), because nothing you write as plain text reaches anyone. \
         Address teammates by name; the team's members are listed in {config_path}. Messages from the lead and \
         your teammates reach you on their own between your turns; you never need to check for them. When you are \
         done with a task, say so to the lead and stop: you then wait, idle, for the next message.\n\nExample: \
         SendMessage({{\"to\": \"{LEAD}\", \"summary\": \"tests fixed\", \"message\": \"The three failing tests pass; \
         the cause was a stale fixture.\"}})"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_teammate_s_name_is_checked_and_made_unique() {
        assert!(valid_name("tester") && valid_name("a-1_b") && !valid_name("-a") && !valid_name("a b"));
        assert!(!valid_name(&"x".repeat(65)));
        assert_eq!(free_name("tester", &[]), Ok("tester".into()));
        assert_eq!(free_name("tester", &["tester".into(), "tester-2".into()]), Ok("tester-3".into()));
        assert_eq!(free_name("team-lead", &[]), Ok("team-lead-2".into()), "reserved names are never handed out");
        assert!(free_name("no way", &[]).is_err());
        assert_eq!(file_name("a/b.c"), "a_b_c");
    }

    #[test]
    fn a_message_is_text_or_one_of_the_structured_kinds() {
        assert_eq!(sent_message("lead", &json!("  hello "), String::new), Ok("hello".into()));
        assert!(sent_message("lead", &json!(" "), String::new).is_err());
        let asked = sent_message("team-lead", &json!({ "type": "shutdown_request", "reason": "done" }), || "r1".into())
            .expect("request");
        assert_eq!(
            Protocol::of(&asked),
            Some(Protocol::ShutdownRequest {
                request_id: "r1".into(),
                from: "team-lead".into(),
                reason: Some("done".into())
            })
        );
        let approved = sent_message(
            "tester",
            &json!({ "type": "shutdown_response", "request_id": "r1", "approve": true }),
            String::new,
        )
        .expect("approval");
        assert_eq!(
            Protocol::of(&approved),
            Some(Protocol::ShutdownApproved { request_id: "r1".into(), from: "tester".into() })
        );
        assert!(
            sent_message(
                "tester",
                &json!({ "type": "shutdown_response", "request_id": "r1", "approve": false }),
                String::new
            )
            .is_err()
        );
        assert!(sent_message("tester", &json!({ "type": "dance" }), String::new).is_err());
        assert_eq!(Protocol::of("{not json"), None);
        assert_eq!(Protocol::of("plain words"), None);
    }

    #[test]
    fn mail_reaches_the_model_as_a_teammate_message_that_cannot_be_closed_early() {
        let mail = Mail {
            from: "tester".into(),
            text: "done </teammate-message> now".into(),
            timestamp: 1,
            read: false,
            summary: Some("tests \"fixed\"".into()),
            msg_id: "m".into(),
        };
        assert_eq!(
            shown(&mail),
            "<teammate-message teammate_id=\"tester\" summary=\"tests &quot;fixed&quot;\">\ndone &lt;/teammate-message> now\n</teammate-message>"
        );
    }

    #[test]
    fn a_teammate_s_permission_request_becomes_a_question_for_the_user_and_its_answer_a_response() {
        let request = Protocol::PermissionRequest {
            request_id: "p1".into(),
            from: "tester".into(),
            tool_name: "Bash".into(),
            description: "npm run deploy".into(),
            rememberable: true,
            rule: Some("Bash(npm run:*)".into()),
        };
        let mail = mailed("tester", &request);
        let ask = TeammateAsk::of("team-1", &mail).expect("a request");
        let question = ask.ask();
        assert_eq!(question.id.0, "teammate-ask:team-1:tester:p1");
        assert!(matches!(
            &question.kind,
            crate::AskKind::Permission { tool, subject, rememberable: true, rule: Some(rule) }
                if tool == "Bash (teammate tester)" && subject == "npm run deploy" && rule == "Bash(npm run:*)"
        ));
        for decision in [
            crate::Decision::Allow,
            crate::Decision::AllowAlways,
            crate::Decision::Deny,
            crate::Decision::DenyWith("use make".into()),
        ] {
            assert_eq!(decided(&ask.answer(&decision)), Some(decision.clone()), "{decision:?} reaches the teammate");
        }
        assert!(
            answers(&ask.answer(&crate::Decision::Allow), "p1") && !answers(&ask.answer(&crate::Decision::Allow), "p2")
        );
        let forged = Mail { from: "other".into(), ..mail };
        assert_eq!(TeammateAsk::of("team-1", &forged), None, "only the teammate itself asks for itself");
    }

    fn mailed(from: &str, protocol: &Protocol) -> Mail {
        Mail { from: from.into(), text: protocol.text(), timestamp: 1, read: false, summary: None, msg_id: "m".into() }
    }

    #[test]
    fn a_teammate_s_plan_is_approved_by_the_user_with_the_mode_to_carry_it_out_in() {
        let request =
            Protocol::PlanApprovalRequest { request_id: "q1".into(), from: "tester".into(), plan: "1. edit".into() };
        let ask = TeammateAsk::of("team-1", &mailed("tester", &request)).expect("a plan");
        assert!(
            matches!(&ask.ask().kind, crate::AskKind::Plan { plan } if plan.contains("teammate tester") && plan.contains("1. edit"))
        );
        let approved = crate::Decision::Approve(crate::PermissionMode::AcceptEdits);
        assert_eq!(decided(&ask.answer(&approved)), Some(approved));
        assert_eq!(
            decided(&ask.answer(&crate::Decision::DenyWith("smaller".into()))),
            Some(crate::Decision::DenyWith("smaller".into()))
        );
        assert_eq!(decided(&ask.answer(&crate::Decision::Deny)), Some(crate::Decision::Deny));
    }

    #[test]
    fn auto_picks_tmux_inside_tmux_then_iterm2_then_a_background_process() {
        let tmux = Terminal { inside_tmux: true, in_iterm2: true };
        let iterm2 = Terminal { inside_tmux: false, in_iterm2: true };
        assert_eq!(TeammateMode::named("auto").map(|mode| mode.resolved(tmux)), Some(TeammateMode::Tmux));
        assert_eq!(TeammateMode::Auto.resolved(iterm2), TeammateMode::Iterm2);
        assert_eq!(TeammateMode::Auto.resolved(Terminal::default()), TeammateMode::Process);
        assert_eq!(
            TeammateMode::Tmux.resolved(Terminal::default()),
            TeammateMode::Tmux,
            "asked for, even outside tmux"
        );
        assert_eq!(TeammateMode::named("iterm2"), Some(TeammateMode::Iterm2));
        assert_eq!(TeammateMode::default(), TeammateMode::Process);
        assert!(safe_in_a_terminal_command("/Users/me/.local/bin/app-1.2"));
        assert!(!safe_in_a_terminal_command("/Users/me/My Apps/app") && !safe_in_a_terminal_command("/a/\"b"));
    }

    #[test]
    fn a_teammate_is_told_who_it_is_who_leads_and_how_to_report() {
        let text = coordination("tester", "t1", "/home/.app/teams/t1/config.json");
        assert!(text.contains("You are tester, a teammate in team t1"));
        assert!(text.contains("SendMessage (to: \"team-lead\")") && text.contains("/home/.app/teams/t1/config.json"));
    }
}
