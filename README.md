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
```

## Commands

```text
gfty-label validate LABEL
gfty-label render LABEL --output PREVIEW.svg
gfty-label export LABEL --output FILLS.json
gfty-label quick --template TEMPLATE --text ID CONTENT --icon BOX ICON
gfty-label watch LABEL --svg PREVIEW.svg --json FILLS.json
```

Only `validate` and the configuration/text parsing core are implemented in the
initial scaffold.
