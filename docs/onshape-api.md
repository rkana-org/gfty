# Onshape API feasibility

## Problem

The flake module can currently produce a browser URL whose `configuration`
query parameter contains both:

- `Config`: generated schema-version-2 label/plate geometry JSON.
- `GFTYUltimateConfig`: Gridfinity Ultimate configuration JSON.

This works for small labels, but Onshape or an upstream web server rejects URLs
around 5-6 KB with HTTP 414. A two-label plate already exceeds that size.
Percent encoding also expands JSON significantly, so browser URLs are not a
viable transport for general plates.

## Recommended approach: configured API export

An ad-hoc API test has confirmed that a complete automatic export works without
modifying the model workspace.

1. Authenticate at runtime with a personal API key or OAuth2.
2. POST the two configuration values to
   `Element/encodeConfigurationMap`:

   ```json
   {
     "parameters": [
       { "parameterId": "Config", "parameterValue": "<label-or-plate JSON>" },
       { "parameterId": "GFTYUltimateConfig", "parameterValue": "<Gridfinity JSON>" }
     ]
   }
   ```

3. Use the returned `encodedId` as the `configuration` field in the JSON request
   body of `PartStudio/createPartStudioTranslation`. For STEP, the essential
   fields are:

   ```json
   {
     "formatName": "STEP",
     "storeInDocument": false,
     "configuration": "<encodedId>",
     "grouping": true,
     "stepVersionString": "AP242"
   }
   ```

4. Poll `Translation/getTranslation` with exponential backoff until its state is
   `DONE` or `FAILED`.
5. With `storeInDocument=false`, download each `resultExternalDataId` through
   `Document/downloadExternalData`. If an export is stored in the document,
   download its `resultElementId` with `BlobElement/downloadFileWorkspace`.

The important distinction is that the potentially large encoded configuration
is carried in POST bodies, not in a browser URL or synchronous export query
string. The configured model can be evaluated from an immutable Onshape version,
so this path should avoid workspace history, races, and cleanup.

### Validated proof of concept

The method was tested against an immutable Part Studio version with a text
configuration variable consumed by FeatureScript:

- Raw configuration JSON: 65,595 bytes, including 65,536 checked padding
  characters and an end sentinel.
- Encoded configuration: 65,637 characters.
- `encodeConfigurationMap`: HTTP 200.
- Generic asynchronous Part Studio translation to STEP: `DONE` with no failure.
- `downloadExternalData`: a 14,839-byte AP242 STEP file.
- The STEP contained two `MANIFOLD_SOLID_BREP` and two `PRODUCT` records, named
  `api-poc-A-65536` and `api-poc-B-65536` as generated from the large payload.
- The translation response referenced the requested version and no workspace,
  proving that no mutable workspace was needed.

The production label model was then tested with `gfty-label-library` inputs. A
6,223-byte single-label geometry request downloaded a 771,162-byte STEP, and a
9,738-byte two-label plate request downloaded a 1,472,745-byte STEP. Each file
contained exactly four `PRODUCT` and `MANIFOLD_SOLID_BREP` records named
`part-0`, `part-1`, `part-2`, and `part-3`, with no generic/helper parts. The
existing `Config` and `GFTYUltimateConfig` string parameters require no upstream
change for complete label/plate exports.

The Gridfinity Ultimate designer model was also verified without modification.
Its immutable version exposes one string parameter with ID `Config`. A
representative 1x1 configuration generated from typed bin TOML downloaded an
843,064-byte grouped STEP containing exactly `Bin`, `SwappableRim`,
`SwappableLabel`, `Base`, and `ConnectorPin`, with no generic parts or workspace
mutation. Suppressing the base cleanly produced a 717,723-byte bin-only STEP with
the first three names.

A base-only configuration is not clean in the current model: suppressing the bin
still leaves a generic `Part 2` or `Part 3` alongside `Base` and the optional
`ConnectorPin`. The downloader correctly rejects this manifest. Base component
selection therefore remains disabled until the model gains an explicit export
component contract or a separate base model is pinned.

The authenticated live OpenAPI document is available from `/api/openapi`. It
shows an important endpoint distinction:

- `PartStudio/createPartStudioTranslation` uses `BTTranslateFormatParams`, which
  has a request-body `configuration` field plus `formatName`, `partIds`,
  `grouping`, and format options. This is the endpoint validated above.
- The format-specific `PartStudio/createPartStudioExportStep` request uses
  `BTBStepExportParams`, which currently has no `configuration` field. Do not use
  that endpoint for this workflow unless its schema changes.

`Translation/getAllTranslatorFormats` should be used to discover actual format
support. STEP preserves multiple named parts in the validated output; the exact
behavior in OrcaSlicer must still be tested. STL may produce one file per part
or a ZIP depending on export options.

### Feature warnings

The translation response exposes `requestState` and `failureReason`, but not
warnings from successful FeatureScript regeneration. `getPartStudioFeatures`
returns an `OK`/`INFO`/`WARNING`/`ERROR` state per feature, but takes
`configuration` in the URL query and the public schema does not include warning
text. It is therefore unsuitable for diagnostics with large label
configurations.

Output invariants should be hard FeatureScript regeneration errors where
possible. The downloader should also verify that a label/plate STEP contains
exactly the expected filament part names and count; an unexpected `Part 1` is
then reported as likely disconnected/out-of-bounds artwork.

## Implemented CLI and Nix interface

API operations are runtime commands, not Nix derivation build steps. Nix builds
remain pure and credentials never enter the Nix store.

The generic interface dispatches by the TOML `kind` field:

```text
gfty export labels/screws.toml --output screws.step
gfty export bins/small-parts.toml --output small-parts.step
gfty export bins/small-parts.toml --component bin
```

The equivalent entity-oriented commands are `gfty label export` and
`gfty label plate export`; `gfty label create --export` handles an unsaved label.
They generate geometry in memory, sign requests, poll with bounded exponential
backoff, download atomically, and validate expected STEP product/body names.

The Nix module exposes manual apps such as `export-label-screws` and
`export-plate-all`, and `export-bin-small-parts`. Their scripts capture only
immutable model/configuration and
font inputs; normal runtime credential discovery happens after `nix run` starts.
Model parameter contracts were verified separately and are pinned by immutable
version, avoiding a redundant `getConfiguration` call on each export.

## Alternative upload approaches

### Update feature or Variable parameters

`getPartStudioFeatures` plus `updatePartStudioFeature` can clone an existing
feature's internal API representation and replace a string parameter with large
JSON. This avoids URL limits, but it mutates a workspace and creates a
microversion. It is subject to races and leaves the shared model in the last
exported state. The docs warn that internal feature parameter representations
may change; always start from a GET response rather than constructing the whole
feature from scratch.

This is a viable fallback if configured Part Studio translations do not accept a
large configuration in their request bodies. Prefer a dedicated workspace or a
temporary document rather than mutating a shared workspace.

### Upload a JSON blob

`BlobElement/uploadFileCreateElement` and `uploadFileUpdateElement` can store
arbitrary JSON files. The FeatureScript importer could be changed to accept a
`JSONData` reference instead of a string variable. Updating a stable blob
reference would carry large data reliably, but still mutates a workspace and
creates microversions. `GFTYUltimateConfig` would need the same treatment or a
separate feature update.

### Application-element structured storage

Application elements provide versioned JSON trees and transactions, but ordinary
FeatureScript cannot directly consume that application-owned storage. It would
require a larger Onshape app architecture and does not simplify the current
importer.

### FeatureScript evaluation

`PartStudio/evalFeatureScript` only evaluates lambda expressions against a
context. It is useful for queries and validation, but does not persist feature
geometry or act as an upload channel.

### Compression

General compression is not an attractive primary solution. URL percent encoding
expands data, FeatureScript has no convenient gzip decoder, and a custom compact
codec would add complexity while still retaining an uncertain URL ceiling.
Geometry-specific delta/quantized encoding might reduce payloads, but not with
enough margin to make arbitrary plates safe.

## Authentication, redirects, and quotas

- Personal/internal automation may use API keys; App Store distribution requires
  OAuth2.
- Basic API-key auth is documented for local testing. Signed requests are safer
  for internal use.
- Never store credentials in Git, Nix expressions, derivation arguments, or Nix
  outputs. Use runtime environment variables or protected credential files.
- Synchronous downloads commonly return HTTP 307. Redirect requests must be
  authenticated for the redirected URL as well.
- Handle HTTP 429 and honor rate-limit headers.
- Successful private API calls count against annual quotas. The checked-in docs
  list 2,500 calls/year for free and standard users. An asynchronous export will
  usually consume several calls, especially while polling.

## Relevant local documentation

- `../onshape-web-api/auth/apikeys.md`
- `../onshape-web-api/auth/limits.md`
- `../onshape-web-api/api-adv/configs.md`
- `../onshape-web-api/api-adv/featureaccess.md`
- `../onshape-web-api/api-adv/fs.md`
- `../onshape-web-api/api-adv/translation.md`
- `../onshape-web-api/app-dev/structuredstorage.md`
