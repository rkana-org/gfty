# Variable-configured Derived

`variable_configured_derived.fs` wraps Onshape's standard `importDerived`
feature. It keeps the native Derived implementation for body selection,
instancing, active sheet metal, properties, placement, and mate-connector
manipulators, but replaces selected source configuration values before calling
it.

## Why

The native Derived dialog can configure the referenced Part Studio, but its
inline configuration fields do not accept `#variable` expressions. This wrapper
maps variables from the current Part Studio into configuration inputs of the
referenced Part Studio, so the target Part Studio's own configuration can drive
the derived model.

## Installation

Create a Feature Studio in the target document and paste
`variable_configured_derived.fs` into it. Update its FeatureScript and standard
library versions if Onshape offers a newer version, then add the
**Variable-configured derived** custom feature to the toolbar.

## Usage

1. Create configuration inputs or variables in the target Part Studio.
2. Add **Variable-configured derived** after those variables are available.
3. Select the source Part Studio and its parts. Its selected configuration is
   retained as the fallback configuration.
4. Add one **Configuration override** per forwarded input:
   - **Source configuration input ID** is the FeatureScript ID in the source
     Part Studio.
   - **Variable name** is the target variable name without `#`.
5. Configure placement and the other Derived options normally.

For example, this mapping forwards target variable `#binWidth` into source
configuration input `width`:

```text
Source configuration input ID: width
Variable name: binWidth
```

Configuration input IDs are not necessarily their displayed names. In the
source Part Studio, use **Edit FeatureScript IDs** to inspect or assign stable
IDs. Giving corresponding target and source inputs the same IDs makes the
mapping especially easy to understand.

Unmapped source inputs keep the values chosen in the Part Studio reference.
Configuration-list variables are converted to the source list's enum when their
internal option IDs match. Other values are passed through with their units and
types intact; the source Part Studio reports incompatible values.

## Limitations

FeatureScript cannot dynamically create one typed feature parameter per
configuration input of a selected Part Studio. Consequently the source input ID
must be entered as a string rather than selected from a generated dropdown.
The feature validates it against `PartStudioData.configurationData` when that
metadata is available.

The wrapper delegates to the standard Derived feature, but it still needs a
final smoke test in Onshape because there is no local FeatureScript compiler.
