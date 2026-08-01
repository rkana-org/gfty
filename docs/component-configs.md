# Constituent Gridfinity configuration

This document specifies the current Gridfinity configuration model. The Rust
schemas, exact named-part STEP/PNG exports, compatible sets, normalized
swappable-label carriers, Nix constituent packages/apps, and runtime cache are
implemented. Retired complete-bin schemas are rejected. Automatic aliasing of
independently named equivalent Nix request derivations is still pending the
pure-Nix normalization described below.

## Goals

- Author bases, rims, bin bodies, and swappable label blanks independently.
- Export each named Onshape part as its own STEP and PNG.
- Compose compatible constituents into a complete set.
- Give geometrically identical constituents the same stable semantic key.
- Keep authenticated Onshape downloads outside Nix derivations and the Nix
  store.

The existing artwork `kind = "label"` remains distinct from the blank
`SwappableLabel` part described here.

## Constituent files

### Base

A base has only X/Y size and base geometry options. It has no `enabled` or
connector-pin setting.

```toml
kind = "base"
version = 1
size = [2, 2]
rounded-corners = false

[magnets]
enabled = true
connector-cutouts = true
```

The standard connector pin is a configuration-free artifact. It is exported by
a dedicated command or included by a set; it is not part of the base key.

### Swappable rim

A rim has only its X/Y envelope and the options that alter the separate rim
body.

```toml
kind = "rim"
version = 1
size = [2, 2]
spring-compensation = true
additional-expansion = "0mm"
```

Its geometry key contains all four values. Its nominal compatibility key can be
only X/Y size, unless model testing finds another socket-interface parameter.

### Bin body

A bin body excludes base and separate-part manufacturing options. It retains the
settings that alter the `Bin` body, including whether it contains integrated or
swappable rim/label interfaces.

```toml
kind = "bin"
version = 2
size = [2, 2, 6]
tub = true
max-print-overhang = 60

[rim-interface]
mode = "swappable" # off | integrated | swappable

[label-interface]
mode = "swappable" # off | integrated | swappable
depth = "10mm"
supports = "auto" # always | auto | off

[divider]
columns = ["auto", "auto", "auto"]
rows = ["auto", "auto"]
merges = []

[easy-grab]
mode = "all"
side = "south"
radius = "21mm"
```

The divider, easy-grab, support, and interface settings remain here because they
alter the main `Bin` body. Rim compensation and label embossing settings do not.

### Swappable label blank

A user cannot reasonably author normalized slot positions, and Gridfinity
Ultimate still needs a divider configuration to construct the body. A
swappable-label source therefore references the bin whose label interface it
must fit:

```toml
kind = "swappable-label"
version = 1
bin = "../bins/small-parts.toml"

[embossing]
clearance = "0.4mm"
inset = "0mm"
```

X size, label depth, and the effective divider walls in row zero are derived
from that bin. Embossing clearance and inset alter the manufactured blank and
therefore its geometry key, but not compatibility.

The referenced bin is provenance, not identity. Two label files may reference
very different bins and still normalize to the same interface and geometry
keys. The user never writes slot ratios, and slot ratios are never sent to
Onshape as a replacement for divider configuration.

### Complete set

A set references constituents and contains no duplicated geometry settings.
Omitted optional files are not exported.

```toml
kind = "bin-set"
version = 1
bin = "./bins/small-parts.toml"
base = "./bases/2x2-magnetic.toml"
rim = "./rims/2x2-standard.toml"
swappable-label = "./swappable-labels/2x-label.toml"
connector-pin = true
```

Set validation requires:

- Base and rim X/Y size equal bin X/Y size.
- A referenced rim requires `rim-interface.mode = "swappable"`.
- The referenced label's normalized X size, depth, and effective row-zero walls
  equal the interface derived from the set's bin. The label may have been
  authored against a different compatible bin.
- A referenced label requires `label-interface.mode = "swappable"`.
- A connector pin may require an applicable magnetic/cutout base policy, though
  the pin geometry itself has no configuration.

A set may produce a grouped STEP, individual artifacts, or both. Individual
artifacts must always use their constituent's canonical carrier configuration,
not the complete set configuration, so the same constituent has the same remote
request in every set.

## Normalizing the label interface

Let `W = size_x * 42mm`. Resolve column tracks exactly as the current designer
and Rust validator do. Let `w[i]` be each resolved column width.

Use the same union-find interpretation of divider merges as the bin model. For
each interior boundary `i` between columns `i - 1` and `i` in label-adjacent row
zero:

```text
if compartment(i - 1, 0) != compartment(i, 0):
    boundary = sum(w[0..i]) / W
```

Boundaries are an internal normalization detail, not a public TOML field. Reduce
them to exact rational or fixed-precision integer values, sort them, and remove
duplicates. Three equal columns normalize to boundaries at `1/3` and `2/3`; a
20mm first column in a two-unit-wide bin normalizes to `5/21`.

To create the actual Onshape request, turn the effective row-zero partition back
into a canonical minimal divider configuration:

1. Coalesce adjacent original columns where row zero has no wall.
2. Use the resulting segment widths as canonical column tracks.
3. Use one `auto` row.
4. Use no merges or easy-grab entries.

For example, four equal columns where the first two are merged in row zero
become canonical column weights `[2, 1, 1]`. Both the original and canonical
divider configurations place label slots at the same positions, while the
canonical form is independent of irrelevant Y size, later rows, easy grabs, and
merges that do not alter row zero. Gridfinity Ultimate receives this synthesized
`bin_tub_divider_config`; it never receives boundary ratios as a new model
parameter.

Use exact decimal rational arithmetic over the values actually serialized to
Gridfinity Ultimate: fractional tracks are canonicalized to 9 decimal places,
fixed lengths to 6 decimal places, and physical label lengths to micrometers.
This avoids floating-point spelling differences changing keys. The canonical
column serialization needs live equivalence tests against representative source
dividers before the contract is frozen.

Two internal keys are useful:

```text
label-interface/v1 = { size-x, depth-um, effective-row-zero-boundaries }
label-geometry/v1  = { interface-key, emboss-clearance-um, emboss-inset-um,
                       any other empirically proven label-body option }
```

Retention supports belong to the bin body: they alter the wall adjacent to the
label, not the separate blank. Before freezing version 1, vary every candidate
model field and compare selected-part geometry to verify the dependency table.

## Onshape resolution

Each constituent resolves to an internal export request containing:

- Immutable model document/version/element IDs.
- A deterministic minimal Gridfinity Ultimate carrier `Config`.
- Expected configured part name (`Base`, `Bin`, `SwappableRim`, or
  `SwappableLabel`).
- Exact expected STEP manifest.
- Semantic compatibility and geometry keys.

The connector pin uses one fixed internal carrier configuration.

At runtime the downloader:

1. Sends the compact carrier configuration to configured-parts discovery.
2. Resolves the expected name to a configuration-dependent `partId`.
3. Fails if the expected part is absent or ambiguous.
4. Supplies that ID through translation `partIds` for an isolated STEP.
5. Uses the same ID and configuration for the isolated shaded-view PNG.
6. Validates the exact one-part STEP manifest before atomic installation.

Carrier configurations need live conformance tests proving that a constituent
selected from its minimal carrier is geometrically equal to the same constituent
selected from a compatible complete set.

## Nix identity and deduplication

Nix derivations should build pure **export request packages**, not contact
Onshape. A request package can contain:

```text
component.toml
request.json
request-key
```

The runtime normalizer hashes canonical fixed-precision data and excludes
user-facing attribute names and source paths. The flake module currently emits
pure constituent packages that retain their named bin references; equivalent
requests converge in the runtime cache. A future pure-Nix implementation of the
same normalization can group definitions by key before constructing packages,
making independently named equivalent labels aliases to one store path as well.

Suggested module shape:

```nix
perSystem.gfty = {
  bases."2x2-magnetic" = {
    size = [ 2 2 ];
    magnets.enabled = true;
    magnets.connectorCutouts = true;
  };

  rims."2x2-standard" = {
    size = [ 2 2 ];
    springCompensation = true;
  };

  bins.small-parts = {
    size = [ 2 2 6 ];
    rimInterface.mode = "swappable";
    labelInterface = {
      mode = "swappable";
      depth = "10mm";
    };
    divider.columns = [ "auto" "auto" "auto" ];
    divider.rows = [ "auto" "auto" ];
  };

  swappableLabels.small-parts = {
    bin = "small-parts";
    embossing.clearance = "0.4mm";
    embossing.inset = "0mm";
  };

  binSets.small-parts = {
    bin = "small-parts";
    base = "2x2-magnetic";
    rim = "2x2-standard";
    swappableLabel = "small-parts";
    connectorPin = true;
  };
};
```

Rust resolves `bin` before remote export and hashes the normalized effective
row-zero interface, not the referenced bin name or complete divider
configuration. Two labels derived from compatible bins therefore share the same
runtime request/cache entry even when their bins have different Y sizes, later
rows, bases, rims, easy grabs, or row-zero merge representations.

Rust remains the authority for validation. The pending Nix normalization/key
implementation needs conformance tests against Rust to prevent evaluation-time
and runtime identities from diverging. Runtime grouping and caching by normalized
request key remain a second deduplication layer independent of Nix aliases.

## Why remote exports are not Nix derivations

Authenticated Onshape exports are not suitable as ordinary Nix derivations:

- Sandboxed builds do not have network access.
- Credentials must not be derivation inputs, logs, or store contents.
- The STEP bytes cannot be predicted before the first export.
- An immutable model version does not make the Onshape translator implementation
  or serialization bytes permanently reproducible.
- Remote STEP and PNG files are intentionally runtime outputs, not store paths.

Fixed-output or impure derivations would require predeclared output hashes,
credential plumbing, and unstable network behavior, while weakening the current
security boundary.

Instead, use a runtime request-addressed cache outside the store, for example:

```text
$XDG_CACHE_HOME/gfty/onshape/<request-key>/artifact.step
$XDG_CACHE_HOME/gfty/onshape/<request-key>/preview-key.png
$XDG_CACHE_HOME/gfty/onshape/<request-key>/manifest.json
```

The request key should include the **normalized** constituent geometry key,
immutable model target, component name, canonical carrier-contract version, and
STEP options—not the source TOML bytes or referenced bin name. Consequently two
different bin/divider requests that normalize to the same label produce the
same canonical carrier and cache key. A preview key additionally includes
dimensions, camera, edges, and background. It is a request identity, not a
prediction of downloaded bytes. The manifest records and verifies the actual
content hash and expected part names before a cached artifact is copied or
hard-linked atomically to an output path.

This preserves the useful Nix behavior—pure normalized identities and shared
request packages—while keeping credentials, API calls, and downloaded geometry
strictly at runtime.
