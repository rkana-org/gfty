# Architecture

`gfty` keeps authored TOML and Nix definitions local and reproducible while
using immutable Onshape Part Studios as geometry engines. Credentials and remote
artifacts stay outside Nix derivations and the Nix store.

## Configuration model

Every TOML file has an explicit `kind` and version. Only current schemas are
accepted:

| kind | version | result |
| --- | ---: | --- |
| `bin` | 2 | exact `Bin` body |
| `base` | 1 | exact `Base` body |
| `rim` | 1 | exact `SwappableRim` body |
| `swappable-label` | 1 | exact `SwappableLabel` body |
| `bin-set` | 1 | declared compatible Gridfinity bodies |
| `label` | 1 | artwork label tied to a bin prototype |

The standard `ConnectorPin` has no authored geometry options. It is exported by
a dedicated command or included in a bin set.

A bin owns body geometry and mating interfaces: size, tub, dividers, merges,
easy grabs, rim mode, label mode/depth/supports, and print overhang. Bases, rims,
and separate label blanks own their independent manufacturing options. A bin set
references those files and validates dimensions and interfaces without copying
settings.

The public schemas are translated into a compact internal Gridfinity Ultimate
`Config`. Internal carrier configurations may generate other helper bodies, but
configured-part discovery and translation filtering ensure constituent outputs
contain only their exact named part.

## Swappable-label normalization

A swappable-label config references a bin rather than exposing model-specific
slot positions. Rust derives its identity from:

- X size;
- label depth in micrometers;
- effective divider walls in row zero, represented as normalized
  parts-per-billion boundaries;
- embossing clearance and inset for geometry identity.

Track lengths are resolved using the same rules as bin validation. Divider
merges are interpreted as connected components. An interior boundary exists
only where adjacent row-zero cells remain in different components.

The normalized boundaries are converted back into a deterministic one-row
carrier: boundary deltas are divided by their greatest common divisor and
serialized as fractional tracks. Consequently four equal columns merged as two
pairs normalize to the same `["1fr", "1fr"]` carrier as two equal columns.

Identity deliberately excludes source paths, names, Y/Z size, later rows, easy
grabs, base/rim settings, and any merge representation that leaves the same
row-zero walls. Equivalent definitions therefore converge on the same runtime
request and cache key.

## Geometry pipelines

### Gridfinity constituents

1. Parse and validate the current TOML schema.
2. Construct the unified model configuration or normalized carrier.
3. Discover configured parts by name and resolve their configuration-dependent
   `partId` values.
4. Submit only the required IDs for STEP translation.
5. Validate exact STEP `PRODUCT` and solid-body names before installation.
6. Optionally request a configured shaded view using the same configuration and
   selected part.

Configured part IDs are never persisted or hard-coded.

### Artwork labels and plates

1. Resolve the SVG template, icons, sidecars, and explicitly available fonts.
2. Normalize SVG primitives, transforms, text, and paint through `usvg`, then
   expand resolved strokes into closed fill contours with `tiny-skia-path`.
   Numerically collinear centerline vertices and tiny stroke-join cancellation
   loops are removed so raster-oriented outlines remain valid CAD regions.
3. Compose local fill and expanded-stroke geometry by filament and serialize
   compact version-2 `M`/`L`/`C`/`Z` path JSON in memory.
4. Send label geometry as `Config` and the referenced bin carrier as
   `GFTYUltimateConfig` to the immutable label model.
5. Download and validate exactly the expected `part-<filament>` bodies.

For plates, labels are placed row-major without rotation. Remote plate exports
also verify that every label's referenced bin has the same X/Y size as the
plate's shared prototype bin.

## Onshape transport

Remote exports use signed runtime API requests:

1. POST configuration values to
   `Element/encodeConfigurationMap`.
2. POST the encoded configuration to
   `PartStudio/createPartStudioTranslation` with `formatName = "STEP"`,
   `storeInDocument = false`, and selected `partIds` when applicable.
3. Poll the translation with bounded exponential backoff.
4. Download the external data result.
5. Validate and atomically install the bytes.

The format-specific synchronous STEP endpoint is not used because its request
contract does not carry arbitrary configured values. Workspace-mutating feature
updates and blob uploads are intentionally unsupported.

Configured shaded views are GET-only, so compact Gridfinity configurations can
produce PNG previews. Artwork geometry commonly exceeds reliable URL limits and
therefore has no remote PNG-preview path.

Credentials are discovered only at runtime from protected files or `GFTY_*`
environment variables. Requests, logs, Nix values, and cache manifests never
contain credentials.

## Runtime cache

Normalized Gridfinity exports use:

```text
$XDG_CACHE_HOME/gfty/onshape/<request-key>/
```

`GFTY_CACHE_DIR` may override the cache root. The key includes the normalized
request, immutable model target, selected parts, and export contract. It excludes
source names and paths.

Each entry records content hashes and the exact expected STEP manifest. Cached
STEP and PNG bytes are verified before atomic installation. `--no-cache`
bypasses the cache.

## Nix boundary

The flake-parts module provides typed definitions under:

```text
perSystem.gfty.bins
perSystem.gfty.bases
perSystem.gfty.rims
perSystem.gfty.swappableLabels
perSystem.gfty.binSets
perSystem.gfty.labels
perSystem.gfty.plates
```

Pure derivations contain validated local definitions and rendered SVG previews;
they do not contact Onshape. Generated `export-*` apps execute the same Rust
runtime exporter after `nix run` starts, keeping credentials, STEP files, and
PNGs outside the sandbox and store.

Rust is the validation and normalization authority. Runtime request keys provide
semantic deduplication even when separately named Nix definitions occupy
different store paths.

## Models, FeatureScripts, and designer

The label and Gridfinity model URLs pin immutable Onshape versions. The browser
designer pins the matching Gridfinity version in `designer/JsonPanel.jsx` and
preserves its client-side **Open in Onshape** workflow.

`featurescripts/labels/gfty_label_instances.fs` consumes current version-2 label
geometry. Configuration and divider FeatureScripts live under
`featurescripts/`. FeatureScript has no local compiler, so source changes require
an Onshape compile and smoke test before repinning an immutable production model.

The designer is deployed by `.github/workflows/designer-pages.yml` from tags of
the form `designer-vN`. The tag suffix must match the designer's pinned
`ONSHAPE_VERSION`.
