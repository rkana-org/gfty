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
gfty-label watch LABEL --svg PREVIEW.svg --json FILLS.json
```

All four design workflows are implemented. Rendering resolves bundled/project
fonts, converts text and SVG primitives to paths with `usvg`, applies filament
colors, and lays out icons without implicit gaps. Export produces centered,
physical-millimeter contours grouped by filament for Onshape.

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

[icon.pointy]
src = "screws/pointy.svg"

[icon.pointy.colors]
"0" = 1
"#ff0000" = 5

[[icons.fasteners]]
icon = "pointy"

[[icons.fasteners]]
spacer = "1mm"
```

Text starts at filament 0. `{}`, `[]`, and `<>` select filaments 1, 2, and 3;
`!N{}` selects any non-negative filament ID. Scopes nest and restore their
parent color. Escape markup characters with a backslash, for example `\{`,
`\!`, and `\\`.

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
and from the font directories bundled by the Nix package. Pass `--system-fonts`
to additionally scan host fonts.

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
          "contours": [
            {
              "start": [-21.0, 10.5],
              "closed": true,
              "segments": [{ "type": "L", "to": [21.0, 10.5] }]
            }
          ]
        }
      ]
    }
  ],
  "instances": [[0.0, 0.0]]
}
```

Each shape corresponds to one filled rendered path. Segment types are `L` for
lines and `C` for cubic Beziers. The single-label exporter places one instance
at the origin; a future plate-layout stage can replace `instances` with label
center points without duplicating geometry.

Plate/grid generation remains intentionally postponed. It will require labels
on one plate to share the same physical viewport dimensions.

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
