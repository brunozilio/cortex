# Kiro Subscription

Id `kiro-subscription`. Signs in through the browser via
`app.kiro.dev/signin`, callback `http://localhost:49153/oauth/callback`.

Kiro is spoken as Kiro CLI 3.1.0 does. Sign-in opens Kiro's portal with a
PKCE challenge and `redirect_uri=http://localhost:49153` (Kiro's sign-in
accepts only that port); the portal returns
`/oauth/callback?login_option=…&code=…`, and Google or GitHub codes are
exchanged at `prod.us-east-1.auth.desktop.kiro.dev/oauth/token` for the
redirect including `?login_option=…`. The answer's `profileArn` names the
account and its region; tokens refresh at `/refreshToken` (also on a 401 or
403). Organization (IAM Identity Center) and Builder ID sign-ins are
refused for now.

Turns are `GenerateAssistantResponse` on `runtime.<region>.kiro.dev` (AWS
JSON 1.0, `x-amz-target`), answered as AWS event-stream frames; that wire
lives in this folder (`wire/`), since only Kiro speaks it. The catalog is
`ListAvailableModels` on `management.<region>.kiro.dev` with
`origin: KIRO_CLI`. Kiro has no system field, so the system prompt opens the
first user message, and it reports context use as a share of the window,
from which the prompt's size is estimated.
