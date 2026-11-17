# GitHub Copilot Subscription

Id `copilot-subscription`. Signs in with GitHub's device flow (a code typed
at `github.com/login/device`) or a GitHub token.

Copilot is spoken as the Copilot CLI does (client `Ov23ctDVkRmgkPke0Mmm`,
`Copilot-Integration-Id: copilot-developer-cli`): the GitHub token goes as
is to the Copilot API named by `copilot_internal/user` (`endpoints.api`); it
is never exchanged or refreshed, and classic `ghp_` tokens are refused.
`GET /models` lists what the picker shows; each model runs on the best wire
its `supported_endpoints` offer (`/v1/messages`, then `/responses`, then
`/chat/completions`). Claude models on `/v1/messages` follow the CLI's
`getAnthropicRequestParams`: a model with reasoning effort gets adaptive
thinking and `output_config.effort` (clamped to the levels it offers), one
that only thinks gets a 1024/2048/4096/32000-token budget. `X-Initiator` is
`user` only when the turn starts with the user's own message, so tool
round-trips are not premium requests.
