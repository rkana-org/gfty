{
  lib,
  runCommand,
  formats,
  gftyLabel,
  args,
}:
let
  inherit (args) template;
  name = args.name or "label";
  filament = args.filament or 0;
  fonts = args.fonts or [ ];
  text = args.text or { };
  icons = args.icons or { };
  bin = args.bin or null;

  # Import a file's complete parent directory so adjacent color sidecars and
  # any SVG-relative resources remain available in the Nix store.
  assetPath =
    path:
    if builtins.isPath path then
      let
        directory = builtins.path {
          path = builtins.dirOf path;
          name = "gfty-label-assets";
        };
      in
      "${directory}/${builtins.baseNameOf path}"
    else
      toString path;

  iconItem =
    item:
    if builtins.isAttrs item then
      if item ? icon then item // { icon = assetPath item.icon; } else item
    else
      { icon = assetPath item; };

  config = (formats.toml { }).generate "${name}-label.toml" (
    {
      kind = "label";
      version = 1;
      template = assetPath template;
      inherit filament;
      text = lib.mapAttrs (_: content: { inherit content; }) text;
      icons = lib.mapAttrs (_: items: map iconItem items) icons;
    }
    // lib.optionalAttrs (bin != null) {
      bin = toString (if builtins.isAttrs bin && bin ? binConfig then bin.binConfig else bin);
    }
  );

  fontArgs = lib.concatMapStringsSep " " (
    font: "--font-dir ${lib.escapeShellArg (toString font)}"
  ) fonts;
in
runCommand name
  {
    nativeBuildInputs = [ gftyLabel ];
    passthru = {
      labelConfig = config;
      labelFonts = fonts;
    };
  }
  ''
    mkdir -p "$out"
    gfty ${fontArgs} label render ${lib.escapeShellArg (toString config)} --output "$out/label.svg"
    cp ${config} "$out/label.toml"
  ''
