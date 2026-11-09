# Diagrams

Two failures cause most broken diagrams; check them for every arrow and every box:
1. An arrow must not cross any box or label other than its source and target. Trace its coordinates against
   every box already placed; if it crosses one, use an L-shaped `<path>` detour.
2. A box is as wide as its longest line: `width = max(title_chars × 8, subtitle_chars × 6.5) + 24`. A 100 px box
   holds a subtitle of about 11 characters.

Tier packing: compute the row's width before placing it. Three boxes of 120 px with 20 px gaps take 400 px: x =
20, 160, 300 (right edge 420). Four boxes need 90 px each (x = 20, 125, 230, 335 with 15 px gaps). For trees,
size the leaves first; a parent is at least as wide as its children.

## Pick the type by intent
Ask whether the user wants to document something or to understand it.
- **Flowchart**: steps in sequence, decisions, data transforming ("what are the steps", "what's the flow").
- **Structural**: things inside other things ("what's the architecture", "where does X live").
- **Illustrative**: a mechanism made visible ("how does X work", "explain X", "I don't get X"). Physical things get
  a simplified cross-section; abstract things get a spatial metaphor (attention is one token with lines of varying
  weight to every other; a hash map is a key falling into one of N buckets; the call stack is a stack of frames).
  This is the default for an unqualified "how does X work". Do not retreat into a flowchart because it feels safer.

Do not mix types in one visual. When both help, draw the intuition first, then the reference, with prose between.

## Flowcharts
- Single-line nodes are 44 px tall, two-line nodes 56 px (title, then subtitle 20 px below). Same content, same
  height.
- At least 40 px between boxes vertically, 20 px horizontally; 10 px between an arrowhead and the box it reaches.
- One direction: all top-down or all left-right. 4 or 5 nodes per diagram. With 6 or more components, draw an
  overview with the boxes and one or two arrows, then one small diagram per interesting sub-flow.
- Cycles are not drawn as rings. Lay the stages out in a line and show the loop with a return path or with
  `↻ returns to start` as a `ts` label near the last stage.
- Arrow labels are rarely needed; if the meaning is not clear from source and target, put it in a subtitle or the
  prose. A label never sits on a line.

Single-line node:
```
<g class="node c-purple" onclick="sendPrompt('Tell me more about T-cells')">
  <rect x="140" y="20" width="160" height="44" rx="8" stroke-width="0.5"/>
  <text class="th" x="220" y="42" text-anchor="middle" dominant-baseline="central">T-cells</text>
</g>
```
Two-line node:
```
<g class="node c-teal" onclick="sendPrompt('Tell me more about dendritic cells')">
  <rect x="120" y="20" width="200" height="56" rx="8" stroke-width="0.5"/>
  <text class="th" x="220" y="38" text-anchor="middle" dominant-baseline="central">Dendritic cells</text>
  <text class="ts" x="220" y="58" text-anchor="middle" dominant-baseline="central">Detect foreign antigens</text>
</g>
```
Connector: `<line x1="220" y1="76" x2="220" y2="110" class="arr" marker-end="url(#arrow)"/>`
Neutral node (start, end, generic step): `<rect class="box" …/>` with plain `t`/`th` text.

Make nodes clickable by default, each sending a specific follow-up question.

## Structural diagrams
- The outer container is a large rect, `rx` 16 to 20, its label at the top left inside it (`th`), 20 px of padding
  inside every container.
- Inner regions are smaller rects, `rx` 8 to 12, side by side with 16 px or more between them. Two or three levels
  of nesting at most.
- Nested regions use different ramps: a related one for parts of the same thing (green around teal), a
  contrasting one for a part that does something else (amber).
- Regions hold text only: a name (`th`) and what happens there (`ts`).
- Inputs and outputs sit outside the container with arrows in and out, labelled with a word or two.
- Containers are dashed or plain rects with a label, not drawings of clouds, servers or organelles.

## Illustrative diagrams
- Draw the mechanism, not a diagram about it: the arrangement carries the meaning, labels annotate. It should
  still make sense with the labels removed.
- Shapes are free: paths, circles, ellipses, polygons, curves, overlapping for depth (later in the source is on
  top). Keep each shape simple: a path of six segments or fewer. A tank is a rounded rect; a flame is three
  triangles.
- Colour encodes intensity: warm ramps (amber, coral, red) for heat, energy, attention, activity; cool ones (blue,
  teal) or gray for cold, calm, dormant. Small indicators are fine: circles for particles, wavy lines for heat.
- Text never touches a stroke: keep 8 px of clear air. Labels go in quiet space, preferably in a column on the
  right with `text-anchor="start"` and a `leader` line to what they name; reserve at least 120 px for them.
- Physical scenes with real colours (sky, water, skin) use hard-coded hex throughout, without `c-*` classes, and
  must read in both themes.

Example, attention from one token (440 wide):
```
<svg width="100%" viewBox="0 0 440 250">
<rect class="c-purple" x="20" y="20" width="400" height="24" rx="6" stroke-width="0.5"/>
<rect class="c-purple" x="20" y="56" width="400" height="24" rx="6" stroke-width="0.5"/>
<text class="ts" x="30" y="32" dominant-baseline="central">Layer 2</text>
<text class="ts" x="30" y="68" dominant-baseline="central">Layer 1</text>
<line stroke="#EF9F27" stroke-linecap="round" x1="220" y1="160" x2="60" y2="80" stroke-width="1" opacity="0.25"/>
<line stroke="#EF9F27" stroke-linecap="round" x1="220" y1="160" x2="140" y2="80" stroke-width="1.5" opacity="0.4"/>
<line stroke="#EF9F27" stroke-linecap="round" x1="220" y1="160" x2="220" y2="80" stroke-width="4"/>
<line stroke="#EF9F27" stroke-linecap="round" x1="220" y1="160" x2="300" y2="80" stroke-width="2.5" opacity="0.7"/>
<line stroke="#EF9F27" stroke-linecap="round" x1="220" y1="160" x2="380" y2="80" stroke-width="1" opacity="0.2"/>
<g class="node" onclick="sendPrompt('What do the attention weights mean?')">
  <rect class="c-gray" x="30" y="160" width="60" height="32" rx="6" stroke-width="0.5"/>
  <rect class="c-gray" x="110" y="160" width="60" height="32" rx="6" stroke-width="0.5"/>
  <rect class="c-amber" x="190" y="160" width="60" height="32" rx="6" stroke-width="1"/>
  <rect class="c-gray" x="270" y="160" width="60" height="32" rx="6" stroke-width="0.5"/>
  <rect class="c-gray" x="350" y="160" width="60" height="32" rx="6" stroke-width="0.5"/>
  <text class="ts" x="60" y="176" text-anchor="middle" dominant-baseline="central">the</text>
  <text class="ts" x="140" y="176" text-anchor="middle" dominant-baseline="central">cat</text>
  <text class="th" x="220" y="176" text-anchor="middle" dominant-baseline="central">sat</text>
  <text class="ts" x="300" y="176" text-anchor="middle" dominant-baseline="central">on</text>
  <text class="ts" x="380" y="176" text-anchor="middle" dominant-baseline="central">the</text>
</g>
<text class="ts" x="220" y="220" text-anchor="middle">Line weight = attention from "sat"</text>
</svg>
```
No boxes named "multi-head attention", no arrows named Q, K, V: those belong in a structural diagram.
