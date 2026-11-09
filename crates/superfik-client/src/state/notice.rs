//! The notice pinned above the canvas. Kept as what happened, not as a
//! sentence, so it reads in whatever language the interface shows.

use superfik_core::{EngineKind, EngineProblem};
use superfik_i18n::{Strings, Text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Engine(EngineProblem, EngineKind),
    ChooseRepository,
    BusySwitch,
    NothingToCompact,
    ContinuingShared(String),
    Delegated { by: String, allowed: bool },
    MemoryShared,
    SharedWithTeam(String),
    NoSuchTeam(String),
    TeamsAdminsOnly,
    Commented(String),
    CommentNeedsOrganization,
    OrganizationRequired,
    Policy(super::PolicyRefusal),
    PublishHow,
    Reconnecting,
    ServerSignedOut,
    GitHubCode(String),
    PublishNotAllowed,
    Published(String),
    ApprovalWaiting(String),
    Opening(String),
    CheckingOut(u64),
    CreatingPullRequest,
    LoadingPullRequest(u64),
    PublishingReview,
    ReviewPublished(u64),
    AgentReviewReady(usize),
    NoLoginForm,
    LoginPageChanged,
    LoginSaved(String),
    PasskeySaved(String),
    NoSavedLogin,
    NoWebPages,
    NoModels,
    Voice(superfik_voice::VoiceProblem),
    Raw(String),
    Recap(String),
    NothingToAsk,
    SideAnswer(String),
    HeadsUp(superfik_core::HeadsUp),
}

impl Notice {
    pub fn is_informative(&self) -> bool {
        matches!(self, Self::Recap(_) | Self::SideAnswer(_) | Self::HeadsUp(_))
    }

    pub fn text(&self, strings: &Strings) -> String {
        match self {
            Notice::Engine(problem, engine) => strings.engine_problem(*problem, *engine),
            Notice::ChooseRepository => strings.t(Text::ChooseRepositoryToStart),
            Notice::BusySwitch => strings.t(Text::BusySwitch),
            Notice::NothingToCompact => strings.t(Text::NothingToCompact),
            Notice::ContinuingShared(owner) => strings.t(Text::ContinuingShared(owner)),
            Notice::Delegated { by, allowed: true } => strings.t(Text::DelegatedAllowed(by)),
            Notice::Delegated { by, allowed: false } => strings.t(Text::DelegatedDenied(by)),
            Notice::ApprovalWaiting(name) => strings.t(Text::ApprovalWaiting(name)),
            Notice::MemoryShared => strings.t(Text::MemoryShared),
            Notice::Policy(super::PolicyRefusal::Unpriced(model)) => strings.t(Text::PolicyUnpriced(model)),
            Notice::Policy(super::PolicyRefusal::Model(model)) => strings.t(Text::PolicyModel(model)),
            Notice::Policy(super::PolicyRefusal::Mode(mode)) => {
                let mode = strings.t(match mode {
                    superfik_core::PermissionMode::Ask => Text::PermissionAsk,
                    superfik_core::PermissionMode::AcceptEdits => Text::PermissionEdits,
                    superfik_core::PermissionMode::Plan => Text::PermissionPlan,
                    superfik_core::PermissionMode::Bypass => Text::PermissionBypass,
                });
                strings.t(Text::PolicyMode(&mode))
            }
            Notice::Policy(super::PolicyRefusal::Budget(dollars)) => strings.t(Text::PolicyBudget(*dollars as usize)),
            Notice::OrganizationRequired => strings.t(Text::OrganizationRequired),
            Notice::Commented(name) => strings.t(Text::Commented(name)),
            Notice::CommentNeedsOrganization => strings.t(Text::CommentNeedsOrganization),
            Notice::SharedWithTeam(team) => strings.t(Text::SharedWithTeam(team)),
            Notice::NoSuchTeam(team) => strings.t(Text::NoSuchTeam(team)),
            Notice::TeamsAdminsOnly => strings.t(Text::TeamsAdminsOnly),
            Notice::PublishHow => strings.t(Text::PublishHow),
            Notice::Reconnecting => strings.t(Text::Reconnecting),
            Notice::ServerSignedOut => strings.t(Text::ServerSignedOut),
            Notice::GitHubCode(code) => strings.t(Text::GitHubCode(code)),
            Notice::PublishNotAllowed => strings.t(Text::PublishNotAllowed),
            Notice::Published(name) => strings.t(Text::Published(name)),
            Notice::Opening(slug) => strings.t(Text::OpeningRepository(slug)),
            Notice::CheckingOut(number) => strings.t(Text::CheckingOutPullRequest(*number)),
            Notice::CreatingPullRequest => strings.t(Text::CreatingPullRequest),
            Notice::LoadingPullRequest(n) => strings.t(Text::LoadingPullRequest(*n)),
            Notice::PublishingReview => strings.t(Text::PublishingReview),
            Notice::ReviewPublished(n) => strings.t(Text::ReviewPublished(*n)),
            Notice::AgentReviewReady(n) => strings.t(Text::AgentReviewReady(*n)),
            Notice::NoLoginForm => strings.t(Text::NoLoginForm),
            Notice::LoginPageChanged => strings.t(Text::LoginPageChanged),
            Notice::LoginSaved(account) => strings.t(Text::LoginSaved(account)),
            Notice::PasskeySaved(account) => strings.t(Text::PasskeySaved(account)),
            Notice::NoSavedLogin => strings.t(Text::NoSavedLogin),
            Notice::NoWebPages => strings.t(Text::NoWebPages),
            Notice::NoModels => strings.t(Text::NoModels),
            Notice::Voice(problem) => super::voice::problem_text(problem, strings),
            Notice::Raw(text) => text.clone(),
            Notice::Recap(text) => strings.t(Text::Recap(text)),
            Notice::NothingToAsk => strings.t(Text::NothingToAsk),
            Notice::SideAnswer(text) => text.clone(),
            Notice::HeadsUp(found) => strings.t(Text::HeadsUp {
                urgent: found.urgent,
                title: &found.title,
                learn: &found.learn,
                explain: &found.explain,
            }),
        }
    }
}

#[cfg(test)]
impl super::Session {
    pub fn notice_text(&self) -> Option<String> {
        self.notice.as_ref().map(|n| n.text(&self.strings()))
    }
}

#[cfg(test)]
mod tests {
    use superfik_i18n::Language;

    use super::*;

    #[test]
    fn notices_read_in_the_chosen_language_and_raw_text_stays() {
        let en = Strings::new(Language::English);
        let pt = Strings::new(Language::Portuguese);
        assert_eq!(Notice::BusySwitch.text(&en), "Wait for the running sessions to finish before switching branches.");
        assert!(Notice::BusySwitch.text(&pt).starts_with("Espere"));
        assert_eq!(Notice::Opening("acme/new".into()).text(&pt), "Abrindo acme/new…");
        let engine = Notice::Engine(EngineProblem::NotLoggedIn, EngineKind::Native);
        assert_eq!(engine.text(&en), EngineProblem::NotLoggedIn.message(EngineKind::Native));
        assert_eq!(Notice::Raw("Overloaded".into()).text(&pt), "Overloaded");
    }
}
