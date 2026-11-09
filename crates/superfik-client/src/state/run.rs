//! Submitting prompts and tracking the run they start.

use superfik_core::{Activity, PromptRequest, Reference, RunId, Turn, UserMessage, title_from_prompt};
use superfik_protocol::ClientCommand;
use superfik_ui::TextEdit;

use super::context::is_image;
use super::{ActiveRun, Effect, Session};

impl Session {
    pub(super) fn retry_turn(&mut self, index: usize, now: f64) -> Vec<Effect> {
        let c = self.conversation();
        let failed = c.turns.get(index).filter(|t| matches!(t.response.activity, Activity::Failed(_)));
        let Some(text) = failed.map(|t| t.user.text.clone()) else { return Vec::new() };
        self.send_text(&text, now)
    }

    pub(super) fn send_text(&mut self, text: &str, now: f64) -> Vec<Effect> {
        let c = self.conversation();
        if c.is_running() || c.is_compacting() {
            return Vec::new();
        }
        let mut prompt = TextEdit::default();
        prompt.insert(text);
        let draft = std::mem::replace(&mut self.conversation_mut().composer, prompt);
        let effects = self.submit(now);
        self.conversation_mut().composer = draft;
        effects
    }

    pub(super) fn submit(&mut self, now: f64) -> Vec<Effect> {
        if let Some(effects) = self.run_composer_command() {
            return effects;
        }
        let author = self.my_name();
        let c = self.conversation();
        if c.is_running() || c.is_compacting() || c.composer.is_empty() {
            return Vec::new();
        }
        if self.awaiting_organization() {
            return Vec::new();
        }
        if !self.has_models() {
            self.notice = Some(super::Notice::NoModels);
            return Vec::new();
        }
        if let Some(refusal) = self.policy_refusal() {
            self.notice = Some(super::Notice::Policy(refusal));
            return Vec::new();
        }
        let Some(root) = self.root.clone() else {
            self.notice = Some(super::Notice::ChooseRepository);
            return vec![Effect::PickWorkspace];
        };
        if self.conversation().shared_by.is_some() {
            self.continue_shared();
        }
        let workspace = self.workspace_id;
        let model = self.model().clone();
        let skills = super::context::skills_in(self.conversation().composer.text(), &self.available_skills);
        let selected = self.selected_skills();
        let disabled_skills: Vec<String> =
            self.available_skills.iter().filter(|s| !selected.contains(s)).map(|s| s.name.clone()).collect();
        let connections: Vec<superfik_core::Connection> = self.session_connections().into_iter().cloned().collect();
        let caption = self.snapshot.repository.as_ref().map(|r| r.branch_caption()).unwrap_or_default();
        let started_unix = self.clock.wall_now(now);
        let handoff = self.pending_handoff.take().map(superfik_core::Handoff::new);
        let shared_memory = self.shared_memory.clone();
        self.link_named_notes();
        let notes = self.linked_notes();
        let spoken = self.voice.is_on();
        let (name, address) = (self.preferences.wake.word, self.preferences.voice_address);
        let output_style = self.preferences.output_style.id().map(str::to_owned);
        let safety_check = self.preferences.safety_check;
        let c = self.conversation_mut();
        let mut effects = c.stop_measuring();
        c.compaction = None;
        let mut request = PromptRequest {
            connections: connections.clone(),
            plugins: None,
            browser: None,
            credentials: Default::default(),
            workspace,
            cwd: root,
            prompt: c.composer.text().trim().to_owned(),
            model,
            effort: c.effort,
            repositories: c.repositories.clone(),
            skills,
            disabled_skills: disabled_skills.clone(),
            references: c.references.iter().map(|path| Reference { path: path.clone() }).collect(),
            notes,
            resume: c.engine_session.clone(),
            command: None,
            handoff,
            memory: None,
            permission: c.permission,
            output_style,
            safety_check,
        };
        c.chores.suggestion = None;
        c.chores.recapped = true;
        let first = c.turns.is_empty();
        let title_from = first.then(|| request.prompt.clone());
        if first {
            c.title = title_from_prompt(&request.prompt);
            c.chores.guessed_title = c.title.clone();
            c.started_unix = Some(started_unix);
        }
        request.memory = Some(superfik_core::MemoryBrief {
            shared: shared_memory,
            ..superfik_core::MemoryBrief::new(c.title.clone())
        });
        c.removed_skills = disabled_skills;
        c.connections = connections;
        if let Some(context) = c.carried.take() {
            request.prompt = format!("{context}\n\n{}", request.prompt);
        }
        if spoken {
            let persona = superfik_core::spoken::Persona { name: name.as_str(), address: address.as_str() };
            request.prompt = superfik_core::spoken::for_speech(&request.prompt, persona);
        }
        let branch_caption = match &author {
            Some(name) if caption.is_empty() => name.clone(),
            Some(name) => format!("{name} · {caption}"),
            None => caption,
        };
        let mut turn = Turn::new(UserMessage {
            text: c.composer.take(),
            images: c.references.iter().filter(|p| is_image(p)).cloned().collect(),
            branch_caption,
        });
        turn.author = author;
        c.turns.push(turn);
        c.agents.clear();
        c.open_agent = None;
        c.agent_started.clear();
        c.research_items.clear();
        let run = RunId::new();
        c.run = Some(ActiveRun { id: run, started_at: now });
        c.finished_at = None;
        self.popover = None;
        if !matches!(self.notice, Some(super::Notice::ContinuingShared(_))) {
            self.notice = None;
        }
        self.conversation_mut().chores.title_from = title_from;
        effects.push(Effect::Command(Box::new(ClientCommand::SubmitPrompt { run, request })));
        effects
    }

    pub(super) fn cancel(&mut self) -> Vec<Effect> {
        match &self.conversation().run {
            Some(run) => vec![Effect::Command(Box::new(ClientCommand::CancelRun { run: run.id }))],
            None => Vec::new(),
        }
    }

    pub(super) fn cancel_all(&self) -> Vec<Effect> {
        self.conversations
            .iter()
            .filter_map(|c| c.run.as_ref())
            .map(|run| Effect::Command(Box::new(ClientCommand::CancelRun { run: run.id })))
            .collect()
    }

    pub(super) fn remove_agent(&mut self, index: usize) -> Vec<Effect> {
        let c = self.conversation_mut();
        if !c.is_running() && index < c.agents.len() {
            c.agents.remove(index);
            for (i, agent) in c.agents.iter_mut().enumerate() {
                agent.index = i;
            }
            c.open_agent = match c.open_agent {
                Some(open) if open == index => None,
                Some(open) if open > index => Some(open - 1),
                other => other,
            };
        }
        Vec::new()
    }
}
