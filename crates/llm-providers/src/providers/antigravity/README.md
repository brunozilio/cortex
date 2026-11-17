# Antigravity Subscription

Id `antigravity-subscription`. Signs in through Google in the browser,
callback `http://localhost:51121/oauth-callback`.

Antigravity is spoken as Antigravity Hub 2.24.0 does. Sign-in is Google
OAuth with PKCE, `access_type=offline` and `prompt=consent`, using
Antigravity's own client; the code is exchanged at
`oauth2.googleapis.com/token`. The account's Cloud Code project then comes
from `v1internal:loadCodeAssist` (`cloudaicompanionProject`, a name or an
object with `id`); without one the account is onboarded
(`v1internal:onboardUser` with the default allowed tier, polled every five
seconds up to ten times), and the shared default project is the last resort.
The project is kept as the credential's account; the email comes from
Google's user info. Tokens refresh at the same token endpoint (also on a 401).

Turns are `v1internal:streamGenerateContent?alt=sse` on
`daily-cloudcode-pa.googleapis.com`: a Gemini `generateContent` request
inside an envelope (`project`, `model`, `request`, `requestType: agent`,
`userAgent: antigravity`). That wire (`cloud_code`) lives in this folder
(`wire/`), since only this provider speaks it; each event carries a Gemini
answer under `response`. Calls and results are paired by id and name.
A thought signature is kept as an empty thinking block just before the part
it came with and replayed there; the first call of a reply without one gets
`skip_thought_signature_validator`. Claude models get a thinking budget and
`VALIDATED` function calling; Gemini 3 and later take the thinking level from
the model's name (`-low`, `-medium`, `-high`) or the effort.

The catalog is `v1internal:fetchAvailableModels` for the project; when it
answers nothing usable, a fixed list of the models Antigravity advertises is
offered.
