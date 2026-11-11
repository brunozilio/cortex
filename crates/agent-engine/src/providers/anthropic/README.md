# Anthropic API

Id `anthropic`. Connects with an API key. Turns go to the Messages API
(`messages` wire) at `api.anthropic.com`; the catalog pages
`GET /v1/models` by `after_id` and fails on a cursor that does not move.
Prices, short model names and thinking rules come from the `anthropic`
vendor.
