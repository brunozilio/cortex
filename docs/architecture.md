# Architecture

Superfik is a Cargo workspace of small crates with one-way dependencies.
Business rules never depend on IO, and UI components never depend on
business rules. Decisions are in `docs/decisions.md` (cited as `ADR NNNN`),
vocabulary in `GLOSSARY.md`.

```
superfik-app ──► superfik-client ──► superfik-ui ──► superfik-render ──► wgpu / cosmic-text
     │          (state, view)      └────────► superfik-motion
     ├──► superfik-i18n ─► superfik-core
     ├──► superfik-sound / -voice / -haptics / -dock / -menu / -unlock
     ├──► superfik-server ──► superfik-backend, superfik-identity
     ├──► superfik-backend ──► superfik-native ──► (provider APIs, MLX)
     │          ├────────────► superfik-engine, superfik-gateway (MLX)
     │          ├────────────► superfik-workspace ─► (git, filesystem)
     │          └────────────► superfik-plugins
     └──► superfik-protocol ─► superfik-core   (everything speaks core)
```

| Crate | Responsibility |
|---|---|
| `superfik-core` | Domain model: turns, context, agents, engine events, ids, `Principal`, and the pure rules of every feature below. |
| `superfik-engine` | `AgentEngine` trait, `EngineRegistry`, "Soon" descriptors for planned engines (Codex, Kiro, Copilot). |
| `superfik-native` | Superfik's own engine (ADR 0015): agent loop, tools, system prompt, sessions, provider APIs (`model`: Anthropic Messages, OpenAI Responses, Chat Completions), HTTP behind `Http`, tested against recorded streams. |
| `superfik-gateway` | Local MLX runtime and shared transport/conversion utilities. |
| `superfik-protocol` | Serialisable `ClientCommand` / `ServerEvent` in an `Envelope` carrying the `Principal`. |
| `superfik-backend` | `Backend` trait and the command handler: runs, memory, notes, plugins, vaults, routines, channels. |
| `superfik-workspace` | Git inspection, branch switching and worktrees behind traits. |
| `superfik-plugins` | Plugin manifests, the embedded catalog, packages, MCP config per account, provider plugins' contracts (pure), skill and agent discovery in Claude Code's layout. |
| `superfik-channels` | Channels agents talk to people through (ADR 0012): `Opener`/`Channel`, WhatsApp by QR, e-mail, official APIs (feature `official`). |
| `superfik-server` | A team's server: workers, connections, the `Hub`, sign-in, settings, command line. Embedded in the app for local mode. |
| `superfik-identity` | A server's own users (ADR 0017): usernames, first-access codes, argon2id passwords, tokens, roles, throttling. |
| `superfik-unlock` | Touch ID / Windows Hello behind `Unlock`, for the app lock. |
| `superfik-oauth` | Browser sign-in for plugins and providers: PKCE and the loopback redirect. |
| `superfik-keychain` | Secrets in the system store (Keychain, Credential Manager, Secret Service), chunked under Windows' size cap. |
| `superfik-process` | Console tools started the same everywhere: no console window on Windows, `Tree` ends a process with its children. |
| `superfik-private-file` | Files only the current user can read (`0600`, protected DACL). |
| `superfik-update` | Self-update: release manifest, download, SHA-256 and Ed25519 checks, staging and swap. |
| `superfik-motion` | Easing, springs, tweens, presence. Explicit time; deterministic. |
| `superfik-render` | GPU renderer: SDF goo surfaces, quads with procedural patterns, text, sprites, pictures. |
| `superfik-ui` | Design tokens, icons (Hugeicons, MIT), brand marks (Simple Icons, CC0) and stateless components painting into a `Painter`. |
| `superfik-i18n` | `Language`, the typed `Text` catalog and `Strings`, one file per language. Pure. |
| `superfik-browser` | Chromium through CEF in off-screen mode, behind `BrowserHost`. |
| `superfik-pages` / `superfik-frames` | Browser pages run on the server and streamed as JPEG tiles. |
| `superfik-sound` | UI sound cues: embedded packs, a pure soundboard, speakers behind `Player`. |
| `superfik-voice` | Voice agent: model catalog, prefs, `Sentences`, Portuguese G2P (pure); device behind feature `native` and `VoiceDevice`. |
| `superfik-haptics` | Trackpad pulses behind `Actuator`. |
| `superfik-menu` / `superfik-dock` | App menu and Dock tile (AppKit) behind traits, with `Plain` and `Recorder` stand-ins. |
| `superfik-client` | What the desktop and the web app share (ADR 0010): reducer, view, input, snapshot scenarios. Pure; builds for wasm32. |
| `superfik-web` | The client in a browser tab (wasm32). |
| `superfik-app` | Desktop shell: window, input, effects, snapshots. |

Every device crate follows the same shape: a pure part, the device behind
a trait, and `Recorder`/silent stand-ins for tests.

## The app: state → view → effects

`state`, `view` and `input` live in `superfik-client`; `superfik-app`
re-exports them.

- `state::Session::apply(Action) -> Vec<Effect>` is a pure reducer. Every
  product rule is unit-tested there.
- `view` turns a session plus animation state (`Motions`) into a `Scene`
  and a `HitMap`. It never mutates the session.
- `input` maps keys, pointer and wheel to actions (`controller`, `keys`,
  `pointer`), purely, for every shell.
- `app` executes effects (backend commands, native dialogs, devices) and
  feeds the device's input to `input`.

Render passes: the canvas (camera pans and zooms it), the sidebar (window
coordinates, second pass), and the overlay (viewer, full-window screens,
popovers, tooltips, Get Started and the lock, painted last). Full-window
screens (Settings, review, routines, update) take keys through their own
`controller::*_key` and keep the canvas still.

## Platform

`Session::platform` says what the client can do; the reducer never asks
for what it lacks and the views hide it. Nothing below the shell uses
`cfg` for this.

- `DESKTOP` (macOS): everything.
- `WINDOWS` (ADR 0013): Ctrl as primary modifier, no haptics, Windows
  Hello, no browser cards yet (`Platform::pages` off: Ctrl+T raises
  `Notice::NoWebPages`, an agent's browser call fails at once).
- `LINUX`: like Windows but with browser cards and login import; no system
  unlock. Credentials go to the Secret Service, else a `0600` file that
  then stays the store, else keyutils (`superfik_keychain::entry` chooses
  once). One ONNX Runtime serves memory and voice (`ort-sys`'s; the
  vendored `sherpa-onnx-sys` leaves its own out, see its `SUPERFIK.md`).
  GPU is Vulkan, else OpenGL through EGL. App id `superfik`.
- `WEB`: no sound, haptics, Get Started, Touch ID, login import, native
  dialogs or voice; `keys::for_platform` leaves the browser its own
  shortcuts (`KeyCommand::kept_by_browser`).

Data lives in Roaming `%APPDATA%\Superfik` / `%LOCALAPPDATA%\Superfik` on
Windows and `$XDG_CONFIG_HOME/superfik` on Linux. Building, packaging and
testing per platform: `docs/platforms.md`.

## Settings

`Session::preferences` (`state/preferences.rs`) holds what Settings edits.
`Action::ChooseSetting` returns `Effect::Persist`, sent to the server as
`SaveSettings` (ADR 0010: the server keeps each user's settings as opaque
JSON; no settings file or env override on the client). At launch the shell
asks `LoadSettings` and waits briefly so the first frame has the right
language and theme.

Settings is a full-window screen: sections at the left, the pane's groups
of `FormCard`s at the right. What each pane lists is pure
(`view/settings_screen/rows.rs`); adding a field is a variant in
`preferences.rs` plus its line in `rows.rs`. Text fields edit in place
(`rows::Control::Field`): Enter, another field or a click away keeps,
Escape drops. "Settings…" in the app menu comes from `superfik-menu`
(`Wake::OpenSettings`).

The theme resolves to an `Appearance` (`theme.resolve(system)`); each
frame `view::palette(session)` picks `colors::LIGHT` or `DARK`, which every
`Painter` carries.

## Language

Every string the user reads comes from `superfik-i18n`: `Strings` turns a
typed `Text` key into a sentence and translates domain values from core.
An exhaustive `match` per language makes a missing translation a compile
error.

- Core and the reducer keep facts, not sentences (`state::Notice` is
  translated when painted). Text from git, the backend or the model is
  shown as it came.
- Components know no language: labels arrive as props.
- Not translated: text sent to the model, tool names, product names.
- Launch language: the saved one, else the system's preferred locales,
  else English. Snapshots take `--lang`.

To add one: a `Language` variant and a file implementing `Catalog`.

## Sound and haptics

`state::sound` maps a `Trigger` and a before/after `Probe` of the session
to a `superfik_sound::Cue`; `Session::apply` appends `Effect::Sound`.
Packs (`assets/sounds/<pack>/`, `PackId`: piano default, sine, internal) voice
the same cues; `Soundboard` (pure) rotates variants, `Speakers` (rodio) is
the only IO. To add a pack: a folder of WAV clips, a `PackId` variant and a
`Pack` mapping.

Haptics are gesture feedback, detected where the gesture lives (the effort
meter in `input::pointer`), not emitted by the reducer. `Haptics` drops
pulses closer than `motion::HAPTIC_GAP`; `Trackpad` strikes the actuator
through MultitouchSupport (`dlopen`), falling back to
`NSHapticFeedbackManager`. Setting: `Preferences::haptics`.

## Voice

Everything runs on this computer; audio never leaves it, even with a
team's server.

- **Listening**: Nemotron 3.5 streaming ASR through sherpa-onnx (chunk 80–
  1120 ms, default 560), in the interface's language. Half a second of
  silence is fed before each utterance or the first word is lost.
- **Speaking**: Kokoro-82M, voice `pm_santa` by default. espeak-ng is GPL,
  so `superfik_voice::g2p` spells Portuguese into IPA itself; English uses
  Kokoro's lexicon.
- **Turn-taking** (`state/voice.rs`, pure): Off → Preparing (downloads
  into `<app data>/models/voice`) → Listening (a pause submits) → Waiting →
  Speaking (`Sentences` cuts the reply as it streams) → Listening. Half
  duplex: the mic is closed while the agent works and speaks. A failure
  turns it off with `Notice::Voice`.
- **Wake word** (`state/wake.rs`, `superfik_voice::wake`; off by
  default): Silero VAD gates the mic, Nemotron transcribes each phrase,
  `wake::find` matches the word tolerantly. A woken conversation returns
  to watching after `motion::WAKE_IDLE`.
- **Holding ⌘** (`state/push_to_talk.rs`): held alone for
  `PUSH_TO_TALK_HOLD` opens the mic until release; any other key or click
  makes it a shortcut instead.
- **Spoken prompts** (`superfik_core::spoken`): a `<voice_mode>` block
  ahead of the user's words asks for short, speakable replies; the
  transcript shows the words alone.
- **Device**: `VoiceDevice` (`native::LocalVoice`, `NoVoice`, `Recorder`),
  polled in `drain_backend`; `SUPERFIK_VOICE=off` leaves it out. macOS
  needs `NSMicrophoneUsageDescription` and the audio-input entitlement.
- `cargo run -p superfik-voice --features native --example talk --
  roundtrip|listen|wake|watch` exercises it.

## Onboarding, lock, tooltips

- **Get Started** (`state/get_started.rs`): shown until dismissed once
  (`Settings::onboarded`), under the lock if locked; every pixel is a hit
  while it shows. Its intro is voiced beat by beat (`view::intro_score`).
  Snapshot: `--scenario get-started --at <seconds>`.
- **App lock** (`Preferences::lock`, where the system can unlock):
  `Effect::Unlock` asks Touch ID on a thread; `Unavailable` sets the lock
  aside. Scenario `locked`.
- **Tooltips**: `Painter::label_fit` notes every label it elides; the
  shell shows the whole text under the pointer after
  `motion::TOOLTIP_DELAY`.

## Sessions

A workspace holds any number of conversations ("sessions").
`state::Conversation` owns one: turns, the run in flight, the engine
session, the composer and the context attached to the next prompt.
`Session` keeps the workspace and environment plus `conversations` and
`active`.

- Backend events carry a `RunId`; the reducer routes each to the
  conversation owning the run, so sessions stream in the background.
  Leaving the workspace cancels every run.
- The timeline groups conversations by local day (`WallClock`, set by the
  shell, so the reducer never reads the clock). ⌃Tab steps through it.
- Each session's canvas (node offsets, camera) is view state, kept in
  `view::placements` and `<app data>/canvas/<project>.json`, never sent to
  the backend.
- Sessions survive restarts: after every action the reducer compares a
  fingerprint of each conversation (`state/history.rs`) and emits
  `Effect::KeepSession` → `SaveSession`; `LoadSessions` brings them back
  (`FileSessionStore`, one JSON per session).
- More repositories can be attached; the workspace's stays primary
  (engine `cwd`, skills, continuity). Pickers list the `gh` account's
  repositories; `LocateRepository` finds or clones one under the clone
  root (default `~/Superfik`).
- Pull requests: `ListPullRequests`, `CheckoutPullRequest`; creating one
  always goes through the sheet (`ConfirmPullRequest` is the only path to
  `CreatePullRequest`).
- Review (`state/review.rs`, `view/review/`): a full-window diff with
  draft comments published with a verdict; "Review with agent" drafts
  comments through a `Review` handoff.
- Handoffs (`superfik_core::handoff`): structured results an agent writes
  to a per-run JSON file, read just before a successful `Completed`.

The sidebar is painted in window coordinates as a second pass; it is as
wide as its titles need and the canvas fades and stops beside it
(`sidebar::beside`, `Camera::fence`). Goo primitives have no clip, so
rows outside the window are culled.

## Composer commands

`/` opens one picker over Skills, Plugins and Commands. A command typed
alone runs instead of going to the model (`state/commands.rs`,
`superfik_core::Command`): node toggles (`/context`, `/memory`, `/skills`,
`/plugins`, `/commands`, `/notes`), popovers (`/repository`, `/model`,
`/mode`), `/effort`, `/browser`, `/upload`, `/compact`, `/voice`,
`/rename`, `/btw`. Skill and account tokens are painted as pills
(`composer::Highlight`). There is no `#` trigger.

## Engines

`AgentEngine::start` takes a `PromptRequest` and emits `EngineEvent`s;
exactly one terminal event (`Completed`, `Failed`, `Cancelled`) ends a
run. Recognised problems arrive first as `Diagnosed(EngineProblem)`.

**The native engine** (ADR 0015, `EngineKind::Native`) runs the agent
loop, tools, permissions, sub-agents and background tasks in process,
behind `ModelApi` (`anthropic::Messages`, `openai::Responses`,
`chat::Chat` for local servers). It offers no models of its own:
`ListModels` asks every connected AI account's catalog and tags each model
with its account (`ModelOption::account`), then lists downloaded MLX
models. Nothing connected lists nothing (`Notice::NoModels`). A run uses
its model's account key (`RunCredentials::model`, never serialized); a
stale or foreign account is refused, never swapped. Sessions are kept by
the engine (`FileSessions`). The server's worker runs filesystem, shell
and stdio MCP work as the configured UID (`superfik-native-worker`).
Operation and provider plugins: `docs/native-engine.md`.

- **Built-in agents**: `Explore`, `Plan` (read only), `general-purpose`,
  `fork` (inherits the conversation) and `superfik` (background jobs,
  closing with `result:` / `needs input:` / `failed:`). Project or user
  agents of the same name replace them.
- **Prompt**: planning mode, output styles (`prompt/styles.rs`; a
  project's `outputStyle` wins), visuals (see Visuals), tool descriptions
  carrying the working rules.
- **Compaction** keeps only the nine-section `<summary>`.
- **Side calls** (`chores/`): `Title`, `Suggest`, `Recap`, `Side`
  (`/btw`), `HeadsUp`, `Rename`: one model turn without tools, answered
  with `EngineEvent::Chore`, kept apart in `Conversation::chores`.
- **Safety check** (`guard/`): in Bypass, every editing or executing
  action is graded by the run's model before it runs; above 50, or a check
  that fails, holds it back.
- **Advisor** (`tools/advisor.rs`): with `advisorModel` set, forwards the
  conversation to that model at its deepest effort.
- **Built-in skills** (`crates/superfik-native/skills/`, embedded by
  `build.rs`, written to `~/.superfik/builtin-skills/<hash>/`, found after
  the project's and the user's).

**Local models** (`superfik_gateway::mlx`). On Apple silicon models live
in `<local data>/models/mlx/<org>/<name>/`. The first use installs
`mlx-lm` with `uv` (the system's, else a pinned download checked by
SHA-256) on a `uv`-managed Python. One `mlx_lm.server` runs at a time on a
loopback port, tethered so a crash never leaves a model in memory, and
stops after `IDLE`. Downloads report `Preparing` then `Fetching`.
`cargo run -p superfik-backend --example native_local` runs one prompt.

The model picker lists each account under its title with its models;
picking a model picks the account, and a kept session remembers both.
Engines without an adapter show as "Soon".

**Adding an adapter**: implement `AgentEngine` with I/O behind seams and
fixture-tested parsers; register it in `LocalServices::detect`. Sub-agents
emit `AgentSpawned`, `AgentUpdate` and `AgentEvent { index, event }`. No
UI, protocol or reducer change is needed.

## Context window

The Context node shows how full the session is and compacts it.
`SessionCommand::MeasureContext` → `ContextMeasured(ContextUsage)`,
`Compact` → `Compacted`. The reducer keeps these in `Conversation::upkeep`,
apart from `run`; a prompt cancels a measurement, waits for a compaction.
`UsageGrid` draws one dot per percent.

## Memory

Agents remember a project between sessions, private to its owner.
Memories (`superfik_core::memory`) are episodic, semantic or procedural,
with tags and author; ids are small per-project numbers.

- **Store**: `MemoryStore` (`FileMemoryStore`, one JSON per project under
  `<app data>/memory/<owner>/`). `LoadMemory` / `EditMemory` → `Memory`.
- **Runs**: the backend fills `PromptRequest::memory`; a fresh engine
  session gets recalled memories plus instructions to write
  `{remember, forget, revise}` to a temp file, applied on `Completed`.
- **Search** (`memory::search`): BM25 plus embedding similarity
  (fastembed/ONNX, `SUPERFIK_MEMORY_MODEL=minilm|mpnet|off`), fused by
  reciprocal rank. The same function serves recall, the explorer and the
  `memory_search` MCP tool (loopback, a bearer token per run).
- **Optimise**: one run proposes merges and rewrites; `UndoMemory`
  restores the previous book.
- **Canvas**: the Memory node; a click opens the explorer (graph, search,
  list, edit). Live search keeps results lit until the answer arrives.

## Notes

Sticky notes owned by the project (`superfik_core::note`); sessions only
*link* them (`Conversation::notes`). Linked notes are on the canvas and in
every prompt. `NoteStore` (`FileNoteStore`); edits (`NoteEdit`) carry the
revision they were written against, so a stale draft is refused, not
lost. Agents get the `superfik-notes` MCP tools (`notes_list`,
`notes_read`, `notes_write`, `notes_check`). Unlinking never deletes;
emptying a note does. Scenarios `notes`, `notes-edit`, `notes-pick`.

## Visuals

Agents draw SVG in a ` ```visual ` fence, shown inline at one SVG unit per
point (`visual::WIDTH`, 440). Core parses it (`Block::Visual`, even
mid-stream); `components::visual` themes it (design-system `var(--…)` and
classes from the palette); `superfik_render::pictures` rasterizes it with
resvg and caches by hash. Parts with `onclick="sendPrompt('…')"` become
`Target::VisualPrompt`. The prompt carries `prompt/visuals/core.md`; the
`VisualGuide` tool serves the rest. Static SVG only. Scenarios `visual`,
`visual-drawing`.

## Reference viewer

Clicking a References tile opens `view/viewer.rs` in the overlay pass,
growing from the tile and shrinking back to wherever the tile is. Hits
exist only while it is open or opening.

## Browser cards

A conversation can show one live web page beside the composer (⌘T / ⌘L).
`superfik-browser` runs Chromium through CEF in accelerated off-screen
mode (`welding`): on macOS each frame is an IOSurface imported as a
texture on the app's own wgpu device; on Linux DMABUF planes imported into
Vulkan, else a CPU bitmap. No web view.

- **Rules** (`state::browser`): one `BrowserCard` per conversation, its
  address bar, focus and last report. It drives the page with
  `Effect::Browser(Call)`; pages answer `Action::Page(id, PageEvent)`.
  Each conversation browses in its own persistent profile; the incognito
  button uses an in-memory one.
- **Host**: `BrowserHost` (`Recorder` for tests); `app::pages` shows each
  page's newest settled frame, crossfading resizes (`PAGE_CROSSFADE`).
  `shell::page_input` forwards input; the app keeps only its own
  shortcuts while the page has the keys, and runs edit shortcuts as
  `EditCommand`s.
- **Event loop**: CEF drains the app's event queue, which must never
  happen inside a winit callback. On macOS its work runs from an
  `NSTimer` in the common run-loop modes (`cef::events::WorkTimer`); on
  Linux it pumps its own GLib loop between winit dispatches. Chromium
  grabs SIGTERM/SIGINT/SIGHUP, so the shell takes them back
  (`shell::quit_signals`).
- **Bundle**: macOS runs CEF only from an `.app` with helper bundles. A
  `cargo run` binary lays out `target/<profile>/Superfik.app` (APFS
  clones, signed with the Developer ID when available) and relaunches
  from it, responsibility disclaimed (`superfik_browser::bundle`). The
  framework's WebAuthn keychain group is rewritten to
  `F3BSQ3FZTF.superfik.webauthn.v1` (same length) so Touch ID passkeys
  work. On Linux CEF's files sit beside the executable (`$ORIGIN`
  runpath). Building needs `cmake` and `ninja`. The CEF build is pinned in
  `scripts/cef-release.lock.json` (`docs/platforms.md`).
- **Agents drive the same page** through the `superfik-browser` MCP tools
  (navigate, snapshot with `[ref=N]`, click, type, scroll, screenshot,
  evaluate…). A call becomes `EngineEvent::Browse`, the reducer routes it
  to the owning conversation's card, and the answer returns as
  `AnswerBrowse`. The page lives with the client, so a remote server's
  agents drive the page the user sees.
- **Pages on the server** (ADR 0010): for the web client the server runs
  pages (`superfik-pages`: `ServerPages`, `PageLoop` on the main thread,
  `Owners`) and streams damaged tiles as JPEG (`superfik-frames`);
  `superfik_client::pages` turns `Effect::Browser` into commands.
  `SUPERFIK_PAGES=off` turns it off.
- **Login import** (Settings › Browser): reads a Chromium-family
  browser's cookies (Keychain or Secret Service key) only when the user
  imports, sets them in open profiles and keeps a `0600` seed for new
  ones. Private pages never take them.
- **Vault logins** (`superfik_core::autofill`: URI rules, field planning;
  `state/autofill.rs`): on load the reducer asks `FindLogins`; a key in
  the toolbar lists candidates; `FillLogin` returns `LoginSecret` straight
  to the page (never through the session; `Debug` hides it). Fills run
  through the DevTools protocol in an isolated world, only if origin and
  path are still the chosen ones. Fill on load is on by default (HTTPS,
  once per address). Agents fill by name (`browser_fill_login`) after the
  user approves through the vault; afterwards `browser_evaluate` and
  `browser_screenshot` are refused on that page. Submitted sign-ins can be
  saved to a vault (`PopoverKind::SaveLogin`).
- **Passkeys** (`state/passkey.rs`, `vault/logins/passkeys.rs`): the
  page's `navigator.credentials` is wrapped; the origin comes from the
  execution context, never the script. The backend checks the relying
  party and signs ES256; the private key never leaves it. Anything else
  falls back to Chromium's own flow.
- Expanded cards cover the window (`view::browser_overlay`).
  `SUPERFIK_BROWSER=<address>` opens a card on launch;
  `cargo run -p superfik-browser --example page_probe -- <url> <out.png>`
  renders one page headless.

## Plugins

A plugin (`superfik-plugin.json`) brings skills, agents and MCP servers
(`stdio` or `http`, with `{{secret.<key>}}`, `{{account}}`, `{{server}}`
templates). Auth: `none`, `api_key`, `oauth` (MCP authorization, PKCE,
loopback) or `cli`. The curated catalog is embedded from
`crates/superfik-plugins/catalog/<id>/`; "Add from folder…" installs any.

- **Connections**: one signed-in account of a plugin
  (`superfik_core::Connection`); sessions pick them with `/` and send them
  as `PromptRequest::connections`. Each account gets its own MCP server
  name (`linear-acme`).
- **Secrets** never leave the backend (`SecretStore`, keyed by owner and
  connection). OAuth tokens are refreshed before a run; MCP config is
  written `0600` and removed on the terminal event.
- **AI providers** (Settings › AI: ChatGPT and Claude subscriptions,
  OpenAI and Anthropic APIs) are provider plugins; native requests go
  straight to the selected provider.
- **UI**: Settings › Integrations and › Vaults, cards with brand tiles,
  inline account sheets. Scenarios `settings-integrations`, `-open`,
  `-connect`, `settings-vaults`.

## Vaults

Agents use passwords and tokens without seeing them. A vault plugin
(Bitwarden through `bw`) opens one folder per account. The backend serves
`list_secrets` (names only) and `run_with_secrets` over MCP
(`superfik-vault`). Every `run_with_secrets` asks the user, in every
permission mode, as an ask on the run (id `vault-…`). The command runs in
the login shell with secrets as environment variables
(`vault::env_problem` refuses `PATH`, `LD_*`, `DYLD_*`…); output comes
back redacted (best effort; the approval is the guard). Adding a vault: a
`VaultKind`, a `VaultProvider` and a catalog manifest.

## Routines

Agent runs started without a human prompt (ADR 0012).

- **Domain** (`superfik_core::routine`, pure, time passed in): name,
  brief, `Schedule`, `Autonomy` (Draft → Plan, Bounded → AcceptEdits,
  Auto → Bypass), model, accounts. `RoutineBook` applies edits, says what
  is `due` (missed firings fire once) and keeps `Report`s.
- **Firing**: the backend's routine clock (`ROUTINE_TICK`, only when
  `LocalServices::routine_clock` is set) turns due routines into prompts
  run as their owner. A team's server checks policy when a routine is
  written and `Budget` before and after each firing.
- **Working apart**: a routine that may edit runs in a new git worktree
  and branch (`routine/<name>-<date>-<time>`), never the member's checkout.
- **Webhooks**: `POST /hooks/<id>` fires a routine; the bearer is checked
  in constant time against the `SecretStore`, never kept in the book.
  Unknown id and wrong token are the same 404.
- **Answering messages**: a routine with `answers` fires for each channel
  message the agent should answer.
- **App**: a full-window screen (routines, form, reports, channel drafts).
  Scenarios `routines`, `routines-new`, `routines-draft`,
  `routines-webhook`.

## Channels

Agents talk to people outside the organization (ADR 0012).

- **Rules** (`superfik_core::channel`, pure): `pace` (typing time, gaps,
  retries), `limits` (hourly, daily, new conversations; sends wait), a
  member answering from the phone pauses the agent for two hours, history
  after pairing wakes nobody. `ChannelKind` adds each official API's
  ceilings and Meta's 24-hour reply window.
- **IO** (`superfik-channels`): WhatsApp by QR (`whatsapp-rust`, own Tokio
  runtime, SQLite session), e-mail (IMAP poll, SMTP reply in thread), and
  with feature `official` WhatsApp Cloud, Instagram, X and LinkedIn over
  the `Http` trait (`ScriptedHttp` in tests). Webhooks land on
  `/hooks/channels/<connection>`; signatures are checked in constant time
  (`superfik_channels::signature`).
- **Backend** (`channels/`): links each owner's accounts, keeps messages
  and files (`MAX_MEDIA_BYTES`), starts listening routines, and sends from
  a paced thread per link whose ceilings survive restarts.
- **Agent tools** (`superfik-channels` MCP): `channel_conversations`,
  `channel_read`, `channel_send`. A reply is a draft a member approves
  unless the run may act alone.
- `cargo run -p superfik-channels --example link` pairs a real number.

## Servers and users

Alone, the app talks to the server embedded in it as the local owner
(ADR 0005). A team's server (`superfik-server`, `docs/self-hosting.md`)
keeps its own users (ADR 0017, `superfik-identity`): an owner or admin
adds a user and gets a first-access code (7 days, kept hashed);
`POST /auth/first-access` sets an argon2id password, `POST /auth/sign-in`
signs in later. Both answer a 90-day token and set the session cookie;
five wrong tries lock a name for 15 minutes. Roles follow ADR 0003. The
command line (`superfik_server::cli`) runs `init`, `user`, `config` and
`install` over the same files.

Settings › Server (`state/server_link.rs`) joins a server: the shell posts
the sign-in, keeps address, name and token in the keychain and swaps the
connection; a kept sign-in reconnects at launch. Owners and admins also
edit the domain, users and governance there. Browsers sign in at the
server's `/sign-in`.

**Clients never run** (ADR 0010). Every command travels in
`Envelope { principal, payload }` and passes an `Authorizer`. The app
holds either the embedded server's `Connection` or a `RemoteBackend`,
never both. On a team's server:

- a workspace is one of the server's checkouts; every path a command
  names is checked against the member's checkouts, every prompt against
  policy and budget (`member_commands.rs`);
- each member's memory, notes, sessions and plugins live under their data
  folder;
- GitHub acts as whoever asked (ADR 0011): the local owner through `gh`, a
  member only through the organization's GitHub App (device flow), and
  that token reaches pushes, clones and every run's process;
- `/ws` takes only the session cookie (`HttpOnly; Secure;
  SameSite=Strict`) from no browser, the server's own host or
  `SUPERFIK_WEB_ORIGINS`;
- `RemoteBackend` reconnects and resends; the `Hub` hands a dropped
  connection's runs to the principal's next one and decides who each event
  reaches (`Recipient`).

Team features: delegated approvals ("Ask an admin",
`superfik_core::delegation`), shared memory (ADR 0004), the library
(`/publish <name>` → the `org` plugin), presence (`Viewing` → `Presence`),
and policy with monthly budgets per member (`superfik_core::policy`; the
server enforces it, the app only keeps choices within it).

## Web app

`superfik-web` builds for `wasm32-unknown-unknown`. `WebClient` is pure
and tested natively (session as `Platform::WEB`, input → actions → `Io`);
`page` runs winit over `<canvas id="superfik">`, builds the GPU
asynchronously and talks to `/ws` on the same origin. The server serves
the bundle `no-cache`. Before connecting the page asks `GET
/auth/session`; signed out, it goes to `/sign-in`. Keys go to a hidden
`<textarea>` (input methods, dictation); uploads use the browser's picker.
`?demo` shows the reference scenario without a server. CJK faces are
fetched on demand.

## Updates

Superfik updates itself (`superfik-update`). Each GitHub release carries
`superfik-<platform>.json` (version, optional minimum, notes, asset URL,
size, SHA-256, Ed25519 signature over the digest). `Updater::prepare`
downloads, checks size, hash and signature (`UPDATE_KEY`), then the
`Installer` (`.app` on macOS with `codesign` and team checks, a Linux
tarball; none for `cargo run`, `.deb` or Windows) stages beside the
install; `restart` runs `swap.sh` detached. Tools sit behind `Commands`,
so every check is tested with stand-ins.

CI signs archives with `SUPERFIK_UPDATE_KEY`
(`.github/workflows/scripts/announce-update.sh`, `xtask sign-update`).
In the app (`state/update.rs`) an optional update downloads in the
background and shows an arrow at the sidebar's foot; a required one
covers the window, above the lock. `SUPERFIK_UPDATES=off` disables it.
Scenarios `update-required`, `updating`, `update-ready`.

## Text

`TextStyle::mono()` uses the first system monospace face (SF Mono/Menlo,
DejaVu, Consolas), else bundled Fira Mono (always on the web). CJK falls
back per script (`superfik_render::text::fallback`), loaded the first
time text needs it; macOS faces stored as `hvgl` are skipped.

## Frame pacing

`App::schedule` renders every display refresh while focused or animating
(only vsync paces procedural patterns evenly), plus one frame after an
animation settles. Where presenting never waits (Xvfb, software GPU),
`shell::pacer::Pacer` falls back to a timer. In the background the loop
drops to `motion::IDLE_FRAME`. `RUST_LOG=superfik=trace` logs frame
times.

## Rendering

All surfaces are signed-distance fields. Primitives of the same layer are
fused with a polynomial smooth-min (the fillets where connectors meet
nodes, popovers growing drop → pill → panel). Each primitive draws one
quad; a fragment evaluates only neighbours whose reach covers it, in
canonical order, so every pixel is written once without seams. Text is
shaped by cosmic-text and drawn from a glyph atlas with optional
per-glyph gaussian blur (word-by-word reveals).
