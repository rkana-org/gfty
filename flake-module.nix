{ self }:
{ lib, flake-parts-lib, ... }:
let
  inherit (flake-parts-lib) mkPerSystemOption;
  inherit (lib) mkOption types;

  defaultOnshapeBaseUrl = "https://cad.onshape.com/documents/089ad0a2edf08cd2cfdc9875/v/02d1ce92af09ce405aff8f7d/e/5bba513a46b691f2bf439aaa";

  # Onshape configuration assignments are semicolon-separated. Each value is
  # form-encoded first, then the complete assignment string is URL-encoded as
  # the outer `configuration` query parameter.
  formEncode = value: lib.replaceStrings [ "%20" ] [ "+" ] (lib.escapeURL value);
  makeOnshapeUrl =
    baseUrl: configJsonPath: gftyUltimateConfig:
    let
      configJson = lib.removeSuffix "\n" (builtins.readFile configJsonPath);
      configuration =
        "Config=${formEncode configJson}"
        + ";GFTYUltimateConfig=${formEncode (builtins.toJSON gftyUltimateConfig)}";
    in
    "${baseUrl}?renderMode=&configuration=${lib.escapeURL configuration}";

  jsonAttrsType = types.addCheck types.attrs (
    value: (builtins.tryEval (builtins.toJSON value)).success
  );

  baseSize =
    owner: gftyUltimateConfig:
    map (field: gftyUltimateConfig.${field} or (throw "${owner}.gfty-ultimate must define ${field}")) [
      "size_x_units"
      "size_y_units"
    ];

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
      gfty-ultimate = mkOption {
        type = jsonAttrsType;
        description = "Gridfinity Ultimate JSON configuration, expressed as a JSON-serializable Nix attribute set.";
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
      gfty-ultimate = mkOption {
        type = jsonAttrsType;
        description = "Gridfinity Ultimate JSON configuration for the plate's shared base model.";
      };
    };
  };
in
{
  options.perSystem = mkPerSystemOption (
    { config, system, ... }:
    let
      package = self.packages.${system}.default;
      withOnshapeUrl =
        output: jsonFile: gftyUltimateConfig:
        output.overrideAttrs (old: {
          passthru = (old.passthru or { }) // {
            onshapeUrl =
              makeOnshapeUrl config.gfty-label.onshapeBaseUrl "${output}/${jsonFile}"
                gftyUltimateConfig;
          };
        });
      rawLabelPackages = lib.mapAttrs (
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
      labelPackages = lib.mapAttrs (
        labelName: output:
        withOnshapeUrl output "label.json" config.gfty-label.labels.${labelName}.gfty-ultimate
      ) rawLabelPackages;
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
      rawPlatePackages = lib.mapAttrs (
        plateName: definition:
        let
          plateSize = baseSize "gfty-label.plates.${plateName}" definition.gfty-ultimate;
          mismatchedLabels = builtins.filter (
            labelName:
            baseSize "gfty-label.labels.${labelName}" (
              config.gfty-label.labels.${labelName}.gfty-ultimate
                or (throw "gfty-label plate ${plateName} refers to unknown label ${labelName}")
            ) != plateSize
          ) (lib.unique definition.labels);
        in
        assert lib.assertMsg (mismatchedLabels == [ ]) (
          "gfty-label plate ${plateName} has a different Gridfinity base size than labels: "
          + lib.concatStringsSep ", " mismatchedLabels
        );
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
      platePackages = lib.mapAttrs (
        plateName: output:
        withOnshapeUrl output "plate.json" config.gfty-label.plates.${plateName}.gfty-ultimate
      ) rawPlatePackages;
      platesOutput = package.mkOutputSet {
        name = "plates";
        entries = platePackages;
      };
    in
    {
      options.gfty-label = {
        onshapeBaseUrl = mkOption {
          type = types.str;
          default = defaultOnshapeBaseUrl;
          description = "Base Onshape workspace URL used by generated package onshapeUrl passthru values.";
        };
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
