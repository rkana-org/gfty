# gfty-label

File-based Gridfinity label composer and Onshape exporter.

The project is under active development. The intended workflow uses SVG templates,
reusable SVG icons with optional TOML color sidecars, and saved label TOML files.

## Project layout

```text
project.toml
templates/
icons/
labels/
fonts/
featurescript/
```

## Commands

```text
gfty-label validate LABEL
gfty-label render LABEL --output PREVIEW.svg
gfty-label export LABEL --output FILLS.json
gfty-label quick --template TEMPLATE --text ID CONTENT --icon BOX ICON
gfty-label list-templates
gfty-label list-icons
gfty-label list-labels
gfty-label list
gfty-label plate --dimensions WIDTH HEIGHT [OPTIONS] LABEL...
gfty-label watch LABEL --svg PREVIEW.svg --json FILLS.json
```

All four design workflows are implemented. Rendering resolves bundled/project
fonts, converts text and SVG primitives to paths with `usvg`, applies filament
colors, and lays out icons without implicit gaps. Export produces centered,
physical-millimeter paths grouped by filament for Onshape.

`quick` runs from anywhere below a project root and treats template/icon paths
as suffixes below `templates/` and `icons/` respectively:

```sh
gfty-label quick \
  --template label-1x1.svg \
  --text main 'M{3}x[10]' \
  --icon fasteners screws/pointy.svg \
  --svg preview.svg \
  --json label.json
```

Omit the path after `--json` to write compact JSON to stdout, for example:

```sh
gfty-label quick --template label-1x1.svg --text main M3 --json | wl-copy
```

The `list-*` commands print sorted paths relative to the project root, including
`templates/`, `icons/`, or `labels/`. `list` prints all three groups together.

`plate` takes label TOML paths directly on the command line; no plate config
file is needed. Repeat a path to repeat that label:

```sh
gfty-label plate \
  --dimensions 200mm 250mm \
  --svg plate.svg \
  --json plate.json \
  labels/m3.toml labels/m3.toml labels/m4.toml
```

Labels are placed in argument order, row-major from the top left, without
rotation. The tool fits as many columns as possible within the maximum width,
then verifies that all required rows fit the maximum height. The final incomplete
row is left-aligned. Column and row gaps default to `5mm`; override them with
`--column-gap` and `--row-gap`. Every label must have exactly the same physical
viewport dimensions. Version 2 JSON keeps each label's local geometry together
with its center so the combined Onshape feature can instantiate and merge it.

`watch` performs an initial build, then watches the label TOML, its template,
icons, sidecars, and project fonts. Failed rebuilds are reported without
stopping the watcher:

```sh
gfty-label watch labels/m3.toml --svg preview.svg --json label.json
```

## Template contract

Templates need physical `width`/`height`, a `viewBox`, unique `<text>` elements
named `text-*`, and unique icon-box `<rect>` elements named `icons-*`. TOML uses
the suffix only (`text-main` becomes `text.main`). The viewport center is the
label origin. Template, icon-box, and icon transforms are preserved through
composition and resolved by `usvg` before JSON coordinates are exported.

```toml
template = "label-1x1.svg"

[text.main]
content = 'M{3}x[10]'

[[icons.fasteners]]
icon = "icons/screws/pointy.svg"

[[icons.fasteners]]
spacer = "1mm"
```

Text starts at filament 0. `{}`, `[]`, and `<>` select filaments 1, 2, and 3;
`!N{}` selects any non-negative filament ID. Scopes nest and restore their
parent color. Escape markup characters with a backslash, for example `\{`,
`\!`, and `\\`.

An `icon` value ending in `.svg` is a filesystem path relative to the project
root and needs no declaration. Values without that suffix are aliases declared
under `[icon.NAME]`; use an alias when label-local settings are needed:

```toml
[icon.pointy]
src = "screws/pointy.svg"

[icon.pointy.colors]
"0" = 1
"#ff0000" = 5

[[icons.fasteners]]
icon = "pointy"
```

An icon sidecar is named after its SVG with a `.toml` extension:

```toml
[colors]
"#000000" = 0
"#ff0000" = 3
```

When present, the sidecar must map every source fill color exactly once. Without
a sidecar, normalized colors are sorted lexicographically and assigned indices
starting at zero. Label-local overrides may be partial; exact hex overrides win
over numeric resolved-index overrides.

By default, fonts are loaded recursively from the project `fonts/` directory
and from the font directories bundled by the Nix package. The example works
without `examples/fonts/` because `DejaVu Sans` and `JetBrains Mono` are among
the Nix-bundled fonts—not because host fonts are scanned. Pass `--system-fonts` to additionally
scan host fonts. Rendering and validation fail with an actionable error when
none of an SVG text element's requested font families is available.

## Terminal previews

Interactive commands rasterize rendered SVGs with `resvg` and display them with
the native Rust `rasteroid` encoder. It selects Kitty, iTerm2, Sixel, or Unicode
symbols based on terminal capabilities. Previews are skipped when stderr is not
a terminal, so JSON and list pipelines remain clean.

```sh
gfty-label --terminal-preview auto render labels/m3.toml -o /tmp/m3.svg
gfty-label --terminal-preview graphics watch labels/m3.toml --svg /tmp/m3.svg
gfty-label --terminal-preview symbols list-labels
gfty-label --terminal-preview never list
```

Use `--terminal-preview-width N` to control thumbnail width. Watch mode redraws
the terminal after successful rebuilds.

## Onshape JSON

`export` and `quick --json` emit compact structured geometry. Coordinates are
millimeters, centered on the template viewport, with SVG's downward Y axis
converted to an upward Y axis. Filament indices remain arbitrary non-negative
integers.

```json
{
  "version": 2,
  "size": [42.0, 21.0],
  "filaments": [0],
  "labels": [
    {
      "center": [0.0, 0.0],
      "size": [42.0, 21.0],
      "parts": [
        {
          "filament": 0,
          "shapes": [
            {
              "path": "M -21 10.5 L 21 10.5 L 21 -10.5 L -21 -10.5 Z"
            }
          ]
        }
      ]
    }
  ]
}
```

Each shape corresponds to one filled rendered path. The compact path notation
uses absolute `M`, `L`, and `C` commands plus `Z`; it is intentionally not the
full SVG path grammar. Geometry in each `labels` entry remains centered and
local; `center` places that label in the overall rectangular `size`. `filaments`
is sorted numerically in priority order.

## Onshape FeatureScript

Only `featurescript/gfty_label_instances.fs` is needed for the current workflow.
Paste the complete version 2 JSON (or read it from a Part Studio string variable),
then select:

1. One finished blank prototype label solid.
2. A mate connector centered on its top artwork surface, with +Z pointing out.
3. A mate connector anywhere on its parallel bottom surface.

The feature copies the prototype once per label and filament, places each
label's artwork on the top connector plane, and unions artwork into its matching
filament copy. All filament copies overlap intentionally. For multiple labels,
each filament receives an identical 1 mm-thick rectangular connector plate at
the bottom plane spanning the complete top-level `size`, including gaps. This
joins every label into one part per filament. Offset the print down by 1 mm in
the slicer so this sacrificial plate is not printed. A single label does not get
a connector plate.

Parts are named `part-<filament>` and zero-padded to the width of the largest
filament ID, for example `part-00`, `part-02`, and `part-10`. This preserves
OrcaSlicer's lexicographic overlap precedence: lower filament IDs come first and
have higher priority. The original selected prototype is deleted after copies
are generated.

`featurescript/gfty_label_importer.fs` is retained only for legacy version 1
JSON and is not part of the current workflow.
