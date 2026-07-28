# gfty-label examples

This directory is a complete project root containing one template, two icons,
and two labels. Run these commands from `label-designer/`:

```sh
nix run .# -- validate examples/labels/metric-fastener.toml
nix run .# -- validate examples/labels/custom-colors.toml

nix run .# -- render examples/labels/metric-fastener.toml \
  --output /tmp/metric-fastener.svg
nix run .# -- export examples/labels/metric-fastener.toml \
  --output /tmp/metric-fastener.json

nix run .# -- plate \
  --columns 2 \
  --column-gap 2mm \
  --svg /tmp/example-plate.svg \
  --json /tmp/example-plate.json \
  examples/labels/metric-fastener.toml \
  examples/labels/custom-colors.toml
```

The bolt uses an exhaustive color sidecar. `custom-colors.toml` demonstrates
both resolved-index and exact-hex icon overrides, while the nut demonstrates
automatic color indexing and an even-odd hole.
