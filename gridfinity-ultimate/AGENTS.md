# Gridfinity Ultimate — development notes

Parametric [Gridfinity](https://gridfinity.xyz) system built as an Onshape CAD model,
configured via JSON. This directory was imported with its complete Git history into the `gfty`
repository. It holds:

| Path | Role |
|------|------|
| `designer/` | Static, fully client-side web configurator that builds the JSON config. Published to GitHub Pages. |
| `extract_json_config.fs` | Onshape FeatureScript: turns a schema JSON + values JSON into Part Studio variables (the entry point for the JSON config inside the CAD model). |
| `wall-generator/` | Onshape FeatureScript that generates the divider wall grid from the layout JSON (see its README for the JSON format). |
| `nix/designer.nix` | Nix build of the designer site, exposed by the root flake. |
| `../.github/workflows/designer-pages.yml` | Deploys the designer to GitHub Pages on `designer-v*` tags. |

## Designer architecture

Plain HTML + CSS + React (global UMD builds), JSX, **no application server and no
bundler**. Every module uses the **global-script pattern** (`window.X = X`, no ES
`import`/`export`) — that is what lets the production build transpile each file in
place without bundling. Keep new modules in this style.

| File | Role |
|------|------|
| `designer/index.html` | **Single source of truth for the page markup.** Dev entry point: loads React + Babel from a CDN and the `.jsx` sources directly. The Nix build rewrites it for production (see below). |
| `designer/logic.js` | Pure, framework-free model/serialisation logic (`window.GF`). No React here. |
| `designer/ui.jsx` | Shared primitives + icons (`window.Icon`, `Section`, `Field`, …). |
| `designer/DividerEditor.jsx` `Controls.jsx` `JsonPanel.jsx` `App.jsx` | UI modules, each exposed on `window.*`. |
| `designer/styles.css` | Design tokens + component styles. |

Adding a new JSX module: add a `<script type="text/babel" src="X.jsx">` tag to
`index.html` — the Nix build picks up `*.jsx` automatically and rewrites the tag.

## Development

Run everything from the `gfty` repository root through its devshell:

- `designer-dev` — serve `gridfinity-ultimate/designer/` on :8080 with live
  reload. JSX is compiled in the browser by `@babel/standalone`.
- `designer-preview` — build and serve the production result on :8081.
- `nix build .#designer` — build the deployable static site.
- `nix fmt` — format/check Rust and Nix across the monorepo.
- `nix flake check` — includes the Rust/web default-configuration conformance
  fixture as well as the normal repository checks.

## Production build (`gridfinity-ultimate/nix/designer.nix`)

No npm/node_modules. The derivation:

1. Transpiles each `designer/*.jsx` with **esbuild** (plain JSX → `React.createElement`).
2. Vendors the **production** React UMD builds via `fetchurl` — the deployed site
   has no runtime CDN dependency (except Google Fonts).
3. Rewrites `index.html`: drops `@babel/standalone`, swaps dev React for the
   vendored production builds, `.jsx` → `.js`. A grep guard fails the build if any
   dev-only reference survives.

The derivation `version` is parsed from `ONSHAPE_VERSION` in `designer/JsonPanel.jsx`.
The React version is pinned in **two** places that must stay in sync:
`designer/index.html` (dev CDN tags) and `nix/designer.nix` (`reactVersion` + hashes).

## Onshape model versioning & releases

The "Open in Onshape" button targets a **version-pinned** model link, defined by
`ONSHAPE_VERSION` / `ONSHAPE_BASE` at the top of `designer/JsonPanel.jsx`. When a new
model version ships, bump **both** constants together so existing configs keep
working against a known-good version.

Release flow: bump `ONSHAPE_VERSION` + `ONSHAPE_BASE`, commit, then tag and push
`designer-v<version>` (for example `designer-v42`). CI strips `designer-` and
refuses to deploy if the remainder does not match `ONSHAPE_VERSION`
(case-insensitive).
