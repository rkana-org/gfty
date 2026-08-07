# Designer development guide

This directory contains the static Gridfinity Ultimate browser configurator.
Repository-wide instructions are in `../AGENTS.md`.

## Architecture

The app is plain HTML, CSS, and React using global UMD objects—there are no ES
modules and no application server.

- `index.html`: development entry point and script ordering.
- `logic.js`: pure configuration defaults, parsing, validation, divider/easy-grab
  geometry, and canonical JSON serialization. Keep React out of this file.
- `syntax.js`: dependency-free JSON, TOML, and Nix tokenizers for output previews.
- `ui.jsx`: shared controls and icons.
- `Controls.jsx`, `DividerEditor.jsx`, `JsonPanel.jsx`, `App.jsx`: UI modules
  exposed through `window.*`.
- `styles.css`: design tokens and component styles.
- `package.nix`: production static-site derivation.

Add a new JSX file to `index.html` as a `text/babel` script. The production build
transpiles every `*.jsx` file independently and rewrites the HTML to use vendored
production React.

## Commands

Run from the repository root:

```sh
nix develop
designer-dev
designer-preview
nix build .#designer
nix flake check
```

The root flake check compares `logic.js` default serialization with the same JSON
fixture used by Rust. Update both implementations intentionally when changing
configuration defaults, units, track sizing, easy-grab behavior, or support
recommendations.

## Onshape versions and releases

`ONSHAPE_VERSION` and `ONSHAPE_BASE` at the top of `JsonPanel.jsx` pin the model.
Bump them together. Designer deployment uses a matching `designer-v<version>`
tag, for example `designer-v42`; CI rejects mismatches.

The browser URL workflow must remain available even when the CLI gains new API
export behavior.
