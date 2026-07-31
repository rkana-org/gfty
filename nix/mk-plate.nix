{
  lib,
  runCommand,
  gfty,
  args,
}:
let
  name = args.name or "plate";
  labels = args.labels or [ ];
  dimensions = args.dimensions or (throw "gfty.mkPlate requires dimensions = [ WIDTH HEIGHT ]");
  columnGap = args.columnGap or "5mm";
  rowGap = args.rowGap or "5mm";

  labelConfig =
    label: if builtins.isAttrs label && label ? labelConfig then label.labelConfig else label;
  configs = map labelConfig labels;
  fonts = lib.unique (
    lib.concatMap (
      label: if builtins.isAttrs label && label ? labelFonts then label.labelFonts else [ ]
    ) labels
    ++ (args.fonts or [ ])
  );
  fontArgs = lib.concatMapStringsSep " " (
    font: "--font-dir ${lib.escapeShellArg (toString font)}"
  ) fonts;
  labelArgs = lib.concatMapStringsSep " " (label: lib.escapeShellArg (toString label)) configs;
in
assert lib.assertMsg (
  builtins.length dimensions == 2
) "gfty.mkPlate dimensions must contain WIDTH and HEIGHT";
assert lib.assertMsg (configs != [ ]) "gfty.mkPlate requires at least one label";
runCommand name
  {
    nativeBuildInputs = [ gfty ];
    passthru = {
      plateLabels = labels;
      labelFonts = fonts;
    };
  }
  ''
    mkdir -p "$out"
    gfty ${fontArgs} label plate create \
      --dimensions ${lib.escapeShellArg (toString (builtins.elemAt dimensions 0))} \
                   ${lib.escapeShellArg (toString (builtins.elemAt dimensions 1))} \
      --column-gap ${lib.escapeShellArg columnGap} \
      --row-gap ${lib.escapeShellArg rowGap} \
      --svg "$out/plate.svg" \
      ${labelArgs}
  ''
