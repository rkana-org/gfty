# gfty plan

## Current architecture

`gfty` accepts only explicit, versioned configuration files:

- `kind = "bin"`, version 2: one bin body and its mating interfaces.
- `kind = "base"`, version 1: one base.
- `kind = "rim"`, version 1: one swappable rim.
- `kind = "swappable-label"`, version 1: one blank derived from a bin's
  normalized row-zero interface.
- `kind = "bin-set"`, version 1: a validated composition of constituents.
- `kind = "label"`, version 1: SVG artwork tied to a bin prototype.

There is no schema inference, complete-bin version-1 input, inline raw
Gridfinity JSON, component selector, or deprecated command/package alias.
Each constituent exports its exact configured Onshape part; a bin set is the
only grouped Gridfinity export.

TOML and Nix definitions remain the source of truth. Gridfinity model JSON and
label geometry JSON are internal transport formats only.

## Remote export contract

- Immutable Onshape versions only.
- Runtime credentials from protected files or explicit environment variables.
- HMAC-signed requests with `storeInDocument = false`.
- Configuration encoding, configured-part discovery, exact `partIds`
  translation, bounded polling, and external-data download.
- Exact STEP product/solid manifest validation before atomic installation.
- Optional configured shaded views for compact Gridfinity requests.
- No credentials, STEP files, or PNG files in Nix derivations or the store.

Normalized constituent requests are cached below
`$XDG_CACHE_HOME/gfty/onshape` or `$GFTY_CACHE_DIR/onshape`. Cache entries are
validated by request key, content hash, and exact STEP manifest.

## Nix interface

The flake-parts module exposes typed definitions under:

```text
perSystem.gfty.bins
perSystem.gfty.bases
perSystem.gfty.rims
perSystem.gfty.swappableLabels
perSystem.gfty.binSets
perSystem.gfty.labels
perSystem.gfty.plates
```

The default package exposes matching `mk*` builders. Runtime `export-*` apps
build pure local definitions through Nix, then perform authenticated downloads
outside the sandbox.

## Remaining work

1. Implement the Rust-equivalent row-zero normalizer in pure Nix so equivalent
   swappable-label definitions alias the same store derivation before runtime.
2. Add shared Rust/Nix conformance fixtures for normalized geometry and
   compatibility keys.
3. Continue live dependency tests for model fields whose geometry boundaries are
   not yet fully isolated, especially print overhang, supports, spring
   compensation, and embossing options.
4. Smoke-test any FeatureScript changes in Onshape; there is no local
   FeatureScript compiler.
5. Publish `gfty`, update `gfty-library`'s lock to the published revision, and
   verify the Pages deployment.

## Deliberate limits

- Artwork-label PNG previews remain unsuitable for the GET-only configured
  shaded-view endpoint because geometry configurations can exceed reliable URL
  limits.
- Configured Onshape part IDs are discovered at runtime and are never persisted.
- Workspace-mutation export fallbacks are not supported.
