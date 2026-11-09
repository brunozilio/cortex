use superfik_core::conversation::format_seconds;
use superfik_core::{
    Activity, AgentRole, AgentStatus, Command, ContextSlice, DayGroup, Effort, EngineKind, EngineProblem, SubAgent,
};

use crate::en::English;
use crate::pt_br::Portuguese;
use crate::{Catalog, Language, Text, days};

#[derive(Clone, Copy)]
pub struct Strings {
    language: Language,
    catalog: &'static dyn Catalog,
}

impl std::fmt::Debug for Strings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Strings").field("language", &self.language).finish()
    }
}

impl Default for Strings {
    fn default() -> Self {
        Self::new(Language::default())
    }
}

impl Strings {
    pub fn new(language: Language) -> Self {
        let catalog: &'static dyn Catalog = match language {
            Language::English => &English,
            Language::Portuguese => &Portuguese,
        };
        Self { language, catalog }
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn t(&self, text: Text<'_>) -> String {
        self.catalog.text(text)
    }

    pub fn activity(&self, activity: &Activity, elapsed_ms: u64) -> String {
        self.catalog.activity(activity, &format_seconds(elapsed_ms))
    }

    pub fn day(&self, group: DayGroup, now_unix: i64, utc_offset_secs: i32) -> String {
        days::label(self.catalog, group, now_unix, utc_offset_secs)
    }

    pub fn effort(&self, effort: Effort) -> &'static str {
        self.catalog.effort(effort)
    }

    pub fn command(&self, command: Command) -> &'static str {
        self.catalog.command(command)
    }

    pub fn role(&self, role: AgentRole) -> &'static str {
        self.catalog.role(role)
    }

    pub fn agent_status(&self, status: &AgentStatus) -> Option<&'static str> {
        self.catalog.status(status)
    }

    pub fn agent_label<'a>(&self, agent: &'a SubAgent) -> &'a str {
        match &agent.custom {
            Some(custom) => custom.name.as_str(),
            None => self.role(agent.role),
        }
    }

    pub fn agent_summary<'a>(&self, agent: &'a SubAgent) -> &'a str {
        if agent.custom.is_none() && agent.summary == agent.role.idle_summary() {
            return self.catalog.idle_summary(agent.role);
        }
        if agent.summary == superfik_core::agents::WAITING_TURN {
            return self.catalog.waiting_turn();
        }
        &agent.summary
    }

    pub fn context_slice<'a>(&self, slice: &'a ContextSlice) -> &'a str {
        match slice {
            ContextSlice::Other(name) => name,
            known => self.catalog.context_slice(known).unwrap_or_default(),
        }
    }

    pub fn step_verb<'a>(&self, verb: &'a str) -> &'a str {
        self.catalog.step_verb(verb).unwrap_or(verb)
    }

    pub fn engine_problem(&self, problem: EngineProblem, engine: EngineKind) -> String {
        self.catalog.engine_problem(problem, engine)
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{CustomAgent, Step, ToolCall};

    use super::*;

    const NOW: i64 = 1_793_282_400;
    const DAY: i64 = 86_400;

    fn en() -> Strings {
        Strings::new(Language::English)
    }

    fn pt() -> Strings {
        Strings::new(Language::Portuguese)
    }

    #[test]
    fn every_language_translates_every_key() {
        for language in Language::ALL {
            let s = Strings::new(language);
            for &text in Text::SAMPLES {
                assert!(!s.t(text).trim().is_empty(), "{language:?} {text:?}");
            }
        }
        for &text in Text::SAMPLES {
            let parametrised = matches!(
                text,
                Text::AgentTitle(_)
                    | Text::Steps(_)
                    | Text::ReadingFile(_)
                    | Text::ReadFiles(_)
                    | Text::OpeningRepository(_)
                    | Text::UpdatePercent(_)
                    | Text::AiLocalFetching(..)
                    | Text::AiLocalFetched(_)
                    | Text::UpdateVersion(_)
                    | Text::AskPermission(_)
            );
            let shared = matches!(
                text,
                Text::Skills
                    | Text::PullRequests
                    | Text::PullRequest
                    | Text::MemoryProcedural
                    | Text::Plugins
                    | Text::Connections
                    | Text::RoutineWebhook
                    | Text::BrowserProfileCanvas
                    | Text::BrowserProfileHardware
                    | Text::BrowserProfileGpu
                    | Text::BrowserProfileProxy
                    | Text::BrowserProfileWebRtc
            );
            if !parametrised && !shared {
                assert_ne!(en().t(text), pt().t(text), "{text:?} is untranslated");
            }
        }
    }

    #[test]
    fn english_keeps_the_shipped_text() {
        assert_eq!(en().t(Text::NewSession), "New session");
        assert_eq!(en().t(Text::Steps(1)), "1 step");
        assert_eq!(en().t(Text::Steps(3)), "3 steps");
        assert_eq!(en().t(Text::ReadFiles(1)), "Read 1 file");
        assert_eq!(en().t(Text::AgentTitle(2)), "Agent 2");
        assert_eq!(en().activity(&Activity::Done, 3_000), "Worked for 3s");
        assert_eq!(en().effort(Effort::XHigh), "Extra");
    }

    #[test]
    fn portuguese_plurals_and_parameters() {
        assert_eq!(pt().t(Text::Steps(1)), "1 passo");
        assert_eq!(pt().t(Text::Steps(0)), "0 passos");
        assert_eq!(pt().t(Text::ReadFiles(4)), "Leu 4 arquivos");
        assert_eq!(pt().t(Text::ReadingFile("src/a.rs")), "Lendo src/a.rs");
        assert_eq!(pt().t(Text::OpeningRepository("a/b")), "Abrindo a/b…");
        assert_eq!(pt().activity(&Activity::Done, 80_000), "Trabalhou por 1m 20s");
        assert_eq!(pt().effort(Effort::Max), "Ultracode");
    }

    #[test]
    fn download_sizes_read_in_each_language() {
        let fetching = Text::AiLocalFetching(1_960_000_000, 4_623_784_971);
        assert_eq!(en().t(fetching), "Downloading… 42% · 1.9 GB of 4.6 GB");
        assert_eq!(pt().t(fetching), "Baixando… 42% · 1,9 GB de 4,6 GB");
        assert_eq!(en().t(Text::AiLocalFetched(640_500_000)), "Downloading… 640 MB");
        assert_eq!(
            en().t(Text::AiLocalFetching(5, 0)),
            "Downloading… 100% · 0 MB of 0 MB",
            "no total never divides by it"
        );
    }

    #[test]
    fn days_read_in_each_language() {
        let at = |days_ago: i64| DayGroup::of(NOW - days_ago * DAY, NOW, 0);
        assert_eq!(en().day(at(0), NOW, 0), "Today");
        assert_eq!(pt().day(at(0), NOW, 0), "Hoje");
        assert_eq!(pt().day(at(1), NOW, 0), "Ontem");
        assert_eq!(en().day(at(3), NOW, 0), "Monday");
        assert_eq!(pt().day(at(3), NOW, 0), "Segunda-feira");
        assert_eq!(en().day(at(9), NOW, 0), "Oct 20");
        assert_eq!(pt().day(at(9), NOW, 0), "20 de out");
        assert_eq!(en().day(at(365), NOW, 0), "Oct 29, 2025");
        assert_eq!(pt().day(at(365), NOW, 0), "29 de out de 2025");
    }

    #[test]
    fn agents_translate_canned_text_and_keep_custom_names_and_progress() {
        let mut agent = SubAgent::for_slot(0, &[]);
        assert_eq!(en().agent_label(&agent), "Explorer");
        assert_eq!(pt().agent_label(&agent), "Explorador");
        assert_eq!(pt().agent_summary(&agent), "Mapeando o código");
        agent.summary = superfik_core::agents::WAITING_TURN.into();
        assert_eq!(pt().agent_summary(&agent), "Aguardando a vez");
        agent.summary = "Reading state/run.rs".into();
        assert_eq!(pt().agent_summary(&agent), "Reading state/run.rs");
        assert_eq!(pt().agent_status(&AgentStatus::Queued), Some("Na fila"));

        let customs = [CustomAgent { name: "code-reviewer".into(), description: "Reviews diffs".into() }];
        let custom = SubAgent::for_slot(0, &customs);
        assert_eq!(pt().agent_label(&custom), "code-reviewer");
        assert_eq!(pt().agent_summary(&custom), "Reviews diffs");
    }

    #[test]
    fn step_verbs_translate_and_tool_names_pass_through() {
        let verb = |name: &str| Step::of(&ToolCall::new("1", name, "x"), None).verb;
        assert_eq!(pt().step_verb(&verb("Read")), "Leu");
        assert_eq!(pt().step_verb(&verb("WebFetch")), "Acessou");
        assert_eq!(pt().step_verb(&verb("Grep")), "Grep");
        assert_eq!(pt().step_verb(&verb("Bash")), "Bash");
        assert_eq!(en().step_verb(&verb("Edit")), "Edit");
    }

    #[test]
    fn engine_problems_in_both_languages() {
        let en_text = en().engine_problem(EngineProblem::NotLoggedIn, EngineKind::Native);
        assert_eq!(en_text, EngineProblem::NotLoggedIn.message(EngineKind::Native));
        let pt_text = pt().engine_problem(EngineProblem::SessionNotFound, EngineKind::Codex);
        assert!(pt_text.contains("Codex") && pt_text.contains("nova"), "{pt_text}");
    }
}
