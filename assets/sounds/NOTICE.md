# UI sound packs

Three packs with the same set of cues, so the app can switch between them
without any code change:

| Folder | Character |
|---|---|
| `piano/` | Soft piano notes (default). |
| `sine/` | Short sine tones; minimal and quiet. |
| `internal/` | Interface sounds; see `internal/NOTICE.md` for source and cue mapping. |

Files in Piano and Sine: `button`, `select`, `tap_01..05`, `type_01..05`,
`swipe_01..05`, `toggle_on`, `toggle_off`, `transition_up`,
`transition_down`, `notification`, `caution`, `disabled`, `celebration`.
Mono, 44.1 kHz, 16-bit WAV.

Source: Material Design sound resources (`SND01_sine`, `SND02_piano`),
distributed under CC BY 4.0: attributed in `THIRD_PARTY_NOTICES.md`.

To add a pack: add a folder of WAV clips here, a `PackId` variant and
a cue mapping in `crates/superfik-sound/src/pack.rs`. The `pack!` macro
handles the Piano/Sine file layout; other packs can map source names directly.
