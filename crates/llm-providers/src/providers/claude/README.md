# Claude Subscription

Id `claude-subscription`. Signs in through the browser with callback
`http://localhost:<free port>/callback`. Turns go to the Messages API
(`messages` wire) signed in with the OAuth beta headers; when the account's
catalog answers empty, a fixed list of current models is offered.
