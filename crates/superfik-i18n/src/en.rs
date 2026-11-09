//! English: the text Superfik shipped with.

use superfik_core::{
    Activity, AgentRole, AgentStatus, Command, ContextSlice, Effort, EngineKind, EngineProblem, OutputStyle,
};

use crate::text::plural;
use crate::{Catalog, Text};

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

#[derive(Debug)]
pub struct English;

impl Catalog for English {
    fn text(&self, text: Text<'_>) -> String {
        let fixed = match text {
            Text::BrowserProfiles => "Laboratory profiles",
            Text::BrowserProfilesCaption => "No profile: pages see this computer as it is.",
            Text::BrowserProfileNew => "New profile",
            Text::BrowserProfileDefaultName(n) => return format!("Profile {n}"),
            Text::BrowserProfileReal => "Actual values",
            Text::BrowserProfileUse => "Use",
            Text::BrowserProfileEdit => "Edit",
            Text::BrowserProfileDelete => "Delete",
            Text::BrowserProfileSave => "Save profile",
            Text::BrowserProfileCancel => "Cancel",
            Text::BrowserProfileRestart => "Saved profile changes take effect after restarting Superfik.",
            Text::BrowserProfileEnabled => "Profile active",
            Text::BrowserProfileCanvas => "Canvas",
            Text::BrowserProfileAudio => "Audio",
            Text::BrowserProfileHttps => "HTTPS to the proxy",
            Text::BrowserProfileRtcSystem => "System",
            Text::BrowserProfileRtcProxy => "No direct UDP",
            Text::BrowserProfileRtcDisabled => "Off",
            Text::BrowserProfileDefaultsHint => {
                "Empty fields keep this computer's actual values. Separate fonts with commas."
            }
            Text::BrowserProfilePasswordHint => "Keep the saved one",
            Text::BrowserProfileField(field) => match field {
                superfik_core::browser_profiles::ProfileField::Name => "Name",
                superfik_core::browser_profiles::ProfileField::Cpu => "CPU threads",
                superfik_core::browser_profiles::ProfileField::Memory => "Memory (GiB)",
                superfik_core::browser_profiles::ProfileField::Seed => "Seed",
                superfik_core::browser_profiles::ProfileField::GpuVendor => "Vendor",
                superfik_core::browser_profiles::ProfileField::GpuRenderer => "Renderer (WebGL)",
                superfik_core::browser_profiles::ProfileField::GpuArchitecture => "Architecture (WebGPU)",
                superfik_core::browser_profiles::ProfileField::GpuDevice => "Device (WebGPU)",
                superfik_core::browser_profiles::ProfileField::GpuDescription => "Description (WebGPU)",
                superfik_core::browser_profiles::ProfileField::Fonts => "Local fonts",
                superfik_core::browser_profiles::ProfileField::Width => "Width",
                superfik_core::browser_profiles::ProfileField::Height => "Height",
                superfik_core::browser_profiles::ProfileField::Scale => "Scale (DPR)",
                superfik_core::browser_profiles::ProfileField::Depth => "Color depth",
                superfik_core::browser_profiles::ProfileField::ProxyHost => "Host",
                superfik_core::browser_profiles::ProfileField::ProxyPort => "Port",
                superfik_core::browser_profiles::ProfileField::ProxyUsername => "Username",
                superfik_core::browser_profiles::ProfileField::ProxyPassword => "Password",
            },
            Text::BrowserProfileActual => "Actual value",
            Text::BrowserProfileInUse => "In use in this conversation",
            Text::BrowserProfileOff => "Off",
            Text::BrowserProfileAllActual => "Nothing changed yet",
            Text::BrowserProfileNewCaption => "Fixed hardware, screen, GPU and proxy for a conversation.",
            Text::BrowserProfileCreate => "Create",
            Text::BrowserProfileHardware => "Hardware",
            Text::BrowserProfileScreen => "Screen",
            Text::BrowserProfileGpu => "GPU",
            Text::BrowserProfileFingerprint => "Fingerprint",
            Text::BrowserProfileProxy => "Proxy",
            Text::BrowserProfileWebRtc => "WebRTC",
            Text::BrowserProfileSeedCaption => "Keeps canvas and audio stable.",
            Text::SettingsMenu => "Settings…",
            Text::SettingsGeneral => "General",
            Text::SettingsSound => "Sound",
            Text::SettingsPlaySounds => "Play interface sounds",
            Text::SettingsSoundPack => "Sound pack",
            Text::SettingsPulseTrackpad => "Pulse the trackpad",
            Text::SettingsHapticsCaption => "A light tap as you slide the effort meter.",
            Text::SettingsFillOnLoad => "Fill logins as pages load",
            Text::SettingsFillOnLoadCaption => "When one login from your vault fits the page, it goes in by itself.",
            Text::SettingsSounds => "Sounds",
            Text::SettingsHaptics => "Haptic feedback",
            Text::SettingsLanguage => "Language",
            Text::SettingsTheme => "Theme",
            Text::SettingsTitle => "Settings",
            Text::SettingsGeneralCaption => "Language and appearance.",
            Text::SettingsSoundCaption => "How Superfik sounds and feels as you work.",
            Text::SettingsLanguageCaption => "Menus, labels and messages.",
            Text::SettingsThemeCaption => "Follow the system, or stay light or dark.",
            Text::SettingsPlaySoundsCaption => "Short cues as you work.",
            Text::SettingsSoundPackCaption => "The instrument the cues are played on.",
            Text::SettingsVoice => "Voice",
            Text::SettingsVoiceCaption => "Talk to the agent and hear it answer. Everything runs on this computer.",
            Text::SettingsVoiceEnabled => "Turn on voice",
            Text::SettingsVoiceEnabledCaption => "Shows the microphone in the composer.",
            Text::SettingsVoiceChunk => "Response time",
            Text::SettingsVoiceChunkCaption => "Shorter is faster; longer, more accurate.",
            Text::SettingsVoiceSpeaker => "Assistant's voice",
            Text::SettingsWakeEnabled => "Wake word",
            Text::SettingsWakeEnabledCaption => "Listens to the microphone all the time for the word.",
            Text::SettingsWakeWord => "Word",
            Text::SettingsWakeWordCaption => "Say it to start talking to the agent.",
            Text::SettingsVoiceAddress => "What to call you",
            Text::SettingsVoiceAddressCaption => "The agent calls you this when it speaks.",
            Text::VoiceListening => "Listening…",
            Text::VoiceWaiting => "Thinking…",
            Text::VoiceSpeaking => "Speaking…",
            Text::VoicePreparing(percent) => return format!("Downloading the voice models… {percent}%"),
            Text::VoiceNoMicrophone => "Superfik cannot use the microphone. Allow it in System Settings › Privacy.",
            Text::VoiceFailed(why) => return format!("Voice stopped: {why}"),
            Text::SettingsBrowser => "Browser",
            Text::SettingsBrowserCaption => "Laboratory profiles and saved logins for browser cards.",
            Text::AiSubscription => "Subscription",
            Text::AiApiKey => "API key",
            Text::AiLocalSelect => "Select model",
            Text::AiLocalOther => "Other model…",
            Text::AiLocalModels => "Local models",
            Text::AiLocalCaption => "Download MLX models from Hugging Face and run them on this Mac.",
            Text::AiLocalEmpty => "No local models downloaded.",
            Text::AiLocalModelId => "Hugging Face model",
            Text::AiLocalDownload => "Download model",
            Text::AiLocalDownloading => "Downloading…",
            Text::AiLocalPreparing => "Preparing MLX (first time only)…",
            Text::AiLocalFetching(done, total) => {
                return format!(
                    "Downloading… {}% · {} of {}",
                    crate::text::percent(done, total),
                    crate::text::size(done, '.'),
                    crate::text::size(total, '.')
                );
            }
            Text::AiLocalFetched(done) => return format!("Downloading… {}", crate::text::size(done, '.')),
            Text::AiLocalInvalidId => "Enter a Hugging Face model: org/name.",
            Text::AiConnectionMethod => "Connect with a subscription or an API key.",
            Text::SettingsAi => "AI",
            Text::SettingsAiCaption => "Connect your AI accounts and manage local models.",
            Text::SettingsIntegrations => "Integrations",
            Text::SettingsIntegrationsCaption => "Connect the services your agents work with.",
            Text::SettingsVaults => "Vaults",
            Text::SettingsVaultsCaption => "Let agents use passwords and tokens without ever seeing them.",
            Text::SettingsServer => "Server",
            Text::SettingsServerCaption => "Work as a team on a Superfik server, and manage who gets in.",
            Text::OrganizationMembers => "Members",
            Text::ServerLocal => "This computer",
            Text::ServerLocalCaption => "Superfik runs here alone. Connect to your team's server to work together.",
            Text::ServerConnectHeading => "Connect to a server",
            Text::ServerAddress => "Address",
            Text::ServerAddressCaption => {
                "Where your team's server answers. Without http:// or https://, it is https://."
            }
            Text::ServerUsername => "Username",
            Text::ServerPassword => "Password",
            Text::ServerNewPassword => "New password",
            Text::ServerNewPasswordCaption => "10 characters or more",
            Text::ServerCode => "First-access code",
            Text::ServerCodeCaption => "The code an admin gave you",
            Text::ServerConnect => "Connect",
            Text::ServerConnecting => "Connecting…",
            Text::ServerFirstAccess => "First access",
            Text::ServerHavePassword => "I have a password",
            Text::ServerAddressInvalid => {
                "That is not a server address. Write a domain, host:port, or an address that starts with https:// or http://."
            }
            Text::ServerInsecure => {
                "This address is plain http://: your password and sign-in would travel unencrypted. Connect anyway only on a network you trust."
            }
            Text::ServerAllowInsecure => "Connect without encryption",
            Text::ServerTryHttp => {
                "Without a scheme the address is tried over https://. A server set up without a domain speaks http://: write http:// before the address."
            }
            Text::ServerConnectedAs(name, address) => return format!("{name} on {address}"),
            Text::ServerDisconnect => "Disconnect",
            Text::ServerHeading => "Server",
            Text::ServerDomain => "Domain",
            Text::ServerDomainCaption => "Where users reach this server; it applies at once",
            Text::ServerSave => "Save",
            Text::ServerUsers => "Users",
            Text::ServerPending => "first access pending",
            Text::ServerNewCode => "New code",
            Text::ServerMakeAdmin => "Make admin",
            Text::ServerMakeMember => "Make member",
            Text::ServerAddUser => "Add a user",
            Text::ServerAddUserCaption => "They sign in once with the code shown and choose a password",
            Text::ServerAdd => "Add",
            Text::ServerCodeFor(name) => return format!("First-access code for {name}"),
            Text::ServerCodeShownCaption => "Works once, for 7 days. It is shown only now.",
            Text::ServerCopy => "Copy",
            Text::ServerDone => "Done",
            Text::RoleOwner => "Owner",
            Text::RoleAdmin => "Admin",
            Text::RoleMember => "Member",
            Text::RoleGuest => "Guest",
            Text::RemoveMember => "Remove",
            Text::CommentBy(name) => return format!("{name} · comment"),
            Text::Commented(name) => return format!("{name} left a comment."),
            Text::CommentNeedsOrganization => "Comments go on sessions of your organization's projects.",
            Text::SharedWithTeam(team) => return format!("Shared with {team}."),
            Text::NoSuchTeam(team) => return format!("No team is called {team}."),
            Text::TeamsAdminsOnly => "Only owners and admins make teams.",
            Text::OrganizationTeams => "Teams",
            Text::TeamMembers(n) => return plural(n, "member", "members"),
            Text::TeamEdit => "Members",
            Text::TeamDone => "Done",
            Text::TeamWho(team) => return format!("Who is in {team}"),
            Text::TeamHint => "Make one with /team <name>; share a session with /share <team>",
            Text::OrganizationRequired => {
                "Your company set this app up for its server. Connect to it in Settings › Server."
            }
            Text::UnlockReason => "unlock Superfik",
            Text::LockTitle => "Superfik is locked",
            Text::LockCaption => "Unlock to pick up where you left off.",
            Text::UnlockWith(method) => return format!("Unlock with {method}"),
            Text::LockCancelled => "Unlocking was cancelled.",
            Text::LockUnavailable => "This computer has no way to unlock, so the lock was set aside.",
            Text::SettingsLock(method) => return format!("Lock with {method}"),
            Text::SettingsLockCaption(method) => return format!("Asks for {method} every time Superfik opens."),
            Text::UpdateRequiredTitle => "Update required",
            Text::UpdateRequiredCaption(version) => return format!("Superfik {version} is no longer supported."),
            Text::UpdateAvailableTitle => "Update available",
            Text::UpdateAvailableCaption(version) => return format!("Superfik {version} is ready to download."),
            Text::UpdateButton => "Update",
            Text::UpdatingTitle => "Updating Superfik",
            Text::UpdatingCaption => "Your agents keep running the whole time.",
            Text::UpdateChecking => "Checking for updates",
            Text::UpdateStarting => "Starting download",
            Text::UpdateVerifying => "Verifying",
            Text::UpdateInstalling => "Installing",
            Text::UpdatePercent(percent) => return format!("{percent}%"),
            Text::UpdateReadyTitle => "Update ready",
            Text::UpdateReadyCaption => "Restart to finish. It takes a few seconds.",
            Text::UpdateVersion(version) => return format!("Superfik {version}"),
            Text::UpdateRestart => "Restart to update",
            Text::UpdateFailedTitle => "The update did not finish",
            Text::UpdateRetry => "Try again",
            Text::LinkCopied => "Copied",
            Text::SettingsLogins => "Import logins",
            Text::ClearCookies => "Clear cookies",
            Text::ClearCookiesCaption => "Signs the cards out of every site.",
            Text::ClearCookiesButton => "Clear",
            Text::ClearingCookies => "Clearing",
            Text::CookiesCleared => "Cleared",
            Text::LoginsImporting => "Importing",
            Text::LoginsImported => "Imported",
            Text::LoginsImportAgain => "Import again",
            Text::LoginsInUse => "New sessions start with these logins.",
            Text::LoginsImport => "Import",
            Text::LoginsNoBrowser => "No browser to import from",
            Text::LoginsNoBrowserCaption => "Chrome, Dia, Arc, Brave, Edge, Chromium or Vivaldi.",
            Text::ThemeSystem => "System",
            Text::ThemeLight => "Light",
            Text::ThemeDark => "Dark",
            Text::SettingsOutputStyle => "Response style",
            Text::SettingsOutputStyleCaption => "How the agent writes.",
            Text::SettingsSuggestions => "Suggest the next prompt",
            Text::SettingsSuggestionsCaption => "After each reply, in grey; Tab takes it.",
            Text::SettingsSafetyCheck => "Check autonomous actions",
            Text::SettingsSafetyCheckCaption => "Holds back risky edits and commands.",
            Text::SettingsHeadsUp => "You should know",
            Text::SettingsHeadsUpCaption => "Points out what matters after long work.",
            Text::OutputStyleName(style) => match style {
                OutputStyle::Default => "Default",
                OutputStyle::Concise => "Concise",
                OutputStyle::Proactive => "Proactive",
                OutputStyle::Explanatory => "Explanatory",
                OutputStyle::Learning => "Learning",
            },
            Text::NewSession => "New session",
            Text::ArchivedSessions => "Archived",
            Text::AskAllow => "Allow",
            Text::AskAlways => "Always allow",
            Text::AskDeny => "Deny",
            Text::AskNext => "Next",
            Text::AskBack => "Back",
            Text::AskAnswer => "Answer",
            Text::AskSkip => "Skip",
            Text::AskPickMany => "Pick any that apply",
            Text::PermissionAsk => "Ask first",
            Text::PermissionEdits => "Accept edits",
            Text::PermissionPlan => "Plan only",
            Text::PermissionBypass => "Full access",
            Text::AskPermission(tool) => return format!("{tool} needs your permission"),
            Text::AskOf(n, total) => return format!("{n} of {total}"),
            Text::Research => "Research",
            Text::Context => "Context",
            Text::ContextMeasuring => "Measuring…",
            Text::ContextEmpty => "No context yet",
            Text::Memory => "Memory",
            Text::MemoryEpisodic => "Episodic",
            Text::MemorySemantic => "Semantic",
            Text::MemoryProcedural => "Procedural",
            Text::SearchMemory => "Search memory",
            Text::MemoryEmpty => "Nothing remembered yet",
            Text::MemoryNoMatch => "Nothing matches",
            Text::Remember => "Remember",
            Text::RememberPlaceholder => "Something worth remembering…",
            Text::Forget => "Forget",
            Text::OptimizeMemory => "Optimize",
            Text::OptimizingMemory => "Optimizing…",
            Text::UndoOptimize => "Undo",
            Text::CompactContext => "Compact context",
            Text::Compacting => "Compacting…",
            Text::ContextFree(tokens) => return format!("{tokens} free"),
            Text::MemoryActivated(n) => return format!("{n} activated"),
            Text::ComposerEmpty => "Ask anything, or type / for skills, plugins and commands.",
            Text::ComposerFollowUp => "Add a follow-up...",
            Text::References => "References",
            Text::Notes => "Notes",
            Text::NewNote => "New note",
            Text::NotePlaceholder => "Title, then lines; - [ ] makes an item",
            Text::NoteMore(n) => return format!("+{n} more"),
            Text::Skills => "Skills",
            Text::Repository => "Repository",
            Text::OpenRepositoryTitle => "Open a repository",
            Text::OpenRepositoryHint => "Choose a folder to work in",
            Text::AddRepository => "Add repository",
            Text::PullRequests => "Pull requests",
            Text::PullRequest => "Pull request",
            Text::ReviewApproved => "Approved",
            Text::ReviewChangesRequested => "Changes requested",
            Text::ReviewRequired => "Review required",
            Text::OpenPullRequest => "Open pull request",
            Text::PullRequestSheetTitle => "Open a pull request",
            Text::PullRequestTitlePlaceholder => "Title",
            Text::PullRequestBodyPlaceholder => "Describe what changed and why…",
            Text::WriteWithAgent => "Write with agent",
            Text::DraftToggle => "Draft",
            Text::CreatePullRequest => "Create pull request",
            Text::Cancel => "Cancel",
            Text::ReviewBack => "Back to canvas",
            Text::ReviewStacked => "Stacked",
            Text::ReviewSplit => "Split",
            Text::ReviewAddComment => "Comment",
            Text::ReviewAskAgent => "Ask agent",
            Text::ReviewApprove => "Approve",
            Text::ReviewRequestChanges => "Request changes",
            Text::ReviewPublishSheetTitle => "Publish review",
            Text::ReviewPublish => "Publish",
            Text::ReviewSummaryPlaceholder => "Summary (optional)…",
            Text::ReviewPending => "Pending",
            Text::ReviewResolved => "Resolved",
            Text::ReviewAgentBadge => "Agent",
            Text::ReviewSave => "Save",
            Text::ReviewNoFiles => "No changed files",
            Text::ReviewWithAgent => "Review with agent",
            Text::PublishingReview => "Publishing the review…",
            Text::NoLoginForm => "This page has no sign-in form to fill.",
            Text::NotNow => "Not now",
            Text::PasskeyElsewhere => "Use another device",
            Text::PasskeySaved(account) => return format!("Passkey saved to {account}."),
            Text::SavePasskeyTo(account) => return format!("Save passkey to {account}"),
            Text::LoginPageChanged => "The page changed before the login was filled.",
            Text::NoSavedLogin => "No vault login is saved for this page.",
            Text::NoWebPages => "Web pages are not available on this system yet.",
            Text::NoModels => "No models yet. Connect an AI account or install a local model in Settings › AI.",
            Text::CreatingPullRequest => "Opening the pull request…",
            Text::NoPullRequests => "No open pull requests",
            Text::NoSkills => "No skills",
            Text::Commands => "Commands",
            Text::AllSkillsAdded => "All skills added",
            Text::Draft => "Draft",
            Text::OpenRepositoryMenu => "Open repository…",
            Text::ChooseRepositoryMenu => "Choose repository…",
            Text::OpenPullRequestMenu => "Open pull request…",
            Text::GitHubSignIn => "Sign in with `gh auth login` to list GitHub",
            Text::GitHubSignInHere => "Sign in with GitHub",
            Text::GitHubCode(code) => return format!("Type {code} at GitHub to sign in."),
            Text::Private => "Private",
            Text::NotInstalled => "Not installed",
            Text::Soon => "Soon",
            Text::ChooseRepositoryToStart => "Choose a repository to start.",
            Text::NothingToCompact => "Nothing to compact right now.",
            Text::NothingToAsk => "Nothing was said in this session yet; ask the agent directly.",
            Text::Recap(text) => return format!("Where you left off: {text}"),
            Text::HeadsUp { urgent, title, learn, explain } => {
                let tag = if urgent { "Heads up" } else { "You should know" };
                return format!("{tag} · {title}\n{learn}\n\n{explain}");
            }
            Text::BusySwitch => "Wait for the running sessions to finish before switching branches.",
            Text::DialogOpenRepository => "Open a repository",
            Text::DialogAddReferences => "Add references",
            Text::DialogAttachRepository => "Attach a repository",
            Text::ImagesFilter => "Images",
            Text::Plugins => "Plugins",
            Text::Connections => "Plugins",
            Text::InstallPlugin => "Install",
            Text::UninstallPlugin => "Remove plugin",
            Text::AddAccount => "Add account",
            Text::AccountNotUsed => "Not used",
            Text::SignInWithBrowser => "Sign in with the browser",
            Text::Browser => "Browser",
            Text::BrowserAddress => "Search or type an address",
            Text::BrowserAgent => "Agent",
            Text::RoutinesTitle => "Routines",
            Text::RoutinesCaption => {
                "Agents that work on their own, on a schedule, and report back here. On this computer they run \
                 while Superfik is open; on a server, around the clock."
            }
            Text::RoutinesHeading => "Routines",
            Text::RoutinesNone => "No routines yet",
            Text::RoutineNew => "New routine",
            Text::RoutineRunNow => "Run now",
            Text::RoutinePause => "Pause",
            Text::RoutineResume => "Resume",
            Text::RoutineEdit => "Edit",
            Text::RoutineDelete => "Delete",
            Text::RoutineName => "Name",
            Text::RoutineNamePlaceholder => "Morning triage",
            Text::RoutineBrief => "What to do",
            Text::RoutineBriefPlaceholder => "Look at the new Sentry issues and propose a fix for each.",
            Text::RoutineWhen => "When",
            Text::CadenceHourly => "Hourly",
            Text::CadenceDaily => "Daily",
            Text::CadenceWeekly => "Weekly",
            Text::RoutineAutonomy => "Autonomy",
            Text::AutonomyDraft => "Propose",
            Text::AutonomyBounded => "Edit files",
            Text::AutonomyAuto => "Act alone",
            Text::RoutineSave => "Save",
            Text::RoutineCancel => "Cancel",
            Text::RoutineEvery(60) => "Every hour",
            Text::RoutineEvery(n) => return format!("Every {n} minutes"),
            Text::RoutineDaily(time) => return format!("Every day at {time}"),
            Text::RoutineWeekly(day, time) => return format!("Every {} at {time}", self.weekday(day)),
            Text::RoutinePaused => "Paused",
            Text::ReportsHeading => "Reports",
            Text::ReportsNone => "Nothing has run yet",
            Text::ReportRunning => "Running",
            Text::ReportFailed => "Failed",
            Text::ReportStopped => "Stopped",
            Text::ReportNeedsInput => "Needs input",
            Text::ReportContinue => "Continue in a session",
            Text::ReportBranch(branch) => return format!("On branch {branch}"),
            Text::RoutineStartFrom => "Start from",
            Text::RoutineAccounts => "Accounts it may use",
            Text::RoutineAccountsNone => "Connect Sentry, Linear, GitHub or Stripe in Settings › Integrations.",
            Text::TemplateProductionUpkeep => "Production upkeep",
            Text::TemplateReleaseNotes => "Release notes",
            Text::TemplateRevenueDigest => "Revenue digest",
            Text::TemplateCustomerSupport => "Customer support",
            Text::PairWithQr => "Pair with QR code",
            Text::ChannelOpening => "Connecting…",
            Text::ChannelPairing => "On the phone: WhatsApp › Linked devices › Link a device, then scan.",
            Text::ChannelLinked(account) => return format!("Linked as {account}"),
            Text::ChannelLoggedOut => "Unlinked from the phone. Remove it and add it again to pair.",
            Text::ChannelFailed(reason) => return format!("Could not connect: {reason}"),
            Text::ChannelWebhook(path) => return format!("Webhook: your server's address + {path}"),
            Text::CopyWebhook => "Copy",
            Text::DraftsHeading => "Waiting for approval",
            Text::DraftSend => "Send",
            Text::DraftDiscard => "Discard",
            Text::RoutineAnswers => "Answer messages",
            Text::RoutineAnswersDetail => "Runs as soon as one of its channel accounts gets a message",
            Text::RoutineWebhook => "Webhook",
            Text::RoutineWebhookDetail => "Runs when another service calls its address on your server",
            Text::RoutineWebhookCall => "Call it",
            Text::RoutineCopyCall => "Copy call",
            Text::UseCliLogin => "Use the signed-in CLI",
            Text::AccountName => "Account name",
            Text::AccountNamePlaceholder => "work, personal…",
            Text::Connect => "Connect",
            Text::Disconnect => "Disconnect",
            Text::NoAccounts => "No accounts connected",
            Text::PluginAccounts => "Accounts",
            Text::AddPluginFolder => "Add from folder…",
            Text::DialogAddPlugin => "Choose a plugin folder",
            Text::SigningIn => "Waiting for the browser…",
            Text::PluginReady => "Ready: pick it with / in the composer",
            Text::StoreInstalled => "Installed",
            Text::CategoryDevelopment => "Developer Tools",
            Text::CategoryProductivity => "Productivity",
            Text::CategoryData => "Data & Payments",
            Text::CategoryWeb => "Web & Browser",
            Text::CategoryOther => "More",
            Text::WelcomeAboard => "Welcome aboard",
            Text::GetStarted => "Get Started",
            Text::GetStartedCanvas => "Context on a canvas",
            Text::GetStartedCanvasBody => "See everything the agent sees, around the composer.",
            Text::GetStartedAgents => "Agents you can watch",
            Text::GetStartedAgentsBody => "Follow each sub-agent live, step by step.",
            Text::GetStartedMemory => "Memory that stays",
            Text::GetStartedMemoryBody => "Each project remembered across sessions.",
            Text::GetStartedReview => "Review without leaving",
            Text::GetStartedReviewBody => "Read diffs and review pull requests, with or without an agent.",
            Text::Continue => "Continue",
            Text::AgentTitle(n) => return format!("Agent {n}"),
            Text::Steps(n) => return plural(n, "step", "steps"),
            Text::ReadingFile(path) => return format!("Reading {path}"),
            Text::SaveLoginTo(account) => return format!("Save to {account}"),
            Text::LoginSaved(account) => return format!("Login saved to {account}."),
            Text::ReadFiles(n) => return format!("Read {}", plural(n, "file", "files")),
            Text::UsageLimitResets(time) => return format!("Usage limit reached · resets at {time}"),
            Text::RetryTurn => "Try again",
            Text::Recentre => "Centre",
            Text::UsageLimitReached => "Usage limit reached",
            Text::ServiceUnavailable => "The service is unavailable right now",
            Text::OpeningRepository(slug) => return format!("Opening {slug}…"),
            Text::ContinuingShared(owner) => return format!("Continuing {owner}'s session in a copy of your own."),
            Text::SharedWithYou => "Shared with you",
            Text::AskApprover => "Ask an admin",
            Text::WaitingForApprover => "Waiting for an admin. You can still answer.",
            Text::DelegatedAllowed(by) => return format!("{by} allowed it."),
            Text::DelegatedDenied(by) => return format!("{by} denied it."),
            Text::ApprovalWaiting(name) => {
                return format!("{name} is waiting for an approval in Settings › Server.");
            }
            Text::WaitingForYourApproval => "Waiting for your approval",
            Text::MemoryShared => "Shared with everyone in this project. Their agents will recall it.",
            Text::PublishHow => "Name a skill or agent of yours: /publish <name>.",
            Text::Reconnecting => "Lost the connection to the server. Reconnecting…",
            Text::ServerSignedOut => "The server no longer accepts this sign-in. Sign in again.",
            Text::PublishNotAllowed => "Only owners and admins publish to the organization's library.",
            Text::Published(name) => return format!("Publishing {name} to the organization's library…"),
            Text::OrganizationLibrary => "Library",
            Text::Here(names) => return format!("{names} here"),
            Text::PolicyUnpriced(model) => {
                return format!(
                    "The cost of {model} is unknown. Choose a model with a reviewed tariff to use the organization budget."
                );
            }
            Text::PolicyModel(model) => {
                return format!("Your organization does not allow {model}. Pick another model.");
            }
            Text::PolicyMode(mode) => return format!("Your organization allows at most “{mode}”."),
            Text::PolicyBudget(n) => {
                return format!("You reached this month's budget of ${n}. An admin can raise it.");
            }
            Text::OrganizationPolicy => "Policy",
            Text::PolicyMaxMode => "Most an agent may do alone",
            Text::PolicyMaxModeCaption => "No member picks a looser mode",
            Text::PolicyAnyMode => "Any",
            Text::PolicyBudgetTitle => "Monthly budget per member",
            Text::PolicyBudgetCaption => "Runs stop once a member spends it",
            Text::PolicyUnlimited => "None",
            Text::PolicyModelCaption => "Allowed for members",
            Text::UsageThisMonth => "Spent this month",
            Text::UsageLine(dollars, runs) => return format!("{dollars} · {}", plural(runs, "run", "runs")),
            Text::LibrarySkillBy(name) => return format!("Skill · published by {name}"),
            Text::LibraryAgentBy(name) => return format!("Agent · published by {name}"),
            Text::CheckingOutPullRequest(n) => return format!("Checking out #{n}…"),
            Text::LoadingPullRequest(n) => return format!("Loading #{n}…"),
            Text::ReviewPublished(n) => return format!("Review published on #{n}."),
            Text::AgentReviewReady(n) => {
                return format!("The agent left {} to approve.", plural(n, "comment", "comments"));
            }
        };
        fixed.to_owned()
    }

    fn activity(&self, activity: &Activity, elapsed: &str) -> String {
        match activity {
            Activity::Thinking => "Thinking".into(),
            Activity::SearchingWeb => "Searching the web".into(),
            Activity::Reading => "Reading files".into(),
            Activity::Writing => "Writing".into(),
            Activity::Done => format!("Worked for {elapsed}"),
            Activity::Failed(_) => "Stopped with an error".into(),
            Activity::Cancelled => "Stopped".into(),
        }
    }

    fn today(&self) -> &'static str {
        "Today"
    }

    fn yesterday(&self) -> &'static str {
        "Yesterday"
    }

    fn weekday(&self, index: usize) -> &'static str {
        WEEKDAYS[index % 7]
    }

    fn date(&self, month: u32, day: u32, year: Option<i64>) -> String {
        let month = MONTHS[(month.clamp(1, 12) - 1) as usize];
        match year {
            Some(year) => format!("{month} {day}, {year}"),
            None => format!("{month} {day}"),
        }
    }

    fn effort(&self, effort: Effort) -> &'static str {
        match effort {
            Effort::Low => "Low",
            Effort::Medium => "Medium",
            Effort::High => "High",
            Effort::XHigh => "Extra",
            Effort::Max => "Ultracode",
        }
    }

    fn command(&self, command: Command) -> &'static str {
        match command {
            Command::New | Command::Clear => "Start a new session",
            Command::Context => "Show the context window",
            Command::Memory => "Show the project's memory",
            Command::Notes => "Show the project's notes",
            Command::Skills => "Show the skills",
            Command::Plugins => "Show the plugins",
            Command::Commands => "Show this list",
            Command::Repository => "Choose the repository",
            Command::Model => "Choose the model",
            Command::Compact => "Compact the context",
            Command::Mode => "Choose the permission mode",
            Command::Upload => "Attach files",
            Command::Effort(_) => "Set the effort",
            Command::Browser => "Open or close the browser",
            Command::Share => "Share the session with your team, or make it private",
            Command::Publish => "Publish a skill or agent to your organization",
            Command::Voice => "Talk to the agent and hear it answer",
            Command::Btw => "Ask a side question without stopping the agent",
            Command::Rename => "Name the session from its conversation, or as you type",
        }
    }

    fn role(&self, role: AgentRole) -> &'static str {
        match role {
            AgentRole::Explorer => "Explorer",
            AgentRole::Builder => "Builder",
            AgentRole::Reviewer => "Reviewer",
            AgentRole::Tester => "Tester",
        }
    }

    fn idle_summary(&self, role: AgentRole) -> &'static str {
        match role {
            AgentRole::Explorer => "Mapping the codebase",
            AgentRole::Builder => "Drafting the change",
            AgentRole::Reviewer => "Checking the diff",
            AgentRole::Tester => "Running the checks",
        }
    }

    fn context_slice(&self, slice: &ContextSlice) -> Option<&'static str> {
        Some(match slice {
            ContextSlice::SystemPrompt => "System prompt",
            ContextSlice::Tools => "Tools",
            ContextSlice::Mcp => "MCP",
            ContextSlice::Agents => "Agents",
            ContextSlice::Memory => "Memory files",
            ContextSlice::Skills => "Skills",
            ContextSlice::Messages => "Messages",
            ContextSlice::Other(_) => return None,
        })
    }

    fn waiting_turn(&self) -> &'static str {
        "Waiting its turn"
    }

    fn status(&self, status: &AgentStatus) -> Option<&'static str> {
        match status {
            AgentStatus::Ready => Some("Ready"),
            AgentStatus::Queued => Some("Queued"),
            AgentStatus::Failed => Some("Failed"),
            AgentStatus::Running { .. } | AgentStatus::Done { .. } => None,
        }
    }

    fn step_verb(&self, verb: &str) -> Option<&'static str> {
        Some(match verb {
            "Read" => "Read",
            "Edit" => "Edit",
            "Write" => "Write",
            "Search" => "Search",
            "Fetch" => "Fetch",
            "Agent" => "Agent",
            "Skill" => "Skill",
            "Plan" => "Plan",
            "Replied" => "Replied",
            _ => return None,
        })
    }

    fn engine_problem(&self, problem: EngineProblem, engine: EngineKind) -> String {
        let name = engine.label();
        match (problem, engine) {
            (EngineProblem::NotInstalled, EngineKind::Native) => {
                "The built-in AI engine is unavailable. Restart Superfik and try again.".to_owned()
            }
            (EngineProblem::NotInstalled, _) => format!("{name} isn't installed on this machine."),
            (EngineProblem::NotLoggedIn, EngineKind::Native) => {
                "Connect an AI account in Settings → AI and try again.".to_owned()
            }
            (EngineProblem::NotLoggedIn, _) => format!("{name} isn't logged in."),
            (EngineProblem::SessionNotFound, _) => {
                format!("The previous {name} conversation was not found; your next message starts a new one.")
            }
        }
    }
}
