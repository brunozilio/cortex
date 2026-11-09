# Superfik reference

What Superfik does, where each thing is, and how to use it. Labels in quotes
are the interface's English text. Shortcuts are written for macOS (⌘); on
Windows and Linux use Ctrl where macOS uses ⌘.

Sections: 1 app · 2 sessions · 3 composer · 4 commands · 5 models · 6
permission modes · 7 effort · 8 replies and sub-agents · 9 context · 10
memory · 11 notes · 12 references, repositories, review · 13 routines · 14
channels · 15 plugins · 16 vaults · 17 browser cards · 18 voice · 19
settings · 20 configuration files · 21 instruction files · 22 skills and
agents · 23 built-ins · 24 servers and teams · 25 updates · 26 shortcuts ·
27 platforms · 28 data folders · 29 messages · 30 Portuguese labels.

## 1. The app at a glance

- **What it is.** A native desktop app (no web views) where AI coding agents
  work on a canvas. The composer sits in the middle; around it, nodes show
  what the agent sees and uses (context, memory, notes, skills, plugins,
  references, repository, browser card). Superfik is open source
  under the Apache License 2.0.
- **Nothing to sign in to.** Alone, Superfik runs its own embedded server on
  the computer as the local owner; the app opens without any account. To
  work as a team, connect to a team's server (section 24). An optional lock
  asks for Touch ID or Windows Hello as the app opens (Settings › General).
- **Agents run in Superfik's own engine.** No other coding CLI is needed:
  the agent loop, its tools and the providers' APIs run inside Superfik,
  using the AI accounts connected in Settings › AI or local models.
- **Sidebar.** At the left: the session timeline. At its foot: the gear
  (Settings), the plug (Settings › Integrations), the loop (Routines screen;
  a dot when reports are unread) and, when an update is available, an arrow
  (the update screen).
- **Get Started.** On first launch an intro screen covers the window;
  "Continue", Enter or Escape dismisses it for good.
- **Canvas.** Pan with two fingers (or the wheel), zoom with a pinch,
  ⌘= / ⌘- and ⌘0 to reset. "Centre" brings the content back into view.
  The canvas of each session (node positions, camera) is kept per session
  and per workspace.

## 2. Sessions and the timeline

- **What.** A workspace (an opened repository or folder) holds any number of
  sessions (conversations). Each has its own transcript, run, composer,
  model, effort, attached repositories, skills, references, accounts and
  notes. Sessions run in the background while another is on screen.
- **Where.** The sidebar's timeline lists the workspace's sessions, newest
  first, grouped by day ("Today", "Yesterday", weekdays, dates).
  "Archived" holds archived ones.
- **New session.** ⌘N, the timeline's "New session" row, or `/new` (also
  `/clear`) in the composer. An untouched session is reused instead of
  making another. The previous one stays in the timeline.
- **Titles.** A session is named from its first prompt (the model names it).
  Double-click a row to rename it in place, or type `/rename <title>`;
  `/rename` alone has the model name it again from the conversation.
- **Archive.** Hover a row and click its archive glyph.
- **Switch.** Click a row, ⌘1–⌘9 for the first nine, ⌃Tab / ⌃⇧Tab for the
  session below / above (wraps around; archived ones skipped).
- **Persistence.** Sessions survive restarts (kept by the server per owner
  and project). A reply cut off by quitting shows as stopped. Coming back to
  a session can show "Where you left off: …", a short recap.
- **Opening a workspace.** ⌘O ("Open repository…") or the Repository
  node's pickers (recent folders, a native folder dialog, or a GitHub
  repository, which Superfik finds among recent checkouts or clones into
  `~/Superfik`). Leaving a workspace cancels its runs.
- **Branches.** The Repository node shows the branch; switching branches
  waits until no session is running ("Wait for the running sessions to
  finish before switching branches.").

## 3. The composer

- **What.** Where prompts are written. Placeholder: "Ask anything, or type /
  for skills, plugins and commands."; once a reply exists, "Add a
  follow-up...".
- **Parts.** The model chip (opens the model picker), the permission chip
  (permission mode), the effort meter (slide or click to change effort),
  the microphone (voice agent, when on), the image/attach button (adds
  references), and Send. While a run is going, Send becomes stop, and
  Escape stops the run (after closing any open popover).
- **Enter** sends; ⇧Enter adds a line.
- **The `/` picker.** Typing `/` opens one picker over three nodes: Skills,
  Plugins (accounts and installed plugins) and Commands, filtered by the
  word typed (an exact name first). ↑/↓ move, Enter picks.
  - A **skill** picked or typed (`/motion`) is attached to the prompt.
  - An **account** (`/linear-acme`) joins the session when the prompt is
    sent and stays for every later prompt (shown in the Plugins/Connections
    node, removable there).
  - A **note** of the project not yet in the session can be named
    (`/release-checklist`); it links to the session when sent.
  - A **command** typed alone and sent (or picked) runs instead of going to
    the model (section 4).
  - Skill and account tokens are shown as tinted pills in the text.
  - There is no `#` trigger: accounts are named with `/`.
- **Suggested next prompt.** After each reply the next likely prompt can
  appear in grey; Tab takes it (Settings › General › "Suggest the next
  prompt").

## 4. Composer commands

Typed alone in the composer (any case) or picked from the `/` picker or the
Commands node. Nodes toggled by a command start hidden.

| Command | Interface description | What it does |
|---|---|---|
| `/context` | Show the context window | Shows or hides the Context node. |
| `/memory` | Show the project's memory | Shows or hides the Memory node. |
| `/notes` | Show the project's notes | Shows or hides the Notes node, which also lists the project's notes not yet linked (click one to link it). |
| `/skills` | Show the skills | Shows or hides the Skills node. |
| `/plugins` | Show the plugins | Shows or hides the Plugins node. |
| `/commands` | Show this list | Shows or hides the Commands node. |
| `/repository` | Choose the repository | Opens or closes the repository popover. |
| `/model` | Choose the model | Opens or closes the model picker. |
| `/mode` | Choose the permission mode | Opens or closes the permission popover. |
| `/effort [level]` | Set the effort | Alone: steps to the next level (wrapping). With `low`, `medium`, `high`, `xhigh`, `max` (or `ultracode`): sets it. |
| `/compact` | Compact the context | Compacts the session's context; shows the Context node. "Nothing to compact right now." when there is nothing. |
| `/upload` | Attach files | Opens the file picker to attach references. |
| `/browser` | Open or close the browser | Opens or closes the session's browser card. |
| `/voice` | Talk to the agent and hear it answer | Turns the voice conversation on or off. |
| `/btw <question>` | Ask a side question without stopping the agent | Answers from what the session already holds, beside the run, without interrupting it. Alone, it writes `/btw ` and waits for the question. Needs a session that has started ("Nothing was said in this session yet; ask the agent directly."). |
| `/new`, `/clear` | Start a new session | New session; the old one stays in the timeline. |
| `/rename [title]` | — | Sets the session's title, or alone has the model name it. |
| `/share [team]` | Share the session with your team, or make it private | On a team's server: shares the session with the project (or the named team), or makes it private again. |
| `/publish <name>` | Publish a skill or agent to your organization | Owners and admins on a team's server: publishes one of their skills or agents to the organization's library. Alone, it says how. |

Only on a team's server, also: `/team <name>` (owners and admins make a
team), `/comment <text>` (a comment on a shared session, shown after the
turn it follows).

Skills and custom commands are also run with `/name arguments` (section 22).

## 5. Models and AI accounts

- **Where.** Settings › AI ("Connect your AI accounts and manage local
  models."). Accounts are grouped by brand; each connects with a
  "Subscription" (browser sign-in) or an "API key". OpenAI offers "ChatGPT
  Subscription" and "OpenAI API"; the other cloud providers listed in the
  pane connect the same ways. A provider can hold several accounts ("Add
  account"), e.g. work and personal, each with a name.
- **The model picker** (model chip, or `/model`). Lists each connected
  account under its title ("Work · ChatGPT Subscription", or the provider
  alone for an unnamed account) with that account's models; "Local models"
  after them. Picking a model picks its account too; a run uses that
  account's credentials and never falls back to another account. A session
  remembers its model and account. Engines not available yet appear as one
  "Soon" row and cannot be picked. Models an organization's policy forbids
  are greyed out.
- **Nothing connected.** The picker says so, and sending raises "No models
  yet. Connect an AI account or install a local model in Settings › AI."
- **Local models.**
  - **MLX (Apple silicon only).** Settings › AI › "Local models": "Select
    model" offers suggestions, "Other model…" takes a Hugging Face id
    (`org/name`, or the model page's address), then "Download model". The
    first use prepares MLX ("Preparing MLX (first time only)…"). One local
    model is held in memory at a time and released after 15 minutes idle.
  - **Ollama.** Models installed in a running Ollama appear in the picker
    automatically (as local models); nothing to configure in Superfik.
- **Usage limits.** A provider's limit shows "Usage limit reached · resets at
  …" with "Try again".
- **Advisor.** Optional: `advisorModel` in `.superfik/settings.json` names a
  stronger model the agent can consult (section 20). It must be a model the
  run's own account serves.

## 6. Permission modes and approvals

- **Where.** The permission chip in the composer, or `/mode`. Per session.
- **Modes.**
  - "Ask first": reads run freely; edits and commands ask.
  - "Accept edits": file edits run; commands ask.
  - "Plan only": read only; edits and commands are refused (the agent
    proposes a plan).
  - "Full access": everything runs without asking (autonomous).
- **Safety check.** With Settings › General › "Check autonomous actions" on
  (default on), every edit or command that would run without the user's
  approval is first graded by a model against Superfik's safety rules;
  risky ones are held back. A check that cannot run also holds the action
  back.
- **The ask card.** When the agent needs permission, a card replaces the
  composer: "Allow" (Enter), "Always allow" (⌘Enter; for that tool's edits,
  or that exact command), "Deny" (Escape). Questions the agent asks show
  options ("Pick any that apply" for multi-choice), "Next", "Back",
  "Answer", "Skip"; digits 1–9 pick an option. Asks queue per session.
- **Organizations.** A team's policy can cap the loosest mode ("Your
  organization allows at most …"). On a team's server, "Ask an admin" hands
  a permission ask to the organization's approvers while it stays
  answerable locally.

## 7. Effort

- **Levels.** "Low", "Medium", "High", "Extra" (`xhigh`), "Ultracode"
  (`max`). How hard the model thinks; deeper is slower and costs more.
  Applies to models that support it.
- **Where.** The effort meter in the composer (slide it; on a Mac trackpad
  each level gives a light haptic tap), or `/effort [level]`. Per session.

## 8. Replies, steps, sub-agents and visuals

- **Activity.** While working the reply shows "Thinking", "Searching the
  web", "Reading files", "Writing"; when done, "Worked for …". Clicking the
  "Worked for 3s · 4 steps" line expands the steps.
- **Sub-agents.** Agents the main agent starts appear as cards on the canvas
  with live progress; click one to open its own session transcript.
- **Research node.** Web sources the agent found while searching are
  gathered in the Research node.
- **Visuals.** The agent can draw a visual (SVG) in its reply; clicking a
  part of it that offers a follow-up sends that prompt.
- **Response style.** Settings › General › "Response style" (section 19), or
  `outputStyle` in settings (section 20).
- **"You should know".** After long work an observer may point out the one
  thing worth knowing about the session ("Heads up" when urgent). Turned on
  or off in Settings › General.

## 9. Context node and compaction

- **What.** How full the session's context window is and what fills it:
  "System prompt", "Tools", "MCP", "Agents", "Memory files", "Skills",
  "Messages", free space, and a reserved buffer. One dot per percent.
- **Where.** The Context node, below the composer at its right end (`/context`
  shows or hides it). It appears once a measurement exists; it is measured
  after every reply.
- **Compact.** "Compact context" on the node, or `/compact`. Summarises the
  session to free space ("Freed …"). Prompts wait while compacting; a
  prompt sent while measuring cancels the measurement.

## 10. Memory

- **What.** Agents remember a project between sessions. Every session of a
  project shares its memory, private to its owner. Kinds: "Episodic" (what
  happened), "Semantic" (facts about the project or the user),
  "Procedural" (how to do things here), with tags.
- **How it works.** A fresh session recalls the relevant memories with its
  first prompt; the agent can search the rest with its `memory_search` tool
  and writes what it learned at the end of a successful run.
- **Where.** The Memory node (`/memory`). Click it to open the explorer: the
  graph on the left (pan with two fingers, pinch or ⌘-scroll to zoom), on
  the right "Search memory" (live), the ranked list (edit and "Forget"
  knobs on each memory) and "Remember" to add one by hand. Escape closes.
- **"Optimize".** Runs one pass with the session's model that merges,
  removes, reclassifies and tightens memories. "Undo" restores the memory
  as it was, until any other edit.
- **Shared memory (team servers).** The person knob on a memory shares a copy
  with the project; teammates' agents recall it. Admins may also write the
  organization's scope.
- **Search model.** Search mixes words and meaning (a local embedding model
  downloaded on first use). `SUPERFIK_MEMORY_MODEL=minilm` (default),
  `mpnet` (larger, better matches) or `off` (words only), set in the app's
  environment.
- Not to be confused with instruction files (section 21) or the per-agent
  memory folders of custom agents (section 22).

## 11. Notes

- **What.** Sticky notes the user writes for a project: a title (first
  line), text and checklist items (`- [ ] item`, `- [x] item`), on coloured
  paper. Notes belong to the project; sessions link them. A linked note is
  on the session's canvas and in every prompt it sends.
- **Where.** The Notes node (`/notes`). "New note"; click a note to write in
  it (Enter adds a line and continues a checklist; Escape or a click
  elsewhere saves); tick a box to check an item; the swatches at its foot
  choose the paper; the knob unlinks it from the session (it is not
  deleted; emptying a note deletes it). `/notes` also lists the project's
  unlinked notes; click one to link it. A note can also be linked by naming
  it with `/` in the composer.
- **Agents.** Agents read, write and check notes with their notes tools; a
  note an agent creates links to its session.

## 12. References, repositories, pull requests and review

- **References.** Files attached to the next prompt: the attach button,
  `/upload` or "Add references". They show in the References node; click a
  tile to view it full window (Escape closes).
- **Repository node.** The workspace's repository is the primary one; "Add
  repository" attaches more to a session (recent folders, a folder dialog,
  or a GitHub repository). The pickers list the GitHub account signed in
  with `gh` ("Sign in with `gh auth login` to list GitHub" when signed out).
- **Pull requests.** The Repository node's "Pull requests" row lists open
  pull requests; choosing one checks it out and adds the Pull request node.
  "Open pull request" opens a sheet: title, description, "Draft", "Write
  with agent" (the agent fills the sheet), "Create pull request" (pushes the
  branch and opens it on GitHub).
- **Review.** Click the Pull request node to open the review screen: the
  diff ("Stacked" or "Split"), select lines to "Comment" or "Ask agent"
  (seeds the composer with them); comments stay drafts until "Publish
  review" with "Approve" or "Request changes" and an optional summary.
  "Review with agent" on the node has the agent draft comments for you to
  edit, drop or publish. Published threads can be resolved.

## 13. Routines

- **What.** Agent runs that start without a prompt: on a schedule, when a
  channel message arrives, or when another service calls a webhook. Each
  firing writes a report. On this computer routines run while Superfik is
  open; on a team's server, around the clock.
- **Where.** The loop at the foot of the sidebar opens the Routines screen:
  the routines ("Run now", "Pause"/"Resume", "Edit", "Delete"), "New
  routine", "Reports", and drafts "Waiting for approval".
- **The form.** "Name", "What to do" (the brief), "When" ("Hourly",
  "Daily", "Weekly", at a time), "Autonomy", the model, "Accounts it may
  use" (plugin accounts such as Sentry, Linear, GitHub, Stripe), "Answer
  messages", "Webhook". Enter moves from the name to the brief, then saves;
  Escape steps back.
- **Autonomy.** "Propose" (read only, proposes: like Plan only), "Edit
  files" (like Accept edits), "Act alone" (like Full access). A routine
  that may edit works on its own branch (`routine/<name>-<date>-<time>`) in
  a separate git worktree, never in the user's checkout; the report names
  the branch. Worktrees are left for the user to review and remove.
- **"Start from" templates.** "Production upkeep" (daily, edits files),
  "Release notes" (weekly, propose), "Revenue digest" (Stripe, weekly,
  propose), "Customer support" (answers WhatsApp messages, daily summary,
  propose).
- **Reports.** Newest first: "Running", "Failed", "Stopped", "Needs input",
  or the result. Opening one marks it read; "Continue in a session" starts a
  session with the report in its composer.
- **Webhooks.** Turn on "Webhook" in the form; "Copy call" copies a ready
  `curl` command (with the secret token) to the clipboard. The service
  calls `POST /hooks/<id>` on the server with that bearer token; the body
  reaches the routine's prompt. Turning the webhook off forgets the token.
- **Answering messages.** With "Answer messages" on, a routine also fires
  for each message one of its channel accounts receives.
- **Budgets.** On a team's server, the organization's policy and budget
  apply to routines too.

## 14. Channels

- **What.** Agents talk to people outside the team through channel plugins:
  WhatsApp (by QR), WhatsApp Business (Meta's Cloud API), E-mail (IMAP and
  SMTP), Instagram, X and LinkedIn.
- **Connecting.** Settings › Integrations, the plugin's card, add an account.
  - **WhatsApp:** "Pair with QR code", then on the phone: WhatsApp › Linked
    devices › Link a device, and scan. Shows "Linked as …" once paired.
  - **E-mail:** address, app password, IMAP and SMTP servers. Checked every
    minute; replies stay in the same thread.
  - **WhatsApp Business, Instagram, LinkedIn:** the provider's ids and tokens
    in the account's fields; the account shows a webhook path ("Webhook:
    your server's address + …", with "Copy") to register at the provider.
  - **X:** the app's API key and secret and the account's access token and
    secret; mentions and direct messages are checked every two minutes.
- **Using them.** A session or routine that picked a linked channel account
  gives the agent tools to list conversations, read and send. Files can be
  received and sent (WhatsApp, e-mail; up to 16 MB kept).
- **Drafts.** Unless the run may act alone ("Full access" or "Act alone"),
  a reply is a draft that waits on the Routines screen under "Waiting for
  approval": "Send", "Discard", or click it to rewrite (Enter sends).
- **Pacing.** Typing delays and hourly/daily ceilings protect the number;
  sends wait rather than fail. A member
  answering from the phone pauses the agent in that conversation for two
  hours. In groups only mentions or replies to the number count. On
  WhatsApp Business and Instagram, replies are only possible within Meta's
  24-hour window.
- **Not yet:** a member's own replies from their mail client do not pause
  the agent; Instagram and X do not send files.

## 15. Plugins and integrations

- **What.** A plugin brings skills, agents and MCP servers (tools), and may
  need a sign-in (OAuth in the browser, an API key, or a signed-in CLI such
  as `gh`). Catalog: Atlassian, Context7, E-mail, Firecrawl, GitHub,
  Instagram, Intercom, Linear, LinkedIn, Notion, Playwright, PostgreSQL
  (read-only), Sentry, Stripe, WhatsApp, WhatsApp Business, X; AI accounts
  are in Settings › AI; Bitwarden in Settings › Vaults.
- **Where.** Settings › Integrations (also the plug at the sidebar's foot).
  Shelves of cards: installed first, then by category ("Developer Tools",
  "Productivity", "Data & Payments", "Web & Browser", "More"); "Search
  plugins".
- **How.** "Install" (or "Get"), then expand the card: "Add account", name it
  ("Account name": work, personal…), fill its fields (secrets are masked) or
  "Sign in with the browser" / "Use the signed-in CLI", "Connect". Tab and
  ⇧Tab move through the sheet. "Disconnect" an account, "Remove plugin" to
  uninstall. "Ready: pick it with / in the composer".
- **Use in a session.** Type `/` and pick the account (e.g.
  `/linear-acme`). It stays on the session, appears in the Plugins node and
  goes with every prompt. Several accounts of one plugin keep separate tools.
- **Your own plugin.** "Add from folder…" installs a folder containing a
  `superfik-plugin.json` manifest (skills in `skills/<name>/SKILL.md`,
  agents in `agents/<name>.md`, MCP servers over stdio or http).
- **Secrets** never leave the server side; agents never see them in plain
  text.
- **Not yet:** plugins cannot add their own canvas nodes.

## 16. Vaults

- **What.** Agents use passwords and tokens without seeing them. Bitwarden is
  the vault today (the `bw` CLI must already be signed in with `bw login`).
- **Where.** Settings › Vaults ("Let agents use passwords and tokens without
  ever seeing them."). Install Bitwarden, add an account with the master
  password; the account's name is the Bitwarden folder it opens ("Superfik"
  when unnamed).
- **Use.** Pick the vault account with `/` in the session. The agent can list
  secrets (names and fields, never values) and run a command with secrets as
  environment variables. Every such command asks for approval on the ask
  card, in every permission mode; its output comes back with values
  redacted.
- Vaults also fill logins and passkeys in browser cards (section 17).

## 17. Browser cards

- **What.** A live web page beside the composer, one per session, rendered
  inside the app. Agents drive the same page with browser tools (navigate,
  snapshot, click, type, scroll, screenshot…); an "Agent" badge shows while
  they do.
- **Open/close.** ⌘T or ⌘L, `/browser`. ⌘R reloads, ⌘[ / ⌘] back and
  forward (or swipe two fingers sideways over the page). The address bar
  takes "Search or type an address". The expand button shows it over the
  whole window (Escape collapses).
- **Profiles.** Each session browses in its own persistent profile (cookies
  never mix between sessions). The incognito button reopens the page in a
  profile kept in memory only.
- **Import logins.** Settings › Browser › "Import logins": one row per
  browser found (Chrome, Dia, Arc, Brave, Edge, Chromium, Vivaldi) with
  "Import". The system may ask to allow reading that browser's keychain item.
  New sessions then start signed in ("New sessions start with these
  logins."); "Import again" renews them. "Clear cookies" signs the cards out
  of every site. Private pages never take imported logins.
- **Vault autofill.** With a vault connected, a key appears in the card's
  toolbar when saved logins fit the page; click it to choose one, or ⌘\ to
  fill the first ("No vault login is saved for this page." when none).
  Settings › Browser › "Fill logins as pages load" (on by default) fills the
  only fitting login (or the last one used on the site) over HTTPS by
  itself. After a sign-in form is sent, the card offers to save the login
  to a vault account ("Save to …", "Not now").
- **Passkeys.** A page asking for a passkey lists the vault's passkeys under
  the key; "Use another device" falls back to the system's own (Touch ID,
  phone, security key). Creating a passkey offers "Save passkey to …".
- **Laboratory profiles.** Settings › Browser › "Laboratory profiles": named
  profiles that fix a session's hardware (CPU threads, memory), screen,
  GPU, fingerprint (canvas, audio, seed, local fonts), proxy and WebRTC.
  "New profile", fill the fields (empty keeps the computer's actual
  value), "Save profile", then "Use" to assign it to the current session.
  Saved changes take effect after restarting Superfik.
- **Not available:** Windows has no browser cards yet ("Web pages are not
  available on this system yet."); login import is macOS and Linux.

## 18. Voice agent and wake word

- **What.** Talk to the agent and hear it answer; recognition (NVIDIA
  Nemotron) and speech (Kokoro) run on this computer, audio never leaves
  it. Listens in the interface's language (English or Portuguese).
- **Start/stop.** The microphone in the composer, or `/voice`. The first
  time, the models download (about 1 GB in all: "Downloading the voice
  models… n%"). While on: "Listening…" (what is heard fills the composer; a
  pause sends it), "Thinking…", "Speaking…", then listening again. The
  microphone is closed while the agent works and speaks. Stopping the run
  silences it.
- **Push to talk (macOS).** Hold left ⌘ alone to talk; let go to send.
- **Wake word.** Settings › Voice › "Wake word" (off by default; it keeps the
  microphone listening). The "Word" defaults to "Piratinha" and can be
  edited in place. Saying it starts a conversation with a chime; words said
  after it are sent at once. After 20 s of silence it returns to waiting for
  the word. The mic button shows a dot while waiting.
- **Settings › Voice.** "Turn on voice" (shows the microphone; on by
  default), "Wake word" and "Word", "What to call you", "Response time"
  (80, 160, 560 or 1120 ms chunks: shorter is faster, longer more accurate),
  "Assistant's voice" (Santa, Alex, Dora in Portuguese; Heart, Michael in
  English).
- **Microphone permission.** "Superfik cannot use the microphone. Allow it
  in System Settings › Privacy." The web client has no voice.

## 19. Settings, pane by pane

Open with the gear at the sidebar's foot, ⌘, or the app menu's
"Settings…". Escape or ⌘W closes. Panes in the left column:

- **General** ("Language and appearance.")
  - "Language": English or Português. Defaults to the system's language.
  - "Theme": "System", "Light", "Dark".
  - "Response style" ("How the agent writes."): "Default", "Concise"
    (leads with the result, no narration), "Proactive" (acts at once,
    interrupts little), "Explanatory", "Learning". A project's
    `outputStyle` in settings overrides it (section 20).
  - "Suggest the next prompt" ("After each reply, in grey; Tab takes it.").
  - "Check autonomous actions" ("Holds back risky edits and commands.").
  - "You should know" ("Points out what matters after long work.").
  - "Lock with Touch ID" / "Lock with Windows Hello": asks every time
    Superfik opens; shown only where the system can unlock; off by default.
- **Sound** ("How Superfik sounds and feels as you work.")
  - "Play interface sounds", "Sound pack" (piano or sine).
  - "Haptic feedback" › "Pulse the trackpad" (macOS Force Touch trackpads).
- **AI**: AI accounts and "Local models" (section 5).
- **Voice**: section 18.
- **Browser** ("Laboratory profiles and saved logins for browser cards."):
  laboratory profiles, "Import logins", "Fill logins as pages load",
  "Clear cookies" (section 17).
- **Integrations**: plugins (section 15) and channel accounts (section 14).
- **Vaults**: section 16.
- **Server** ("Work as a team on a Superfik server, and manage who gets
  in."): section 24.

Settings are kept by the server the app is connected to, per user. The web
client has no Sound or Voice pane.

## 20. Agent configuration files

`settings.json` and `settings.local.json` in `.superfik/` (Superfik's own)
and `.claude/` (read for compatibility). Read from the user's home
(`~/.superfik/`, `~/.claude/`) and then from each folder from the
filesystem root down to the project; a project's value overrides the
user's, and `.superfik/` overrides `.claude/` at the same level. Hooks from
every file add up.

| Key | Meaning |
|---|---|
| `outputStyle` | `"Default"`, `"Concise"`, `"Proactive"`, `"Explanatory"`, `"Learning"` (any case), or the name of a custom style in `output-styles/<name>.md` (letters, digits, `-`, `_`). Overrides the app's Response style. |
| `agent` | Name of an agent (custom or built-in) to run as the main agent, e.g. `"superfik"` for unattended work. |
| `advisorModel` | A model id the Advisor tool consults (a stronger reviewer that reads the whole conversation). No advisor without it. |
| `hooks` | Programs or checks run on agent events (below). |
| `autoMemoryEnabled` | `false` turns off the per-agent memory folders custom agents declare with `memory:`. Also `SUPERFIK_DISABLE_AUTO_MEMORY=1` in the environment. |
| `jsonSchema` | A JSON schema the final answer must match (structured output). |

```json
{ "outputStyle": "Concise", "agent": "reviewer", "advisorModel": "<model id>" }
```

- **Custom output styles.** `.superfik/output-styles/<name>.md` (project or
  `~/.superfik/`). The body is the style's instructions. Frontmatter:
  `keep-coding-instructions: true` keeps Superfik's coding guidance (off by
  default for custom styles), `turn-reminder:` a line repeated every turn.
- **Hooks.** `"hooks": { "<Event>": [ { "matcher": "<regex or *>", "hooks":
  [ { "type": "command", "command": "…", "timeout": 60 } ] } ] }`.
  - Events: `PreToolUse`, `PostToolUse`, `PostToolUseFailure`,
    `PermissionRequest`, `Stop`, `SubagentStart`, `SubagentStop`,
    `PreCompact`, `PostCompact`, `UserPromptSubmit`, `SessionStart`,
    `SessionEnd`. Others are refused with an error.
  - Types: `command`, `http` (`url`), `prompt` and `agent` (`prompt`; not on
    SessionStart, SessionEnd, SubagentStart, PreCompact, PostCompact).
    `timeout` 1–600 seconds.
  - Exit code 2 or a JSON decision can block an action; `PreToolUse` can
    rewrite the tool's input (`updatedInput`); `PermissionRequest` can allow
    within the current mode; `PostToolUse` can replace the output
    (`updatedToolOutput`); `Stop` can require the agent to continue;
    `SessionStart` and `UserPromptSubmit` output is added as context. Hooks
    never loosen Plan only or the parent's limits.

## 21. Instruction files and rules

- **Files read, in order:** `SUPERFIK.md`, `.superfik/SUPERFIK.md`,
  `CLAUDE.md`, `.claude/CLAUDE.md`, `AGENTS.md`, then the personal ones
  `SUPERFIK.local.md` and `CLAUDE.local.md` (keep these out of git). From
  each folder between the repository and the project folder; user-wide from
  `~/.superfik/SUPERFIK.md` and `~/.claude/CLAUDE.md`. `SUPERFIK.md` is
  Superfik's own; the others are read for compatibility.
- **Nested files.** The same files in subfolders load when the agent works
  on a path inside them.
- **Imports.** `@path/to/file.md` in an instruction file includes that file
  (relative to it, inside the trusted folder, up to five levels; not inside
  code blocks or comments). Files up to 1 MiB, 4 MiB with imports.
- **Rules.** Markdown files in `.superfik/rules/` (or `.claude/rules/`, or
  under `~/.superfik/`). Without frontmatter a rule always applies; with
  `paths:` (a list of globs such as `src/**/*.{ts,tsx}`) it applies when the
  agent touches a matching path.
- **`init` skill.** `/init` has the agent write a `SUPERFIK.md` for the
  repository.

## 22. Skills, custom commands and custom agents

- **Skills.** A folder with a `SKILL.md` (frontmatter `name`,
  `description`; the body is the instructions; other files beside it).
  Found in `.superfik/skills/` and `.claude/skills/` (project),
  `~/.superfik/skills/` and `~/.claude/skills/` (user), and installed
  plugins (named `<plugin>:<skill>`). On a name clash the first found wins;
  your own skill replaces a built-in one of the same name. The agent loads a
  skill when its description fits; you can name one with `/` in the
  composer, or run it with `/name arguments` (`$ARGUMENTS` in the body is
  replaced). `disable-model-invocation: true` means only you can start it.
  The Skills node (`/skills`) lists the session's skills.
- **Custom commands.** Markdown files in `.superfik/commands/` or
  `.claude/commands/`, run with `/name arguments`.
- **Custom agents.** `<name>.md` in `.superfik/agents/` or `.claude/agents/`
  (project), `~/.superfik/agents/` (user), or a plugin's `agents/`.
  Frontmatter `name` and `description`; the body is the agent's
  instructions. Optional: `tools`, `disallowedTools`, `mcpServers`,
  `permissionMode` (including `dontAsk`), `maxTurns`, `model`, `effort`,
  `skills`, `background`, `autoCompactWindow`, `memory` (`user`, `project`
  or `local`: the agent keeps its own memory folder under
  `.superfik/agent-memory`), `isolation: worktree` (works in its own git
  worktree), `initialPrompt`, `hooks`. A project or user agent with a
  built-in's name replaces it. Make one the main agent with `"agent"` in
  settings.
- **Creating them.** The `skill-creator` skill helps write and test skills.
- **Organization library.** On a team's server, `/publish <name>` publishes
  one of your skills or agents; members get them as the `org` plugin
  (`org:<name>`).

## 23. Built-in agents and built-in skills

Built-in agents (the main agent starts them with its Agent tool):

| Agent | What it is |
|---|---|
| `general-purpose` | The default sub-agent: searches, analyses and carries out multi-step tasks with the same tools, and reports back. |
| `Explore` | Fast, read-only search for locating code (asked to search "quick", "medium" or "very thorough"). |
| `Plan` | Read-only software architect: returns a step-by-step plan, the critical files and trade-offs. |
| `fork` | Inherits the whole conversation and model and carries out one directive: side tasks that need what is already known. |
| `superfik` | Background job agent: narrates, then ends with a `result:`, `needs input:` or `failed:` line. Pick it as the main agent (`"agent": "superfik"`) for unattended work. |

Built-in skills (run with `/name`, or used by the agent when they fit):

| Skill | What it does |
|---|---|
| `code-review` | Reviews the current diff, or a PR number, branch or path, for correctness bugs at a level (`low`, `medium`, `high` default, `xhigh`, `max`); `--comment` posts inline PR comments, `--fix` applies findings, `--max-findings <n>`. |
| `simplify` | Reviews changed code for reuse, simplification and efficiency, then applies the fixes (not a bug hunt). |
| `debug` | Diagnoses a problem with Superfik itself from its logs and settings (only when you run it). |
| `security-review` | Security review of the pending changes on the current branch. |
| `init` | Writes a `SUPERFIK.md` describing the codebase. |
| `verify` | Exercises a change end to end to confirm it works (only when you run it). |
| `run` | Launches and drives the project's app to see a change working. |
| `dataviz` | Guidance for any chart, graph or dashboard the agent draws. |
| `skill-creator` | Creates, edits and evaluates skills. |
| `research` | Researches a question across many web sources in the browser card, asking first only when the request is unclear, and reports with cited links. |
| `superfik-guide` | Answers questions about Superfik (this guide). |

## 24. Servers, teams and organizations

- **Alone.** Superfik runs its own server on the computer; nothing to sign in
  to. Settings › Server shows "This computer" ("Superfik runs here alone.
  Connect to your team's server to work together.").
- **Connecting to a team's server.** Settings › Server › "Connect to a
  server": "Address", "Username", "Password", "Connect". The first time,
  "First access": the address, your name, the "First-access code" an admin
  gave you, and a "New password" (10 characters or more). The app keeps the
  sign-in in the system keychain. "Disconnect" returns to this computer.
  When connected, every agent runs on the server; a workspace is one of the
  server's checkouts, opened from the repositories (this computer's folders
  are not offered).
- **Forgotten password.** An admin makes a "New code" for you; it also ends
  every sign-in. Five wrong tries lock a name for 15 minutes.
- **Running a server.** A self-hosted Linux server (or Docker), installed
  with the project's `install-server.sh` script; managed from Settings ›
  Server by owners and admins ("Domain"; "Users": "Add a user", "New code",
  "Make admin" / "Make member", "Remove") or from its command line
  (`superfik-server init`, `user add|code|role|remove|list`,
  `config domain`). Roles: "Owner", "Admin", "Member", "Guest".
- **Team features** (on a team's server):
  - Sharing: `/share` shares a session with the project or a team; others'
    shared sessions appear under "Shared with you"; continuing one makes
    your own copy. `/comment <text>` comments on a shared session.
  - Teams: `/team <name>` (owners and admins); members listed under "Teams".
  - Presence: the timeline shows "… here" on sessions others have open.
  - Shared memory (section 10) and the library (`/publish`, section 22).
  - Delegated approvals: "Ask an admin" on a permission ask; approvers
    allow or deny it in Settings (the asks waiting appear there).
  - "Policy" (owners and admins): "Most an agent may do alone" (the loosest
    permission mode), "Monthly budget per member", the models allowed for
    members; "Spent this month" per member. The server enforces it on every
    prompt ("You reached this month's budget of $… An admin can raise
    it.").
  - GitHub on the server acts as each member through "Sign in with GitHub"
    in a repository picker (a code typed at GitHub), when the server's
    admin set up a GitHub App.
- **Web client.** A team's server can serve Superfik in a browser tab:
  sign in at the server's address. It has no sound, voice, haptics, Get
  Started, lock, login import or native dialogs; shortcuts the browser
  keeps (⌘N, ⌘W, ⌘T, ⌘L, ⌘R, ⌘O, ⌘,, ⌘1–9, zoom…) are a click away instead.
- **Managed devices.** IT can require the app to work only on the team's
  server (MDM key `RequireOrganization`): "Your company set this app up for
  its server. Connect to it in Settings › Server."
- There is no account, license, plan, demo or payment: Superfik is
  free and open source, and teams self-host.

## 25. Updates

- Superfik updates itself, never through an app store, from its GitHub
  releases (signed and checked before installing). It checks at launch and
  every 6 hours.
- An optional update downloads in the background while agents keep
  working; an arrow at the sidebar's foot (with a dot once ready) opens the
  screen: "Update ready" › "Restart to update" (takes a few seconds). A
  required update ("Update required") covers the window until installed.
  "The update did not finish" › "Try again".
- Self-update works for the macOS `.app` and the Linux tarball when their
  folder is writable. Not for a `.deb` install (update through the package)
  or Windows (install the new release). `SUPERFIK_UPDATES=off` turns
  checking off.

## 26. Keyboard shortcuts and gestures

| Keys | Action |
|---|---|
| Enter / ⇧Enter | Send / new line |
| Escape | Close the top thing (screen, popover, viewer, sheet), else stop the run |
| ⌘N | New session |
| ⌘1–⌘9 | Go to session 1–9 |
| ⌃Tab / ⌃⇧Tab | Next / previous session |
| ⌘O | Open a repository |
| ⌘, | Settings |
| ⌘W | Close Settings / the current screen |
| ⌘T, ⌘L | Open the browser card (address bar) |
| ⌘R | Reload the page |
| ⌘[ / ⌘] | Page back / forward |
| ⌘\ | Fill the first vault login on the page |
| ⌘= / ⌘- / ⌘0 | Zoom in / out / reset the canvas |
| Tab | Take the suggested next prompt |
| Enter / ⌘Enter / Escape on an ask | Allow / Always allow / Deny; 1–9 pick an option |
| Hold left ⌘ (macOS) | Push to talk |
| ⌘C ⌘X ⌘V ⌘A ⌘Z ⌘⇧Z | Edit commands (also inside a browser page) |

Gestures: two fingers pan, pinch zooms, ⌘-scroll zooms the memory graph;
two fingers sideways over a page step through its history; double-click a
session row to rename it; double-click the window's top strip to maximise
(macOS).

## 27. Platforms

| | macOS | Windows (x64) | Linux (x86-64) | Web client |
|---|---|---|---|---|
| Shortcut key | ⌘ | Ctrl | Ctrl | ⌘/Ctrl, browser keeps its own |
| Browser cards | Yes | Not yet | Yes | Yes (pages run on the server) |
| Import logins | Yes | Not yet | Yes | No |
| Lock | Touch ID | Windows Hello | No | No |
| Haptics | Force Touch trackpad | No | No | No |
| Voice agent | Yes | Yes | Yes | No |
| Local MLX models | Apple silicon | No | No | No |
| Self-update | Yes | No | Tarball only | n/a |

- Linux needs Debian 13 / Ubuntu 24.04 or newer; packages are `.deb` and
  `.tar.gz`; credentials go to the Secret Service (GNOME Keyring, KWallet)
  or, without one, a `0600` file.
- macOS asks for the microphone the first time the voice is used, and may
  ask to allow reading another browser's keychain item when importing
  logins.

## 28. Where Superfik keeps things

- App data (settings kept by the local server, sessions, memory, notes,
  plugins, routines): `~/Library/Application Support/Superfik` (macOS),
  `%APPDATA%\Superfik` (Windows), `$XDG_CONFIG_HOME/superfik` or
  `~/.config/superfik` (Linux).
- Local data (models, worktrees, browser profiles, the agent's sessions in
  `native-sessions`, logs on Windows): the same folder on macOS and Linux;
  `%LOCALAPPDATA%\Superfik` on Windows.
- Downloaded models: `<local data>/models` (MLX under `models/mlx/`, voice
  under `models/voice/`).
- Routine worktrees: `<app data>/worktrees/<project>/`.
- Clones of GitHub repositories: `~/Superfik` by default.
- Logs: Windows `%LOCALAPPDATA%\Superfik\logs\`; macOS and Linux write to
  standard error (start the app from a terminal to capture it;
  `RUST_LOG=superfik=debug` for more). The `debug` skill helps.
- Agent configuration: `~/.superfik/` (user) and `.superfik/` (project).

## 29. Messages the user may see

| Message | Meaning / fix |
|---|---|
| "No models yet. Connect an AI account or install a local model in Settings › AI." | Nothing to run on: connect an account or add a local model. |
| "Connect an AI account in Settings → AI and try again." | The run's account is missing or signed out. |
| "The built-in AI engine is unavailable. Restart Superfik and try again." | Restart the app. |
| "The previous … conversation was not found; your next message starts a new one." | The engine session was lost; the next prompt starts fresh. |
| "Usage limit reached · resets at …" | The provider's limit; "Try again" later or pick another account. |
| "Choose a repository to start." | Open a workspace first (⌘O). |
| "Your organization does not allow …" / "allows at most …" / budget reached | The team's policy; ask an admin. |
| "Lost the connection to the server. Reconnecting…" | Reconnects on its own. |
| "The server no longer accepts this sign-in. Sign in again." | Sign in again in Settings › Server. |
| "Web pages are not available on this system yet." | No browser cards on Windows yet. |
| "No vault login is saved for this page." | No vault login fits the page. |
| "Superfik cannot use the microphone…" | Allow the microphone in the system's privacy settings. |
| "This computer has no way to unlock, so the lock was set aside." | No Touch ID / Windows Hello available. |

## 30. Interface in Portuguese

With "Language" set to Português the labels change. Common ones:

| English | Português |
|---|---|
| Settings (General, Sound, AI, Voice, Browser, Integrations, Vaults, Server) | Ajustes (Geral, Som, IA, Voz, Navegador, Integrações, Cofres, Servidor) |
| Response style | Estilo das respostas |
| Suggest the next prompt | Sugerir o próximo prompt |
| Check autonomous actions | Verificar ações autônomas |
| You should know | Vale saber |
| Lock with … | Bloquear com … |
| Ask first / Accept edits / Plan only / Full access | Pedir antes / Aceitar edições / Só planejar / Acesso total |
| Always allow | Sempre permitir |
| Low / Medium / High / Extra / Ultracode | Baixo / Médio / Alto / Extra / Ultracode |
| Context / Memory / Notes / Commands / References / Repository | Contexto / Memória / Notas / Comandos / Referências / Repositório |
| Compact context / Optimize | Compactar contexto / Otimizar |
| New session | Nova sessão |
| Routines; Propose / Edit files / Act alone | Rotinas; Propor / Editar arquivos / Agir sozinho |
| Local models | Modelos locais |
| Laboratory profiles | Perfis de laboratório |
| Fill logins as pages load / Import logins | Preencher logins ao carregar a página / Importar logins |
| Turn on voice / Wake word / What to call you / Response time / Assistant's voice | Ativar voz / Palavra de ativação / Como te chamar / Tempo de resposta / Voz do assistente |
