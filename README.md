<img width="5041" height="2028" alt="image" src="https://github.com/user-attachments/assets/48da24d9-6c2d-4896-b691-8758da34f7e2" />

[🌐 Web designer](https://rkana-org.github.io/gfty/) | [<kbd>>_</kbd> CLI](#usage-cli) | [❄️ Nix](#usage-nix) | [🌈 Examples](examples/)

## gfty

`gfty` is a tool to generate fully customizable gridfinity bins with
compartments, baseplates and detachable labels and rims based on my [Gridfinity Ultimate Onshape model](https://cad.onshape.com/documents/044aa38d921c6673acd89aef/w/ec26a4ac88951ab051a8d0c0/e/47f09ccd9b344504691f98d4).
The tool itself is CLI-first and designed with automation in mind, allowing you
to define a library of bins and labels via TOML or Nix that can then be
automatically generated and downloaded from onshape by the tool. This includes
multi-colored labels with aritrary SVG artwork and text as one predictably
named part per color, ready for automatic import in a slicer software of your
choosing.

Additionally this tool has a web UI (thanks clanker) that can be used to
quickly test some designs in onshape or to help with the TOML or Nix
configuration. So the **TL;DR** is:

- ▶️ **Model previews and live label previews** for any object using the `gfty` CLI tool
- 🧰 **Fully parametric model** that includes bins, bases, detachable rims, detachable label blanks, connector pins, artwork labels, and multi-label plates
- 🌐 **Web designer** for quick bin configuration and one-off exports
- ❄️ **Declarative and automation-first** Nix definitions → typed TOML → FeatureScript JSON → parameterized Onshape models → automatic STEP file & preview image downloads <sub>(i may or may not have created over 1000 labels for my shop, send help)</sub>
- 🌈 **Multi-color swappable labels** with reusable templates, arbitrary SVG artwork, multi-color text and artwork
- 🎩 **Swappable rims** to allow for re-coloring the top for categorization of parts (or whatever you want to do with it)

## But.. Why??

Ehh.. because why not. The rundown is that it started out as me wanting a
parametric gridfinity model with detachable rims, mainly so I can change their
color for organizational purposes. Then I decided printing labels on a label
printer sucks and I got focused on detachable 3D-printed labels which came with
their own problems: Basically onshape and SVG is not a thing (well now it is
😅), automatic exports require calling the (free) API, and so on. Then I
noticed that generating over a thousand labels by hand is not gonna happen, so
I made the whole process controlled via a TOML file and added generation of
full plates of labels at once. Thanks for coming to my TED talk. Anyhow, have
fun with it.

## 🌐 Usage: Web Designer

Open the hosted [Web Designer](https://rkana-org.github.io/gfty/). You can play
around with the settings on the left, merge compartments (or configure scoops)
in the middle section and see the resulting JSON config on the right plus a
button to open that specific config on onshape (no account required). You can
also view the configuration as TOML for the `gfty` CLI tool or as `nix` code
for the more powerful reproducible variant [(see below)](#usage-nix).

Everything runs locally in the browser or in onshape. Labels and SVG artwork can
currently only be created in TOML or Nix due to the complexity. Usually you'd
want to create the SVGs in specialized software like Inkscape anyway. And
designing 1000 labels in a web ui is no fun, so automate all the things!

<a id="usage-cli"></a>

## 💻 Usage: gfty CLI

The `gfty` CLI reads TOML, configures pinned immutable Onshape models, and
downloads STEP files. Validation, SVG rendering, and plate layout run locally.
Only model export needs Onshape credentials. You will need to create a API key
in your onshape account if you want to do that, it's free and has monthly
limits (which are huge for this purpose, so no big deal).

### Install

Installing via nix is the easiest, just run `nix shell github:rkana-org/gfty`
and you are put into a shell with the latest version of the tool once it has
compiled (takes a minute or two). Otherwise, if you don't want nix, you can
install it via cargo (which you need to get yourself):

```sh
cargo install --locked --git https://github.com/rkana-org/gfty
```

Text rendering only uses explicitly enabled fonts for reproducibiltiy. Pass
`--system-fonts` to use host fonts, or repeat `--font-dir PATH` for selected
font directories.

### Configuration files

Every TOML has an explicit `kind` and `version`. Relative paths resolve from the
TOML that contains them. You can find examples in the [`examples/`](examples/) directory.
If you are still lost, your favourite AI will be able to help you out
(especially coding CLIs as they can also generate stuff for you).

| Kind | Version | Purpose | Example |
| --- | ---: | --- | --- |
| `bin` | 2 | Bin body, dividers, easy grabs, and mating interfaces | [`constituent-2x2.toml`](examples/bins/constituent-2x2.toml) |
| `base` | 1 | Independent Gridfinity base | [`2x2-magnetic.toml`](examples/bases/2x2-magnetic.toml) |
| `rim` | 1 | Detachable rim | [`2x2-standard.toml`](examples/rims/2x2-standard.toml) |
| `swappable-label` | 1 | Detachable blank derived from a bin interface | [`2x-compatible.toml`](examples/swappable-labels/2x-compatible.toml) |
| `bin-set` | 1 | Checked set of compatible bin parts | [`constituent-2x2.toml`](examples/sets/constituent-2x2.toml) |
| `label` | 1 | SVG artwork tied to a bin prototype | [`hello.toml`](examples/labels/hello.toml) |

A bin definition looks like this (let the Web Designer help you here):

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
colors = { 1 = 2 } # Resolved color index (or exact hex) to filament ID.

[[icons.fasteners]]
spacer = "1mm"

[[icons.fasteners]]
icon = "../icons/nut.svg"
scale = 1.1 # Optionally combine with scale-x and scale-y.
```

Templates need physical `width` and `height`, a `viewBox`, `text-NAME`
elements, and `icons-NAME` rectangles. Label TOML uses `NAME` as the field key
to set the text or SVG contents of the respective item. Plain text uses filament
1, the label body will be filament id 0. Text enclosed within `{}`, `[]`, and
`<>` select filaments 2, 3, and 4. `!N{}` selects any non-negative filament ID
`N`.

### Export STEP

`gfty export` creates STEP files (and preview images) for things.
First, store Onshape API credentials in `~/.config/gfty/onshape.toml`:

```toml
# ~/.config/gfty/onshape.toml
access-key = "..."
secret-key = "..."
```

Use `--onshape-credentials PATH` to select another file. The environment
variables `GFTY_ONSHAPE_ACCESS_KEY` and `GFTY_ONSHAPE_SECRET_KEY` are also
supported. A read-only document API key is sufficient.

Label STEP files contain one single part per filament ID. Import them as a
multi-part object and assign each part to its filament. Stable part names sort
lower filament IDs first for overlap priority. Multi-label plates also contain
a 1 mm sacrificial (and sacrilegious) connector layer. Lower the object by 1 mm
in the slicer to hide it below the build plate, it is just necessary to make
the object be one part so you don't have to assign a filament to hundreds of objects.

Examples:

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

### Useful commands

For a live-preview when designing a label try this command:

```sh
gfty --system-fonts label watch your-label.toml
```

Render a label TOML to SVG and/or the exact compact geometry JSON locally,
without contacting Onshape:

```sh
gfty --system-fonts label render your-label.toml \
  --output your-label.svg \
  --json your-label.json
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

Run `gfty --help` or `gfty <command> --help` for the complete command reference.

<a id="usage-nix"></a>

## ❄️ Usage: Nix

So for more advanced stuff, gfty comes with a flake-parts module that allows
defining parts super easily (if you know nix i guess), while allowing you to
use the full power of the nix language to define stuff. As opposed to TOML this
is also typechecked and also somewhat semantically checked (validates
references to other things).

Builds produce typed TOML and SVG previews fully locally without requiring
access to onshape. For convenience it automatically generates export scripts
that can export all defined items individually or all at once.

An example gfty based parts library could look like this:

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

Generated apps build local inputs, call onshape at runtime, and write the STEP
to the current directory. I purposefully don't do that in a nix build because I
cannot guarantee that onshape is reproducible (because it isn't, the same model
may generate different output tomorrow or even today). Also dealing with the
FOD hashes is no fun anyway. So instead, the export script for each part is
formalized as an app and can be run on demand. It will detect previously
downloaded files and skip exporting it again.

```sh
nix run .#export-bin-set-fasteners
nix run .#export-label-m3
nix run .#export-plate-labels
nix run .#export-label-m3 -- --output custom.step --force
```

Runtime export keeps credentials and remote files outside the Nix store. As
said previously, remote STEP bytes are not assumed to be reproducible. They are
cached by normalized request and validated before use.

For direct builders, add `inputs.gfty.overlays.default` to your pkgs instance
and use the passthrough functions `mkBin`, `mkBase`, `mkRim`,
`mkSwappableLabel`, `mkBinSet`, `mkLabel`, or `mkPlate` available on
`pkgs.gfty` (e.g. `pkgs.gfty.mkBin`). See the buildable
[`examples/`](examples/) flake for a complete integration.

## ❤️ Contributing

Contributions and bug reports are welcome. I know you may be a human if you are
reading this, in that case please still refer to [`AGENTS.md`](AGENTS.md)
before changing behavior or public interfaces. It should capture the important
architectural details.

```sh
# enter devshell with all dependencies (cargo commands work in there automatically, no need to install anything else)
nix develop
# format files
nix fmt
# run tests and clippy
cargo test
cargo clippy --all-targets --all-features -- -D warnings
# run tests and clippy on the production build or the examples
nix flake check
nix flake check ./examples
```

If you do FeatureScript changes you will have to fork the onshape model as there is no local compiler.

## 📜 License

Licensed under the MIT license. See [`LICENSE`](LICENSE) or
<https://opensource.org/licenses/MIT>.
