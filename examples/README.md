# gfty-label examples

This directory contains templates, icons, labels, and a standalone Nix flake.
No project marker is required. Run these commands from `label-designer/`:

```sh
nix run .# -- validate examples/labels/metric-fastener.toml
nix run .# -- validate examples/labels/custom-colors.toml

nix run .# -- render examples/labels/metric-fastener.toml \
  --output /tmp/metric-fastener.svg
nix run .# -- export examples/labels/metric-fastener.toml \
  --output /tmp/metric-fastener.json

nix run .# -- plate \
  --dimensions 100mm 50mm \
  --column-gap 2mm \
  --svg /tmp/example-plate.svg \
  --json /tmp/example-plate.json \
  examples/labels/metric-fastener.toml \
  examples/labels/custom-colors.toml
```

`metric-fastener.toml` references SVG paths relative to its own location,
without icon declarations. The bolt uses an exhaustive color sidecar.
`custom-colors.toml` demonstrates aliases with both resolved-index and exact-hex
icon overrides, while the nut demonstrates automatic color indexing and an
even-odd hole.

The flake demonstrates the generated overlay, `mkLabel`, `mkPlate`, and the
flake-parts module:

```sh
nix build ./examples#screws
nix build ./examples#plate
nix build ./examples#label-module-example
nix build ./examples#plate-module-example
```

Label outputs contain `label.svg`, `label.json`, and `label.toml`; plate outputs
contain `plate.svg` and `plate.json`.
