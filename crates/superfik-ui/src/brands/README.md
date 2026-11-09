# Plugin artwork

Taken from each service's own site, manifest, brand page or repository;
`sources.json` records each asset's URL, discovery page, resolution, SHA-256
and `tile` padding. Marks belong to their owners and only identify the
connected service. The app makes no requests for them.

Refresh with `uv run scripts/fetch_plugin_icons.py` and review the result
(site assets change). The script validates every download before writing
and rejects rasters under 144 px. SVGs stay vector; rasters become lossless
PNG inside an SVG tile (square, one rounded clip, per-service padding),
drawn at full size with no extra surface or tint.

Exceptions: Claude's manifest refuses automated requests, so its icon comes
from `claude.ai/images/claude_app_icon.png`; Anthropic's API uses
Anthropic's mark; OpenAI and ChatGPT share ChatGPT's knot; Context7 uses the
mark from its wordmark; Sentry and Intercom use inline SVGs from their pages.
