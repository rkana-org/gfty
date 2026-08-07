# gfty

<table>
  <tr>
    <td width="50%">
      <img src="screenshots/web-designer.png" alt="Gridfinity Ultimate web designer" width="100%" />
      <br /><sub>Web designer</sub>
    </td>
    <td width="50%">
      <img src="screenshots/plate-preview.png" alt="gfty multi-label plate preview" width="100%" />
      <br /><sub>Multi-label plate preview</sub>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="screenshots/web-designer-onshape.png" alt="Web designer output in Onshape" width="100%" />
      <br /><sub>Web designer output in Onshape</sub>
    </td>
    <td width="50%">
      <img src="screenshots/plate-onshape.png" alt="Generated multi-label plate in Onshape" width="100%" />
      <br /><sub>Generated plate in Onshape</sub>
    </td>
  </tr>
</table>

[Web designer](https://rkana-org.github.io/gfty/) | [CLI](#usage-cli) | [Nix](#usage-nix) | [Examples](examples/)

## About

`gfty` generates Gridfinity Ultimate parts and multi-color labels from typed
configuration files. The source of truth is config, not manually edited CAD
state. Onshape does the CAD work. `gfty` handles the plumbing and validates the
result.

- Automation-ready pipeline: Nix definitions -> typed TOML -> FeatureScript JSON
  -> parameterized Onshape models -> verified STEP downloads. Stable multipart
  output supports automatic slicer jobs.
- Reproducible definitions, validation, and SVG previews with Nix.
- Gridfinity bins, bases, detachable rims, detachable label blanks, connector
  pins, artwork labels, and multi-label plates.
- SVG templates, reusable icons, text outlining, and arbitrary filament IDs.
- Browser designer for quick bin configuration and one-off models.
- Native terminal previews and dependency-aware label rebuilds.

## Usage: web designer

Open the hosted [Gridfinity Ultimate designer](https://rkana-org.github.io/gfty/).
It edits one Gridfinity configuration and opens the pinned model in Onshape.
The configuration includes bin dimensions, dividers, easy-grab faces, and the
related base, rim, and blank-label options.

1. Configure the bin.
2. Copy the generated JSON, typed TOML files, or flake-parts module.
3. Select **Open in Onshape**.

The designer runs entirely in the browser. It does not create SVG artwork
labels or plates containing multiple labels. Use the CLI or Nix for those.

## Usage: CLI

The CLI reads typed TOML, configures pinned immutable Onshape models, and
downloads validated STEP files. Validation, SVG rendering, and plate layout run
locally. Only model export needs Onshape credentials.

### Install

Install the current version with Cargo:

```sh
cargo install --locked --git https://github.com/rkana-org/gfty
```

Text rendering only uses explicitly enabled fonts. Pass `--system-fonts` to use
host fonts, or repeat `--font-dir PATH` for selected font directories.

### Configuration files

Every TOML has an explicit `kind` and `version`. Relative paths resolve from the
TOML that contains them.

| Kind | Version | Purpose | Example |
| --- | ---: | --- | --- |
| `bin` | 2 | Bin body, dividers, easy grabs, and mating interfaces | [`constituent-2x2.toml`](examples/bins/constituent-2x2.toml) |
| `base` | 1 | Independent Gridfinity base | [`2x2-magnetic.toml`](examples/bases/2x2-magnetic.toml) |
| `rim` | 1 | Detachable rim | [`2x2-standard.toml`](examples/rims/2x2-standard.toml) |
| `swappable-label` | 1 | Detachable blank derived from a bin interface | [`2x-compatible.toml`](examples/swappable-labels/2x-compatible.toml) |
| `bin-set` | 1 | Checked set of compatible bin parts | [`constituent-2x2.toml`](examples/sets/constituent-2x2.toml) |
| `label` | 1 | SVG artwork tied to a bin prototype | [`hello.toml`](examples/labels/hello.toml) |

A bin definition can be compact:

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

A label references its bin and an SVG template:

```toml
kind = "label"
version = 1
template = "../templates/label.svg"
bin = "../bins/fasteners.toml"
filament = 0

[text.main]
content = 'M{3}x[10]'

[[icons.fasteners]]
icon = "../icons/bolt.svg"

[[icons.fasteners]]
spacer = "1mm"

[[icons.fasteners]]
icon = "../icons/nut.svg"
```

Templates need physical `width` and `height`, a `viewBox`, `text-NAME` elements,
and `icons-NAME` rectangles. Label TOML uses `NAME` as the field key. Plain text
uses filament 1. `{}`, `[]`, and `<>` select filaments 2, 3, and 4. `!N{}`
selects any non-negative filament ID.

### Validate and preview

```sh
gfty bin validate bins/fasteners.toml
gfty bin inspect bins/fasteners.toml

gfty --system-fonts label validate labels/m3.toml
gfty --system-fonts label render labels/m3.toml --output m3.svg
gfty --system-fonts --preview label inspect labels/m3.toml
gfty --system-fonts label watch labels/m3.toml --svg m3.svg
```

Create and optionally save a label without writing TOML first:

```sh
gfty --system-fonts label create \
  --template templates/label.svg \
  --bin bins/fasteners.toml \
  --text main 'M{3}x[10]' \
  --icon fasteners icons/bolt.svg \
  --save labels/m3x10.toml \
  --svg m3x10.svg
```

The CLI has no plate TOML. A plate is an ordered list of label TOMLs. Labels are
placed row-major without rotation.

```sh
gfty --system-fonts label plate create \
  --dimensions 200mm 250mm \
  --svg labels.svg \
  labels/m3.toml labels/m4.toml labels/m5.toml
```

### Export STEP

`gfty export` dispatches any supported TOML to the correct exporter:

```sh
gfty export bins/fasteners.toml --output fasteners.step --image fasteners.png
gfty export bases/2x2.toml --output base.step
gfty export rims/2x2.toml --output rim.step
gfty export sets/fasteners.toml --output fastener-set.step
gfty connector-pin export --output connector-pin.step

gfty --system-fonts export labels/m3.toml --output m3.step
gfty --system-fonts label plate export \
  --bin bins/fasteners.toml \
  --dimensions 200mm 250mm \
  --output labels.step \
  labels/m3.toml labels/m4.toml labels/m5.toml
```

Label STEP files contain one overlapping part per filament ID. Import them as a
multi-part object and assign each part to its filament. Stable part names sort
lower filament IDs first for overlap priority in OrcaSlicer. Multi-label plates
also contain a 1 mm sacrificial connector layer. Lower the object by 1 mm in the
slicer to hide it below the build plate.

Store Onshape API credentials in a mode-0600 file:

```toml
# ~/.config/gfty/onshape.toml
access-key = "..."
secret-key = "..."
```

Use `--onshape-credentials PATH` to select another file. The environment
variables `GFTY_ONSHAPE_ACCESS_KEY` and `GFTY_ONSHAPE_SECRET_KEY` are also
supported. A read-only document API key is sufficient.

Run `gfty --help` or `gfty <command> --help` for the complete command reference.

## Usage: Nix

The flake-parts module defines a library under `perSystem.gfty`. Evaluation
checks references and compatible dimensions. Builds produce typed TOML and SVG
previews without contacting Onshape.

```nix
{
  inputs.flake-parts.url = "github:hercules-ci/flake-parts";
  inputs.gfty.url = "github:rkana-org/gfty";

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.gfty.flakeModules.default ];
      systems = [ "x86_64-linux" "aarch64-linux" ];

      perSystem = { ... }: {
        gfty = {
          bins.fasteners = {
            size = [ 2 2 6 ];
            divider.columns = [ "auto" "auto" "auto" ];
            divider.rows = [ "auto" "auto" ];
          };

          bases.fasteners = {
            size = [ 2 2 ];
            magnets.enabled = true;
            magnets.connectorCutouts = true;
          };

          rims.fasteners = {
            size = [ 2 2 ];
          };

          swappableLabels.fasteners = {
            bin = "fasteners";
          };

          binSets.fasteners = {
            bin = "fasteners";
            base = "fasteners";
            rim = "fasteners";
            swappableLabel = "fasteners";
            connectorPin = true;
          };

          labels.m3 = {
            bin = "fasteners";
            template = ./templates/label.svg;
            text.main = "M{3}";
            icons.fasteners = [ ./icons/bolt.svg ];
          };

          plates.labels = {
            bin = "fasteners";
            dimensions = [ "200mm" "250mm" ];
            labels = [ "m3" "m3" ];
          };
        };
      };
    };
}
```

Build individual definitions or complete bin and label collections:

```sh
nix build .#bins.fasteners
nix build .#bin-sets.fasteners
nix build .#labels.m3
nix build .#plates.labels
nix build .#bins.all
nix build .#labels.all
```

Generated apps build local inputs, call Onshape at runtime, and write the STEP to
the current directory:

```sh
nix run .#export-bin-set-fasteners
nix run .#export-label-m3
nix run .#export-plate-labels
nix run .#export-label-m3 -- --output custom.step --force
```

Runtime export keeps credentials and remote files outside the Nix store. Remote
STEP bytes are not assumed to be reproducible. They are cached by normalized
request and validated before use.

For direct builders, add `inputs.gfty.overlays.default` and use
`pkgs.gfty.mkBin`, `mkBase`, `mkRim`, `mkSwappableLabel`, `mkBinSet`, `mkLabel`,
or `mkPlate`. See the buildable [`examples/`](examples/) flake for a complete
integration.

## Contributing

Contributions and bug reports are welcome. Read [`AGENTS.md`](AGENTS.md) before
changing behavior or public interfaces.

```sh
nix develop
nix fmt
cargo test
cargo clippy --all-targets --all-features -- -D warnings
nix flake check
nix flake check ./examples
```

FeatureScript has no local compiler. FeatureScript changes also need an Onshape
compile and smoke test.

## License

Licensed under the MIT license. See [`LICENSE`](LICENSE) or
<https://opensource.org/licenses/MIT>.
