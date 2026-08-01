{
  lib,
  runCommand,
  formats,
  args,
}:
let
  allowedArguments = [
    "name"
    "size"
    "tub"
    "maxPrintOverhang"
    "rimInterface"
    "labelInterface"
    "divider"
    "easyGrab"
  ];
  unknownArguments = builtins.attrNames (builtins.removeAttrs args allowedArguments);
  name = args.name or "bin";
  dividerSource = args.divider or { };
  divider =
    lib.optionalAttrs (dividerSource ? columns) { inherit (dividerSource) columns; }
    // lib.optionalAttrs (dividerSource ? rows) { inherit (dividerSource) rows; }
    // lib.optionalAttrs (dividerSource ? merges) {
      merges = map (merge: { inherit (merge) columns rows; }) dividerSource.merges;
    };
  easyGrabSource = args.easyGrab or { };
  easyGrab =
    lib.optionalAttrs (easyGrabSource ? mode) { inherit (easyGrabSource) mode; }
    // lib.optionalAttrs (easyGrabSource ? side) { inherit (easyGrabSource) side; }
    // lib.optionalAttrs (easyGrabSource ? radius) { inherit (easyGrabSource) radius; }
    // lib.optionalAttrs (easyGrabSource ? faces) {
      faces = map (
        face:
        {
          inherit (face) side columns rows;
        }
        // lib.optionalAttrs (face ? radius && face.radius != null) { inherit (face) radius; }
      ) easyGrabSource.faces;
    };
  definition = {
    kind = "bin";
    version = 2;
    inherit (args) size;
    tub = args.tub or true;
    max-print-overhang = args.maxPrintOverhang or 60;
    rim-interface.mode = (args.rimInterface or { }).mode or "swappable";
    label-interface = {
      mode = (args.labelInterface or { }).mode or "swappable";
      depth = (args.labelInterface or { }).depth or "10mm";
      supports = (args.labelInterface or { }).supports or "auto";
    };
  }
  // lib.optionalAttrs (divider != { }) { inherit divider; }
  // lib.optionalAttrs (easyGrab != { }) { easy-grab = easyGrab; };
  config = (formats.toml { }).generate "${name}-bin.toml" definition;
in
assert lib.assertMsg (unknownArguments == [ ]) (
  "gfty.mkBin received unsupported arguments: " + lib.concatStringsSep ", " unknownArguments
);
runCommand name
  {
    passthru.binConfig = config;
  }
  ''
    mkdir -p "$out"
    cp ${lib.escapeShellArg (toString config)} "$out/bin.toml"
  ''
