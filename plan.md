# gfty roadmap

## Goal

Evolve `gfty-label` into `gfty`: a reproducible command-line and Nix interface
for authoring Gridfinity labels, multi-label plates, bins, and eventually other
Gridfinity artifacts. Local commands validate and preview deterministic TOML
inputs; explicit export actions ask an immutable Onshape model to generate a
multipart STEP file and download it outside the Nix store.

The Onshape API is the geometry backend, not the configuration source of truth.
TOML and Nix remain the source of truth.

## Confirmed Onshape capabilities

The configured export method has been validated end to end:

- `Element/encodeConfigurationMap` accepted 65,595 bytes of raw configuration
  JSON and returned a 65,637-character encoded configuration.
- `PartStudio/createPartStudioTranslation` accepted that configuration in its
  POST body while targeting an immutable version.
- The translation completed without creating or modifying a workspace.
- `Document/downloadExternalData` returned an AP242 STEP containing two
  separately named solid bodies.
- A read-only document API key was sufficient with `storeInDocument = false`.

The generic translation endpoint must be used. Its `BTTranslateFormatParams`
request has a `configuration` field; the format-specific
`createPartStudioExportStep` request currently does not.

Automatic API export always requires authentication. The browser can open a
small configured URL using its Onshape session, but anonymous REST translations
are not the replacement for API credentials. Personal use can use an API key;
OAuth2 is a later concern if `gfty` becomes a distributed Onshape application.

## File kinds and dispatch

Every new gfty-owned TOML format will have an explicit discriminator and schema
version:

```toml
kind = "label"
version = 1
```

Planned kinds are:

- `label`
- `label-plate`
- `bin`
- A future separate base/baseplate kind if its configuration and workflow prove
  semantically distinct from selecting the base component of a bin.

An explicit `kind` is safer than guessing from fields, gives useful errors, and
allows the top-level convenience command to dispatch without repeating a noun:

```sh
gfty export labels/screws.toml
gfty export plates/all-screws.toml
gfty export bins/small-parts.toml
```

Existing label TOML without `kind` remains compatible during migration and is
interpreted as `label`.

## Command organization

Commands are entity-oriented. Export is available where the entity is authored,
while `gfty export FILE` is a convenient generic dispatcher.

```text
gfty export FILE [export options]

gfty label validate [FILE]
gfty label render FILE
gfty label create [label options]
gfty label inspect FILE
gfty label watch FILE
gfty label export FILE [export options]

gfty label plate validate FILE
gfty label plate render FILE
gfty label plate create [plate options]
gfty label plate export FILE [export options]

gfty bin validate FILE
gfty bin inspect FILE
gfty bin export FILE [export options]
```

`gfty export FILE` and the entity-specific `export` commands call the same Rust
implementation. The generic form is ideal for scripts and Nix apps; the
entity-specific forms are discoverable and allow a user to stay in the relevant
command category.

The current `quick` command becomes `gfty label create`. It can save, render, or
export an unsaved label in one invocation, for example:

```sh
gfty label create \
  --template templates/1x1.svg \
  --text top "M3 screws" \
  --bin bins/1x1-label.toml \
  --export m3-screws.step
```

`plate` moves below `label` because it arranges labels and shares their rendering
and font pipeline.

### Existing command migration

- `validate` -> `gfty label validate`
- `render` -> `gfty label render`
- `quick` -> `gfty label create`
- `inspect` -> `gfty label inspect`
- `plate` -> `gfty label plate ...`
- `watch` -> `gfty label watch`
- The current JSON-producing `export` is removed.
- `build` is removed from the public interface or retained only as an internal
  Nix operation if still necessary.

A `gfty-label` compatibility executable can forward old local commands to
`gfty label` while downstream flakes migrate. It must not preserve obsolete JSON
output indefinitely.

## Exporting components

A label is tied to the bin configuration that provides its blank/prototype, so a
label definition may reference a bin definition. Label and label-plate exports
always contain every generated filament part; component selection does not apply
to them. An unexpected additional body is an invalid label rather than an
optional export component.

Component selection remains useful for standalone Gridfinity objects, for
example:

```sh
gfty export bins/small-parts.toml
gfty export bins/small-parts.toml --component base
```

The supported component values are defined per TOML kind and validated before
making an API request. A future separate base/baseplate kind can still be added
if it has its own useful authoring model.

Configured Onshape part IDs are configuration-dependent, and obtaining them
through a large configuration query would recreate the URL-size problem.
Therefore `partIds` should not be the primary design. If the Gridfinity Ultimate
model cannot suppress unwanted standalone-bin components through existing enable
flags, add a small configuration parameter such as `ExportComponent`. That
parameter can be sent beside `Config` in the same API request.

## JSON policy

JSON remains an internal wire format because the current FeatureScripts consume
it, but users should not need to manage geometry JSON.

The API exporter is working and these public outputs have been removed:

- The current compact-JSON `export` command.
- `quick --json`.
- `plate --json`.
- `watch --json`.
- Top-level `label.json` and `plate.json` package artifacts.
- `onshapeUrl` passthru values.

The Rust geometry serializer, schema validation, and tests remain. Export
commands generate geometry JSON in memory. Pure Nix builders may retain a
private intermediate in their output only while needed for migration; runtime
apps should eventually consume TOML/configuration inputs and let `gfty` generate
the request in memory.

SVG remains a useful public preview artifact.

## Onshape model contracts

Each model target is an immutable Part Studio version plus named configuration
parameter IDs.

### Label model

Expected parameters:

- `Config`: gfty label/plate geometry JSON.
- `GFTYUltimateConfig`: Gridfinity Ultimate JSON used to create the bin and
  label prototype.

The currently pinned label model has already been inspected and exported through
the API:

- `Config` is `BTMConfigurationParameterString-872` with parameter ID `Config`.
- `GFTYUltimateConfig` is `BTMConfigurationParameterString-872` with parameter ID
  `GFTYUltimateConfig`.

A `gfty-label-library` label and two-label plate were exported from the immutable
version. Both STEP files contained exactly four solid/product records named
`part-0`, `part-1`, `part-2`, and `part-3`, with no generic or helper parts. The
model is already structurally and geometrically compatible with POST-body
configuration; it needs no component selector.

### Gridfinity Ultimate model

Expected parameter:

- `Config`: canonical Gridfinity Ultimate JSON.
- Potential future `ExportComponent` if existing enable flags are insufficient.

The web designer continues to generate its current browser URL. Its JSON is
small enough for that to remain a convenient interactive workflow. API export
uses the same `Config` value through a POST body.

### Contract handling

- Built-in defaults pin known immutable version URLs.
- Nix and CLI users may override model targets.
- Workspace URLs are rejected by default for export; an explicit unsafe/developer
  override may be added if useful.
- Parameter IDs are stable contracts and are not rediscovered on every export,
  avoiding an API call. A separate inspect/check operation can verify a new model
  version.

## API export implementation

Implement only the narrow API surface required by gfty:

1. Encode the configuration map.
2. Start generic Part Studio translation with:

   ```json
   {
     "formatName": "STEP",
     "storeInDocument": false,
     "configuration": "<encodedId>",
     "grouping": true,
     "stepVersionString": "AP242"
   }
   ```

3. Poll translation state with bounded exponential backoff.
4. Download every external result.
5. Report `failureReason` and useful Onshape request context.

Use `grouping = true` to preserve separate named parts in one STEP. After
writing the temporary STEP, validate that label/plate `PRODUCT` and
`MANIFOLD_SOLID_BREP` names and counts exactly match the expected filament parts.
A generic `Part 1` or extra body is an actionable label/model error, likely from
artwork disconnected from the blank. Reject the export and do not install the
temporary file. Actual bin, appearance, and OrcaSlicer behavior must still be
smoke-tested.

Downloads must:

- Never write binary data to stdout.
- Write to a temporary sibling file and rename atomically.
- Refuse to overwrite unless `--force` is given.
- Remove incomplete temporary files on failure.
- Use the server filename only when no explicit/default local name exists.
- Print concise progress and final path/size information to stderr.

The initial implementation may call `encodeConfigurationMap` for correctness.
Later, its simple text-parameter encoding can be reproduced and tested locally
to save one API call per export. Annual API quotas make unnecessary discovery
and polling calls worth avoiding.

### FeatureScript diagnostics

The translation response exposes `requestState` and `failureReason`, but not
successful-regeneration warnings. `getPartStudioFeatures` exposes each feature's
`OK`, `INFO`, `WARNING`, or `ERROR` state, but its `configuration` is a query
parameter and therefore reintroduces the large-URL limit; the public response
schema also does not include the warning text. It is not a reliable diagnostic
channel for large labels.

Invalid output invariants should therefore be hard FeatureScript regeneration
errors where practical, not warnings. The CLI should additionally validate the
downloaded STEP part manifest. This gives useful protection even when Onshape's
translation response reports only `DONE`.

## Credentials

Prefer a protected credentials file. Do not accept raw secrets as command-line
arguments.

Suggested TOML format:

```toml
access-key = "..."
secret-key = "..."
```

Resolution order:

1. Explicit `--onshape-credentials FILE`.
2. `GFTY_ONSHAPE_CREDENTIALS_FILE` pointing to a file.
3. `$XDG_CONFIG_HOME/gfty/onshape.toml`, or
   `~/.config/gfty/onshape.toml` when XDG is unset.
4. Optional `GFTY_ONSHAPE_ACCESS_KEY` and `GFTY_ONSHAPE_SECRET_KEY` fallback for
   CI or ephemeral shells.

The credentials file should be a regular file readable only by its owner; warn
or fail on unsafe permissions on Unix. API requests should use Onshape request
signatures rather than Basic authentication in the maintained implementation.
Redirects must be followed deliberately and re-signed where required.

Never put credentials or credential file contents into:

- Git.
- Nix expressions or flake options.
- Derivation arguments.
- The Nix store.
- Process arguments.
- Diagnostic output.

Nix export apps rely on normal runtime credential discovery. They never pass a
secret or credential path captured during evaluation.

## Nix interface

Nix builds remain pure and deterministic. Remote STEP export is always a manual
runtime action.

### Pure outputs

Retain or introduce:

```text
packages.labels.<name>
packages.plates.<name>
packages.bins.<name>
```

These can contain canonical TOML and SVG previews. They must not perform network
access. Combined output/link farms may remain where useful.

### Export apps

Generate flat flake app names for reliable flake-schema behavior:

```sh
nix run .#export-label-screws
nix run .#export-plate-all-screws
nix run .#export-bin-small-parts
```

Each app references only non-secret store inputs and invokes the same generic
`gfty export FILE` path. It writes the STEP to the caller's current directory.
Extra CLI arguments can select a supported bin component, output path, or
overwrite behavior.

The final STEP is not a Nix package or fixed-output derivation. Even with an
immutable model version, Onshape translator updates and file metadata may make
STEP bytes non-reproducible.

Remove:

- Label/plate `onshapeUrl` passthru values.
- `perSystem.gfty-label.onshapeBaseUrl` once migration is complete.

Replace them with immutable model target options and export apps. Model URLs are
non-secret and safe in Nix.

### Named bins

Move duplicated per-label `gfty-ultimate` sets into named bin definitions:

```nix
perSystem.gfty = {
  bins.small-parts = {
    # Typed Gridfinity configuration.
  };

  labels.screws = {
    bin = "small-parts";
    template = ./templates/1x1.svg;
    text.top = "M3";
  };

  plates.all-screws = {
    bin = "small-parts";
    labels = [ "screws" "nuts" ];
  };
};
```

A transition can still accept an inline `gfty-ultimate` set, but references to a
named bin are the target design.

## Bin TOML

Bin TOML should represent the same semantic information as the web designer but
remain pleasant to author. Prefer a versioned, typed, hierarchical format over
copying every flat CAD key directly:

```toml
kind = "bin"
version = 1
size = [2, 2, 6]

[base]
enabled = true
magnets = true
rounded-corners = false

[bin]
enabled = true
nesting = true

[label]
enabled = true
depth = "10mm"
swappable = true
embossing-clearance = "0.4mm"

[divider]
columns = ["auto", "auto", "auto"]
rows = ["auto", "auto"]
```

Rust converts this to canonical Gridfinity Ultimate JSON. The Rust and web
implementations should share fixture pairs and conformance tests so defaults,
units, divider layout, easy-grab behavior, and generated JSON do not drift. Do
not attempt a large cross-language schema generator in the first implementation.

## Rename and compatibility

The target names are:

- Repository/package: `gfty`
- Binary: `gfty`
- Nix package/overlay: `pkgs.gfty`
- Flake-parts namespace: `perSystem.gfty`

The `gfty` binary and final command hierarchy were introduced after the label
API exporter worked. Temporary `gfty-label` binary/Nix aliases were retained
through the `gfty-label-library` migration and have now been removed. The hosted
repository, local directory, package, overlay, and module namespace all use the
target names above.

## Implementation sequence

### 1. Verify and prepare the label Onshape model

1. **Done:** inspect the pinned immutable version's configuration contract; both
   required parameters exist as strings with the expected IDs.
2. **Done:** build `gfty-label-library`'s `machine-torx-M3x2` label and `test`
   two-label plate with the current local gfty package.
3. **Done:** export both through the generic translation endpoint. The 6,223-byte
   label geometry produced a 771,162-byte STEP; the 9,738-byte plate geometry
   produced a 1,472,745-byte STEP.
4. **Done:** inspect both STEP files. Each contains exactly four named filament
   solids/products (`part-0` through `part-3`) and no generic/helper bodies.
5. No label component selector or upstream document change is required.
6. **Done:** validate downloaded STEP `PRODUCT` and `MANIFOLD_SOLID_BREP`
   names/counts against expected filaments before atomically installing output.
   Upstream warning plumbing is intentionally omitted.

### 2. Add minimal label STEP export

1. **Done:** add mode-checked credentials files and HMAC-signed API requests.
2. **Done:** add the four narrow encode/translate/poll/download operations with
   bounded polling, redirect re-signing, and useful API errors.
3. **Done:** generate label and plate geometry JSON in memory.
4. **Done for current kinds:** `gfty export FILE` dispatches legacy/versioned
   labels and versioned bins; entity commands share the same implementations.
   Saved label-plate TOML remains a separate future addition.
5. **Done for transition:** accept existing Gridfinity JSON/Nix-generated
   configuration; named bin TOML follows later.
6. **Done:** live-test signed CLI and Nix-app exports for both a label and plate
   against the pinned immutable production model.

### 3. Introduce `gfty` and the Nix export apps

1. **Done:** install `gfty`; temporary `gfty-label` command and Nix aliases were
   retained through migration and removed in phase 7.
2. **Done:** add `export-label-<name>` and `export-plate-<name>` runtime apps.
3. **Done:** remove `onshapeUrl` and `onshapeBaseUrl`; add the immutable
   `labelModelUrl` option.
4. **Done:** keep pure label/plate previews and definitions unchanged during the
   transition.
5. **Done:** discover credentials only after `nix run` starts; no secret or
   credentials path is captured by Nix.

### 4. Reorganize and simplify label commands

1. **Done:** move authoring commands below `gfty label`, while retaining generic
   `gfty export LABEL` dispatch.
2. **Done:** rename `quick` to `create` and support direct `--export`.
3. **Done:** move plate creation/export below `gfty label plate`.
4. **Done:** remove public geometry JSON flags, commands, and package artifacts.
5. **Done:** remove public `build`; pure Nix builders use render/create commands.
6. **Done:** update package examples and migrate `gfty-label-library` after the
   new interfaces were published.

### 5. Verify the Gridfinity Ultimate model

1. **Done:** inspect the designer's pinned immutable version; `Config` is a
   string parameter with ID `Config`.
2. **Done:** export the representative 1x1 Gridfinity configuration used by the
   label library. A 1,007-byte config produced an 843,064-byte grouped AP242
   STEP from the immutable version.
3. **Done:** inspect five named products/solids: `Bin`, `SwappableRim`,
   `SwappableLabel`, `Base`, and `ConnectorPin`, with no generic parts.
4. Existing enable flags are sufficient for the normal complete export. Exact
   standalone component semantics can be added with bin TOML; no new Onshape
   parameter is currently required.
5. The web designer continues to use the unchanged `Config` browser URL and
   pinned version.
6. No upstream model change or new version is required.

### 6. Add bins

1. **Done:** define required `kind = "bin"`, version 1 hierarchical TOML and
   typed Rust structures.
2. **Done:** port designer defaults, unit handling, track sizing, merge and
   easy-grab face validation, automatic supports, and canonical JSON conversion.
   A frozen designer-default fixture checks the complete resulting JSON value.
3. **Done:** add `gfty bin validate`, `inspect`, and `export`, plus generic
   `gfty export BIN` dispatch and exact expected STEP manifest validation.
4. **Done:** add typed Nix `bins`, `packages.bins.<name>`, `packages.bins.all`,
   `pkgs.gfty.mkBin`, `binModelUrl`, and `export-bin-<name>` runtime apps.
5. **Done:** labels may reference bin TOML and Nix labels/plates may reference a
   named bin. The legacy inline `gfty-ultimate` set remains an exclusive
   transition alternative.
6. **Partly done:** `--component bin` works through existing enable flags and was
   live-tested. Base-only export still produces an unexpected generic `Part N`
   even after dependent flags are disabled; do not expose it until the model has
   an `ExportComponent` contract or a separate base model.

### 7. Complete migration

1. **Done:** Cargo/Nix package names, development outputs, environment variables,
   examples, flake-parts namespace, local directory, GitHub repository, and
   local remote are now `gfty`.
2. **Done:** migrate `gfty-label-library` to `inputs.gfty`, `perSystem.gfty`, and
   a shared named bin; its lock now references the renamed GitHub repository.
3. **Done:** remove the deprecated `gfty-label` command wrapper and
   `pkgs.gfty-label` overlay alias after the consumer migration.
4. **Done:** update primary documentation and examples to use `gfty`; retain only
   historical references where they explain the migration.

### 8. Integrate Gridfinity Ultimate

1. **Done:** merge the complete nine-commit `gridfinity-ultimate` history as a
   second parent. Its files were imported under a temporary prefix before the
   monorepo cleanup moved them to their final domain directories.
2. **Done:** expose the static browser designer as root `packages.designer` and
   add `designer-dev`/`designer-preview` to the root development shell.
3. **Done:** move the Pages workflow to the root and use `designer-v*` tags so
   designer releases do not collide with CLI releases.
4. **Done:** remove the redundant nested flake/devshell/lock while retaining the
   designer derivation and domain-specific guide.
5. **Done:** run the browser's default serializer against the same JSON fixture
   used by Rust during `nix flake check`.
6. **Done:** consolidate the browser app under `designer/` and every maintained
   FeatureScript under `featurescripts/`, then remove the temporary import prefix.
7. Keep the old standalone checkout only as a local backup until the cleaned
   layout and deployment have been pushed and verified.

## Out of scope for the first implementation

- OAuth2 UI and token refresh.
- A general-purpose Onshape API client.
- Automatic workspace mutation or blob uploads.
- Network access during Nix builds or checks.
- Claiming byte-for-byte reproducibility for downloaded STEP files.
- Dynamically querying configured part IDs through oversized URLs.
