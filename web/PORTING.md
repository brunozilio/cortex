# Port do agentes-chat: checklist endpoint → feature

Gerado a partir de `packages/client/src/api/http-client.ts` e `packages/client/src/dashboards/api.ts` do agentes-chat (branch `main`). `@` = `/api/app/organizations/:orgId`. Cada endpoint vira uma rota em `/api/console/<feature>/…` no crate `console`, servida por uma feature do backend (`crates/backend/src/features/<feature>/`). Marque `[x]` quando a rota existir, tiver teste e a tela correspondente funcionar.

## Resumo por fase

| Fase | Endpoints |
|---|---|
| 2 Base | 10 |
| 3 Perfil e org local | 30 |
| 4 Chat | 62 |
| 5 Interação | 20 |
| 6 Agentes e settings | 123 |
| 7 Espaço de trabalho | 98 |
| 8 Integrações | 0 |
| 9 Browser | 22 |
| Fora do escopo | 247 |
| **Total** | 612 |

## Grupos do cliente

| Grupo | Fase | Feature | Fonte no nosso lado | Escopo |
|---|---|---|---|---|
| `system` | 2 Base | `instance` | `brand::NAME`, settings | telemetria, feedback, chaves de GIF e de voz |
| `account` | 3 Perfil e org local | `accounts` | — | usuário, multi-conta, troca de e-mail, exclusão; `phoneSignIn` fica fora |
| `orgs` | 3 Perfil e org local | `orgs, members, usage` | `spending()` para uso | org, perfil `me/*`, sidebar, membros, convites, limites, auditoria, team-week |
| `invites` | 3 Perfil e org local | `invites` | — | links de convite |
| `joinRequests` | 3 Perfil e org local | `invites` | — | pedidos de entrada |
| `roles` | 3 Perfil e org local | `roles` | `domain::access` (novo) | papéis e permissões |
| `apps` | 3 Perfil e org local | `apps` | — | consentimento OAuth de apps e tokens pessoais |
| `access` | 3 Perfil e org local | `roles` | — | acesso por agente |
| `scopes` | 3 Perfil e org local | `roles` | — | escopos por agente |
| `chat` | 4 Chat | `channels` | `Session::send`, `Sdk::history` | canais, categorias, mensagens, membros do canal; reações, correção e `answer-now` na fase 5 |
| `threads` | 4 Chat | `threads` | `Sdk::sessions`, `rename/archive/delete_session` | thread ↔ sessão do engine; `act` de missions vai para a fase 7 |
| `dms` | 4 Chat | `channels` | `sdk.agents(cwd)` | DMs com agentes e pessoas, leituras |
| `reads` | 4 Chat | `channels` | `seen_session` | não lidos, mute |
| `threadFollows` | 4 Chat | `threads` | — | seguir thread |
| `working` | 4 Chat | `runs` | `RunControl`, `EngineEvent::Tasks` | runs ativos e task manager |
| `approvals` | 4 Chat | `approvals` | `Asked` + `Run::answer(Decision)` | fila, regras e cobertura de aprovação |
| `questions` | 4 Chat | `approvals` | `AskKind::Questions` | respostas a perguntas do agente |
| `actions` | 4 Chat | `runs` | `Session::rewind`, checkpoints | desfazer e refazer ações do agente |
| `actionCards` | 5 Interação | `blocks` | — | cards de ação |
| `pins` | 5 Interação | `channels` | — | pins |
| `artifacts` | 7 Espaço de trabalho | `drive` | — | Drive e artefatos; polls, blocks e choices vão para a fase 5 |
| `voice` | 5 Interação | `media` | trait `Speech` | voz |
| `gifs` | 5 Interação | `media` | trait `Gifs` | GIFs |
| `search` | 5 Interação | `search` | Postgres `tsvector` | busca global |
| `reminders` | 5 Interação | `reminders` | agendador do backend | lembretes |
| `agents` | 6 Agentes e settings | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` | agente persistido no Postgres com instruções, modelo e canais; credenciais ↔ provedores; runs ↔ `Run` |
| `agentGroups` | 6 Agentes e settings | `agents` | — | grupos de agentes |
| `agentForYou` | 6 Agentes e settings | `agents` | — | resumo do agente para o usuário |
| `memory` | 6 Agentes e settings | `memory` | `ProjectMemory` | memórias do agente e do time |
| `proposals` | 6 Agentes e settings | `memory` | `ProjectMemory::revise` | propostas de mudança de contexto |
| `skills` | 6 Agentes e settings | `skills` | `skills(cwd)` + escrita em disco | skills, import e updates |
| `connectors` | 6 Agentes e settings | `connectors` | `mcp_servers`, `sign_in_mcp_server` | catálogo e conexões; Composio fica fora |
| `customConnectors` | 6 Agentes e settings | `connectors` | `add/remove_mcp_server` | servidores MCP próprios |
| `notifications` | 6 Agentes e settings | `notifications` | `notices()` | inbox, contagem, preferências |
| `sandbox` | 6 Agentes e settings | `settings` | `platform::sandbox` | configurações de sandbox; `computer*` fica fora |
| `storage` | 8 Integrações | `media` | trait `Blobs` (disco ou MinIO) | storage e migração |
| `secrets` | 8 Integrações | `vault` | — | pedidos de segredo |
| `vault` | 8 Integrações | `vault` | — | cofre |
| `wiki` | 7 Espaço de trabalho | `wiki` | — | wiki |
| `issues` | 7 Espaço de trabalho | `issues` | — | issues |
| `calendar` | 7 Espaço de trabalho | `calendar` | — | calendário |
| `dashboards` | 7 Espaço de trabalho | `dashboards` | — | dashboards e widgets |
| `routines` | 7 Espaço de trabalho | `routines` | `cli/tui/features/schedules` → backend | rotinas e webhooks de entrada |
| `imports` | 8 Integrações | `imports` | — | imports de outros apps |
| `mail` | 8 Integrações | `mail` | trait `Mailer` | caixa do agente |
| `githubApp` | 8 Integrações | `github` | — | GitHub App |
| `repositories` | 8 Integrações | `github` | — | repos |
| `databases` | 8 Integrações | `databases` | — | bancos que o agente consulta |
| `push` | 8 Integrações | `push` | Web Push (VAPID) | só navegador |
| `browser` | 9 Browser | `browser` | ferramenta nova no `agent-engine` | browser ao vivo e handoff |
| `browserProfiles` | 9 Browser | `browser` | — | perfis do browser |
| `machines` | Fora do escopo | `—` | — | computadores remotos e contas de IA em máquinas; `ai-accounts` → provedores (fase 6) |
| `devices` | Fora do escopo | `—` | — | celulares, pareamento, Xcode |
| `desktop` | Fora do escopo | `—` | — | tela do agente (noVNC) |

## Fora do cliente REST

- **Auth (Better Auth, `/api/auth/*`): fora.** O console tem um único usuário, o dono local, e o acesso é pelo token do link. Endpoints: `sign-in/email`, `sign-up/email`, `sign-in/social` (google, github, apple), `update-user`, `send-verification-email`, `request-password-reset`, `reset-password`, `sign-in/one-time-token`, `owner-claim`. `desktop/*` fica fora.
- [ ] **Health, fase 2:** `GET /api/health`.
- [ ] **Realtime WebSocket `/api/ws?orgId=`, fases 2 e 4:**
  - Frames do cliente: `subscribe`, `unsubscribe`, `typing`, `ping`.
  - Frames do servidor: `ready`, `subscribed`, `unsubscribed`, `error`, `pong`, `event{channelId}` e `event{scope}`.
  - Os `ChannelEvent` são traduzidos de `EngineEvent` pelo adaptador TS.
- [ ] **Frames do browser `/api/browser/frames?ticket=` e `browser.*` no WS, fase 9.**

## Rotas da UI

| Rota | Fase | Status |
|---|---|---|
| `__root`, `index`, `org.$orgId`, `org.$orgId.index`, `org.$orgId.home` | 2 | porta |
| `org.$orgId.c.$channelId`, `dm.agent.$agentId`, `threads.index`, `threads.$threadId`, `approvals`, `waiting`, `task-manager` | 4 | porta |
| `org.$orgId.search` | 5 | porta |
| `org.$orgId.agents.new`, `agents.$agentId.index`, `agents.$agentId.memory`, `proposals`, `notifications`, `settings`, `settings_.$section` | 6 | porta |
| `org.$orgId.wiki`, `wiki_.$slug`, `drive`, `issues.index`, `issues.$number`, `dashboards.index`, `dashboards.$dashboardId` | 7 | porta |
| `terms`, `termos`, `privacy`, `privacidade`, `dev.gallery` | 2 | porta (textos legais revisados) |
| `settings_.devices.*`, `settings_.computers.$machineId`, `conectar`, `conectando` | — | **fora** (mobile e máquinas) |
| `desktop.*`, `download`, `baixar`, `screen.html` | — | **fora** (app desktop e noVNC) |
| `login`, `signup`, `reset-password`, `invite.$token`, `owner`, `onboarding`, `org.$orgId.onboarding`, `autorizar`, `email-undo` | — | **fora** (contas e login) |
| `org.$orgId.members`, `org.$orgId.team`, `settings_.roles.$roleId`, `dm.person.$userId` | — | **fora** (pessoas da org, interação entre humanos) |
| `org.$orgId.calendar`, `org.$orgId.routines` | — | **fora** (calendário e rotinas) |

## Fora do escopo por decisão

- **Contas e login:** conta, login, signup, multi-conta, Better Auth.
- **Pessoas da org:** membros, convites, pedidos de entrada, papéis, times, team-week.
- **Controle e uso:** auditoria, uso e limites por pessoa, billing, apps/tokens, acesso e escopos por pessoa.
- **Interação entre humanos:** DMs entre pessoas, read receipts, typing e presença de humanos, cobertura de aprovação.
- **Integrações:** mail, GitHub app, web push, vault/segredos, bancos, imports, storage, GIFs, voz.
- **Calendário, rotinas e lembretes.**
- **Mobile, máquinas e noVNC.**

## Decisões que este checklist expõe

- **Agentes viram entidades persistidas.** No agentes-chat, um agente tem nome, instruções, modelo, canais, skills, memória e credenciais próprias. Os nossos agentes são definições `.md` por `cwd`. A feature `agents` guarda o agente no Postgres e, a cada run, monta uma `Session` com o modelo, o `cwd` do canal e as instruções dele. Os `.md` existentes aparecem como agentes importáveis.
- **Credenciais = provedores.** `@/credentials` e `ai-accounts` mapeiam para `providers()`, `connect_provider` e `sign_in_provider`.
- **Composio** (`usage/composio`) fica fora: connectors usam só MCP.
- **Máquinas** ficam fora junto com noVNC. As missions rodam no próprio host do servidor, usando os worktrees do `agent-engine`.

## 2 Base (10)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|
| [ ] | `system.instance` | GET | `/api/instance` | `instance` | `brand::NAME`, settings |
| — | `system.telemetry` | GET | `/api/app/telemetry` | — | fora: console local não envia telemetria |
| — | `system.setTelemetry` | PUT | `/api/app/telemetry` | — | fora: console local não envia telemetria |
| — | `system.sendFeedback` | POST | `/api/app/feedback` | — | fora: não há serviço de feedback do agentes-chat |
| — | `system.dropFeedbackFile` | DELETE | `/api/app/feedback/files/:id` | — | fora: não há serviço de feedback do agentes-chat |
| — | `system.decideFeedbackOffer` | POST | `/api/app/feedback/offers/:messageId` | — | fora: não há serviço de feedback do agentes-chat |
| — | `system.jev` | GET | `/api/app/jev` | — | fora: Jev só decide stickers e GIFs, que não existem aqui |
| — | `system.saveJev` | PUT | `/api/app/jev` | — | fora: Jev só decide stickers e GIFs, que não existem aqui |
| — | `system.checkJev` | POST | `/api/app/jev/check` | — | fora: Jev só decide stickers e GIFs, que não existem aqui |
| — | `system.removeJev` | DELETE | `/api/app/jev` | — | fora: Jev só decide stickers e GIFs, que não existem aqui |

## 3 Perfil e org local (30)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|
| — | `orgs.create` | POST | `/api/app/organizations` | — | fora: o console tem uma só org, `local` |
| — | `orgs.saveOnboarding` | PATCH | `@/onboarding` | — | fora: sem onboarding de workspace no console |
| [x] | `orgs.setAvatar` | PUT | `@/me/avatar` | `profile` | preferência `profile.avatar` (data URL) |
| [x] | `orgs.removeAvatar` | DELETE | `@/me/avatar` | `profile` | preferência `profile.avatar` |
| [x] | `orgs.setLogo` | PUT | `@/logo` | `organization` | preferência `organization.logo` (data URL) |
| [x] | `orgs.removeLogo` | DELETE | `@/logo` | `organization` | preferência `organization.logo` |
| [x] | `orgs.limits` | GET | `@/limits` | `organization` | dono sozinho, sem limites |
| [x] | `orgs.updateMe` | PATCH | `@/me` | `profile` | preferência `profile` |
| — | `orgs.setStatus` | PUT | `@/me/status` | — | fora: presença e status de humanos |
| [x] | `orgs.setStart` | PUT | `@/me/start` | `profile` | preferência `profile` |
| [x] | `orgs.setShowActivity` | PUT | `@/me/activity` | `profile` | preferência `profile` |
| [x] | `orgs.setAgentStyle` | PUT | `@/me/agent-style` | `profile` | preferência `profile` |
| [x] | `orgs.setChatPrefs` | PUT | `@/me/chat` | `profile` | preferência `profile` |
| [x] | `orgs.sidebarItems` | GET | `@/me/sidebar` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.setSidebarItem` | PATCH | `@/me/sidebar` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.setSidebarItems` | PUT | `@/me/sidebar` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.sidebarOrder` | GET | `@/me/sidebar/order` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.setSidebarOrder` | PUT | `@/me/sidebar/order` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.issuesView` | GET | `@/me/issues-view` | `profile` | preferência `profile` |
| [x] | `orgs.setIssuesView` | PUT | `@/me/issues-view` | `profile` | preferência `profile` |
| [x] | `orgs.sidebarView` | GET | `@/me/sidebar/view` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.setSidebarView` | PUT | `@/me/sidebar/view` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.sidebarToolOrder` | GET | `@/me/sidebar/tools` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.setSidebarToolOrder` | PUT | `@/me/sidebar/tools` | `sidebar` | preferência `sidebar` |
| [x] | `orgs.settings` | GET | `@/settings` | `organization` | preferência `organization` |
| [x] | `orgs.updateSettings` | PATCH | `@/settings` | `organization` | preferência `organization` |
| [x] | `orgs.setApprovalsLevel` | PUT | `@/approvals-level` | `organization` | preferência `organization`; `ownAgents` 0 até a fase 6 |
| [x] | `orgs.overview` | GET | `@/overview` | `organization` | contagens zeradas até as fases 4 e 6 |
| [x] | `orgs.usageReport` | GET | `@/usage/report` | `usage` | forma vazia: `spending()` é por sessão do SDK |
| [x] | `orgs.planUsage` | GET | `@/usage/plans` | `usage` | forma vazia: sem contas de plano |

## 4 Chat (62)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|
| [ ] | `actions.undo` | POST | `@/actions/:actionId/undo` | `runs` | `Session::rewind`, checkpoints |
| [ ] | `actions.redo` | POST | `@/actions/:actionId/redo` | `runs` | `Session::rewind`, checkpoints |
| [ ] | `actions.undoMessage` | POST | `@/messages/:messageId/actions/undo` | `runs` | `Session::rewind`, checkpoints |
| [ ] | `threadFollows.follow` | PUT | `@/threads/:threadId/follow` | `threads` | — |
| [ ] | `threadFollows.unfollow` | DELETE | `@/threads/:threadId/follow` | `threads` | — |
| [ ] | `reads.mute` | PUT | `@/channels/:channelId/mute` | `channels` | `seen_session` |
| [ ] | `reads.markThreadRead` | POST | `@/threads/:threadId/read` | `channels` | `seen_session` |
| [ ] | `reads.unreadThreads` | GET | `@/unread-threads` | `channels` | `seen_session` |
| [ ] | `reads.unreadTasks` | GET | `@/unread-tasks` | `channels` | `seen_session` |
| [ ] | `working.list` | GET | `@/active-runs` | `runs` | `RunControl`, `EngineEvent::Tasks` |
| [ ] | `working.taskManager` | GET | `@/task-manager` | `runs` | `RunControl`, `EngineEvent::Tasks` |
| [ ] | `working.taskSteps` | GET | `@/task-manager/:runId/steps` | `runs` | `RunControl`, `EngineEvent::Tasks` |
| [ ] | `working.continueTasks` | POST | `@/task-manager/continue` | `runs` | `RunControl`, `EngineEvent::Tasks` |
| [ ] | `chat.channels` | GET | `@/channels` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.createChannel` | POST | `@/channels` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.categories` | GET | `@/channel-categories` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.createCategory` | POST | `@/channel-categories` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.renameCategory` | PATCH | `@/channel-categories/:categoryId` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.deleteCategory` | DELETE | `@/channel-categories/:categoryId` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.placeCategory` | PUT | `@/channel-categories/:categoryId/place` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.place` | PUT | `@/channels/:channelId/place` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.join` | POST | `@/channels/:channelId/join` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.leave` | POST | `@/channels/:channelId/leave` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.card` | GET | `@/channels/:channelId/card` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.messageCard` | GET | `@/messages/:messageId/card` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.idRef` | GET | `@/refs/:id` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.update` | PATCH | `@/channels/:channelId` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.archive` | DELETE | `@/channels/:channelId` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.unarchive` | POST | `@/channels/:channelId/unarchive` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.archived` | GET | `@/archived-channels` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.deleteChannel` | POST | `@/channels/:channelId/delete` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.roster` | GET | `@/channels/:channelId/roster` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.members` | GET | `@/channels/:channelId/members` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.addMember` | POST | `@/channels/:channelId/members` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.removeMember` | DELETE | `@/channels/:channelId/members/agents/:agentId` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.setAutoReply` | PATCH | `@/channels/:channelId/members/agents/:agentId` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.permissions` | GET | `@/channels/:channelId/permissions` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.setPermission` | PUT | `@/channels/:channelId/permissions` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `chat.post` | POST | `@/channels/:channelId/messages` | `channels` | `Session::send`, `Sdk::history` |
| [ ] | `approvals.list` | GET | `@/approvals` | `approvals` | `Asked` + `Run::answer(Decision)` |
| [ ] | `approvals.get` | GET | `@/approvals/:id` | `approvals` | `Asked` + `Run::answer(Decision)` |
| [ ] | `approvals.approve` | POST | `@/approvals/:id/approve` | `approvals` | `Asked` + `Run::answer(Decision)` |
| [ ] | `approvals.refuse` | POST | `@/approvals/:id/refuse` | `approvals` | `Asked` + `Run::answer(Decision)` |
| [ ] | `approvals.cancel` | POST | `@/approvals/:id/cancel` | `approvals` | `Asked` + `Run::answer(Decision)` |
| [ ] | `approvals.decideMany` | POST | `@/approvals/decide` | `approvals` | `Asked` + `Run::answer(Decision)` |
| [ ] | `approvals.rules` | GET | `@/agents/:agentId/approval-rules` | `approvals` | `Asked` + `Run::answer(Decision)` |
| [ ] | `approvals.removeRule` | DELETE | `@/agents/:agentId/approval-rules/:ruleId` | `approvals` | `Asked` + `Run::answer(Decision)` |
| [ ] | `dms.list` | GET | `@/dms` | `channels` | `sdk.agents(cwd)` |
| [ ] | `dms.open` | POST | `@/dms` | `channels` | `sdk.agents(cwd)` |
| [ ] | `dms.markRead` | POST | `@/channels/:channelId/read` | `channels` | `sdk.agents(cwd)` |
| [ ] | `dms.reads` | GET | `@/channels/:channelId/reads` | `channels` | `sdk.agents(cwd)` |
| [ ] | `threads.list` | GET | `@/threads` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `threads.inChannel` | GET | `@/channels/:channelId/threads` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `threads.get` | GET | `@/threads/:threadId` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `threads.create` | POST | `@/threads` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `threads.update` | PATCH | `@/threads/:threadId` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `threads.remove` | DELETE | `@/threads/:threadId` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `threads.act` | POST | `@/threads/:threadId/:x` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `threads.act` | PUT | `:x/command-approval` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `threads.act` | PUT | `@/agents/:agentId/mission-command-approval` | `threads` | `Sdk::sessions`, `rename/archive/delete_session` |
| [ ] | `questions.answer` | POST | `@/questions/:questionId/answer` | `approvals` | `AskKind::Questions` |
| [ ] | `questions.dismiss` | POST | `@/questions/:questionId/dismiss` | `approvals` | `AskKind::Questions` |

## 5 Interação (20)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|
| [ ] | `actionCards.get` | GET | `@/action-cards/:messageId` | `blocks` | — |
| [ ] | `actionCards.decide` | POST | `@/action-cards/:messageId/:x` | `blocks` | — |
| [ ] | `pins.list` | GET | `@/channels/:channelId/pins` | `channels` | — |
| [ ] | `pins.pin` | POST | `@/channels/:channelId/pins/:messageId` | `channels` | — |
| [ ] | `pins.unpin` | DELETE | `@/channels/:channelId/pins/:messageId` | `channels` | — |
| [ ] | `chat.reactions` | GET | `@/messages/:messageId/reactions` | `channels` | — |
| [ ] | `chat.react` | POST | `@/messages/:messageId/reactions` | `channels` | — |
| [ ] | `chat.unreact` | DELETE | `@/messages/:messageId/reactions` | `channels` | — |
| [ ] | `chat.correct` | POST | `@/messages/:messageId/correction` | `channels` | — |
| [ ] | `chat.answerNow` | POST | `@/messages/:messageId/answer-now` | `channels` | — |
| [ ] | `chat.followUps` | GET | `@/channels/:channelId/follow-ups` | `channels` | — |
| [ ] | `chat.cancelFollowUp` | POST | `@/messages/:messageId/cancel-follow-up` | `channels` | — |
| [ ] | `search.query` | GET | `@/search` | `search` | Postgres `tsvector` |
| [ ] | `artifacts.blockState` | GET | `@/blocks/:id/state` | `blocks` | — |
| [ ] | `artifacts.setBlockState` | PUT | `@/blocks/:id/state` | `blocks` | — |
| [ ] | `artifacts.drawBlock` | POST | `@/blocks/:id/draw` | `blocks` | — |
| [ ] | `artifacts.poll` | GET | `@/polls/:id` | `blocks` | — |
| [ ] | `artifacts.vote` | PUT | `@/polls/:id/vote` | `blocks` | — |
| [ ] | `artifacts.choices` | GET | `@/choices/:id` | `blocks` | — |
| [ ] | `artifacts.answerChoices` | POST | `@/choices/:id/answer` | `blocks` | — |

## 6 Agentes e settings (123)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|
| [ ] | `agentForYou.get` | GET | `@/agents/:agentId/for-you` | `agents` | — |
| [ ] | `agentGroups.list` | GET | `@/agent-groups` | `agents` | — |
| [ ] | `agentGroups.create` | POST | `@/agent-groups` | `agents` | — |
| [ ] | `agentGroups.update` | PATCH | `@/agent-groups/:groupId` | `agents` | — |
| [ ] | `agentGroups.remove` | DELETE | `@/agent-groups/:groupId` | `agents` | — |
| [ ] | `agentGroups.move` | PUT | `@/agents/:agentId/group` | `agents` | — |
| [ ] | `agents.list` | GET | `@/agents` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.planUsage` | GET | `@/agents/:agentId/usage` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.team` | GET | `@/team` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.setLines` | PUT | `@/agent-lines` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.proposeChannelMove` | POST | `@/team/channel-move` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.create` | POST | `@/agents` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.proposal` | GET | `@/agent-proposals/:messageId` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.denyProposal` | POST | `@/agent-proposals/:messageId/deny` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.createProposed` | POST | `@/agent-proposals/:messageId/create` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.askProposal` | POST | `@/agent-proposals/:messageId/ask` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.createTeam` | POST | `@/team-proposals/:messageId/create` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.denyTeam` | POST | `@/team-proposals/:messageId/deny` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.undoTeam` | POST | `@/team-proposals/:messageId/undo` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.adjustTeamAgent` | PATCH | `@/team-proposals/:messageId/agents/:handle` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.teamTemplates` | GET | `@/team-templates` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.applyTeamTemplate` | POST | `@/team-templates/:templateId/use` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.decideCostly` | POST | `@/autonomy-proposals/:signalId` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.proposalRoutine` | POST | `@/agent-proposals/:messageId/routine` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.update` | PATCH | `@/agents/:agentId` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.confirmSetup` | POST | `@/agents/:agentId/setup/:step` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.mailLoss` | POST | `@/agents/:agentId/audience/mail-loss` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.archive` | DELETE | `@/agents/:agentId` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.unarchive` | POST | `@/agents/:agentId/unarchive` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.purge` | DELETE | `@/agents/:agentId` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.former` | GET | `@/former-agents` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.models` | GET | `/api/app/models` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.credentials` | GET | `@/credentials` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.addCredential` | POST | `@/credentials` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.endpointModels` | POST | `@/credentials/endpoint-models` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.setEndpointModels` | PUT | `@/credentials/:id/models` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.rotateCredential` | POST | `@/credentials/:id/rotate` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.removeCredential` | DELETE | `@/credentials/:id` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.makeDefaultCredential` | POST | `@/credentials/:id/make-default` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.removeCredentialCopies` | POST | `@/credentials/:id/remove-copies` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.checkCredential` | POST | `@/credentials/:id/check` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.platformKeyAvailable` | GET | `@/credentials/platform` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.aiStatus` | GET | `@/ai-status` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.statuses` | GET | `@/agent-status` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.usageQueue` | GET | `@/agents/:agentId/usage-queue` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.cancelUsageQueue` | POST | `@/agents/:agentId/usage-queue/cancel` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.importFromGithub` | POST | `@/agent-imports/github` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.detail` | GET | `@/agents/:agentId` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.channels` | GET | `@/agents/:agentId/channels` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.setChannels` | PUT | `@/agents/:agentId/channels` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.brain` | GET | `@/agents/:agentId/brain…` | `agents` | `set_model`, `set_effort` |
| [ ] | `agents.setBrain` | PUT | `@/agents/:agentId/brain` | `agents` | `set_model`, `set_effort` |
| [ ] | `agents.brainMachines` | GET | `@/brain-machines` | `agents` | `set_model`, `set_effort` |
| [ ] | `agents.applySetup` | POST | `@/agent-setup` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.newAgentSetup` | GET | `@/agent-setup` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.runs` | GET | `@/runs` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.run` | GET | `@/runs/:runId` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.cancelRun` | POST | `@/runs/:runId/cancel` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.stopReason` | POST | `@/runs/:runId/stop-reason` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.continueRun` | POST | `@/runs/:runId/continue` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.activeRuns` | GET | `@/channels/:channelId/active-runs` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.consultations` | GET | `@/channels/:channelId/consultations` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.consultation` | GET | `@/channels/:channelId/consultations/:consultationId` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `agents.usage` | GET | `@/usage` | `agents` | `sdk.agents(cwd)`, `models()`, `providers()`, `Run::cancel` |
| [ ] | `connectors.catalog` | GET | `/api/app/connectors/catalog` | `connectors` | `mcp_servers`, `sign_in_mcp_server` |
| [ ] | `connectors.list` | GET | `@/connectors` | `connectors` | `mcp_servers`, `sign_in_mcp_server` |
| [ ] | `connectors.connectWithKey` | POST | `@/connectors` | `connectors` | `mcp_servers`, `sign_in_mcp_server` |
| [ ] | `connectors.startOAuth` | POST | `@/connectors/:connector/oauth/start` | `connectors` | `mcp_servers`, `sign_in_mcp_server` |
| [ ] | `connectors.composioUsage` | GET | `@/usage/composio` | `connectors` | `mcp_servers`, `sign_in_mcp_server` |
| [ ] | `connectors.completeOAuth` | POST | `@/connectors/oauth/complete` | `connectors` | `mcp_servers`, `sign_in_mcp_server` |
| [ ] | `connectors.rename` | PATCH | `@/connectors/:connectionId` | `connectors` | `mcp_servers`, `sign_in_mcp_server` |
| [ ] | `connectors.disconnect` | DELETE | `@/connectors/:connectionId` | `connectors` | `mcp_servers`, `sign_in_mcp_server` |
| [ ] | `memory.listAll` | GET | `@/memories` | `memory` | `ProjectMemory` |
| [ ] | `memory.listTeam` | GET | `@/team-memories` | `memory` | `ProjectMemory` |
| [ ] | `memory.makeTeam` | POST | `@/memories/:id/team` | `memory` | `ProjectMemory` |
| [ ] | `memory.list` | GET | `@/agents/:agentId/memories` | `memory` | `ProjectMemory` |
| [ ] | `memory.edit` | PATCH | `@/memories/:id` | `memory` | `ProjectMemory` |
| [ ] | `memory.setPinned` | PATCH | `@/memories/:id` | `memory` | `ProjectMemory` |
| [ ] | `memory.setArchived` | PATCH | `@/memories/:id` | `memory` | `ProjectMemory` |
| [ ] | `memory.confirm` | PATCH | `@/memories/:id` | `memory` | `ProjectMemory` |
| [ ] | `memory.revert` | POST | `@/memories/:id/revert` | `memory` | `ProjectMemory` |
| [ ] | `sandbox.settings` | GET | `@/sandbox-settings` | `settings` | `platform::sandbox` |
| [ ] | `sandbox.saveSettings` | PUT | `@/sandbox-settings` | `settings` | `platform::sandbox` |
| [ ] | `skills.list` | GET | `@/skills` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.get` | GET | `@/skills/:id` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.create` | POST | `@/skills` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.import` | POST | `@/skills/import` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.importPicked` | POST | `@/skills/import` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.update` | PATCH | `@/skills/:id` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.reimport` | POST | `@/skills/:id/reimport` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.remove` | DELETE | `@/skills/:id` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.forAgent` | GET | `@/agents/:agentId/skills` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.setForAgent` | PUT | `@/agents/:agentId/skills` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.learned` | GET | `@/agents/:agentId/learned-skills` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.revert` | POST | `@/skills/:id/revert` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.restore` | POST | `@/skills/:id/restore` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.updateFromSource` | POST | `@/skills/:id/update` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.approveUpdate` | POST | `@/skills/:id/update/approve` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.dismissUpdate` | POST | `@/skills/:id/update/dismiss` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.reopenUpdate` | POST | `@/skills/:id/update/reopen` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.undoUpdate` | POST | `@/skills/:id/update/undo` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.settings` | GET | `@/skill-settings` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `skills.saveSettings` | PUT | `@/skill-settings` | `skills` | `skills(cwd)` + escrita em disco |
| [ ] | `notifications.list` | GET | `@/notifications` | `notifications` | `notices()` |
| [ ] | `notifications.count` | GET | `@/notifications/count` | `notifications` | `notices()` |
| [ ] | `notifications.waiting` | GET | `@/waiting` | `notifications` | `notices()` |
| [ ] | `notifications.read` | POST | `@/notifications/:id/read` | `notifications` | `notices()` |
| [ ] | `notifications.done` | POST | `@/notifications/:id/done` | `notifications` | `notices()` |
| [ ] | `notifications.snooze` | POST | `@/notifications/:id/snooze` | `notifications` | `notices()` |
| [ ] | `notifications.readAll` | POST | `@/notifications/read-all` | `notifications` | `notices()` |
| [ ] | `notifications.doneAll` | POST | `@/notifications/done-all` | `notifications` | `notices()` |
| [ ] | `notifications.prefs` | GET | `@/notification-prefs` | `notifications` | `notices()` |
| [ ] | `notifications.setPref` | PUT | `@/notification-prefs` | `notifications` | `notices()` |
| [ ] | `notifications.askAdmins` | POST | `@/admin-asks` | `notifications` | `notices()` |
| [ ] | `customConnectors.list` | GET | `@/connectors/custom` | `connectors` | `add/remove_mcp_server` |
| [ ] | `customConnectors.connect` | POST | `@/connectors/custom` | `connectors` | `add/remove_mcp_server` |
| [ ] | `customConnectors.refresh` | POST | `@/connectors/custom/:serverId/refresh` | `connectors` | `add/remove_mcp_server` |
| [ ] | `customConnectors.saveTools` | PUT | `@/connectors/custom/:serverId/tools` | `connectors` | `add/remove_mcp_server` |
| [ ] | `customConnectors.disconnect` | DELETE | `@/connectors/custom/:serverId` | `connectors` | `add/remove_mcp_server` |
| [ ] | `proposals.list` | GET | `@/proposals…` | `memory` | `ProjectMemory::revise` |
| [ ] | `proposals.accept` | POST | `@/proposals/:id/accept` | `memory` | `ProjectMemory::revise` |
| [ ] | `proposals.refuse` | POST | `@/proposals/:id/refuse` | `memory` | `ProjectMemory::revise` |
| [ ] | `proposals.ask` | POST | `@/proposals/:id/ask` | `memory` | `ProjectMemory::revise` |

## 7 Espaço de trabalho (98)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|
| [ ] | `artifacts.list` | GET | `@/artifacts` | `drive` | — |
| [ ] | `artifacts.page` | GET | `@/artifacts` | `drive` | — |
| [ ] | `artifacts.get` | GET | `@/artifacts/:id` | `drive` | — |
| [ ] | `artifacts.keep` | POST | `@/artifacts/:id/keep` | `drive` | — |
| [ ] | `artifacts.reply` | POST | `@/artifacts/:id/reply` | `drive` | — |
| [ ] | `artifacts.versions` | GET | `@/artifacts/:id/versions` | `drive` | — |
| [ ] | `artifacts.download` | GET | `@/artifacts/:id/download` | `drive` | — |
| [ ] | `artifacts.create` | POST | `@/artifacts` | `drive` | — |
| [ ] | `artifacts.update` | POST | `@/artifacts/:id/versions` | `drive` | — |
| [ ] | `artifacts.setSharing` | PUT | `@/artifacts/:id/sharing` | `drive` | — |
| [ ] | `artifacts.remove` | DELETE | `@/artifacts/:id` | `drive` | — |
| [ ] | `artifacts.trash` | GET | `@/artifacts-trash` | `drive` | — |
| [ ] | `artifacts.restore` | POST | `@/artifacts/:id/restore` | `drive` | — |
| [ ] | `artifacts.purge` | DELETE | `@/artifacts-trash/:id` | `drive` | — |
| [ ] | `issues.list` | GET | `@/issues` | `issues` | — |
| [ ] | `issues.get` | GET | `@/issues/by-number/:x` | `issues` | — |
| [ ] | `issues.create` | POST | `@/issues` | `issues` | — |
| [ ] | `issues.update` | PATCH | `@/issues/:id` | `issues` | — |
| [ ] | `issues.delete` | DELETE | `@/issues/:id` | `issues` | — |
| [ ] | `issues.share` | POST | `@/issues/:id/share` | `issues` | — |
| [ ] | `issues.setAudience` | PUT | `@/issues/:id/audience` | `issues` | — |
| [ ] | `issues.setCriterion` | PUT | `@/issues/:id/criteria/:criterionId` | `issues` | — |
| [ ] | `issues.activity` | GET | `@/issues/:id/activity` | `issues` | — |
| [ ] | `issues.comment` | POST | `@/issues/:id/comments` | `issues` | — |
| [ ] | `issues.askAgent` | POST | `@/issues/:id/ask-agent` | `issues` | — |
| [ ] | `issues.conversation` | POST | `@/issues/:id/conversation` | `issues` | — |
| [ ] | `issues.columns` | GET | `@/issue-columns` | `issues` | — |
| [ ] | `issues.addColumn` | POST | `@/issue-columns` | `issues` | — |
| [ ] | `issues.updateColumn` | PATCH | `@/issue-columns/:columnId` | `issues` | — |
| [ ] | `issues.deleteColumn` | DELETE | `@/issue-columns/:columnId` | `issues` | — |
| [ ] | `issues.projects` | GET | `@/issue-projects` | `issues` | — |
| [ ] | `issues.createProject` | POST | `@/issue-projects` | `issues` | — |
| [ ] | `issues.updateProject` | PATCH | `@/issue-projects/:projectId` | `issues` | — |
| [ ] | `issues.deleteProject` | DELETE | `@/issue-projects/:projectId` | `issues` | — |
| [ ] | `threads.act` | GET | `@/missions` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | GET | `@/missions/eligible-agents` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `:x/join-requests` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `@/missions` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `@/threads/:threadId/tasks` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `:x/worktree/retry` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | PATCH | `:x/participants/:userId` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | DELETE | `:x/participants/:userId` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | PUT | `:x/machine` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | PUT | `:x/brain` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `:x/commands` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `:x/commands/retry` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `:x/deliveries/:deliveryId/accept` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `:x/decisions/:decisionId/choose` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `:x/decisions/approve-provisional` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | POST | `:x/reviews/:reviewId/:x` | `missions` | worktrees do `agent-engine` |
| [ ] | `threads.act` | GET | `:x/files` | `missions` | worktrees do `agent-engine` |
| [ ] | `wiki.tree` | GET | `@/wiki/pages` | `wiki` | — |
| [ ] | `wiki.get` | GET | `@/wiki/pages/:slug` | `wiki` | — |
| [ ] | `wiki.search` | GET | `@/wiki/search` | `wiki` | — |
| [ ] | `wiki.create` | POST | `@/wiki/pages` | `wiki` | — |
| [ ] | `wiki.update` | PUT | `@/wiki/pages/:slug` | `wiki` | — |
| [ ] | `wiki.rename` | POST | `@/wiki/pages/:slug/rename` | `wiki` | — |
| [ ] | `wiki.history` | GET | `@/wiki/pages/:slug/history` | `wiki` | — |
| [ ] | `wiki.version` | GET | `@/wiki/pages/:slug/versions/:x` | `wiki` | — |
| [ ] | `wiki.graph` | GET | `@/wiki/graph` | `wiki` | — |
| [ ] | `wiki.move` | POST | `@/wiki/pages/:slug/move` | `wiki` | — |
| [ ] | `wiki.lock` | POST | `@/wiki/pages/:slug/lock` | `wiki` | — |
| [ ] | `wiki.pin` | POST | `@/wiki/pages/:slug/pin` | `wiki` | — |
| [ ] | `wiki.audience` | PUT | `@/wiki/pages/:slug/audience` | `wiki` | — |
| [ ] | `wiki.folderAudience` | GET | `@/wiki/folder-audience?folder=:folder` | `wiki` | — |
| [ ] | `wiki.setFolderAudience` | PUT | `@/wiki/folder-audience` | `wiki` | — |
| [ ] | `wiki.remove` | DELETE | `@/wiki/pages/:slug` | `wiki` | — |
| [ ] | `wiki.removeMany` | POST | `@/wiki/bulk/delete` | `wiki` | — |
| [ ] | `wiki.trash` | GET | `@/wiki/pages` | `wiki` | — |
| [ ] | `wiki.restoreMany` | POST | `@/wiki/trash/restore` | `wiki` | — |
| [ ] | `wiki.purge` | POST | `@/wiki/trash/purge` | `wiki` | — |
| [ ] | `wiki.setAgents` | PUT | `@/wiki/bulk/agents` | `wiki` | — |
| [ ] | `wiki.context` | POST | `@/wiki/pages/:slug/context` | `wiki` | — |
| [ ] | `wiki.reads` | GET | `@/wiki/reads` | `wiki` | — |
| [ ] | `wiki.import` | POST | `@/wiki/import` | `wiki` | — |
| [ ] | `wiki.append` | POST | `@/wiki/pages/:slug/append` | `wiki` | — |
| [ ] | `dashboards.list` | GET | `@/dashboards` | `dashboards` | — |
| [ ] | `dashboards.get` | GET | `@/dashboards/:dashboardId` | `dashboards` | — |
| [ ] | `dashboards.home` | GET | `@/dashboards/home` | `dashboards` | — |
| [ ] | `dashboards.setHome` | GET | `@/dashboards/home-choice` | `dashboards` | — |
| [ ] | `dashboards.create` | POST | `@/dashboards` | `dashboards` | — |
| [ ] | `dashboards.update` | PATCH | `@/dashboards/:dashboardId` | `dashboards` | — |
| [ ] | `dashboards.share` | PUT | `…` | `dashboards` | — |
| [ ] | `dashboards.remove` | DELETE | `@/dashboards/:dashboardId` | `dashboards` | — |
| [ ] | `dashboards.saveLayout` | PUT | `…` | `dashboards` | — |
| [ ] | `dashboards.decideProposal` | POST | `…` | `dashboards` | — |
| [ ] | `dashboards.createWidget` | POST | `…` | `dashboards` | — |
| [ ] | `dashboards.testWidget` | POST | `…` | `dashboards` | — |
| [ ] | `dashboards.confirmWidget` | POST | `…` | `dashboards` | — |
| [ ] | `dashboards.pinWidget` | POST | `…` | `dashboards` | — |
| [ ] | `dashboards.widget` | GET | `@/widgets/:widgetId` | `dashboards` | — |
| [ ] | `dashboards.updateWidget` | PATCH | `@/widgets/:widgetId` | `dashboards` | — |
| [ ] | `dashboards.removeWidget` | DELETE | `@/widgets/:widgetId` | `dashboards` | — |
| [ ] | `dashboards.refreshWidget` | POST | `…` | `dashboards` | — |
| [ ] | `dashboards.askWidgetChange` | POST | `…` | `dashboards` | — |
| [ ] | `dashboards.widgetCost` | GET | `@/widget-cost` | `dashboards` | — |
| [ ] | `dashboards.widgetSuggestions` | GET | `@/widget-suggestions/:agentId` | `dashboards` | — |
| [ ] | `dashboards.pinnableBlocks` | GET | `@/widget-pinnable` | `dashboards` | — |

## 8 Integrações (0)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|

## 9 Browser (22)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|
| [ ] | `browser.settings` | GET | `@/agents/:agentId/browser` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.setDomains` | PUT | `@/agents/:agentId/browser` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.agentHandoff` | GET | `@/agents/:agentId/:x/handoff` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.agentControl` | GET | `@/agents/:agentId/:x/control` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.handoff` | GET | `@/browser/handoffs/:handoffId` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.takeControl` | POST | `@/browser/handoffs/:handoffId/control` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.done` | POST | `@/browser/handoffs/:handoffId/done` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.failed` | POST | `@/browser/handoffs/:handoffId/failed` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.cancel` | POST | `@/browser/handoffs/:handoffId/cancel` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.takeover` | POST | `@/agents/:agentId/:x/takeover` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.open` | POST | `@/agents/:agentId/browser/open` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.stop` | POST | `@/agents/:agentId/browser/stop` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browser.restart` | POST | `@/agents/:agentId/browser/restart` | `browser` | ferramenta nova no `agent-engine` |
| [ ] | `browserProfiles.list` | GET | `@/browser-profiles` | `browser` | — |
| [ ] | `browserProfiles.create` | POST | `@/browser-profiles` | `browser` | — |
| [ ] | `browserProfiles.update` | PATCH | `@/browser-profiles/:profileId` | `browser` | — |
| [ ] | `browserProfiles.remove` | DELETE | `@/browser-profiles/:profileId` | `browser` | — |
| [ ] | `browserProfiles.takeover` | POST | `@/browser-profiles/:profileId/takeover` | `browser` | — |
| [ ] | `browserProfiles.handback` | POST | `@/browser-profiles/:profileId/handback` | `browser` | — |
| [ ] | `browserProfiles.open` | POST | `@/browser-profiles/:profileId/open` | `browser` | — |
| [ ] | `browserProfiles.setDefault` | PUT | `@/agents/:agentId/browser-profile` | `browser` | — |
| [ ] | `browserProfiles.request` | POST | `@/agents/:agentId/browser-profile/request` | `browser` | — |

## Fora do escopo (247)

| ✓ | Cliente | Método | Rota | Feature | Fonte |
|---|---|---|---|---|---|
| — | `mail.providers` | GET | `@/mail/providers` | `—` | — |
| — | `mail.accounts` | GET | `@/mail/accounts` | `—` | — |
| — | `mail.connectImap` | POST | `@/mail/accounts/imap` | `—` | — |
| — | `mail.connectComposio` | POST | `@/mail/accounts/composio` | `—` | — |
| — | `mail.update` | PATCH | `@/mail/accounts/:accountId` | `—` | — |
| — | `mail.disconnect` | DELETE | `@/mail/accounts/:accountId` | `—` | — |
| — | `mail.suggestedAddresses` | GET | `@/mail/accounts/:accountId/suggested-addresses` | `—` | — |
| — | `mail.folders` | GET | `@/mail/accounts/:accountId/folders` | `—` | — |
| — | `mail.agent` | GET | `@/agents/:agentId/mail` | `—` | — |
| — | `mail.setAgent` | PUT | `@/agents/:agentId/mail/:accountId` | `—` | — |
| — | `databases.list` | GET | `@/databases` | `—` | — |
| — | `databases.connect` | POST | `@/databases` | `—` | — |
| — | `databases.update` | PATCH | `@/databases/:connectionId` | `—` | — |
| — | `databases.disconnect` | DELETE | `@/databases/:connectionId` | `—` | — |
| — | `databases.tables` | GET | `@/databases/:connectionId/tables…` | `—` | — |
| — | `databases.serverDatabases` | GET | `@/databases/:connectionId/databases:x` | `—` | — |
| — | `databases.agent` | GET | `@/agents/:agentId/databases` | `—` | — |
| — | `databases.setAgent` | PUT | `@/agents/:agentId/databases/:connectionId` | `—` | — |
| — | `desktop.status` | GET | `@/agents/:agentId/desktop` | `—` | — |
| — | `desktop.picture` | GET | `@/agents/:agentId/desktop/picture` | `—` | — |
| — | `desktop.holds` | GET | `@/agents/:agentId/holds` | `—` | — |
| — | `desktop.stopHold` | DELETE | `@/agents/:agentId/holds/:holdId` | `—` | — |
| — | `vault.overview` | GET | `@/vault` | `—` | — |
| — | `vault.item` | GET | `@/vault/items/:itemId` | `—` | — |
| — | `vault.create` | POST | `@/vault/items` | `—` | — |
| — | `vault.update` | PATCH | `@/vault/items/:itemId` | `—` | — |
| — | `vault.remove` | DELETE | `@/vault/items/:itemId` | `—` | — |
| — | `vault.importCsv` | POST | `@/vault/import` | `—` | — |
| — | `vault.importKeys` | POST | `@/vault/keys` | `—` | — |
| — | `vault.move` | POST | `@/vault/items/:itemId/move` | `—` | — |
| — | `vault.connect` | POST | `@/vault/connections` | `—` | — |
| — | `vault.updateConnection` | PATCH | `@/vault/connections/:connectionId` | `—` | — |
| — | `vault.syncConnection` | POST | `@/vault/connections/:connectionId/sync` | `—` | — |
| — | `vault.removeConnection` | DELETE | `@/vault/connections/:connectionId` | `—` | — |
| — | `gifs.search` | GET | `@/gifs` | `—` | — |
| — | `reminders.create` | POST | `@/reminders` | `—` | — |
| — | `reminders.list` | GET | `@/reminders` | `—` | — |
| — | `reminders.cancel` | DELETE | `@/reminders/:reminderId` | `—` | — |
| — | `system.gifKey` | GET | `/api/app/gif-key` | `—` | — |
| — | `system.saveGifKey` | PUT | `/api/app/gif-key` | `—` | — |
| — | `system.removeGifKey` | DELETE | `/api/app/gif-key` | `—` | — |
| — | `system.speechKey` | GET | `/api/app/speech-key` | `—` | — |
| — | `system.saveSpeechKey` | PUT | `/api/app/speech-key` | `—` | — |
| — | `system.removeSpeechKey` | DELETE | `/api/app/speech-key` | `—` | — |
| — | `account.authConfig` | GET | `/api/app/auth-config` | `—` | — |
| — | `account.signInOptions` | POST | `/api/app/sign-in-options` | `—` | — |
| — | `account.me` | GET | `/api/app/me` | `—` | — |
| — | `account.unread` | GET | `/api/app/me/unread` | `—` | — |
| — | `account.deviceAccounts` | GET | `/api/app/device-accounts` | `—` | — |
| — | `account.switchDeviceAccount` | POST | `/api/app/device-accounts/switch` | `—` | — |
| — | `account.signOutDeviceAccount` | POST | `/api/app/device-accounts/sign-out` | `—` | — |
| — | `account.updateMe` | PATCH | `/api/app/me` | `—` | — |
| — | `account.setWorkArea` | PUT | `/api/app/me/work-area` | `—` | — |
| — | `account.phoneSignIn` | POST | `/api/app/phone-sign-in` | `—` | — |
| — | `account.addPassword` | POST | `/api/app/me/password` | `—` | — |
| — | `account.deletionPlan` | GET | `/api/app/account/deletion` | `—` | — |
| — | `account.deletionCode` | POST | `/api/app/account/deletion/code` | `—` | — |
| — | `account.delete` | DELETE | `/api/app/account` | `—` | — |
| — | `account.restore` | DELETE | `/api/app/account/deletion` | `—` | — |
| — | `account.changeEmail` | POST | `/api/app/account/email` | `—` | — |
| — | `account.undoEmailChange` | POST | `/api/app/email-undo` | `—` | — |
| — | `push.webKey` | GET | `/api/app/push/web` | `—` | — |
| — | `push.register` | POST | `/api/app/push/devices` | `—` | — |
| — | `push.unregister` | POST | `/api/app/push/devices/forget` | `—` | — |
| — | `push.stale` | POST | `/api/app/push/stale` | `—` | — |
| — | `orgs.deletionCode` | POST | `@/deletion/code` | `—` | — |
| — | `orgs.deleteWorkspace` | POST | `@/deletion` | `—` | — |
| — | `orgs.restore` | DELETE | `@/deletion` | `—` | — |
| — | `orgs.billingLink` | POST | `@/billing-link` | `—` | — |
| — | `orgs.members` | GET | `@/members` | `—` | — |
| — | `orgs.changeRole` | PATCH | `@/members/:userId` | `—` | — |
| — | `orgs.removal` | GET | `@/members/:userId/removal` | `—` | — |
| — | `orgs.removeMember` | DELETE | `@/members/:userId` | `—` | — |
| — | `orgs.passwordLink` | POST | `@/members/:userId/password-link` | `—` | — |
| — | `orgs.invitations` | GET | `@/invitations` | `—` | — |
| — | `orgs.invite` | POST | `@/invitations` | `—` | — |
| — | `orgs.revokeInvitation` | DELETE | `@/invitations/:id` | `—` | — |
| — | `orgs.previewInvitation` | GET | `/api/app/invitations/:token` | `—` | — |
| — | `orgs.invitationState` | GET | `/api/app/invitations/:token/state` | `—` | — |
| — | `orgs.acceptInvitation` | POST | `/api/app/invitations/:token/accept…` | `—` | — |
| — | `orgs.people` | GET | `@/people` | `—` | — |
| — | `orgs.allInvitations` | GET | `@/invitations/all` | `—` | — |
| — | `orgs.inviteMany` | POST | `@/invitations/batch` | `—` | — |
| — | `orgs.resendInvitation` | POST | `@/invitations/:id/resend` | `—` | — |
| — | `orgs.updateInvitation` | PATCH | `@/invitations/:id` | `—` | — |
| — | `orgs.peopleUsage` | GET | `@/usage/people` | `—` | — |
| — | `orgs.personUsage` | GET | `@/usage/people/:userId` | `—` | — |
| — | `orgs.personLimits` | GET | `@/usage/limits` | `—` | — |
| — | `orgs.setPersonLimits` | PUT | `@/usage/limits` | `—` | — |
| — | `orgs.requestMoreUsage` | POST | `@/usage/limits/request` | `—` | — |
| — | `orgs.raisePersonLimit` | POST | `@/usage/people/:userId/raise` | `—` | — |
| — | `orgs.refuseMoreUsage` | POST | `@/usage/people/:userId/refuse` | `—` | — |
| — | `orgs.teamWeek` | GET | `@/team-week` | `—` | — |
| — | `orgs.setTeamWeek` | PUT | `@/team-week` | `—` | — |
| — | `orgs.actOnTeamWeek` | POST | `@/team-week/:messageId` | `—` | — |
| — | `orgs.audit` | GET | `@/audit` | `—` | — |
| — | `roles.list` | GET | `@/roles` | `—` | — |
| — | `roles.create` | POST | `@/roles` | `—` | — |
| — | `roles.update` | PATCH | `@/roles/:roleId` | `—` | — |
| — | `roles.remove` | DELETE | `@/roles/:roleId` | `—` | — |
| — | `roles.reorder` | PUT | `@/roles/order` | `—` | — |
| — | `roles.setFor` | PUT | `@/members/:userId/roles` | `—` | — |
| — | `roles.setRow` | PUT | `@/permissions/:row` | `—` | — |
| — | `roles.mailLoss` | POST | `@/roles/:roleId/members/preview` | `—` | — |
| — | `imports.apply` | POST | `@/imports` | `—` | — |
| — | `imports.sources` | GET | `@/imports` | `—` | — |
| — | `voice.allowed` | GET | `@/voice/allowed` | `—` | — |
| — | `chat.answerGitHub` | POST | `@/messages/:messageId/github-answer` | `—` | — |
| — | `approvals.covers` | GET | `@/me/approval-cover` | `—` | — |
| — | `approvals.setCover` | PUT | `@/me/approval-cover` | `—` | — |
| — | `approvals.endCover` | DELETE | `@/approval-covers/:userId` | `—` | — |
| — | `approvals.rightHand` | GET | `@/me/right-hand-approvals` | `—` | — |
| — | `approvals.setRightHand` | PUT | `@/me/right-hand-approvals` | `—` | — |
| — | `approvals.undoRightHand` | POST | `@/right-hand-decisions/:decisionId/undo` | `—` | — |
| — | `githubApp.info` | GET | `@/github-app` | `—` | — |
| — | `githubApp.install` | POST | `@/github-app/install` | `—` | — |
| — | `githubApp.complete` | POST | `@/github-app/complete` | `—` | — |
| — | `githubApp.remove` | DELETE | `@/github-app/installations/:id` | `—` | — |
| — | `githubApp.startManifest` | POST | `/api/app/github-app/manifest` | `—` | — |
| — | `githubApp.removeSettings` | DELETE | `/api/app/github-app/settings` | `—` | — |
| — | `githubApp.repositories` | GET | `@/github/repositories` | `—` | — |
| — | `githubApp.access` | GET | `@/github/access` | `—` | — |
| — | `sandbox.computer` | GET | `@/agents/:agentId/computer` | `—` | — |
| — | `sandbox.setTarget` | PUT | `@/agents/:agentId/computer/target` | `—` | — |
| — | `devices.list` | GET | `@/devices` | `—` | — |
| — | `devices.updateComputer` | PATCH | `@/devices/computers/:machineId` | `—` | — |
| — | `devices.removeComputer` | DELETE | `@/devices/computers/:machineId` | `—` | — |
| — | `devices.updatePhone` | PATCH | `@/devices/phones/:phoneId` | `—` | — |
| — | `devices.forgetPhone` | DELETE | `@/devices/phones/:phoneId` | `—` | — |
| — | `devices.stopPhone` | POST | `@/devices/phones/:phoneId/stop` | `—` | — |
| — | `devices.createLink` | POST | `@/devices/connect-links` | `—` | — |
| — | `devices.link` | GET | `@/devices/connect-links/:linkId` | `—` | — |
| — | `devices.approveRequest` | POST | `@/devices/requests/:requestId/approve` | `—` | — |
| — | `devices.denyRequest` | POST | `@/devices/requests/:requestId/deny` | `—` | — |
| — | `devices.decideAccess` | POST | `@/devices/access-requests/:requestId/decide` | `—` | — |
| — | `devices.deviceWait` | POST | `@/runs/:runId/device-wait` | `—` | — |
| — | `devices.startWifiPairing` | POST | `@/devices/computers/:machineId/wifi-pairings` | `—` | — |
| — | `devices.wifiPairing` | GET | `@/devices/computers/:machineId/wifi-pairings/:pairingId` | `—` | — |
| — | `devices.cancelWifiPairing` | DELETE | `@/devices/computers/:machineId/wifi-pairings/:pairingId` | `—` | — |
| — | `devices.lookForPhones` | POST | `@/devices/computers/:machineId/phone-looks` | `—` | — |
| — | `devices.wifiIphones` | GET | `@/devices/computers/:machineId/wifi-iphones` | `—` | — |
| — | `devices.pickIphoneOverWifi` | PUT | `@/devices/computers/:machineId/wifi-iphones/:serial` | `—` | — |
| — | `devices.startXcodeFix` | POST | `@/devices/computers/:machineId/xcode-fix` | `—` | — |
| — | `devices.xcodeFix` | GET | `@/devices/computers/:machineId/xcode-fix` | `—` | — |
| — | `devices.cancelXcodeFix` | DELETE | `@/devices/computers/:machineId/xcode-fix` | `—` | — |
| — | `devices.openXcode` | POST | `@/devices/computers/:machineId/xcode-open` | `—` | — |
| — | `machines.list` | GET | `@/machines` | `—` | — |
| — | `machines.rename` | PATCH | `@/machines/:machineId` | `—` | — |
| — | `machines.remove` | DELETE | `@/machines/:machineId` | `—` | — |
| — | `machines.host` | GET | `@/machines/host` | `—` | — |
| — | `machines.hostOn` | POST | `@/machines/host` | `—` | — |
| — | `machines.computerSetup` | GET | `@/computer-setup` | `—` | — |
| — | `machines.device` | GET | `/api/app/machines/device/:userCode` | `—` | — |
| — | `machines.confirmDevice` | POST | `@/machines/device/:userCode/confirm` | `—` | — |
| — | `machines.denyDevice` | POST | `/api/app/machines/device/:userCode/deny` | `—` | — |
| — | `machines.reduceAccess` | POST | `@/machines/:machineId/access/reduce` | `—` | — |
| — | `machines.allowTool` | POST | `@/machines/:machineId/access/allow` | `—` | — |
| — | `machines.askTool` | POST | `@/machines/:machineId/access/ask` | `—` | — |
| — | `machines.checkClis` | POST | `@/machines/:machineId/clis/check` | `—` | — |
| — | `machines.cliInstall` | DELETE | `@/machines/:machineId/cli-install?tool=:tool` | `—` | — |
| — | `machines.askCliInstall` | POST | `@/machines/:machineId/cli-install/ask` | `—` | — |
| — | `machines.setPause` | POST | `@/machines/:machineId/:x` | `—` | — |
| — | `machines.desktopLatest` | GET | `/api/desktop/latest` | `—` | — |
| — | `machines.outages` | GET | `@/machines/outages` | `—` | — |
| — | `machines.queue` | GET | `@/machines/:machineId/queue` | `—` | — |
| — | `machines.moveAgents` | POST | `@/machines/:machineId/move-agents` | `—` | — |
| — | `machines.beginAccountLogin` | POST | `@/machines/:machineId/account-logins` | `—` | — |
| — | `machines.sendAccountCode` | POST | `@/machines/:machineId/account-logins/:loginId/code` | `—` | — |
| — | `machines.accountLogin` | GET | `@/machines/:machineId/account-logins/:loginId` | `—` | — |
| — | `machines.cancelAccountLogin` | DELETE | `@/machines/:machineId/account-logins/:loginId` | `—` | — |
| — | `machines.refreshAccountUsage` | POST | `@/ai-accounts/refresh` | `—` | — |
| — | `machines.aiAccounts` | GET | `@/ai-accounts` | `—` | — |
| — | `machines.takeBackPlan` | POST | `@/ai-accounts/:accountId/take-back` | `—` | — |
| — | `machines.setAccountServes` | PUT | `@/ai-accounts/:accountId/serves` | `—` | — |
| — | `machines.renameAccount` | PATCH | `@/machines/:machineId/accounts/:accountId` | `—` | — |
| — | `machines.removeAccount` | DELETE | `@/machines/:machineId/accounts/:accountId` | `—` | — |
| — | `apps.consent` | GET | `/api/app/authorize/:requestId` | `—` | — |
| — | `apps.decide` | POST | `/api/app/authorize/:requestId` | `—` | — |
| — | `apps.list` | GET | `@/apps` | `—` | — |
| — | `apps.disconnect` | DELETE | `@/apps/:grantId` | `—` | — |
| — | `apps.createToken` | POST | `@/tokens` | `—` | — |
| — | `apps.revokeToken` | DELETE | `@/tokens/:grantId` | `—` | — |
| — | `access.get` | GET | `@/agents/:agentId/access` | `—` | — |
| — | `access.set` | PUT | `@/agents/:agentId/access` | `—` | — |
| — | `invites.createLink` | POST | `@/invite-links` | `—` | — |
| — | `invites.editLink` | PATCH | `@/invite-links/:linkId` | `—` | — |
| — | `invites.links` | GET | `@/invite-links` | `—` | — |
| — | `invites.turnOff` | DELETE | `@/invite-links/:linkId` | `—` | — |
| — | `invites.renewLink` | POST | `@/invite-links/:linkId/renew` | `—` | — |
| — | `invites.sendCode` | POST | `/api/app/invitations/:token/code` | `—` | — |
| — | `invites.checkCode` | POST | `/api/app/invitations/:token/code/check` | `—` | — |
| — | `invites.ask` | POST | `/api/app/invitations/:token/requests` | `—` | — |
| — | `invites.requestNew` | POST | `/api/app/invitations/:token/request-new` | `—` | — |
| — | `invites.claim` | POST | `/api/app/invitations/:token/requests/claim` | `—` | — |
| — | `joinRequests.list` | GET | `@/join-requests` | `—` | — |
| — | `joinRequests.approve` | POST | `@/join-requests/:id/approve` | `—` | — |
| — | `joinRequests.deny` | POST | `@/join-requests/:id/deny` | `—` | — |
| — | `joinRequests.suggest` | POST | `@/join-requests/suggest` | `—` | — |
| — | `joinRequests.mine` | GET | `@/join-requests/mine` | `—` | — |
| — | `joinRequests.cancel` | DELETE | `@/join-requests/:id` | `—` | — |
| — | `storage.get` | GET | `@/storage` | `—` | — |
| — | `storage.test` | POST | `@/storage/test` | `—` | — |
| — | `storage.save` | PUT | `@/storage` | `—` | — |
| — | `storage.migrate` | POST | `@/storage/migrate` | `—` | — |
| — | `storage.migration` | GET | `@/storage/migration` | `—` | — |
| — | `routines.list` | GET | `@/routines` | `—` | — |
| — | `routines.parseSchedule` | POST | `@/routines/parse-schedule` | `—` | — |
| — | `routines.create` | POST | `@/routines` | `—` | — |
| — | `routines.setActive` | PATCH | `@/routines/:id` | `—` | — |
| — | `routines.runNow` | POST | `@/routines/:id/run` | `—` | — |
| — | `routines.stop` | POST | `@/routines/:id/stop` | `—` | — |
| — | `routines.takeOver` | POST | `@/routines/:id/take-over` | `—` | — |
| — | `routines.continueUsage` | POST | `@/routines/:id/continue` | `—` | — |
| — | `routines.runs` | GET | `@/routines/:id/runs` | `—` | — |
| — | `routines.parseRule` | POST | `@/routines/parse-rule` | `—` | — |
| — | `routines.describeRule` | POST | `@/routines/describe-rule` | `—` | — |
| — | `routines.update` | PATCH | `@/routines/:id` | `—` | — |
| — | `routines.remove` | DELETE | `@/routines/:id` | `—` | — |
| — | `routines.duplicate` | POST | `@/routines/:id/duplicate` | `—` | — |
| — | `routines.hook` | GET | `@/routines/:id/hook` | `—` | — |
| — | `routines.newHookAddress` | POST | `@/routines/:id/hook/address` | `—` | — |
| — | `routines.newHookSecret` | POST | `@/routines/:id/hook/secret` | `—` | — |
| — | `routines.removeHookSecret` | DELETE | `@/routines/:id/hook/secret` | `—` | — |
| — | `routines.testHook` | POST | `@/routines/:id/hook/test` | `—` | — |
| — | `routines.webhookSetup` | GET | `@/webhooks/:provider` | `—` | — |
| — | `routines.ensureWebhook` | PUT | `@/webhooks/:provider` | `—` | — |
| — | `routines.setWebhookSecret` | PUT | `@/webhooks/:provider/secret` | `—` | — |
| — | `routines.addWebhookToRepo` | POST | `@/webhooks/github/repository` | `—` | — |
| — | `calendar.range` | GET | `@/calendar` | `—` | — |
| — | `calendar.create` | POST | `@/calendar/events` | `—` | — |
| — | `calendar.confirmProposal` | POST | `@/calendar/proposals/:id/confirm` | `—` | — |
| — | `calendar.dismissProposal` | POST | `@/calendar/proposals/:id/dismiss` | `—` | — |
| — | `calendar.respond` | POST | `@/calendar/events/:id/rsvp` | `—` | — |
| — | `calendar.update` | PATCH | `@/calendar/events/:id` | `—` | — |
| — | `calendar.remove` | DELETE | `@/calendar/events/:id` | `—` | — |
| — | `secrets.get` | GET | `@/secret-requests/:requestId` | `—` | — |
| — | `secrets.deliver` | POST | `@/secret-requests/:requestId/deliver` | `—` | — |
| — | `secrets.decline` | POST | `@/secret-requests/:requestId/decline` | `—` | — |
| — | `secrets.list` | GET | `@/agents/:agentId/secrets` | `—` | — |
| — | `secrets.remove` | DELETE | `@/agents/:agentId/secrets/:name…` | `—` | — |
| — | `secrets.saveToVault` | POST | `@/secret-requests/:requestId/vault` | `—` | — |
| — | `repositories.get` | GET | `@/agents/:agentId/repositories` | `—` | — |
| — | `repositories.set` | PUT | `@/agents/:agentId/repositories` | `—` | — |
| — | `repositories.merge` | GET | `@/agents/:agentId/merge-repositories` | `—` | — |
| — | `repositories.setMerge` | PUT | `@/agents/:agentId/merge-repositories` | `—` | — |
| — | `scopes.get` | GET | `@/agents/:agentId/scopes` | `—` | — |
| — | `scopes.set` | PUT | `@/agents/:agentId/scopes` | `—` | — |

