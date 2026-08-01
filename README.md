# gfty

Reproducible Gridfinity bin and label authoring with immutable Onshape STEP
export. Labels use SVG templates and reusable icons; bins, labels, and their Nix
definitions remain the source of truth.

The repository also contains the complete Gridfinity Ultimate browser designer
under [`designer/`](designer/README.md) and every Onshape FeatureScript under
[`featurescripts/`](featurescripts/README.md). The original Gridfinity Ultimate
Git history is preserved as a second parent of the import commit.

Build or develop the designer from the same root flake:

```sh
nix build .#designer
nix develop             # then: designer-dev or designer-preview
```

## Paths

No project marker or fixed directory structure is required. Absolute paths work
everywhere. Paths inside a saved label TOML are resolved relative to that TOML;
paths passed to `label create` are resolved relative to the current directory.

Pathless `validate` scans `./labels` by convention. Pass global `--root PATH`
to scan another root. `inspect` always takes an explicit file.

## Commands

```text
gfty export FILE [EXPORT OPTIONS]

gfty bin validate BIN
gfty bin inspect BIN
gfty bin export BIN [EXPORT OPTIONS]
gfty connector-pin export [EXPORT OPTIONS]

gfty label validate [LABEL]
gfty label render LABEL [--output PREVIEW.svg]
gfty label create [CREATE OPTIONS]
gfty label inspect FILE [--preview]
gfty label watch LABEL [--svg PREVIEW.svg]
gfty label export LABEL [EXPORT OPTIONS]

gfty label plate create --dimensions WIDTH HEIGHT [OPTIONS] LABEL...
gfty label plate export --bin BIN --dimensions WIDTH HEIGHT [EXPORT OPTIONS] LABEL...
```

`gfty` is the package and executable.

Rendering resolves bundled and explicitly supplied fonts, converts text and SVG
primitives to paths with `usvg`, applies filament colors, and lays out icons
without implicit gaps. Geometry JSON is generated only as an internal Onshape
wire format.

`label create` replaces the old `quick` command and accepts ordinary relative or
absolute template and icon paths:

```sh
gfty label create \
  --template templates/label-1x1.svg \
  --bin bins/1x1.toml \
  --filament 0 \
  --text main 'M{3}x[10]' \
  --icon fasteners icons/screws/pointy.svg \
  --save labels/m3x10.toml \
  --svg preview.svg
```

It can also export an unsaved label directly:

```sh
gfty label create \
  --template templates/label-1x1.svg \
  --text main M3 \
  --bin bins/1x1.toml \
  --export m3.step
```

`--save PATH` stores a normal reusable label TOML. The label is fully rendered
and validated before any save, SVG, or export output is written.

Both `gfty export LABEL` and `gfty label export LABEL` render the label, send its
geometry and referenced bin configuration to the pinned immutable Onshape model
in POST bodies, and download one grouped AP242 STEP containing every named
filament part:

```sh
gfty export labels/m3.toml --output m3.step
```

Every label declares `bin = "../bins/1x1.toml"`; raw Gridfinity JSON and
implicit prototype configurations are not accepted.

The output defaults to the label file stem in the current directory. Existing
files are rejected unless `--force` is given. The downloaded STEP is checked for
exactly the expected `part-N` products and solid bodies before it is installed.
Use `--onshape-model URL` to override the pinned immutable model version.

Prefer a mode-0600 credentials file:

```toml
access-key = "..."
secret-key = "..."
```

Pass it with `--onshape-credentials PATH`, set
`GFTY_ONSHAPE_CREDENTIALS_FILE`, or install it as
`$XDG_CONFIG_HOME/gfty/onshape.toml` (defaulting to
`~/.config/gfty/onshape.toml`). `GFTY_ONSHAPE_ACCESS_KEY` and
`GFTY_ONSHAPE_SECRET_KEY` are a fallback. Credentials are loaded only at runtime and
requests use Onshape HMAC signatures. A read-only document API key is sufficient
because exports use `storeInDocument=false`.

`label inspect` accepts a label TOML, template SVG, or icon SVG. It reports known
size, fields, icon boxes, color mappings, filaments, and resolved paths. Add
`--preview` for a terminal thumbnail:

```sh
gfty label inspect templates/label.svg
gfty label inspect labels/m3.toml --preview
```

`label plate create` takes label TOML paths directly; no plate config file is
needed yet. Repeat a path to repeat that label:

```sh
gfty label plate create \
  --dimensions 200mm 250mm \
  --svg plate.svg \
  labels/m3.toml labels/m3.toml labels/m4.toml
```

Without `--svg`, it previews interactively. `label plate export` additionally
requires `--bin PATH` for the shared Gridfinity prototype, accepts the normal
remote export options, and downloads STEP.
Labels are placed in argument order, row-major from the top left, without
rotation. The tool fits as many columns as possible within the maximum width,
then verifies that all required rows fit the maximum height. The final incomplete
row is left-aligned. Column and row gaps default to `5mm`; override them with
`--column-gap` and `--row-gap`. Every label must have exactly the same physical
viewport dimensions.

`label watch` watches the label TOML, template, icons, sidecars, and explicit font
directories. Failed rebuilds are reported without stopping the watcher:

```sh
gfty label watch labels/m3.toml --svg preview.svg
```

`label validate LABEL` checks one label. With no path, it validates every TOML
below `labels/`, prints failures, and finishes with an `X/N valid` summary.
`label render LABEL` writes an SVG when `--output` is supplied; without it, the
label is rendered directly in a supported interactive terminal.

## Bin TOML

Bin files use a typed, hierarchical format which is converted to the flat
Gridfinity Ultimate `Config` JSON in memory:

```toml
kind = "bin"
version = 2
size = [2, 2, 6]
tub = true
max-print-overhang = 60

[rim-interface]
mode = "swappable"

[label-interface]
mode = "swappable"
depth = "10mm"
supports = "auto"

[divider]
columns = ["auto", "auto", "auto"]
rows = ["auto", "auto"]

[easy-grab]
mode = "all"
side = "south"
radius = "21mm"
```

Tracks accept `auto`, fractional values such as `1fr`, or fixed physical lengths
such as `21mm`. Divider merges use inclusive zero-based ranges. Custom easy-grab
faces use the same ranges and are rejected unless they describe a complete,
capped wall face. Defaults and automatic label-support behavior match the web
designer.

```sh
gfty bin validate bins/small-parts.toml
gfty bin inspect bins/small-parts.toml
gfty export bins/small-parts.toml
gfty export bins/small-parts.toml --image small-parts.png
gfty bin export bins/small-parts.toml
```

A bin TOML always exports exactly the named `Bin` body. Complete grouped sets
come only from `kind = "bin-set"`. Optional `--image PATH` downloads a 512×512
isometric PNG from Onshape's configured Part Studio shaded-view endpoint. It is
opt-in because it consumes an additional API request. The camera is front-facing
with Z up, matching Onshape's documented isometric view. Shaded views are available for Gridfinity constituents and sets: Onshape
exposes them through a GET query, while artwork-label geometry can exceed
reliable URL limits even though STEP translation uses POST bodies. Each
constituent resolves its configured Onshape part ID and exports exactly its
named body.

Independent constituent TOML is also supported:

```toml
# base.toml
kind = "base"
version = 1
size = [2, 2]

[magnets]
enabled = true
connector-cutouts = true
```

```toml
# rim.toml
kind = "rim"
version = 1
size = [2, 2]
spring-compensation = true
additional-expansion = "0mm"
```

```toml
# swappable-label.toml
kind = "swappable-label"
version = 1
bin = "../bins/small-parts.toml"

[embossing]
clearance = "0.4mm"
inset = "0mm"
```

Bin TOML contains only bin-body and mating-interface settings. A
`kind = "bin-set"` file composes a bin, base, rim, swappable label, and optional
standard connector pin while checking dimensions and normalized label
compatibility. See [`docs/component-configs.md`](docs/component-configs.md) and
the direct files under `examples/`.

Constituent downloads use a normalized request cache under
`$XDG_CACHE_HOME/gfty/onshape` (or `$GFTY_CACHE_DIR/onshape`). Source paths and
irrelevant bin settings are excluded from swappable-label identity, so compatible
first-row divider layouts share one STEP and preview. Cached bytes and exact STEP
manifests are verified before atomic installation; pass `--no-cache` to bypass
the cache.

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
kind = "label"
version = 1
template = "../templates/label-1x1.svg"
bin = "../bins/1x1.toml"
filament = 0 # Blank prototype filament; defaults to 0.

[text.main]
content = 'M{3}x[10]'

[[icons.fasteners]]
icon = "../icons/screws/pointy.svg"

[[icons.fasteners]]
spacer = "1mm"
```

Labels require `kind = "label"`, `version = 1`, and a `bin` reference. The blank
prototype uses the label's `filament`, which
defaults to 0. Plain text
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

When present, the sidecar must map every effective fill and stroke color exactly
once. Inherited paints and SVG's default black fill are included. Without a
sidecar, normalized colors are sorted lexicographically and assigned indices
starting at one, reserving filament 0 for the default prototype. Label-local
overrides may be partial; exact hex overrides win over numeric resolved-index
overrides.

By default, fonts are loaded from the directories bundled by the Nix package.
Add repeatable `--font-dir PATH` options for local or Nix-provided fonts, or pass
`--system-fonts` to additionally scan host fonts. Rendering and validation fail
with an actionable error when none of an SVG text element's requested font
families is available.

## Nix builders

The default package exposes `mkBin`, `mkBase`, `mkRim`, `mkSwappableLabel`,
`mkBinSet`, `mkLabel`, and `mkPlate` passthru functions. Add the
flake's `easyOverlay`-generated `overlays.default` to nixpkgs to make the same
package available as `pkgs.gfty`:

```nix
# Import nixpkgs with overlays = [ inputs.gfty.overlays.default ].
let
  smallParts = pkgs.gfty.mkBin {
    name = "small-parts";
    size = [ 2 2 6 ];
    divider.columns = [ "auto" "auto" "auto" ];
    divider.rows = [ "auto" "auto" ];
  };
  screws = pkgs.gfty.mkLabel {
    name = "screws-label"; # Derivation pname only.
    bin = smallParts;
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
pkgs.gfty.mkPlate {
  name = "fastener-plate";
  dimensions = [ "200mm" "250mm" ];
  labels = [ screws screws ];
}
```

A bin derivation contains `bin.toml`, a label contains `label.svg` and
`label.toml`, and a plate contains `plate.svg`. Geometry JSON is generated in memory by runtime export
apps rather than exposed as a package artifact. Font outputs are added at build
time through `--font-dir` and do not rebuild `gfty`.
Adjacent SVG color sidecars are retained automatically.

Without an overlay, use
`inputs'.gfty.packages.default.mkLabel`. A flake-parts module is also exported
as `inputs.gfty.flakeModules.default`:

```nix
imports = [ inputs.gfty.flakeModules.default ];

perSystem = { ... }: {
  gfty = {
    bins.small-parts = {
      size = [ 2 2 6 ];
      divider.columns = [ "auto" "auto" "auto" ];
      divider.rows = [ "auto" "auto" ];
    };
    bases.small-parts = {
      size = [ 2 2 ];
      magnets.enabled = true;
      magnets.connectorCutouts = true;
    };
    rims.small-parts = {
      size = [ 2 2 ];
    };
    swappableLabels.small-parts = {
      bin = "small-parts";
    };
    binSets.small-parts = {
      bin = "small-parts";
      base = "small-parts";
      rim = "small-parts";
      swappableLabel = "small-parts";
      connectorPin = true;
    };
    labels.screws = {
      bin = "small-parts";
      template = ./templates/label.svg;
      text.main = "Screws";
    };
    plates.all = {
      bin = "small-parts";
      dimensions = [ "200mm" "250mm" ];
      labels = [ "screws" "screws" ];
    };
  };
};
```

The module validates bins, labels, dividers, merges, easy-grab faces, and plates
with typed options during evaluation. Labels and plates require a named bin; the
module verifies that every plate label has the same X/Y bin size.

Outputs are grouped under `packages.bins`, `bases`, `rims`,
`swappable-labels`, `bin-sets`, `labels`, and `plates`. The module also generates
explicit runtime apps which
build local inputs through Nix, then download the configured STEP outside the
Nix sandbox and store:

```sh
nix run .#export-bin-small-parts
nix run .#export-base-small-parts
nix run .#export-rim-small-parts
nix run .#export-swappable-label-small-parts
nix run .#export-bin-set-small-parts
nix run .#export-connector-pin
nix run .#export-label-screws
nix run .#export-plate-all
nix run .#export-label-screws -- --output custom.step --force
```

The default output is `<name>.step` in the caller's current directory. Apps use
normal runtime credential discovery and never capture credentials in Nix. They
are intentionally manual `nix run` actions rather than derivations because
Onshape translator output is not guaranteed byte-reproducible. Normalized
runtime caching still prevents repeat API exports without putting remote files
or credentials in the store. Override the pinned immutable models with
`perSystem.gfty.labelModelUrl` or `binModelUrl` when testing a new model
version.

`packages.bins.all` and `packages.labels.all` link every generated definition,
making it convenient to install or copy the complete set. See
`examples/flake.nix` and `examples/module.nix` for a buildable module example.

## Terminal previews

Interactive commands rasterize rendered SVGs with `resvg` and display them with
the native Rust `rasteroid` encoder. It selects Kitty, iTerm2, Sixel, or Unicode
symbols based on terminal capabilities. Previews are skipped when stderr is not
a terminal, so JSON pipelines remain clean.

```sh
gfty --terminal-preview auto label render labels/m3.toml -o /tmp/m3.svg
gfty --terminal-preview graphics label watch labels/m3.toml --svg /tmp/m3.svg
gfty --terminal-preview symbols label inspect labels/m3.toml --preview
gfty --terminal-preview never label inspect templates/label.svg
```

Use `--terminal-preview-width N` to control thumbnail width. Watch mode clears
before its initial preview and every rebuild, then prints the rebuild counter,
local timestamp, and elapsed render time before the preview. Interactive status
uses Cargo-like action coloring when supported; ordinary text stays uncolored.
Colors are disabled for redirected output and when `NO_COLOR` is set.

## Internal Onshape geometry

Remote label and plate exports generate compact structured geometry in memory.
Coordinates are
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

Only `featurescripts/labels/gfty_label_instances.fs` is needed for the current
label workflow.
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

For workflows which derive the blank prototype from another configured Part
Studio, `featurescripts/configuration/variable_configured_derived.fs` wraps
Onshape's native
Derived implementation and forwards current Part Studio variables into selected
source configuration inputs. See
`featurescripts/configuration/variable_configured_derived.md` for installation,
mapping, and
limitations.
