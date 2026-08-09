# gfty examples

This directory demonstrates both direct TOML authoring and the typed flake-parts
module.

## Direct files

- `bins/constituent-2x2.toml`: bin body with swappable interfaces.
- `bins/compatible-label-2x1.toml`: differently expressed compatible label layout.
- `bases/2x2-magnetic.toml` and `rims/2x2-standard.toml`: independent parts.
- `swappable-labels/2x-compatible.toml` and `2x-from-set-bin.toml`: labels
  derived from different but compatible first-row layouts; both normalize to
  one runtime request/cache entry.
- `sets/constituent-2x2.toml`: validated composition of all constituents.
- `labels/hello.toml`: artwork label referencing a bin TOML.

```sh
gfty bin validate examples/bins/constituent-2x2.toml
gfty bin inspect examples/bins/constituent-2x2.toml
gfty label validate examples/labels/hello.toml
gfty label render examples/labels/hello.toml \
  --output /tmp/hello.svg --json /tmp/hello.json
```

Constituent exports use configured part discovery and Onshape's `partIds`
filter, so each STEP and PNG contains only its requested named part even though
the upstream model remains unified.

## Flake-parts module

The standalone `examples/flake.nix` imports the root module and `module.nix`.
Build individual definitions through nested outputs:

```sh
nix build ./examples#bins.module-example
nix build ./examples#bins.bin-only
nix build ./examples#bases.module-example
nix build ./examples#rims.module-example
nix build ./examples#swappable-labels.module-example
nix build ./examples#bin-sets.module-example
nix build ./examples#labels.module-example
nix build ./examples#plates.module-example
```

Build combined collections with:

```sh
nix build ./examples#bins.all
nix build ./examples#labels.all
```

A bin contains `bin.toml`, a label contains `label.svg` and `label.toml`, and a
plate contains `plate.svg`. Exact geometry JSON can be generated locally with
`gfty label render LABEL.toml --json OUTPUT.json`.

Every definition that supports remote export has an explicit runtime app:

```sh
nix run ./examples#export-bin-module-example
nix run ./examples#export-bin-module-example -- --image preview.png
nix run ./examples#export-bin-bin-only
nix run ./examples#export-base-module-example
nix run ./examples#export-rim-module-example
nix run ./examples#export-swappable-label-module-example
nix run ./examples#export-bin-set-module-example
nix run ./examples#export-connector-pin
nix run ./examples#export-label-module-example
nix run ./examples#export-plate-module-example
```

These commands obtain credentials through `gfty` at runtime and send typed named
bin configurations in API POST bodies. Credentials and downloaded STEP files
never enter Nix evaluation or the store.
