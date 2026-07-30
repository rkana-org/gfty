# gfty-label

File-based Gridfinity label composer and Onshape exporter.

The project is under active development. The intended workflow uses SVG templates,
reusable SVG icons with optional TOML color sidecars, and saved label TOML files.

## Paths

No project marker or fixed directory structure is required. Absolute paths work
everywhere. Paths inside a saved label TOML are resolved relative to that TOML;
paths passed to `quick` are resolved relative to the current directory.

Pathless `validate` scans `./labels` by convention. Pass global `--root PATH`
to scan another root. `inspect` always takes an explicit file.

## Commands

```text
gfty-label validate [LABEL]
gfty-label render LABEL [--output PREVIEW.svg]
gfty-label build LABEL --output DIR
gfty-label export LABEL [--output FILLS.json]
gfty-label quick --template TEMPLATE --text ID CONTENT --icon BOX ICON [--save LABEL.toml]
gfty-label inspect FILE [--preview]
gfty-label plate --dimensions WIDTH HEIGHT [OPTIONS] LABEL...
gfty-label watch LABEL --svg PREVIEW.svg --json FILLS.json
```

Rendering resolves bundled and explicitly supplied fonts, converts text and SVG
primitives to paths with `usvg`, applies filament
colors, and lays out icons without implicit gaps. Export produces centered,
physical-millimeter paths grouped by filament for Onshape.

`quick` accepts ordinary relative or absolute template and icon paths:

```sh
gfty-label quick \
  --template templates/label-1x1.svg \
  --filament 0 \
  --text main 'M{3}x[10]' \
  --icon fasteners icons/screws/pointy.svg \
  --save labels/m3x10.toml \
  --svg preview.svg \
  --json label.json
```

`quick --save PATH` stores the invocation as a normal reusable label TOML. It
can be used by itself or together with SVG/JSON output; the label is fully
rendered and validated before it is saved.

`export` writes compact JSON to stdout by default. `quick --json` also uses
stdout when its optional path is omitted:

```sh
gfty-label export labels/m3.toml | wl-copy
gfty-label quick --template templates/label-1x1.svg --text main M3 --json | wl-copy
```

`inspect` accepts a label TOML, template SVG, or icon SVG. It reports known
size, fields, icon boxes, color mappings, filaments, and resolved paths. Add
`--preview` for a terminal thumbnail:

```sh
gfty-label inspect templates/label.svg
gfty-label inspect labels/m3.toml --preview
```

`plate` takes label TOML paths directly on the command line; no plate config
file is needed. Repeat a path to repeat that label. With no `--svg` or `--json`
option, plate JSON goes to stdout; `--json` without a path does the same:

```sh
gfty-label plate --dimensions 200mm 250mm labels/*.toml | wl-copy

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
icons, sidecars, and explicit font directories. Failed rebuilds are reported without
stopping the watcher:

```sh
gfty-label watch labels/m3.toml --svg preview.svg --json label.json
```

`validate LABEL` checks one label. With no path, it validates every TOML below
`labels/`, prints only failures, and finishes with an `X/N valid` summary.
`render LABEL` writes an SVG when `--output` is supplied; without it, the label
is rendered directly in a supported interactive terminal.

## Template contract

Templates need physical `width`/`height`, a `viewBox`, unique `<text>` elements
named `text-*`, and unique icon-box `<rect>` elements named `icons-*`. TOML uses
the suffix only (`text-main` becomes `text.main`). The viewport center is the
label origin. Template, icon-box, and icon transforms are preserved through
composition and resolved by `usvg` before JSON coordinates are exported.

Icon content is centered horizontally by default. Set layout attributes on an
`icons-*` rectangle to control flow and alignment:

```xml
<!-- A left-aligned horizontal row. -->
<rect id="icons-tools" x="4" y="4" width="70" height="14"
      data-gfty-direction="horizontal" data-gfty-align="left"/>

<!-- A vertical list ordered from top to bottom and aligned to the top. -->
<rect id="icons-status" x="4" y="4" width="14" height="70"
      data-gfty-direction="vertical" data-gfty-align="top"/>
```

Horizontal alignment accepts `left`, `center`, or `right`; vertical alignment
accepts `top`, `center`, or `bottom`. Icons retain their order and aspect ratio,
spacers act along the selected direction, and no implicit gaps are added.

```toml
template = "../templates/label-1x1.svg"
filament = 0 # Blank prototype filament; defaults to 0.

[text.main]
content = 'M{3}x[10]'

[[icons.fasteners]]
icon = "../icons/screws/pointy.svg"

[[icons.fasteners]]
spacer = "1mm"
```

The blank prototype uses the label's `filament`, which defaults to 0. Plain text
starts at filament 1. `{}`, `[]`, and `<>` select filaments 2, 3, and 4;
`!N{}` selects any non-negative filament ID. Scopes nest and restore their
parent color. Escape markup characters with a backslash, for example `\{`,
`\!`, and `\\`.

An `icon` value ending in `.svg` is an absolute path or a filesystem path
relative to the label TOML and needs no declaration. Values without that suffix are aliases declared
under `[icon.NAME]`; use an alias when label-local settings are needed:

```toml
[icon.pointy]
src = "../icons/screws/pointy.svg"

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
starting at one, reserving filament 0 for the default prototype. Label-local overrides may be partial; exact hex overrides win
over numeric resolved-index overrides.

By default, fonts are loaded from the directories bundled by the Nix package.
Add repeatable `--font-dir PATH` options for local or Nix-provided fonts, or pass
`--system-fonts` to additionally scan host fonts. Rendering and validation fail
with an actionable error when none of an SVG text element's requested font
families is available.

## Nix builders

The default package exposes `mkLabel` and `mkPlate` passthru functions. Add the
flake's `easyOverlay`-generated `overlays.default` to nixpkgs to make the same
package available as `pkgs.gfty-label`:

```nix
# Import nixpkgs with overlays = [ inputs.gfty-label.overlays.default ].
let
  screws = pkgs.gfty-label.mkLabel {
    name = "screws-label"; # Derivation pname only.
    template = ./templates/label.svg;
    filament = 0;
    fonts = [ pkgs.jetbrains-mono ];
    icons.fasteners = [
      ./icons/bolt.svg
      { spacer = "1mm"; }
      ./icons/nut.svg
    ];
    text.main = "M{3}x[10]";
  };
in
pkgs.gfty-label.mkPlate {
  name = "fastener-plate";
  dimensions = [ "200mm" "250mm" ];
  labels = [ screws screws ];
}
```

A label derivation contains `label.svg`, `label.json`, and the generated
`label.toml`; a plate contains `plate.svg` and `plate.json`. Font outputs are
added at build time through `--font-dir` and do not rebuild `gfty-label`.
Adjacent SVG color sidecars are retained automatically.

Without an overlay, use
`inputs'.gfty-label.packages.default.mkLabel`. A flake-parts module is also
exported as `inputs.gfty-label.flakeModules.default`:

```nix
imports = [ inputs.gfty-label.flakeModules.default ];

perSystem = { ... }: {
  gfty-label = {
    labels.screws = {
      template = ./templates/label.svg;
      text.main = "Screws";
    };
    plates.all = {
      dimensions = [ "200mm" "250mm" ];
      labels = [ "screws" "screws" ];
    };
  };
};
```

This creates `packages.label-screws` and `packages.plate-all`. See
`examples/flake.nix` for a buildable overlay, passthru, plate, and module example.

## Terminal previews

Interactive commands rasterize rendered SVGs with `resvg` and display them with
the native Rust `rasteroid` encoder. It selects Kitty, iTerm2, Sixel, or Unicode
symbols based on terminal capabilities. Previews are skipped when stderr is not
a terminal, so JSON pipelines remain clean.

```sh
gfty-label --terminal-preview auto render labels/m3.toml -o /tmp/m3.svg
gfty-label --terminal-preview graphics watch labels/m3.toml --svg /tmp/m3.svg
gfty-label --terminal-preview symbols inspect labels/m3.toml --preview
gfty-label --terminal-preview never inspect templates/label.svg
```

Use `--terminal-preview-width N` to control thumbnail width. Watch mode clears
before its initial preview and every rebuild, then prints the rebuild counter,
local timestamp, and elapsed render time before the preview. Interactive status
uses Cargo-like action coloring when supported; ordinary text stays uncolored.
Colors are disabled for redirected output and when `NO_COLOR` is set.

## Onshape JSON

`export` and `quick --json` emit compact structured geometry. Coordinates are
millimeters, centered on the template viewport, with SVG's downward Y axis
converted to an upward Y axis. Filament indices remain arbitrary non-negative
integers.

```json
{
  "version": 2,
  "size": [42.0, 21.0],
  "filaments": [0, 1],
  "labels": [
    {
      "center": [0.0, 0.0],
      "size": [42.0, 21.0],
      "filament": 0,
      "parts": [
        {
          "filament": 1,
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

The feature copies the prototype for each label's base filament and artwork
filaments, places each label's artwork on the top connector plane, and unions artwork into its matching
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
are generated. By default, the feature also assigns a stable display appearance
to each filament ID so coincident parts are distinguishable in Onshape. These
are appearance colors only, not physical material assignments, and can be
disabled with **Assign filament appearances**. The default palette, repeated for
higher IDs, is `#EAEAEA`, `#43484D`, `#A7D293`, `#8AAED6`, `#E1927A`,
`#F5D578`, `#A795D2`, `#89DAD3`, `#EAB97D`, and `#999487`.

`featurescript/gfty_label_importer.fs` is retained only for legacy version 1
JSON and is not part of the current workflow.
