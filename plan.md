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
label definition may reference a bin definition. The default export should
produce the complete printable object represented by the TOML.

Users may also need a subset such as only the label parts, only the bin, or only
the base. The intended interface is a semantic component selector rather than a
separate file kind for every subset:

```sh
gfty export labels/screws.toml --component all
gfty export labels/screws.toml --component label
gfty export labels/screws.toml --component bin
gfty export bins/small-parts.toml --component base
```

The supported component values must be defined per TOML kind and validated
before making an API request.

Configured Onshape part IDs are configuration-dependent, and obtaining them
through a large configuration query would recreate the URL-size problem.
Therefore `partIds` should not be the primary design. If the production models
cannot already suppress unwanted components through existing enable flags, add
a small configuration parameter such as `ExportComponent` and make the model
produce only the requested bodies. That parameter can be sent beside the large
JSON values in the same API request. We will decide the exact model change only
after inspecting a real label STEP.

## JSON policy

JSON remains an internal wire format because the current FeatureScripts consume
it, but users should not need to manage geometry JSON.

Remove these public outputs after API export is working:

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
- Potential future `ExportComponent`: small enum/string selecting all, label,
  bin, or base output.

The currently pinned label model has already been inspected through the API:

- `Config` is `BTMConfigurationParameterString-872` with parameter ID `Config`.
- `GFTYUltimateConfig` is `BTMConfigurationParameterString-872` with parameter ID
  `GFTYUltimateConfig`.

It is already structurally compatible with POST-body configuration. No upstream
change should be made until a real generated label is exported and its parts are
inspected.

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

Use `grouping = true` to preserve separate named parts in one STEP. Actual label,
bin, appearance, and OrcaSlicer behavior must be smoke-tested.

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
4. Optional `ONSHAPE_ACCESS_KEY` and `ONSHAPE_SECRET_KEY` compatibility fallback
   for CI or ephemeral shells.

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
Extra CLI arguments can select a component, output path, or overwrite behavior.

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

Introduce the `gfty` binary and final command hierarchy after the real label API
export works but before publishing export apps. Retain `gfty-label` and the old
Nix names briefly as compatibility aliases while `gfty-label-library` migrates.
Rename the repository directory after the interfaces have moved, avoiding a
large path-only change during API debugging.

## Implementation sequence

### 1. Verify and prepare the label Onshape model

1. Inspect the pinned immutable version's configuration contract. **Done:** both
   required parameters already exist as strings with the expected IDs.
2. Build a representative real label and a multi-label plate with current Nix
   inputs.
3. Export both through the validated generic translation endpoint.
4. Inspect the STEP body count, names, unwanted helper/bin parts, and configured
   geometry.
5. Decide whether `ExportComponent` or another model-side selector is needed.
6. Only then edit the source workspace, compile/smoke-test FeatureScript, create
   a new immutable version, and update the pinned target.

### 2. Add minimal label STEP export

1. Add credentials-file loading and signed API requests.
2. Add the four narrow encode/translate/poll/download operations.
3. Generate label geometry JSON in memory.
4. Add generic `gfty export FILE` dispatch for labels and
   `gfty label export FILE` as the entity-oriented spelling.
5. Initially accept the existing Gridfinity JSON/Nix-generated configuration;
   named bin TOML follows later.
6. Test actual label and plate downloads against the pinned model.

### 3. Introduce `gfty` and the Nix export apps

1. Install `gfty`; retain a temporary `gfty-label` compatibility executable.
2. Add manual per-definition export apps.
3. Remove `onshapeUrl` and `onshapeBaseUrl`.
4. Keep pure label/plate previews and definitions.
5. Ensure credentials are discovered only after `nix run` starts.

### 4. Reorganize and simplify label commands

1. Move commands below `gfty label`.
2. Rename `quick` to `create` and add its optional direct export action.
3. Move plate behavior below `gfty label plate`.
4. Remove public geometry JSON flags, commands, and package artifacts.
5. Internalize or remove `build`.
6. Update the example and consumer flakes.

### 5. Verify the Gridfinity Ultimate model

1. Inspect its `Config` contract.
2. Export a representative immutable configured bin to grouped STEP.
3. Check base/bin/rim/helper parts and names.
4. Determine component-selection behavior.
5. Preserve and test the web designer's existing Open in Onshape URL.
6. Create a new immutable version only if the model contract changes.

### 6. Add bins

1. Define versioned bin TOML and typed Rust structures.
2. Port designer defaults, unit handling, divider validation, easy-grab logic,
   and canonical JSON conversion with fixtures.
3. Add `gfty bin validate`, `inspect`, and `export`.
4. Add typed Nix `bins` and per-bin export apps.
5. Let labels and plates reference named bins.
6. Add explicit component export once model support is validated.

### 7. Complete migration

1. Rename package, overlay, flake module namespace, and repository.
2. Migrate `gfty-label-library`.
3. Remove compatibility aliases after consumers have moved.
4. Update all documentation and examples.

## Out of scope for the first implementation

- OAuth2 UI and token refresh.
- A general-purpose Onshape API client.
- Automatic workspace mutation or blob uploads.
- Network access during Nix builds or checks.
- Claiming byte-for-byte reproducibility for downloaded STEP files.
- Dynamically querying configured part IDs through oversized URLs.
