{ self }:
{ lib, flake-parts-lib, ... }:
let
  inherit (flake-parts-lib) mkPerSystemOption;
  inherit (lib) mkOption types;

  labelType = types.submodule {
    options = {
      name = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Derivation pname; defaults to the label attribute name.";
      };
      template = mkOption {
        type = types.path;
        description = "SVG template path.";
      };
      filament = mkOption {
        type = types.ints.unsigned;
        default = 0;
        description = "Filament ID for the blank prototype body.";
      };
      fonts = mkOption {
        type = types.listOf (types.either types.package types.path);
        default = [ ];
        description = "Additional font packages or directories.";
      };
      text = mkOption {
        type = types.attrsOf types.str;
        default = { };
        description = "Text field contents, keyed without the text- prefix.";
      };
      icons = mkOption {
        type = types.attrsOf (types.listOf types.path);
        default = { };
        description = "Ordered icon paths for each icon box, keyed without the icons- prefix.";
      };
    };
  };

  plateType = types.submodule {
    options = {
      name = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Derivation pname; defaults to the plate attribute name.";
      };
      dimensions = mkOption {
        type = types.addCheck (types.listOf types.str) (values: builtins.length values == 2);
        description = "Maximum plate width and height, for example [ \"200mm\" \"250mm\" ].";
      };
      labels = mkOption {
        type = types.listOf types.str;
        description = "Ordered label names from gfty-label.labels; names may be repeated.";
      };
      fonts = mkOption {
        type = types.listOf (types.either types.package types.path);
        default = [ ];
        description = "Additional font packages or directories for every plate label.";
      };
      columnGap = mkOption {
        type = types.str;
        default = "5mm";
        description = "Horizontal gap between labels.";
      };
      rowGap = mkOption {
        type = types.str;
        default = "5mm";
        description = "Vertical gap between labels.";
      };
    };
  };
in
{
  options.perSystem = mkPerSystemOption (
    { config, system, ... }:
    let
      package = self.packages.${system}.default;
      labelPackages = lib.mapAttrs (
        labelName: definition:
        package.mkLabel {
          name = if definition.name == null then labelName else definition.name;
          inherit (definition)
            template
            filament
            fonts
            text
            icons
            ;
        }
      ) config.gfty-label.labels;
      allLabels = package.mkOutputSet {
        name = "all-labels";
        entries = labelPackages;
      };
      labelsOutput =
        if builtins.hasAttr "all" labelPackages then
          throw "gfty-label.labels.all is reserved for the combined label output"
        else
          package.mkOutputSet {
            name = "labels";
            entries = labelPackages;
            extra.all = allLabels;
          };
      platePackages = lib.mapAttrs (
        plateName: definition:
        package.mkPlate {
          name = if definition.name == null then plateName else definition.name;
          inherit (definition)
            dimensions
            fonts
            columnGap
            rowGap
            ;
          labels = map (
            labelName:
            labelPackages.${labelName}
              or (throw "gfty-label plate ${plateName} refers to unknown label ${labelName}")
          ) definition.labels;
        }
      ) config.gfty-label.plates;
      platesOutput = package.mkOutputSet {
        name = "plates";
        entries = platePackages;
      };
    in
    {
      options.gfty-label = {
        labels = mkOption {
          type = types.attrsOf labelType;
          default = { };
          description = "Declarative gfty-label label definitions.";
        };
        plates = mkOption {
          type = types.attrsOf plateType;
          default = { };
          description = "Declarative gfty-label plate definitions.";
        };
      };

      config.packages = {
        labels = labelsOutput;
        plates = platesOutput;
      };
    }
  );
}
