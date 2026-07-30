# gfty-label development guide

## Scope and purpose

`gfty` (in the transitional `gfty-label` repository/package) is a Rust CLI, Nix
module, and small set of Onshape FeatureScripts for reproducibly turning SVG
label artwork into colored Onshape geometry. It supports individual labels and
row-major multi-label plates, with bins planned next.

The current end-to-end model is:

1. Label TOML + SVG template/icons -> composed SVG.
2. Composed SVG -> compact schema-version-2 geometry JSON grouped by filament.
3. A Gridfinity Ultimate configuration produces the blank prototype in Onshape.
4. `featurescript/gfty_label_instances.fs` copies that prototype, creates the
   artwork, and joins multi-label output with sacrificial connector plates.
5. Nix builders expose reproducible label/plate bundles and flake-parts outputs.

Read the repository `README.md` before changing user-facing behavior. The parent
repository also has `/home/malte/onshape/AGENTS.md`; in particular, local
FeatureScript docs are in `../featurescript-docs/` and standard-library source
is in `../std-library/`.

## Repository map

- `src/main.rs`: command dispatch, output behavior, and inspection.
- `src/cli.rs`: Clap interface. Keep stdout clean for data-producing commands.
- `src/config.rs`: TOML schema, path resolution, discovery, and validation.
- `src/create.rs`: unsaved label creation and reusable TOML saving.
- `src/credentials.rs`: protected Onshape credential-file discovery.
- `src/onshape.rs`: signed encode/translate/poll/download API operations.
- `src/step.rs`: expected filament manifest validation and atomic downloads.
- `src/template.rs`: SVG template contract and icon-box metadata.
- `src/compose.rs`: shared label composition pipeline.
- `src/color.rs`: SVG fill/stroke discovery, sidecars, overrides, preview colors.
- `src/layout.rs`: icon flow/alignment.
- `src/svg.rs`: reusable `usvg` parser and text outlining.
- `src/export.rs`: schema-version-2 geometry export.
- `src/plate.rs`: dimension-constrained row-major plate layout.
- `src/terminal_preview.rs`: native rasteroid previews.
- `src/watch.rs`: dependency watching and rebuild presentation.
- `featurescript/gfty_label_instances.fs`: current version-2 Onshape importer.
- `featurescript/gfty_label_importer.fs`: legacy version-1 importer only.
- `featurescript/variable_configured_derived.fs`: wrapper around native Derived
  which forwards current Part Studio variables into source configuration IDs.
- `nix/mk-label.nix`, `nix/mk-plate.nix`: passthru builders.
- `flake-module.nix`: typed flake-parts label/plate definitions and nested output
  packages.
- `examples/`: standalone flake-parts integration test and documentation.

## Core behavior and invariants

### Paths and discovery

- New label TOML uses `kind = "label"` and `version = 1`; legacy files without
  those fields remain compatible.
- There is no required project marker or directory layout.
- Absolute paths work everywhere.
- Paths in saved label TOML resolve relative to that TOML.
- `gfty label create` paths resolve relative to the current working directory.
- Pathless `validate` recursively scans `ROOT/labels`; `--root` overrides ROOT.
- `.svg` icon values are paths. Other icon values are aliases in `[icon.NAME]`.

### Templates and composition

- Templates require physical `width`/`height`, a `viewBox`, unique `text-*`
  elements, and unique `icons-*` rectangles.
- `data-gfty-direction` is `horizontal` or `vertical`.
- `data-gfty-align` is `left|center|right` horizontally or
  `top|center|bottom` vertically.
- Icons preserve input order and aspect ratio. There are no implicit gaps;
  spacers are explicit.
- Keep XML mutable with `xmltree`, then let `usvg` resolve viewport transforms,
  affine transforms, primitives, strokes, and text outlines.
- Requested fonts must exist. Do not silently substitute missing text fonts.
- Default Nix fonts are bundled. `--font-dir` is repeatable; host fonts are only
  scanned with `--system-fonts`.

### Filaments and colors

- Filament IDs are arbitrary non-negative integers.
- The blank prototype filament defaults to `0`.
- Plain text is filament `1`; `{}`, `[]`, and `<>` select `2`, `3`, and `4`;
  `!N{}` selects any ID.
- Missing icon sidecars map effective inherited fill and stroke colors in
  normalized lexical order starting at filament `1`.
- Present sidecars are exhaustive. Exact hex overrides beat resolved-index
  overrides.
- Preserve the stable preview/FeatureScript palette documented in `README.md`.

### Export schema

Current JSON is version 2:

```json
{
  "version": 2,
  "size": [36, 10],
  "filaments": [0, 1],
  "labels": [{
    "center": [0, 0],
    "size": [36, 10],
    "filament": 0,
    "parts": [{
      "filament": 1,
      "shapes": [{ "path": "M ... L ... C ... Z" }]
    }]
  }]
}
```

- Coordinates are centered physical millimeters with Y pointing upward.
- Paths use compact absolute `M`, `L`, `C`, and `Z` only.
- Top-level filaments are sorted and include base plus artwork filaments.
- Geometry remains local to each label; `center` places it in the overall size.
- Keep old version-2 JSON without a per-label `filament` compatible in
  FeatureScript when practical.

### Plates

- Plate generation is CLI-only and never rotates labels.
- Labels are placed row-major from the top left; default gaps are 5 mm.
- Every label in one plate has the same physical viewport size.
- `--dimensions WIDTH HEIGHT` is a maximum bounding size.
- Multi-label Onshape output gets a full-layout, 1 mm-thick connector plate per
  filament, including gaps. It is hidden by moving the print down 1 mm in the
  slicer. Single labels do not get this plate.

### Onshape geometry

- Pattern all coincident prototype copies before booleans; use
  `qPatternInstances` to isolate identities.
- Skip `opBoolean(UNION)` for a one-body query (`BOOLEAN_BAD_INPUT`).
- Name bodies before union. Names are zero-padded `part-<filament>` so lower
  filament IDs sort first in OrcaSlicer and retain overlap priority.
- Each label may have a distinct base filament.
- Prototype copies should only be created where a label uses the filament as
  base or artwork.
- FeatureScript has no local compiler. Any change must be called out as needing
  an Onshape compile and smoke test, especially query identity, booleans,
  appearance propagation, sheet metal, and manipulators.

## CLI presentation

- Data-producing commands should emit only requested data on stdout.
- Avoid redundant success messages.
- Errors must include actionable label/template/icon/sidecar/output context.
- Use restrained Cargo-style status colors, respect terminal capability and
  `NO_COLOR`, and leave ordinary output mostly uncolored.
- Listing commands were intentionally replaced by `inspect FILE`.
- Previews occur only when explicitly requested, except interactive `render`
  without `--output` and watch mode.
- Terminal graphics are native through `rasteroid` (Kitty, iTerm2, Sixel, then
  Unicode fallback); do not add a Chafa dependency.

## Nix interfaces

- `packages.default` exposes `mkLabel` and `mkPlate` passthru builders.
- The packaged main program is `gfty`; `gfty-label` is a compatibility symlink.
- `overlays.default` is generated with flake-parts `easyOverlay` and exposes both
  `pkgs.gfty` and the compatibility name `pkgs.gfty-label`.
- The flake module uses typed options under `perSystem.gfty-label`.
- Outputs are accessed as `packages.labels.<name>` and
  `packages.plates.<name>`. `packages.labels.all` is a link farm containing one
  symlink per label name.
- Label bundles contain `label.svg` and `label.toml`; plate bundles contain
  `plate.svg`. Runtime exporters generate geometry JSON in memory.
- Every module label and plate has a JSON-serializable `gfty-ultimate` attribute
  set. Plates use their own configuration; child configurations are ignored
  except that `size_x_units` and `size_y_units` must match.
- Browser `onshapeUrl` passthru values were removed because they fail around
  5-6 KB. The module generates `export-label-<name>` and
  `export-plate-<name>` apps instead. These perform runtime API exports outside
  the Nix sandbox; credentials must never be captured by Nix.
- `perSystem.gfty-label.labelModelUrl` pins the immutable model version used by
  generated apps.

## Onshape REST API direction

Local API documentation is in `../onshape-web-api/`. Read at least:

- `auth/apikeys.md` and `auth/limits.md`
- `api-adv/configs.md`
- `api-adv/featureaccess.md`
- `api-adv/fs.md`
- `api-adv/translation.md`

The preferred avenue for large geometry is a configured API export rather than
a browser URL:

1. POST `Config` and `GFTYUltimateConfig` in the body of
   `Element/encodeConfigurationMap`.
2. Pass the returned `encodedId` as `configuration` in the JSON body of
   `PartStudio/createPartStudioTranslation`, with `formatName = "STEP"` and
   `storeInDocument = false` for the validated STEP workflow.
3. Poll the translation with exponential backoff until `DONE` or `FAILED`.
4. Download `resultExternalDataIds` with `downloadExternalData`, or a stored blob
   via `downloadFileWorkspace`.

This was validated against an immutable version with 65,595 bytes of raw JSON.
The production model was also exported with `gfty-label-library` single-label
and two-label plate inputs; each STEP contained exactly the four expected named
filament bodies and no helper parts. The generic `createPartStudioTranslation`
body schema has `configuration`; the format-specific
`createPartStudioExportStep` schema currently does not. Translation responses do
not expose successful FeatureScript warnings, so enforce invalid output as
regeneration errors and validate downloaded STEP part names/counts. See
`docs/onshape-api.md` for request and test details. The authenticated live schema
can be retrieved from `/api/openapi`.

Fallbacks which mutate a workspace are possible but less desirable:

- GET the feature list, clone the returned internal feature definition, modify
  the `Config`/`GFTYUltimateConfig` string parameters, and POST
  `updatePartStudioFeature`.
- Upload/update a JSON blob and change the FeatureScript importer to consume a
  `JSONData` reference.

Both create microversions, have concurrency concerns, and require a mutable
workspace. `evalFeatureScript` evaluates lambdas but does not persist geometry,
so it is useful for queries/validation, not data upload.

API credentials are secrets:

- Never put access keys, secret keys, authorization headers, or generated
  credential files in Git, Nix expressions, derivation arguments, or the Nix
  store.
- Prefer environment variables or runtime-only files with restrictive
  permissions.
- Personal automation may use API keys; App Store distribution requires OAuth2.
- Handle 307 redirects by re-authenticating the redirected request.
- Respect 429 responses and annual quotas (free/standard accounts currently get
  2,500 successful API calls per year). Poll slowly with exponential backoff.

## Development and verification

Use the Nix development shell so bundled fonts and tool versions match builds:

```sh
nix develop
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
nix flake check
nix build --no-link
nix flake check ./examples
nix build ./examples#labels.module-example --no-link
nix build ./examples#labels.all --no-link
nix build ./examples#plates.module-example --no-link
```

`nix fmt` runs treefmt (Rust and Nix formatting plus deadnix/statix checks).
There are currently unit tests in the binary crate; add focused regressions for
parser, transform, layout, color, and export bugs. For terminal behavior, use a
PTY smoke test when changing previews/watch output.

Before finishing:

1. Run `nix fmt` and `git diff --check`.
2. Run Rust tests and strict Clippy for Rust changes.
3. Run the main flake check and relevant example builds for Nix changes.
4. Verify stdout/stderr behavior manually for CLI changes.
5. State explicitly which FeatureScript behavior still requires Onshape testing.
6. Do not modify or delete unrelated user files or local/untracked files.
