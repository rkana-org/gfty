# Gridfinity Ultimate — development notes

Parametric [Gridfinity](https://gridfinity.xyz) system built as an Onshape CAD model,
configured via JSON. This repo holds:

| Path | Role |
|------|------|
| `designer/` | Static, fully client-side web configurator that builds the JSON config. Published to GitHub Pages. |
| `extract_json_config.fs` | Onshape FeatureScript: turns a schema JSON + values JSON into Part Studio variables (the entry point for the JSON config inside the CAD model). |
| `wall-generator/` | Onshape FeatureScript that generates the divider wall grid from the layout JSON (see its README for the JSON format). |
| `nix/` | Nix build of the designer site + devshell. |
| `.github/workflows/pages.yml` | Deploys the designer to GitHub Pages on `v*` tags. |

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

Everything runs through the flake devshell (`nix develop`, or direnv):

- `dev` — serve `designer/` on :8080 with live reload. JSX is compiled in the
  browser by `@babel/standalone`; just edit and the page reloads. No build step.
- `preview` — `nix build` and serve the production result on :8081.
- `nix build` — builds the static site to `./result` (what CI deploys).
- `nix fmt .` — format Nix files (note: bare `nix fmt` without `.` does not work
  with plain nixfmt).
- pre-commit hooks (nixfmt, deadnix, statix) are installed on shell entry;
  run manually with `pre-commit run -a`.

## Production build (`nix/designer.nix`)

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
`v<version>` (e.g. `v40`). CI refuses to deploy if the tag does not match
`ONSHAPE_VERSION` (case-insensitive).
