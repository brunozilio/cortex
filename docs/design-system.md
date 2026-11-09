# Design system

One premise: **minimal surfaces, rich detail, physical motion.** Everything
is a soft, top-lit surface on a dark canvas. Nothing appears out of nowhere —
it grows out of what spawned it.

All values live in `crates/superfik-ui/src/theme`. Never hard-code a colour,
size or duration in a component.

## Colour
Two palettes share one shape: `colors::DARK` and `colors::LIGHT`, both of
type `Palette`. The painter carries the active one, and components read
`p.palette().<token>`, never a literal. A new token is a `Palette` field
with a value in both palettes. A test checks body text against the canvas,
surfaces, wells and bubbles at ≥ 4.5:1 contrast in each palette. The GPU
patterns (meter, ultra strip, sparkle, shimmer) take their tones from
`Palette::patterns`.

- **Dark.** Canvas `#232323`. Surfaces fade `#3b3b3b → #373737` with a faint
  top highlight and a soft shadow. Wells are carved `#303030` with a dark top
  rim. Text: `#e0e0e0` primary, `#9d9d9d` secondary, `#686868` tertiary.
- **Light.** Canvas `#ececec`. Surfaces fade `#fcfcfc → #f6f6f6` with a white
  top highlight and a faint shadow. Wells are `#efefef` with a light rim.
  Text: `#1e1e1e` primary, `#6a6a6a` secondary, `#9a9a9a` tertiary.
- One accent per node kind (repository pink, skills violet, references pink,
  research blue, agents teal, model coral, ultra lilac, context periwinkle, memory amber). Light mode uses
  deeper shades of the same hues so they keep their contrast.

## Type
Inter only. Body 15/23, input 15/22, titles 13.5 medium, labels 13, headers
12.5, captions 11.5.

## Shape
Nodes 216 wide, radius 10, padding 10, header 40. Rows 30 high with 7 gaps,
radius 7. Connectors are 8px goo strokes with 12px corners melting 7px into
what they touch.

## Motion
- Popover: stem grows (0–32%), pill widens then deepens (22–78%), content
  resolves from a 4px blur (62–100%). 520ms open, 280ms close.
- Node: connector extends, surface swells from a pill at its connector end,
  content resolves from blur. 550ms in, 380ms out (reverse).
- Streaming text: each word fades in from blur, staggered 28ms.
- Layout changes glide on critically-damped springs; nothing bounces twice.

## Emote
The composer header's tile holds the emote (`superfik-ui/emote`, painted by
`components/emote`): the app's face, a ring of light with two eyes, the
same mark as the app icon. Geometry is pure over `(state, time)` in a unit
box. In the window the ring wears the node accents (`emote_ring`, twelve
stops clockwise from the right) so it sits with the rest of the UI; the
eyes (`emote_eye`) and the amber and green of its marks (`emote_alert`,
`emote_done`) are tokens too. On the Dock tile the same face is drawn by
`emote::raster` with the icon's own rainbow (`ICON_LOOK`), marks riding
the ring's corners as badges; `app/dock.rs` redraws it only as its motion
needs (`EMOTE_DOCK_LIVE` while thinking or morphing, `EMOTE_DOCK_CALM`
while a mark pulses, around blinks when idle). `SUPERFIK_DOCK=off`
leaves the bundled icon alone.
States: `Idle` blinks every `EMOTE_BLINK_PERIOD` and glances around;
`Thinking` squints, searches side to side and sets the ring's trails
flowing; `Alert` tints the ring amber, widens the eyes and shows `!`;
`Question` raises one eye and shows `?`; `Done` closes the eyes into happy
arcs with a thumbs up for `EMOTE_DONE_HOLD` after a run. Marks sit in the
tile's free space right of the ring. State changes morph over
`EMOTE_MORPH`: poses are numbers and are interpolated, marks cross-fade.
The view maps the session onto a state in `view/composer.rs`.

The dotted thought-orbs (`superfik-ui/orbs`, a port of thinking-orbs)
remain available as a component with their nine states and golden tests.

## Timeline
The sidebar lists sessions on a rail: day groups on the outer rail,
sessions on the inner one, joined by two quarter arcs. Idle dots are grey;
the selected session's dot is the accent and travels along the rail on a
layout spring, leaving a trail that fades in 500ms. A session running in
the background pulses its dot and title. Hovered rows get a faint lighter
pill. Tokens: `TIMELINE_*` in colours, metrics and motion.

## Context node
One dot per percent of the window in a 20×5 grid on a well: coloured runs
per kind of content (`Palette::context_tones`), faint dots for free space,
rings for the autocompact reserve. Every non-empty share keeps at least one
dot. Below it: `used / window` and the room left, one legend line per
share, and the "Compact context" row, whose badge shows what the last
compaction freed. Accent: periwinkle (`Palette::context`).

## Memory node
A brain of dots on a well (`components::MemoryGraph`): one dot per memory,
coloured by kind (`Palette::memory_tones`: episodic blue, semantic green,
procedural orange), joined by dotted links to its nearest neighbours and
to memories sharing a tag. A memory's place comes from its id (a Halton
point inside the outline), so nothing moves as memory grows; crowded
graphs shrink their dots. The node shows only the graph; the search and the
legend live in the explorer, where searching dims everything the search
does not reach and lights the rest with their links. Freshly learned memories
pulse for `MEMORY_LEARNED` while particles run down the node's connector.
The explorer grows out of the node over `MEMORY_OPEN`. Accent: amber
(`Palette::memory`).

## Get Started
The first launch is a short film, then a page (`components/get_started/`:
`choreography` is the timing, pure and tested; `intro`; `cards`).

- **Night.** The window darkens to `stage` (the same near-black in both
  themes; the intro paints with the dark palette). "Welcome aboard"
  (`GET_STARTED_WELCOME`, 40 semibold) resolves word by word from a deep
  blur. Then the cast pops in on a row, each growing from
  `GET_STARTED_CAST_FROM` of its size: coffee (amber), the emote, a person,
  a loop (teal). Trails of dots draw between them as each one arrives, and
  a light keeps running along them (`GET_STARTED_TRAIL_FLOW`).
- **Dawn.** The greeting and the cast rise and blur away. The emote flies
  to its place above the title, growing to `GET_STARTED_EMOTE`, while the
  stage lightens into the canvas.
- **Page.** The title (`GET_STARTED_TITLE`, 32 semibold) resolves, then
  each feature card grows from a pill at its tile, like a node from its
  connector, staggered `GET_STARTED_CARD_STAGGER`, and resolves its
  content: a grained role tile with the glyph of the node it lives in, in
  that node's accent (Skills violet, Agents teal, Memory amber, Pull
  request branch), its name (14.5 medium) and a wrapped line (13.5,
  secondary). Continue lands last (`GET_STARTED_OPEN`, about 6 s) and is
  clickable only once it shows. Continue fades the whole screen out.

Tokens: `GET_STARTED_*` in metrics, typography and motion; `stage` in
the palettes.

## Lock screen
Laid out like Get Started's page, on `canvas`: the emote
(`GET_STARTED_EMOTE`; thinking while Touch ID asks, alert
after a failure), the title, the caption and any error centred line by
line in `SIGN_IN_WIDTH` (`Painter::text_centered`), then Get Started's
button. The web pages around signing in (a server's `/sign-in`, the
loopback's "Signed in") follow the same system: a light page, the app's
dark palette.

## Update
Like the reference's update sheet, with the emote for the logo
(`UPDATE_EMOTE`): alert for "Update required", thinking while updating,
done once ready. While updating, a band of light sweeps across the title
(`UPDATE_TITLE`) once per `UPDATE_SHIMMER`; under the caption a
`ProgressBar` (`UPDATE_BAR`, `form_accent` on a `well` track) fills with
the download or passes a segment once per `UPDATE_BAR_PASS` while the
stage has no measure, the stage or percentage under it.

## Settings screen
Over the whole window, on `canvas`: the screens' header, then a column of
sections (`SETTINGS_NAV_*`; icon and name in `MENU`, the shown one on a
`timeline_hover` tile) and the pane centred at up to
`SETTINGS_CONTENT_WIDTH`: its name in `STORE_DISPLAY`, a caption in
`STORE_BODY` secondary text, then groups. A group (`FormCard`) is an
optional `HEADER` heading over a card (`surface_top`→`surface_bottom`,
`divider` edge, `FORM_CARD_RADIUS`); rows are `FORM_ROW` tall, or
`FORM_ROW_DETAILED` with a `SUBTITLE` detail line, parted by inset
hairlines. Controls sit at the right: a `Switch` (`form_accent` track when
on, `form_box` with a `form_box_edge` edge when off, a `form_mark` knob;
`SWITCH_*`) or a `Segmented` choice. A control that waits on another fades
to `DISABLED_OPACITY` and takes no clicks. Tokens: `SETTINGS_*`, `FORM_*`,
`SWITCH_*`.

## Components
Stateless, props in, geometry out (`crates/superfik-ui/src/components`).
A pattern used twice becomes a component. A component that grows past ~150
lines or mixes concerns is split. Components never know what a click means;
the app maps returned rects to targets. Components never hold copy: every
label arrives as a prop, already in the interface language.
