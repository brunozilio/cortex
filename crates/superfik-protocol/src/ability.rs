//! The ability each command needs, for an authorizer to weigh against the
//! sender's roles (`superfik_core::access`).

use superfik_core::access::Ability;

use crate::ClientCommand;

impl ClientCommand {
    pub fn ability(&self) -> Ability {
        match self {
            Self::Hello { .. }
            | Self::ListMembers
            | Self::ListModels
            | Self::InspectWorkspace { .. }
            | Self::ListSkills { .. }
            | Self::ListAgents { .. }
            | Self::InspectRepository { .. }
            | Self::ListGitHubRepositories
            | Self::ListPullRequests { .. }
            | Self::LoadPullRequest { .. }
            | Self::LoadMemory { .. }
            | Self::LoadNotes { .. }
            | Self::LoadRoutines { .. }
            | Self::ListChannels
            | Self::SearchMemory { .. }
            | Self::LoadSessions { .. }
            | Self::ListDelegated
            | Self::LoadSharedMemory { .. }
            | Self::ListLibrary
            | Self::ListTeams
            | Self::LoadComments { .. }
            | Self::Viewing { .. }
            | Self::GetPolicy
            | Self::ReadLibraryItem { .. }
            | Self::ListPlugins
            | Self::ListProviders
            | Self::LoadSettings
            | Self::FindLogins { .. }
            | Self::FindPasskeys { .. } => Ability::Read,
            Self::SubmitPrompt { .. }
            | Self::CancelRun { .. }
            | Self::AnswerAsk { .. }
            | Self::DelegateAsk { .. }
            | Self::AnswerDelegated { .. }
            | Self::ShareMemory { .. }
            | Self::UnshareMemory { .. }
            | Self::InstallLibrary { .. }
            | Self::AddComment { .. }
            | Self::ReportUsage { .. }
            | Self::AnswerBrowse { .. }
            | Self::Page(_)
            | Self::SetPermissionMode { .. }
            | Self::SwitchBranch { .. }
            | Self::LocateRepository { .. }
            | Self::CheckoutPullRequest { .. }
            | Self::CreatePullRequest { .. }
            | Self::SubmitReview { .. }
            | Self::ResolveThread { .. }
            | Self::EditMemory { .. }
            | Self::EditNotes { .. }
            | Self::EditRoutines { .. }
            | Self::RevealWebhook { .. }
            | Self::ApproveDraft { .. }
            | Self::DiscardDraft { .. }
            | Self::OptimizeMemory { .. }
            | Self::UndoMemory { .. }
            | Self::SaveSession { .. }
            | Self::FillLogin { .. }
            | Self::SaveLogin { .. }
            | Self::SignPasskey { .. }
            | Self::CreatePasskey { .. }
            | Self::Upload { .. }
            | Self::SaveSettings { .. }
            | Self::SignInToGitHub
            | Self::SignOutOfGitHub => Ability::Run,
            Self::InstallPlugin { .. }
            | Self::UninstallPlugin { .. }
            | Self::Connect { .. }
            | Self::Disconnect { .. }
            | Self::ConnectProvider { .. }
            | Self::DownloadModel { .. }
            | Self::DisconnectProvider { .. } => Ability::Configure,
            Self::PublishToLibrary { .. }
            | Self::AddUser { .. }
            | Self::NewFirstAccessCode { .. }
            | Self::RemoveUser { .. }
            | Self::SetUserRole { .. }
            | Self::LoadServerSettings
            | Self::SaveServerSettings { .. }
            | Self::RemoveFromLibrary { .. }
            | Self::SetPolicy { .. }
            | Self::CreateTeam { .. }
            | Self::RemoveTeam { .. }
            | Self::SetTeamMember { .. }
            | Self::ListUsage => Ability::Administer,
        }
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{ConnectionId, PluginId, RunId};

    use super::*;

    #[test]
    fn looking_needs_reading() {
        for command in [
            ClientCommand::ListModels,
            ClientCommand::InspectWorkspace { path: "/repo".into() },
            ClientCommand::LoadSessions { path: "/repo".into(), project: None },
            ClientCommand::LoadMemory { path: "/repo".into() },
            ClientCommand::ListPlugins,
        ] {
            assert_eq!(command.ability(), Ability::Read, "{}", command.name());
        }
    }

    #[test]
    fn working_needs_running() {
        let run = RunId::new();
        for command in [
            ClientCommand::CancelRun { run },
            ClientCommand::SwitchBranch { path: "/repo".into(), branch: superfik_core::BranchRef::remote("main") },
            ClientCommand::CheckoutPullRequest { path: "/repo".into(), number: 7 },
            ClientCommand::UndoMemory { path: "/repo".into() },
            ClientCommand::SignInToGitHub,
            ClientCommand::SignOutOfGitHub,
        ] {
            assert_eq!(command.ability(), Ability::Run, "{}", command.name());
        }
    }

    #[test]
    fn plugins_and_accounts_need_configuring() {
        let plugin = PluginId("linear".into());
        for command in [
            ClientCommand::InstallPlugin { plugin: Some(plugin.clone()), folder: None },
            ClientCommand::UninstallPlugin { plugin },
            ClientCommand::Disconnect { connection: ConnectionId::new() },
        ] {
            assert_eq!(command.ability(), Ability::Configure, "{}", command.name());
        }
    }

    #[test]
    fn providers_are_read_freely_and_need_configuring_to_change() {
        use superfik_core::provider::ModelProvider;
        assert_eq!(ClientCommand::ListProviders.ability(), Ability::Read);
        let provider = ModelProvider::OpenAi;
        for command in [
            ClientCommand::ConnectProvider { provider, key: "k".into() },
            ClientCommand::DisconnectProvider { provider },
        ] {
            assert_eq!(command.ability(), Ability::Configure, "{}", command.name());
            assert!(command.paths().is_empty(), "{}", command.name());
        }
    }
}
