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
viewport dimensions. Geometry is translated into plate coordinates and merged
by filament ID; `instances` records the corresponding label centers.

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
label origin.

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
without `examples/fonts/` because `DejaVu Sans` is one of those Nix-bundled
fonts—not because host fonts are scanned. Pass `--system-fonts` to additionally
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
  ],
  "instances": [[0.0, 0.0]]
}
```

Each shape corresponds to one filled rendered path. The compact path notation
uses absolute `M`, `L`, and `C` commands plus `Z`; it is intentionally not the
full SVG path grammar. The single-label exporter places one instance at the
origin. The `plate` command emits the full plate size, flattened placed geometry,
and one center point per label.

## Onshape FeatureScripts

`featurescript/gfty_label_importer.fs` consumes exported JSON and builds one
solid named `part-<filament>` per filament. Filament numbers are zero-padded to
the width of the largest index. Each solid receives a full-viewport, 1 mm helper
plate behind the artwork so disconnected islands remain one STEP part. JSON may
be pasted or read from a Part Studio string variable.

`featurescript/gfty_label_instances.fs` patterns a selected set of prototype
filament parts to the center points in `instances`, without rotation. The
prototype must be centered at the selected layout plane origin. An `[0, 0]`
instance keeps the prototype; if no origin instance exists, the prototype is
deleted after copies are made. This feature also supports pasted JSON or a
Part Studio string variable.
