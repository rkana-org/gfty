{
  runCommand,
  formats,
  lib,
  args,
}:
let
  name = args.name or "rim";
  config = (formats.toml { }).generate "${name}-rim.toml" {
    kind = "rim";
    version = 1;
    inherit (args) size;
    spring-compensation = args.springCompensation or true;
    additional-expansion = args.additionalExpansion or "0mm";
  };
in
runCommand name { passthru.rimConfig = config; } ''
  mkdir -p "$out"
  cp ${lib.escapeShellArg (toString config)} "$out/rim.toml"
''
