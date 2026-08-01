{
  runCommand,
  formats,
  lib,
  args,
}:
let
  name = args.name or "swappable-label";
  embossing = args.embossing or { };
  config = (formats.toml { }).generate "${name}-swappable-label.toml" {
    kind = "swappable-label";
    version = 1;
    bin = toString args.bin.binConfig;
    embossing = {
      clearance = embossing.clearance or "0.4mm";
      inset = embossing.inset or "0mm";
    };
  };
in
runCommand name { passthru.swappableLabelConfig = config; } ''
  mkdir -p "$out"
  cp ${lib.escapeShellArg (toString config)} "$out/swappable-label.toml"
''
