{
  lib,
  runCommand,
  formats,
  args,
}:
let
  name = args.name or "bin";
  rename =
    mappings: attrs:
    builtins.listToAttrs (
      lib.filter (entry: entry != null) (
        map (
          mapping:
          let
            source = builtins.elemAt mapping 0;
            destination = builtins.elemAt mapping 1;
          in
          if builtins.hasAttr source attrs then
            {
              name = destination;
              value = attrs.${source};
            }
          else
            null
        ) mappings
      )
    );
  base = rename [
    [
      "enabled"
      "enabled"
    ]
    [
      "roundedCorners"
      "rounded-corners"
    ]
    [
      "magnets"
      "magnets"
    ]
    [
      "connectorCutouts"
      "connector-cutouts"
    ]
    [
      "connectorPin"
      "connector-pin"
    ]
  ] (args.base or { });
  bin = rename [
    [
      "enabled"
      "enabled"
    ]
    [
      "nesting"
      "nesting"
    ]
    [
      "swappableRim"
      "swappable-rim"
    ]
    [
      "springCompensation"
      "spring-compensation"
    ]
    [
      "additionalRimExpansion"
      "additional-rim-expansion"
    ]
    [
      "tub"
      "tub"
    ]
  ] (args.bin or { });
  label = rename [
    [
      "enabled"
      "enabled"
    ]
    [
      "depth"
      "depth"
    ]
    [
      "swappable"
      "swappable"
    ]
    [
      "supports"
      "supports"
    ]
    [
      "embossingClearance"
      "embossing-clearance"
    ]
    [
      "embossingInset"
      "embossing-inset"
    ]
    [
      "fullWidth"
      "full-width"
    ]
    [
      "widthUnits"
      "width-units"
    ]
  ] (args.label or { });
  dividerSource = args.divider or { };
  divider =
    rename [
      [
        "columns"
        "columns"
      ]
      [
        "rows"
        "rows"
      ]
    ] dividerSource
    // lib.optionalAttrs (dividerSource ? merges) {
      merges = map (merge: {
        inherit (merge) columns rows;
      }) dividerSource.merges;
    };
  easyGrabSource = args.easyGrab or { };
  easyGrab =
    rename [
      [
        "mode"
        "mode"
      ]
      [
        "side"
        "side"
      ]
      [
        "radius"
        "radius"
      ]
    ] easyGrabSource
    // lib.optionalAttrs (easyGrabSource ? faces) {
      faces = map (
        face:
        {
          inherit (face) side columns rows;
        }
        // lib.optionalAttrs (face ? radius && face.radius != null) { inherit (face) radius; }
      ) easyGrabSource.faces;
    };
  print = rename [
    [
      "maxOverhang"
      "max-overhang"
    ]
  ] (args.print or { });
  version = args.version or 1;
  legacyDefinition = {
    inherit (args) size;
  }
  // lib.optionalAttrs (base != { }) { inherit base; }
  // lib.optionalAttrs (bin != { }) { inherit bin; }
  // lib.optionalAttrs (label != { }) { inherit label; }
  // lib.optionalAttrs (divider != { }) { inherit divider; }
  // lib.optionalAttrs (easyGrab != { }) { easy-grab = easyGrab; }
  // lib.optionalAttrs (print != { }) { inherit print; };
  constituentDefinition = {
    inherit (args) size;
    tub = args.tub or true;
    max-print-overhang = (args.print or { }).maxOverhang or 60;
    rim-interface.mode = (args.rimInterface or { }).mode or "swappable";
    label-interface = {
      mode = (args.labelInterface or { }).mode or "swappable";
      depth = (args.labelInterface or { }).depth or "10mm";
      supports = (args.labelInterface or { }).supports or "auto";
    };
  }
  // lib.optionalAttrs (divider != { }) { inherit divider; }
  // lib.optionalAttrs (easyGrab != { }) { easy-grab = easyGrab; };
  config = (formats.toml { }).generate "${name}-bin.toml" (
    {
      kind = "bin";
      inherit version;
    }
    // (if version == 1 then legacyDefinition else constituentDefinition)
  );
in
runCommand name
  {
    passthru.binConfig = config;
  }
  ''
    mkdir -p "$out"
    cp ${lib.escapeShellArg (toString config)} "$out/bin.toml"
  ''
