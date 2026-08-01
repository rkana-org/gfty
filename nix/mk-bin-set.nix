{
  runCommand,
  formats,
  lib,
  args,
}:
let
  name = args.name or "bin-set";
  optionalPath = value: field: if value == null then { } else { ${field} = toString value; };
  base = args.base or null;
  rim = args.rim or null;
  swappableLabel = args.swappableLabel or null;
  definition = {
    kind = "bin-set";
    version = 1;
    bin = toString args.bin.binConfig;
    connector-pin = args.connectorPin or false;
  }
  // optionalPath (if base == null then null else base.baseConfig) "base"
  // optionalPath (if rim == null then null else rim.rimConfig) "rim"
  // optionalPath (
    if swappableLabel == null then null else swappableLabel.swappableLabelConfig
  ) "swappable-label";
  config = (formats.toml { }).generate "${name}-bin-set.toml" definition;
in
runCommand name { passthru.binSetConfig = config; } ''
  mkdir -p "$out"
  cp ${lib.escapeShellArg (toString config)} "$out/bin-set.toml"
''
