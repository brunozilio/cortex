//! The folders and files a command points the server at. A client never
//! runs anything (ADR 0010), so every path in a command is a path on the
//! server; an organization's server lets a member reach only their own
//! checkouts, and checks every path this lists before the command runs.

use std::path::Path;

use crate::ClientCommand;

impl ClientCommand {
    pub fn paths(&self) -> Vec<&Path> {
        match self {
            Self::SubmitPrompt { request, .. } => {
                let mut paths = vec![request.cwd.as_path()];
                paths.extend(request.repositories.iter().map(|repository| repository.root.as_path()));
                paths.extend(request.references.iter().map(|reference| reference.path.as_path()));
                paths
            }
            Self::ReadLibraryItem { path, .. }
            | Self::InspectWorkspace { path }
            | Self::ListSkills { path }
            | Self::SwitchBranch { path, .. }
            | Self::ListAgents { path }
            | Self::InspectRepository { path }
            | Self::ListPullRequests { path }
            | Self::CheckoutPullRequest { path, .. }
            | Self::CreatePullRequest { path, .. }
            | Self::LoadPullRequest { path, .. }
            | Self::SubmitReview { path, .. }
            | Self::ResolveThread { path, .. }
            | Self::LoadMemory { path }
            | Self::SearchMemory { path, .. }
            | Self::OptimizeMemory { path, .. }
            | Self::UndoMemory { path }
            | Self::EditMemory { path, .. }
            | Self::LoadNotes { path }
            | Self::EditNotes { path, .. }
            | Self::LoadRoutines { path }
            | Self::EditRoutines { path, .. }
            | Self::RevealWebhook { path, .. }
            | Self::LoadSessions { path, .. }
            | Self::SaveSession { path, .. } => vec![path.as_path()],
            Self::InstallPlugin { folder, .. } => folder.iter().map(|folder| folder.as_path()).collect(),
            Self::LocateRepository { .. } => Vec::new(),
            Self::Hello { .. }
            | Self::ListMembers
            | Self::AddUser { .. }
            | Self::NewFirstAccessCode { .. }
            | Self::RemoveUser { .. }
            | Self::SetUserRole { .. }
            | Self::LoadServerSettings
            | Self::SaveServerSettings { .. }
            | Self::CancelRun { .. }
            | Self::AnswerAsk { .. }
            | Self::DelegateAsk { .. }
            | Self::ListDelegated
            | Self::AnswerDelegated { .. }
            | Self::LoadSharedMemory { .. }
            | Self::ShareMemory { .. }
            | Self::UnshareMemory { .. }
            | Self::Viewing { .. }
            | Self::GetPolicy
            | Self::SetPolicy { .. }
            | Self::ReportUsage { .. }
            | Self::ListUsage
            | Self::LoadComments { .. }
            | Self::AddComment { .. }
            | Self::ListTeams
            | Self::CreateTeam { .. }
            | Self::RemoveTeam { .. }
            | Self::SetTeamMember { .. }
            | Self::ListLibrary
            | Self::PublishToLibrary { .. }
            | Self::RemoveFromLibrary { .. }
            | Self::InstallLibrary { .. }
            | Self::AnswerBrowse { .. }
            | Self::Page(_)
            | Self::SetPermissionMode { .. }
            | Self::ListModels
            | Self::DownloadModel { .. }
            | Self::ListGitHubRepositories
            | Self::ListPlugins
            | Self::ListChannels
            | Self::ApproveDraft { .. }
            | Self::DiscardDraft { .. }
            | Self::Upload { .. }
            | Self::SignInToGitHub
            | Self::SignOutOfGitHub
            | Self::LoadSettings
            | Self::SaveSettings { .. }
            | Self::UninstallPlugin { .. }
            | Self::Connect { .. }
            | Self::Disconnect { .. }
            | Self::ListProviders
            | Self::ConnectProvider { .. }
            | Self::DisconnectProvider { .. }
            | Self::FindLogins { .. }
            | Self::FillLogin { .. }
            | Self::FindPasskeys { .. }
            | Self::SignPasskey { .. }
            | Self::CreatePasskey { .. }
            | Self::SaveLogin { .. } => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{EngineKind, ModelOption, PromptRequest, Reference, RunId};

    use super::*;

    #[test]
    fn a_prompt_names_its_folder_and_every_file_it_brings() {
        let model = ModelOption::new(EngineKind::Native, "claude-opus-5-5", "Opus 5.5");
        let mut request = PromptRequest::plain(PathBuf::from("/srv/a/repo"), "hi", model);
        request.references.push(Reference { path: PathBuf::from("/srv/a/notes.png") });
        let command = ClientCommand::SubmitPrompt { run: RunId::new(), request };
        assert_eq!(command.paths(), vec![Path::new("/srv/a/repo"), Path::new("/srv/a/notes.png")]);
    }

    #[test]
    fn a_plugin_folder_is_named_and_commands_without_paths_name_none() {
        let install = ClientCommand::InstallPlugin { plugin: None, folder: Some(PathBuf::from("/srv/u/p")) };
        assert_eq!(install.paths(), vec![Path::new("/srv/u/p")]);
        assert!(ClientCommand::ListPlugins.paths().is_empty());
    }
}
