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

The checked-in Onshape API documentation indicates that a complete automatic
export should be feasible without modifying the model workspace.

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
   body of an asynchronous Part Studio export/translation.
4. Poll `Translation/getTranslation` with exponential backoff until its state is
   `DONE` or `FAILED`.
5. With `storeInDocument=false`, download each `resultExternalDataId` through
   `Document/downloadExternalData`. If an export is stored in the document,
   download its `resultElementId` with `BlobElement/downloadFileWorkspace`.

The important distinction is that the potentially large encoded configuration
is carried in POST bodies, not in a browser URL or synchronous export query
string. The configured model can be evaluated from an immutable Onshape version,
so this path should avoid workspace history, races, and cleanup.

The docs explicitly demonstrate:

- Configuration discovery and `encodeConfigurationMap`.
- `encodedId` in asynchronous export request bodies.
- Asynchronous Part Studio exports/translations.
- Polling and external-data/blob downloads.
- Configured asynchronous assembly export.

The local snapshot does not show the complete request schema for every Part
Studio translator. Before implementing this path, use the live API Explorer to
confirm which of these accepts `configuration`, desired part selection, and the
required format options:

- `PartStudio/createPartStudioExportStep`
- `PartStudio/createPartStudioTranslation`
- Any format-specific asynchronous STL/3MF endpoint currently available

`Translation/getAllTranslatorFormats` should be used to discover actual format
support. STEP is a promising first target because it can preserve multiple named
parts; the exact behavior in OrcaSlicer must be tested. STL may produce one file
per part or a ZIP depending on export options.

## Suggested CLI shape

API operations must be runtime commands, not Nix derivation build steps. Nix
builds must remain pure and credentials must never enter the Nix store.

A reasonable incremental interface is:

```text
gfty-label onshape inspect TARGET_URL
gfty-label onshape export LABEL \
  --gfty-config gfty-ultimate.json \
  --target TARGET_URL \
  --format step \
  --output label.step

gfty-label onshape export-plate plate.json \
  --gfty-config gfty-ultimate.json \
  --target TARGET_URL \
  --format step \
  --output plate.step
```

The Nix module could expose non-secret passthru metadata such as the target
version URL, generated geometry path, Gridfinity JSON path, and a ready-to-run
command. Actual API requests should happen only when the user invokes the CLI.

A first prototype should:

1. Parse and validate document/version-or-workspace/element IDs from the target
   URL.
2. Call `getConfiguration` and verify the `Config` and
   `GFTYUltimateConfig` parameter IDs and string-compatible types.
3. Encode the configuration with a POST body.
4. Start one asynchronous STEP export with `storeInDocument=false`.
5. Poll conservatively and download atomically to a requested path.
6. Report Onshape `failureReason`, feature regeneration failures, response
   request IDs, and rate-limit information clearly.

After this succeeds, add format discovery, STL/3MF options, part filtering,
retries, and Nix-generated command metadata.

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
