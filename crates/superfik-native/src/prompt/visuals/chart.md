# Charts

Charts are drawn as plain SVG: no chart library runs. Compute every coordinate yourself and check it.

## Layout (440 wide)
- Plot area from x = 50 to 420 and from y = 20 to H − 40; axis labels in the margins. The chart is 200 to 260 px
  tall unless the data needs more.
- Axes and gridlines are 0.5 px in `var(--b)`; tick labels are `ts`. Few gridlines: 3 to 5 on the value axis.
- Pick round tick values (0, 25, 50, 75, 100) and map each value with one linear formula; write it down for
  yourself before placing marks.
- Bars: equal widths, gaps about a third of a bar. Up to 12 categories show every label; rotate nothing (no rotated
  text): shorten labels or use horizontal bars, 28 px per bar, labels at the left.
- Lines: one `<polyline fill="none">` per series, 2 px, with round joins; points as 3 px circles only when there are
  few of them.
- Round every number shown: integers for counts, one or two decimals for rates, separators for thousands. A
  negative amount is written -$5M, sign before the currency.

## Colour and legend
- One series: one ramp's 400 stop (or gray plus one accent for the highlighted bar).
- Several series: up to four, from distinct ramps (purple #7F77DD, teal #1D9E75, coral #D85A30, amber #BA7517).
- A legend sits above the plot: a 10 px square with 2 px corners, then the name (and its value or share for
  categorical data) in `ts`. No legend for a single series.
- Values may label the bars directly (`ts`, just above or at the end of each bar) instead of a value axis.

## Summary numbers
For a few key numbers above a chart, draw metric tiles: a `box` rect with `rx="8"`, a `ts` label on top and the
number in `th` below it; two to four tiles in a row.

Make bars and points that deserve a drill-down clickable with `sendPrompt` (for example "Break down Q4 by
region").
