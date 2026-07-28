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

`validate` and `render` are implemented. Rendering resolves bundled/project
fonts, converts text and SVG primitives to paths with `usvg`, applies filament
colors, and lays out icons without implicit gaps. `export`, `quick`, and `watch`
are currently placeholders.

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
