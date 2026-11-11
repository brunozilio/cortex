# ChatGPT Subscription

Id `openai-subscription`. Signs in through the browser with callback
`http://localhost:1455/auth/callback`; the account id comes from the
token. Turns go to the Codex backend (`responses` wire) at
`chatgpt.com/backend-api/codex`, always with a reasoning effort; tokens are
renewed before they expire and once after a 401. The catalog is
`/codex/models`.
