# FeatureScripts

All FeatureScript sources maintained by `gfty` live below this directory.
FeatureScript has no local compiler, so changes must be compiled and smoke-tested
in Onshape.

## Labels

- `labels/gfty_label_instances.fs`: version-2 label/plate importer. It instances
  the prototype, builds filament artwork, names multipart output, and adds
  multi-label connector plates.

## Configuration

- `configuration/extract_json_config.fs`: converts schema/value JSON into Part
  Studio variables used by Gridfinity Ultimate.
- `configuration/variable_configured_derived.fs`: wraps native Derived and maps
  current Part Studio variables into source configuration parameter IDs.
- `configuration/variable_configured_derived.md`: installation and mapping guide.

## Dividers

- `dividers/walls_grid.fs`: divider-wall generator driven by layout JSON.
- `dividers/README.md`: wall layout contract.
- `dividers/preview.html`: standalone visual preview for divider layouts.
