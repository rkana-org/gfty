{
  runCommand,
  formats,
  lib,
  args,
}:
let
  name = args.name or "base";
  config = (formats.toml { }).generate "${name}-base.toml" {
    kind = "base";
    version = 1;
    inherit (args) size;
    rounded-corners = args.roundedCorners or false;
    magnets =
      let
        magnets = args.magnets or { };
      in
      {
        enabled = magnets.enabled or true;
        connector-cutouts = magnets.connectorCutouts or true;
      };
  };
in
runCommand name { passthru.baseConfig = config; } ''
  mkdir -p "$out"
  cp ${lib.escapeShellArg (toString config)} "$out/base.toml"
''
