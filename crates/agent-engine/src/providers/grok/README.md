# Grok Subscription

Id `grok-subscription`. Uses a SuperGrok or X Premium subscription.
Signs in with xAI's device flow: `auth.x.ai/oauth2/device/code` gives a code
and a link to approve it, then `auth.x.ai/oauth2/token` is polled
(`authorization_pending`, `slow_down`) until it answers with the tokens.
The client is the one xAI's own Grok CLI and the other coding agents use
(`b1a00492-…`), with scopes `openid profile email offline_access
grok-cli:access api:access`; without `api:access` turns are refused with 403.

Turns go to xAI's Responses API (`responses` wire) at `api.x.ai/v1` with the
access token as bearer. Tokens last about six hours and are renewed with a
form `refresh_token` grant before they expire and once after a 401; the
refresh token rotates on every use, so the new one is kept. The catalog is
`GET /models`, keeping Grok chat models (no image, video, speech or
embedding models), with the largest of `context_windows`, `context_window`
or `context_length` as the window.

Reasoning rules come from the `xai` vendor: only models that accept
`reasoning.effort` get one (`grok-3-mini`, `grok-4.20-multi-agent`,
`grok-4.3`, `grok-4.5`, `grok-4.6`, or what the catalog says), sent without a
summary; `grok-4.6` accepts `xhigh`, the others stop at `high`. Other Grok
models reason on their own and get no `reasoning` field, which xAI refuses
for them.
