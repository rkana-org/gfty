# gfty development guide

## Scope and purpose

`gfty` is a Rust CLI/package, Nix module, browser designer, and Onshape FeatureScripts for reproducibly turning
SVG label artwork and typed bin TOML into colored Onshape geometry. It supports
standalone bins, individual labels, row-major multi-label plates, and the
Gridfinity Ultimate web workflow.

The current end-to-end model is:

1. Typed bin TOML -> canonical Gridfinity Ultimate configuration JSON.
2. Label TOML + SVG template/icons -> composed SVG.
3. Composed SVG -> compact schema-version-2 geometry JSON grouped by filament.
4. A referenced bin configuration produces the blank prototype in Onshape.
5. `featurescripts/labels/gfty_label_instances.fs` copies that prototype, creates the
   artwork, and joins multi-label output with sacrificial connector plates.
6. Nix builders expose reproducible bin/label/plate bundles and flake-parts
   outputs.

Read the repository `README.md` before changing user-facing behavior. The parent
repository also has `/home/malte/onshape/AGENTS.md`; in particular, local
FeatureScript docs are in `../featurescript-docs/` and standard-library source
is in `../std-library/`.

## Repository map

- `designer/`: Gridfinity Ultimate browser application and Nix site package.
- `featurescripts/`: every maintained FeatureScript, organized by labels,
  configuration, and divider generation.
- `src/main.rs`: command dispatch, output behavior, and inspection.
- `src/cli.rs`: Clap interface. Keep stdout clean for data-producing commands.
- `src/config.rs`: label TOML schema, path resolution, discovery, and validation.
- `src/bin_config.rs`: current bin TOML, divider normalization, canonical model
  JSON conversion, and generated-part discovery.
- `src/component_config.rs`: independent base/rim/swappable-label/set schemas,
  compatibility checks, carrier configurations, and semantic request keys.
- `src/artifact_cache.rs`: verified runtime STEP/PNG cache outside the Nix store.
- `src/create.rs`: unsaved label creation and reusable TOML saving.
- `src/credentials.rs`: protected Onshape credential-file discovery.
- `src/onshape.rs`: signed encode/translate/poll/download API operations.
- `src/step.rs`: expected filament manifest validation and atomic downloads.
- `docs/ARCHITECTURE.md`: current configuration, geometry, Onshape transport,
  cache, Nix-boundary, FeatureScript, and designer contracts.
- `src/template.rs`: SVG template contract and icon-box metadata.
- `src/compose.rs`: shared label composition pipeline.
- `src/color.rs`: SVG fill/stroke discovery, sidecars, overrides, preview colors.
- `src/layout.rs`: icon flow/alignment.
- `src/svg.rs`: reusable `usvg` parser and text outlining.
- `src/export.rs`: schema-version-2 geometry export.
- `src/plate.rs`: dimension-constrained row-major plate layout.
- `src/terminal_preview.rs`: native rasteroid previews.
- `src/watch.rs`: dependency watching and rebuild presentation.
- `featurescripts/labels/gfty_label_instances.fs`: version-2 Onshape importer.
- `featurescripts/configuration/variable_configured_derived.fs`: wrapper around
  native Derived which forwards current variables into source configuration IDs.
- `featurescripts/configuration/extract_json_config.fs`: Gridfinity JSON-to-variable
  bridge.
- `featurescripts/dividers/walls_grid.fs`: divider and easy-grab wall generator.
- `nix/mk-label.nix`, `nix/mk-plate.nix`: passthru builders.
- `flake-module.nix`: typed flake-parts label/plate definitions and nested output
  packages.
- `examples/`: standalone flake-parts integration test and documentation.
- `tests/bin-designer-conformance.js`: verifies the browser default
  configuration against a frozen JSON fixture.
- `.github/workflows/designer-pages.yml`: deploys `packages.designer` from
  `designer-v*` tags.

## Core behavior and invariants

### Paths and discovery

- Bin TOML requires `kind = "bin"` and `version = 2`.
- Label TOML requires `kind = "label"`, `version = 1`, and a bin reference.
- Do not add compatibility parsing for retired schemas or implicit kinds.
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
- Icon layout/aspect ratio is based on the icon SVG viewBox, but icon geometry
  outside the icon SVG canvas/viewBox must remain visible after composition.
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
- Every label must provide its base `filament`; reject incomplete older JSON.

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

- `packages.default` exposes builders for bins, bases, rims, swappable labels,
  bin sets, artwork labels, and plates.
- The package and main program are `gfty`.
- `overlays.default` is generated with flake-parts `easyOverlay` and exposes
  `pkgs.gfty`.
- The flake module uses typed options under `perSystem.gfty`.
- Outputs are grouped under `packages.bins`, `bases`, `rims`,
  `swappable-labels`, `bin-sets`, `labels`, and `plates`.
- Label bundles contain `label.svg` and `label.toml`; plate bundles contain
  `plate.svg`. Runtime exporters generate geometry JSON in memory.
- Module labels and plates require a named bin. Plates own the reference, and
  all child labels must use the same X/Y bin size.
- Browser `onshapeUrl` passthru values were removed because they fail around
  5-6 KB. The module generates `export-label-<name>` and
  `export-plate-<name>` apps instead. These perform runtime API exports outside
  the Nix sandbox; credentials must never be captured by Nix.
- `perSystem.gfty.labelModelUrl` and `binModelUrl` pin immutable model
  versions used by generated apps.
- Each constituent TOML exports its one exact named part; `bin-set` exports its
  declared group. Resolve configured IDs at runtime because IDs change with
  configuration and must never be hard-coded.
- Constituent `base`, `rim`, `swappable-label`, `bin`, and `bin-set` TOML are
  supported. Swappable labels reference bins, then normalize only X,
  depth, and effective row-zero divider boundaries. Source paths, Y/Z, later
  rows, and unrelated settings must not affect their runtime request key.
- The runtime cache lives outside the store, is keyed by normalized request plus
  immutable model/export options, and validates both content hashes and exact
  STEP manifests. `--no-cache` bypasses it. Never cache credentials.
- A generic helper in whole-studio base-only output is safely excluded by named
  part selection. Shaded views and configured-parts discovery remain GET-only,
  so this approach suits compact Gridfinity configuration but not large
  artwork-label geometry.

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
The production label model was also exported with `gfty-library`
single-label and two-label plate inputs; each STEP contained exactly the four
expected named filament bodies and no helper parts. The Gridfinity Ultimate
designer model exported five expected named bodies from its existing `Config`
parameter, so neither upstream model needs an API-specific change. The generic
`createPartStudioTranslation`
body schema has `configuration`; the format-specific
`createPartStudioExportStep` schema currently does not. Translation responses do
not expose successful FeatureScript warnings, so enforce invalid output as
regeneration errors and validate downloaded STEP part names/counts. See
`docs/ARCHITECTURE.md` for the stable transport contract. The authenticated live
schema can be retrieved from `/api/openapi`.

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
nix build .#designer --no-link
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
