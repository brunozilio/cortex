//! Scripted sessions used by snapshots and visual checks.

use std::path::PathBuf;

use superfik_core::{
    Activity, AgentStatus, Effort, EngineKind, EngineProblem, GitHubRepository, PullRequest, PullRequestState,
    Repository, ResearchItem, ResearchKind, ReviewDecision, RunId, Skill, SkillIcon, SubAgent, ToolCall, Turn,
    TurnResponse, UserMessage, WorkspaceSnapshot,
};

use crate::state::{Action, ActiveRun, Notice, PopoverKind, Session, UpdateStageReport};

pub type SyntheticImage = (PathBuf, Vec<u8>, u32);

pub fn by_name(name: &str) -> Option<(Session, Vec<SyntheticImage>)> {
    match name {
        site if site.starts_with("site-") => super::scenario_site::by_name(site),
        "empty" => Some((Session::new(Some(PathBuf::from("/tmp"))), Vec::new())),
        "get-started" => Some((get_started(), Vec::new())),
        "locked" => Some((locked(), Vec::new())),
        "update-required" => Some((update(true, None), Vec::new())),
        "updating" => Some((update(true, Some(UpdateStageReport::Downloading { per_mille: 520 })), Vec::new())),
        "update-ready" => Some((update_ready(), Vec::new())),
        "reference" => Some(reference()),
        "references-open" => Some(references_open()),
        "add-skill" => Some(add_skill()),
        "commands" => Some(commands()),
        "context-empty" => Some(context_empty()),
        "slash-picker" => Some(slash_picker()),
        "slash-many" => Some(slash_many()),
        "skills-many" => Some(skills_many()),
        "alert" => Some(alert()),
        "done" => Some(done()),
        "question" => Some(question()),
        "overflow" => Some(overflow()),
        "markdown" => Some(markdown()),
        "cjk" => Some(cjk()),
        "visual" => Some(super::scenario_visual::visual(true)),
        "visual-drawing" => Some(super::scenario_visual::visual(false)),
        "github" => Some(github()),
        "repositories" => Some(repositories()),
        "pull-requests" => Some(pull_requests()),
        "pull-request" => Some(pull_request()),
        "pr-compose" => Some(pr_compose()),
        "review" => Some(review()),
        "review-publish" => Some(review_publish()),
        "sessions" => Some(sessions()),
        "settings" => Some(settings(crate::state::SettingsPane::General)),
        "settings-sound" => Some(settings(crate::state::SettingsPane::Sound)),
        "settings-voice" => Some(settings(crate::state::SettingsPane::Voice)),
        "settings-voice-wake" => Some(settings_wake()),
        "voice" => Some((voice(), Vec::new())),
        "settings-server" => Some(organization()),
        "settings-governance" => Some(super::scenario_org::governance()),
        "settings-server-connect" => Some(super::scenario_org::connect(false)),
        "settings-server-first-access" => Some(super::scenario_org::connect(true)),
        "settings-server-users" => Some(super::scenario_org::users()),
        "claude" => Some(super::scenario_claude::claude()),
        "claude-models" => Some(super::scenario_claude::claude_picker()),
        "claude-agent" => Some(super::scenario_claude::claude_agent()),
        "context" => Some(super::scenario_context::context()),
        "compacted" => Some(super::scenario_context::compacted()),
        "memory" => Some(super::scenario_memory::memory()),
        "memory-search" => Some(super::scenario_memory::memory_search()),
        "memory-open" => Some(super::scenario_memory::memory_open()),
        "memory-edit" => Some(super::scenario_memory::memory_edit()),
        "notes" => Some(super::scenario_notes::notes()),
        "notes-edit" => Some(super::scenario_notes::notes_edit()),
        "notes-pick" => Some(super::scenario_notes::notes_pick()),
        "routines" => Some(super::scenario_routines::routines()),
        "routines-new" => Some(super::scenario_routines::routines_new()),
        "routines-draft" => Some(super::scenario_routines::routines_draft()),
        "routines-webhook" => Some(super::scenario_routines::routines_webhook()),
        "permission" => Some(super::scenario_ask::permission()),
        "permission-queue" => Some(super::scenario_ask::permission_queue()),
        "question-choice" => Some(super::scenario_ask::question_choice()),
        "waiting-sessions" => Some(super::scenario_ask::waiting_sessions()),
        "full-access" => Some(super::scenario_ask::full_access()),
        "permission-menu" => Some(super::scenario_ask::permission_menu()),
        "settings-ai" => Some((super::scenario_plugins::ai(false), Vec::new())),
        "settings-ai-local-picker" => {
            let mut s = super::scenario_plugins::ai_local();
            s.plugins.panel.local_picker = true;
            Some((s, Vec::new()))
        }
        "settings-ai-local" => Some((super::scenario_plugins::ai_local(), Vec::new())),
        "settings-ai-connect" => Some((super::scenario_plugins::ai(true), Vec::new())),
        "settings-integrations" => Some(super::scenario_plugins::plugins()),
        "settings-integrations-open" => Some(super::scenario_plugins::plugin_page()),
        "settings-integrations-connect" => Some(super::scenario_plugins::plugins_connect()),
        "settings-whatsapp" => Some(super::scenario_plugins::whatsapp()),
        "settings-whatsapp-cloud" => Some(super::scenario_channels::whatsapp_cloud()),
        "settings-instagram" => Some(super::scenario_channels::instagram()),
        "settings-x" => Some(super::scenario_channels::x()),
        "settings-linkedin" => Some(super::scenario_channels::linkedin()),
        "settings-vaults" => Some(super::scenario_plugins::vaults()),
        "connections" => Some(super::scenario_plugins::connections()),
        "plugins-node" => Some(super::scenario_plugins::plugins_node()),
        "plugin-accounts" => Some(super::scenario_plugins::plugin_accounts()),
        "plugins-many" => Some(super::scenario_plugins::plugins_many()),
        "browser" => Some(super::scenario_browser::browser()),
        "browser-passkey" => Some(super::scenario_browser::browser_passkey()),
        "browser-editing" => Some(super::scenario_browser::browser_editing()),
        "browser-loading" => Some(super::scenario_browser::browser_loading()),
        "browser-autofill" => Some(super::scenario_browser::browser_autofill()),
        "browser-save-login" => Some(super::scenario_browser::browser_save_login()),
        "settings-browser" => Some(super::scenario_browser::settings_browser()),
        "settings-browser-profiles" => Some(super::scenario_profiles::profiles(false)),
        "settings-browser-profile-editor" => Some(super::scenario_profiles::profiles(true)),
        _ => None,
    }
}

fn get_started() -> Session {
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.apply(Action::ShowGetStarted, 0.0);
    s
}

fn locked() -> Session {
    use crate::state::{LockAction, LockProblem};
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.apply(Action::Lock(LockAction::Locked), 0.0);
    s.apply(Action::Lock(LockAction::Failed(LockProblem::Cancelled)), 0.0);
    s
}

fn update(required: bool, stage: Option<UpdateStageReport>) -> Session {
    use crate::state::{UpdateAction, UpdateOffer};
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    let offer = UpdateOffer { version: "0.2.0".into(), required };
    s.apply(Action::Update(UpdateAction::Offered(Some(offer))), 0.0);
    if let Some(stage) = stage {
        s.apply(Action::Update(UpdateAction::Primary), 0.0);
        s.apply(Action::Update(UpdateAction::Stage(stage)), 0.0);
    }
    s
}

fn update_ready() -> Session {
    use crate::state::UpdateAction;
    let mut s = update(false, None);
    s.apply(Action::Update(UpdateAction::Ready), 0.0);
    s.apply(Action::Update(UpdateAction::Open), 0.0);
    s
}

fn voice() -> Session {
    use crate::state::VoiceAction;
    use superfik_voice::VoiceEvent;
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.apply(Action::Voice(VoiceAction::Toggle), 0.0);
    s.apply(Action::Voice(VoiceAction::Device(VoiceEvent::Ready)), 0.0);
    let heard = VoiceEvent::Heard { text: "Quero criar o agente de voz".into(), ended: false };
    s.apply(Action::Voice(VoiceAction::Device(heard)), 0.0);
    s
}

fn settings(pane: crate::state::SettingsPane) -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.apply(Action::OpenSettings, 0.0);
    s.apply(Action::ShowSettingsPane(pane), 0.0);
    (s, images)
}

fn settings_wake() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = settings(crate::state::SettingsPane::Voice);
    s.apply(Action::ChooseSetting(crate::state::Setting::WakeEnabled(true)), 0.0);
    s.apply(Action::Voice(crate::state::VoiceAction::Device(superfik_voice::VoiceEvent::Ready)), 0.0);
    s.apply(Action::VoiceField(crate::state::VoiceFieldAction::Focus(crate::state::VoiceField::WakeWord)), 0.0);
    (s, images)
}

fn organization() -> (Session, Vec<SyntheticImage>) {
    use superfik_core::access::{Colleague, OrgRole};
    use superfik_core::{Member, OrgId, Principal, UserId};
    use superfik_protocol::ServerEvent;
    let (mut s, images) = reference();
    let me = Member { user: UserId::new(), org: OrgId::new() };
    let welcome =
        ServerEvent::Welcome { version: superfik_protocol::PROTOCOL_VERSION, principal: Principal::Member(me) };
    s.apply(Action::Server(welcome), 0.0);
    let colleague = |user, name: &str, role, pending| Colleague { user, name: name.into(), role, pending };
    s.apply(
        Action::Server(ServerEvent::Members(vec![
            colleague(UserId::new(), "ana", OrgRole::Admin, false),
            colleague(UserId::new(), "bia", OrgRole::Guest, true),
            colleague(UserId::new(), "caio", OrgRole::Member, false),
            colleague(me.user, "olga", OrgRole::Member, false),
        ])),
        0.0,
    );
    let joined = crate::state::Joined { address: "superfik.acme.io".into(), name: "olga".into() };
    s.apply(Action::ServerLink(crate::state::ServerAction::Joined(Ok(joined))), 0.0);
    s.apply(Action::OpenSettings, 0.0);
    s.apply(Action::ShowSettingsPane(crate::state::SettingsPane::Server), 0.0);
    (s, images)
}

fn github() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    if let Some(repo) = s.snapshot.repository.as_mut() {
        repo.branch = "prototypes".into();
        repo.remote_branches = vec!["night-sky".into()];
        repo.pull_request = Some(PullRequest {
            number: 42,
            title: "Night sky".into(),
            author: "zzzzshawn".into(),
            head_branch: "night-sky".into(),
            base_branch: "main".into(),
            additions: 120,
            deletions: 14,
            changed_files: 5,
            ..PullRequest::default()
        });
    }
    let c = s.conversation_mut();
    c.run = None;
    if let Some(turn) = c.turns.last_mut() {
        turn.response.activity = Activity::Done;
        turn.response.elapsed_ms = 26_000;
    }
    s.branch = Some("prototypes".into());
    s.popover = Some(PopoverKind::Branch);
    (s, images)
}

fn open_pull_requests() -> Vec<PullRequest> {
    let pr = |number, title: &str, author: &str, head: &str, decision| PullRequest {
        number,
        title: title.into(),
        author: author.into(),
        head_branch: head.into(),
        base_branch: "main".into(),
        additions: 120,
        deletions: 14,
        changed_files: 5,
        review_decision: decision,
        ..PullRequest::default()
    };
    vec![
        pr(42, "Night sky", "zzzzshawn", "night-sky", Some(ReviewDecision::ReviewRequired)),
        pr(41, "Dither the moon", "ana", "moon", Some(ReviewDecision::Approved)),
        PullRequest { state: PullRequestState::Draft, ..pr(40, "Star field", "zzzzshawn", "stars", None) },
        pr(38, "Haze layers", "ana", "haze", Some(ReviewDecision::ChangesRequested)),
    ]
}

fn pull_requests() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.pull_requests = open_pull_requests();
    s.popover = Some(PopoverKind::PullRequests);
    (s, images)
}

fn pr_compose() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.apply(crate::state::Action::ProposePullRequest, 0.0);
    if let Some(composer) = s.pr_composer.as_mut() {
        composer.title.set("Night sky scene from dither minis");
        composer.body.set("Adds a small night sky built from the dither minis: a dithered gradient sky, a soft moon and layered ridges.\n\nTested with the reference snapshots.");
        composer.focus = Some(crate::state::PrField::Body);
    }
    (s, images)
}

const REVIEW_PATCH: &str = "diff --git a/src/sky.rs b/src/sky.rs\n--- a/src/sky.rs\n+++ b/src/sky.rs\n@@ -12,9 +12,11 @@ impl Sky {\n     pub fn paint(&self, p: &mut Painter) {\n         let gradient = self.gradient();\n-        let moon = Moon::new(0.5);\n-        moon.paint(p, 1.0);\n+        let moon = Moon::dithered(0.5, self.seed);\n+        moon.paint(p, self.glow);\n+        for ridge in &self.ridges {\n+            ridge.paint(p, gradient.horizon());\n+        }\n         p.fill(gradient);\n     }\n }\ndiff --git a/src/moon.rs b/src/moon.rs\n--- a/src/moon.rs\n+++ b/src/moon.rs\n@@ -1,4 +1,7 @@\n pub struct Moon {\n     radius: f32,\n+    seed: u32,\n+    glow: f32,\n }\n+\n diff --git a/assets/ridges.png b/assets/ridges.png\nnew file mode 100644\nBinary files /dev/null and b/assets/ridges.png differ\n";

fn review() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = pull_request();
    s.apply(crate::state::Action::OpenReview, 0.0);
    let pr = s.conversation().pull_request.clone().unwrap_or_default();
    let detail = superfik_core::PullRequestDetail {
        pull_request: pr,
        description: "Adds a dithered moon and layered ridges.".into(),
        files: superfik_core::diff::parse(REVIEW_PATCH),
        comments: vec![superfik_core::ReviewComment {
            id: 900,
            path: "src/sky.rs".into(),
            line: 16,
            side: superfik_core::CommentSide::New,
            body: "Should the glow come from the palette instead?".into(),
            author: "ana".into(),
            origin: superfik_core::CommentOrigin::User,
            state: superfik_core::CommentState::Published,
            thread: Some("T1".into()),
        }],
    };
    s.apply(crate::state::Action::Server(superfik_protocol::ServerEvent::PullRequestLoaded(Box::new(detail))), 0.0);
    if let Some(review) = s.review.as_mut() {
        review.add_agent_drafts(vec![superfik_core::ReviewCommentDraft {
            path: "src/sky.rs".into(),
            line: 18,
            side: superfik_core::CommentSide::New,
            body: "Ridges are painted after the moon, so a ridge can cover it. Paint them first, or clip the moon to the sky above the horizon.".into(),
        }]);
    }
    s.apply(
        crate::state::Action::SelectDiffLine { side: superfik_core::CommentSide::New, line: 14, extend: false },
        0.0,
    );
    s.apply(
        crate::state::Action::SelectDiffLine { side: superfik_core::CommentSide::New, line: 15, extend: true },
        0.0,
    );
    s.apply(crate::state::Action::AddReviewComment, 0.0);
    s.apply(
        crate::state::Action::Edit(crate::state::EditKey::Insert(
            "Nice. Does the seed need to be stable across frames?".into(),
        )),
        0.0,
    );
    s.apply(crate::state::Action::SaveReviewComment, 0.0);
    s.notice = None;
    (s, images)
}

fn review_publish() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = review();
    s.apply(crate::state::Action::BeginPublishReview(superfik_core::ReviewVerdict::Approve), 0.0);
    s.apply(
        crate::state::Action::Edit(crate::state::EditKey::Insert("Looks good, one question on the seed.".into())),
        0.0,
    );
    (s, images)
}

fn pull_request() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.pull_requests = open_pull_requests();
    let pr = s.pull_requests[0].clone();
    if let Some(repo) = s.snapshot.repository.as_mut() {
        repo.branch = pr.head_branch.clone();
        repo.pull_request = Some(pr.clone());
    }
    s.branch = Some(pr.head_branch.clone());
    s.conversation_mut().pull_request = Some(pr);
    (s, images)
}

fn repositories() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.github = Some(crate::state::GitHubListing {
        signed_in: true,
        repositories: ["zzzzshawn/dither-minis", "zzzzshawn/dither-engine", "zzzzshawn/site", "acme/rocket"]
            .iter()
            .enumerate()
            .map(|(i, slug)| GitHubRepository { slug: (*slug).to_owned(), private: i % 2 == 1 })
            .collect(),
    });
    s.recent_workspaces = vec!["/reference".into(), "/code/scratch".into()];
    s.popover = Some(PopoverKind::Repository);
    (s, images)
}

pub(super) fn sessions() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.clock = crate::state::WallClock { unix_at_zero: 1_793_282_400, utc_offset_secs: 3600 };
    let started = s.clock.unix_at_zero;
    let model = s.conversation().model;
    let effort = s.conversation().effort;
    s.conversation_mut().started_unix = Some(started - 300);
    let done = |title: &str, started_unix: i64, running: bool| {
        let mut c = crate::state::Conversation::new(model, effort);
        c.title = Some(title.into());
        c.started_unix = Some(started_unix);
        let mut turn = Turn::new(UserMessage { text: title.into(), images: Vec::new(), branch_caption: String::new() });
        turn.response = TurnResponse {
            activity: Activity::Done,
            text: "Done.".into(),
            tools: reads(2),
            elapsed_ms: 9_000,
            failure: None,
            usage: Vec::new(),
            structured: None,
        };
        c.turns.push(turn);
        if running {
            c.run = Some(ActiveRun { id: RunId::new(), started_at: -4.0 });
        }
        c
    };
    let yesterday = started - 86_400 + 3600;
    let mut others = vec![
        done("Researched ETF investing strategies", yesterday, false),
        done("Drafted a polite client email", yesterday + 1200, false),
        done("Asked for a high-protein meal plan", started - 7200, false),
        done("Brainstormed side projects", started - 600, true),
    ];
    others.append(&mut s.conversations);
    s.conversations = others;
    s.collapsed_days.insert(superfik_core::days::local_day(yesterday, s.clock.utc_offset_secs));
    (s, images)
}

fn alert() -> (Session, Vec<SyntheticImage>) {
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.notice = Some(Notice::Engine(EngineProblem::NotInstalled, EngineKind::Native));
    (s, Vec::new())
}

pub(super) fn done() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    let c = s.conversation_mut();
    c.run = None;
    c.finished_at = Some(0.0);
    if let Some(turn) = c.turns.last_mut() {
        turn.response.activity = Activity::Done;
        turn.response.elapsed_ms = 17_000;
    }
    (s, images)
}

fn question() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = done();
    let c = s.conversation_mut();
    if let Some(turn) = c.turns.last_mut() {
        turn.response.text = format!("{}\n\nShould the moon sit on the left or the right?", turn.response.text);
    }
    (s, images)
}

fn markdown() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = done();
    let c = s.conversation_mut();
    if let Some(turn) = c.turns.last_mut() {
        turn.response.text = "## Plan\nRun **`cargo test`** first, then *check* the [docs](https://example.com).\n\n\
            ```sh\ncargo test --workspace\ncargo clippy\n```\n\n- A **bold** bullet\n- Plain `code` bullet\n\n\
            | Step | Command | Cost |\n|---|---|---|\n| Build | `cargo build` | **low** |\n| Ship | see the [docs](https://example.com) for the long release checklist | high |"
            .into();
    }
    c.composer.set("Please **bold** this, run `cargo build` and read [the docs](https://example.com).");
    (s, images)
}

fn cjk() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = done();
    let c = s.conversation_mut();
    c.title = Some("日本語 中文 한국어".into());
    if let Some(turn) = c.turns.last_mut() {
        turn.response.text = "日本語 中文 한국어\n\nこんにちは、世界。你好，世界。안녕하세요, 세계.\n\n```\nlet greeting = \"你好\";\n```".into();
    }
    c.composer.set("日本語 中文 한국어 — mixed with English");
    (s, images)
}

fn overflow() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    let text = "Delegue um subagent para listar os ".repeat(5) + "commits de hoje.";
    s.conversation_mut().composer.set(text);
    (s, images)
}

fn references_open() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    let c = s.conversation_mut();
    c.references.push(PathBuf::from("/reference/notes.md"));
    c.expanded_reference = Some(1);
    (s, images)
}

fn context_empty() -> (Session, Vec<SyntheticImage>) {
    let mut s = Session::new(Some(PathBuf::from("/tmp")));
    s.nodes.context = true;
    (s, Vec::new())
}

fn commands() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.nodes.commands = true;
    s.conversation_mut().pages.insert(crate::state::Carousel::Commands, 1);
    (s, images)
}

fn slash_picker() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.apply(Action::Edit(crate::state::EditKey::Insert("/".into())), 0.0);
    (s, images)
}

fn slash_many() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = skills_many();
    s.apply(Action::Edit(crate::state::EditKey::Insert("/skill".into())), 0.0);
    for _ in 0..6 {
        s.apply(Action::MovePick(1), 0.0);
    }
    (s, images)
}

fn add_skill() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    s.conversation_mut().removed_skills.push("motion".into());
    s.popover = Some(crate::state::PopoverKind::AddSkill);
    (s, images)
}

fn skills_many() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = reference();
    let icons = [SkillIcon::Cube, SkillIcon::Bolt, SkillIcon::Sparkle, SkillIcon::Book];
    s.available_skills =
        (0..28).map(|i| skill(&format!("skill-{i}"), &format!("Skill {}", i + 1), icons[i % icons.len()])).collect();
    s.conversation_mut().pages.insert(crate::state::Carousel::Skills, 1);
    (s, images)
}

fn skill(name: &str, title: &str, icon: SkillIcon) -> Skill {
    Skill { name: name.into(), title: title.into(), description: String::new(), icon }
}

fn research(title: &str, kind: ResearchKind) -> ResearchItem {
    ResearchItem { title: title.into(), url: None, kind }
}

fn reads(count: usize) -> Vec<ToolCall> {
    (0..count).map(|i| ToolCall::new(i.to_string(), "Read", "file")).collect()
}

const GREETING: &str = "Hey! I’m on prototypes/main, with app/agent-composer open.\n\nWhat are we working on? I can tune how something moves, chase down a bug, or tidy up a component.";

const STREAMING: &str = "Here’s a small night sky, made from the dither minis.\n\n- The sky is one gradient, dithered from deep navy at the top to a faint glow at the horizon.\n- A moon sits as a soft dithered disc, with a";

const RUN_STARTED: f64 = -16.0;

pub(super) fn reference() -> (Session, Vec<SyntheticImage>) {
    let images: Vec<SyntheticImage> =
        (0..3).map(|i| (PathBuf::from(format!("/reference/{i}.png")), sky(i, 128), 128)).collect();
    let mut s = Session::new(Some(PathBuf::from("/reference")));
    s.snapshot = WorkspaceSnapshot {
        repository: Some(Repository {
            root: "/reference".into(),
            slug: "zzzzshawn/dither-minis".into(),
            head_short_sha: "a1f3c2e".into(),
            head_subject: "Tighten the outline".into(),
            branch: "main".into(),
            branches: vec!["main".into(), "prototypes".into()],
            ..Repository::default()
        }),
    };
    s.branch = Some("main".into());
    s.available_skills = vec![
        skill("dither-minis", "Dither minis skill", SkillIcon::Cube),
        skill("motion", "Motion skill", SkillIcon::Bolt),
    ];
    let c = s.conversation_mut();
    c.title = Some("Create a small night sky scene.".into());
    c.started_unix = Some(0);
    c.repositories = vec![Repository {
        root: "/reference/dither-engine".into(),
        slug: "zzzzshawn/dither-engine".into(),
        branch: "main".into(),
        ..Repository::default()
    }];
    c.references = images.iter().map(|(p, _, _)| p.clone()).collect();
    c.research_items = vec![
        research("Painting night skies in flat colour", ResearchKind::Article),
        research("Reading the Milky Way's bands", ResearchKind::Document),
        research("The colour of moonlight", ResearchKind::Discussion),
        research("Haze and layered ridges", ResearchKind::Video),
        research("Dithering gradients by hand", ResearchKind::Article),
        research("Stars that twinkle slowly", ResearchKind::Document),
    ];
    c.model = 1;
    c.effort = Effort::Max;
    c.agents = (0..3)
        .map(|i| {
            let mut agent = SubAgent::new(i);
            agent.status = AgentStatus::Done { seconds: [5, 4, 5][i] };
            agent.summary = ["Mapping the...", "Drafting the c...", "Checking the..."][i].into();
            agent
        })
        .collect();
    c.turns = vec![
        Turn {
            id: Default::default(),
            user: UserMessage { text: "hey".into(), images: Vec::new(), branch_caption: "prototypes/main".into() },
            response: TurnResponse {
                activity: Activity::Done,
                text: GREETING.into(),
                tools: reads(1),
                elapsed_ms: 3_000,
                failure: None,
                usage: Vec::new(),
                structured: None,
            },
            author: None,
        },
        Turn {
            id: Default::default(),
            user: UserMessage {
                text: "/dither-minis /motion Create a small night sky scene.".into(),
                images: c.references.clone(),
                branch_caption: "dither-minis/main".into(),
            },
            response: TurnResponse {
                activity: Activity::Writing,
                text: STREAMING.into(),
                tools: reads(7),
                elapsed_ms: 0,
                failure: None,
                usage: Vec::new(),
                structured: None,
            },
            author: None,
        },
    ];
    c.run = Some(ActiveRun { id: RunId::new(), started_at: RUN_STARTED });
    (s, images)
}

fn sky(variant: usize, size: u32) -> Vec<u8> {
    let palettes: [([f32; 3], [f32; 3]); 3] = [
        ([0.10, 0.12, 0.25], [0.45, 0.30, 0.42]),
        ([0.07, 0.07, 0.08], [0.12, 0.12, 0.12]),
        ([0.08, 0.16, 0.14], [0.30, 0.62, 0.22]),
    ];
    let (top, bottom) = palettes[variant % palettes.len()];
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let t = y as f32 / size as f32;
            let mut c = [0.0; 3];
            for k in 0..3 {
                c[k] = top[k] + (bottom[k] - top[k]) * t;
            }
            let star = (x * 73 + y * 151 + variant as u32 * 997).is_multiple_of(211) && t < 0.7;
            let px = c.map(|v| if star { 255 } else { (v.clamp(0.0, 1.0) * 255.0) as u8 });
            out.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_scenarios_exist() {
        assert!(by_name("reference").is_some());
        assert!(by_name("empty").is_some());
        assert!(by_name("github").is_some_and(|(s, _)| s.popover == Some(PopoverKind::Branch) && !s.is_running()));
        let (sessions, _) = by_name("sessions").expect("sessions");
        let groups = sessions.timeline(0.0);
        assert_eq!(groups.iter().map(|g| g.label.as_str()).collect::<Vec<_>>(), vec!["Today", "Yesterday"]);
        assert_eq!(groups[0].items.len(), 3);
        assert!(groups[0].items[0].selected && groups[0].items[1].running);
        assert!(
            by_name("repositories")
                .is_some_and(|(s, _)| s.github_choices().len() == 4 && s.recent_choices().len() == 1)
        );
        assert!(by_name("cjk").is_some_and(|(s, _)| s.conversation().composer.text().starts_with("日本語")));
        assert!(by_name("overflow").is_some_and(|(s, _)| s.conversation().composer.text().len() > 150));
        assert!(by_name("pull-requests").is_some_and(|(s, _)| s.pull_requests.len() == 4));
        assert!(by_name("permission").is_some_and(|(s, _)| s.pending_ask().is_some()));
        assert!(by_name("permission-queue").is_some_and(|(s, _)| s.conversation().asks.len() == 2));
        assert!(by_name("question-choice").is_some_and(|(s, _)| s.pending_ask().is_some_and(|a| a.cursor == 1)));
        assert!(
            by_name("full-access")
                .is_some_and(|(s, _)| s.conversation().permission == superfik_core::PermissionMode::Bypass)
        );
        let (waiting, _) = by_name("waiting-sessions").expect("waiting-sessions");
        assert!(waiting.pending_ask().is_none());
        assert!(waiting.timeline(0.0)[0].items.iter().any(|i| i.waiting));
        assert!(by_name("pull-request").is_some_and(|(s, _)| s.conversation().pull_request.is_some()));
        assert!(by_name("pr-compose").is_some_and(|(s, _)| s.pr_composer.as_ref().is_some_and(|c| c.can_confirm())));
        assert!(
            by_name("review")
                .is_some_and(|(s, _)| s.review_open() && s.review.as_ref().is_some_and(|r| r.drafts.len() == 2))
        );
        assert!(by_name("review-publish").is_some_and(|(s, _)| s.review.as_ref().is_some_and(|r| r.publish.is_some())));
        assert!(by_name("nope").is_none());
    }

    #[test]
    fn reference_matches_the_design_state() {
        let (session, images) = reference();
        assert_eq!(session.selected_skills().len(), 2);
        assert_eq!(session.conversation().agents.len(), 3);
        assert_eq!(images.len(), 3);
        assert_eq!(images[0].1.len(), 128 * 128 * 4);
        assert_eq!(session.model().label, "Fable 5.1");
        assert!(session.is_running() && !session.is_gathering());
        assert_eq!(crate::view::plan::repository_activity(&session).as_deref(), Some("Read 7 files"));
    }
}
