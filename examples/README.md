# gfty examples

This directory demonstrates both direct TOML authoring and the typed flake-parts
module.

## Direct files

- `bins/with-magnetic-base.toml`: complete 1×1 bin with an explicit magnetic
  base and connector settings.
- `bins/bin-only.toml`: the same bin with base generation disabled.
- `labels/hello.toml`: label referencing the complete bin TOML.
- `bases/magnetic.nix` and `bases/plain.nix`: reusable Nix base-section presets.

```sh
gfty bin validate examples/bins/with-magnetic-base.toml
gfty bin inspect examples/bins/bin-only.toml
gfty label validate examples/labels/hello.toml
gfty label render examples/labels/hello.toml --output /tmp/hello.svg
```

Bases are currently sections of bin definitions rather than a standalone file
kind. See `bases/README.md` for the model limitation behind that choice.

## Flake-parts module

The standalone `examples/flake.nix` imports the root module and `module.nix`.
Build individual definitions through nested outputs:

```sh
nix build ./examples#bins.module-example
nix build ./examples#bins.bin-only
nix build ./examples#labels.module-example
nix build ./examples#plates.module-example
```

Build combined collections with:

```sh
nix build ./examples#bins.all
nix build ./examples#labels.all
```

A bin contains `bin.toml`, a label contains `label.svg` and `label.toml`, and a
plate contains `plate.svg`. Geometry JSON remains internal to runtime export.

Every definition that supports remote export has an explicit runtime app:

```sh
nix run ./examples#export-bin-module-example
nix run ./examples#export-bin-module-example -- --image preview.png
nix run ./examples#export-bin-bin-only
nix run ./examples#export-label-module-example
nix run ./examples#export-plate-module-example
```

These commands obtain credentials through `gfty` at runtime and send typed named
bin configurations in API POST bodies. Credentials and downloaded STEP files
never enter Nix evaluation or the store.
