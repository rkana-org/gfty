{ self }:
{ lib, flake-parts-lib, ... }:
let
  inherit (flake-parts-lib) mkPerSystemOption;
  inherit (lib) mkOption types;

  defaultLabelModelUrl = "https://cad.onshape.com/documents/089ad0a2edf08cd2cfdc9875/v/02d1ce92af09ce405aff8f7d/e/5bba513a46b691f2bf439aaa";
  defaultBinModelUrl = "https://cad.onshape.com/documents/044aa38d921c6673acd89aef/v/793cbd4a9bdd57cb44baa08a/e/47f09ccd9b344504691f98d4";

  jsonAttrsType = types.addCheck types.attrs (
    value: (builtins.tryEval (builtins.toJSON value)).success
  );
  positiveUnsigned = types.addCheck types.ints.unsigned (value: value > 0);
  positiveNumber = types.addCheck types.number (value: value > 0);
  rangeType = types.addCheck (types.listOf types.ints.unsigned) (
    values: builtins.length values == 2 && builtins.elemAt values 0 <= builtins.elemAt values 1
  );
  tracksType = types.addCheck (types.listOf types.str) (values: values != [ ]);

  mergeType = types.submodule {
    options = {
      columns = mkOption {
        type = rangeType;
        description = "Inclusive zero-based divider column range.";
      };
      rows = mkOption {
        type = rangeType;
        description = "Inclusive zero-based divider row range.";
      };
    };
  };

  easyGrabFaceType = types.submodule {
    options = {
      side = mkOption {
        type = types.enum [
          "north"
          "south"
          "east"
          "west"
        ];
      };
      columns = mkOption { type = rangeType; };
      rows = mkOption { type = rangeType; };
      radius = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Optional per-face radius with a physical unit.";
      };
    };
  };

  binBaseType = types.submodule {
    options = {
      enabled = mkOption {
        type = types.bool;
        default = true;
      };
      roundedCorners = mkOption {
        type = types.bool;
        default = false;
      };
      magnets = mkOption {
        type = types.bool;
        default = true;
      };
      connectorCutouts = mkOption {
        type = types.bool;
        default = true;
      };
      connectorPin = mkOption {
        type = types.bool;
        default = true;
      };
    };
  };

  binBodyType = types.submodule {
    options = {
      enabled = mkOption {
        type = types.bool;
        default = true;
      };
      nesting = mkOption {
        type = types.bool;
        default = true;
      };
      swappableRim = mkOption {
        type = types.bool;
        default = true;
      };
      springCompensation = mkOption {
        type = types.bool;
        default = true;
      };
      additionalRimExpansion = mkOption {
        type = types.str;
        default = "0mm";
      };
      tub = mkOption {
        type = types.bool;
        default = true;
      };
    };
  };

  binLabelType = types.submodule {
    options = {
      enabled = mkOption {
        type = types.bool;
        default = true;
      };
      depth = mkOption {
        type = types.str;
        default = "10mm";
      };
      swappable = mkOption {
        type = types.bool;
        default = true;
      };
      supports = mkOption {
        type = types.enum [
          "always"
          "auto"
          "off"
        ];
        default = "auto";
      };
      embossingClearance = mkOption {
        type = types.str;
        default = "0.4mm";
      };
      embossingInset = mkOption {
        type = types.str;
        default = "0mm";
      };
      fullWidth = mkOption {
        type = types.bool;
        default = true;
      };
      widthUnits = mkOption {
        type = positiveNumber;
        default = 1;
      };
    };
  };

  dividerType = types.submodule {
    options = {
      columns = mkOption {
        type = tracksType;
        default = [
          "auto"
          "auto"
          "auto"
        ];
        description = "Column tracks as auto, fractional values such as 1fr, or physical lengths.";
      };
      rows = mkOption {
        type = tracksType;
        default = [
          "auto"
          "auto"
        ];
      };
      merges = mkOption {
        type = types.listOf mergeType;
        default = [ ];
      };
    };
  };

  easyGrabType = types.submodule {
    options = {
      mode = mkOption {
        type = types.enum [
          "none"
          "custom"
          "all"
        ];
        default = "all";
      };
      side = mkOption {
        type = types.enum [
          "north"
          "south"
          "east"
          "west"
        ];
        default = "south";
      };
      radius = mkOption {
        type = types.str;
        default = "21mm";
      };
      faces = mkOption {
        type = types.listOf easyGrabFaceType;
        default = [ ];
      };
    };
  };

  printType = types.submodule {
    options.maxOverhang = mkOption {
      type = types.addCheck types.number (value: value >= 0 && value <= 90);
      default = 60;
      description = "Maximum printable overhang angle in degrees.";
    };
  };

  binType = types.submodule {
    options = {
      name = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Derivation pname; defaults to the bin attribute name.";
      };
      size = mkOption {
        type = types.addCheck (types.listOf positiveUnsigned) (values: builtins.length values == 3);
        description = "Gridfinity X, Y, and Z units.";
      };
      base = mkOption {
        type = binBaseType;
        default = { };
      };
      bin = mkOption {
        type = binBodyType;
        default = { };
      };
      label = mkOption {
        type = binLabelType;
        default = { };
      };
      divider = mkOption {
        type = dividerType;
        default = { };
      };
      easyGrab = mkOption {
        type = easyGrabType;
        default = { };
      };
      print = mkOption {
        type = printType;
        default = { };
      };
    };
  };

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
      bin = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Named bin from gfty.bins used as the label prototype.";
      };
      gfty-ultimate = mkOption {
        type = types.nullOr jsonAttrsType;
        default = null;
        description = "Legacy inline Gridfinity Ultimate JSON configuration.";
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
        description = "Ordered label names; names may be repeated.";
      };
      fonts = mkOption {
        type = types.listOf (types.either types.package types.path);
        default = [ ];
        description = "Additional font packages or directories for every plate label.";
      };
      columnGap = mkOption {
        type = types.str;
        default = "5mm";
      };
      rowGap = mkOption {
        type = types.str;
        default = "5mm";
      };
      bin = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Named bin used by the plate's shared base model.";
      };
      gfty-ultimate = mkOption {
        type = types.nullOr jsonAttrsType;
        default = null;
        description = "Legacy inline Gridfinity Ultimate JSON for the shared base model.";
      };
    };
  };

in
{
  options.perSystem = mkPerSystemOption (
    {
      config,
      system,
      ...
    }:
    let
      package = self.packages.${system}.default;
      fontArguments =
        fonts:
        lib.concatMap (font: [
          "--font-dir"
          (toString font)
        ]) fonts;
      makeExportApp =
        {
          name,
          arguments,
          fonts ? [ ],
          modelUrl,
          gftyUltimateConfig ? null,
        }:
        let
          gridfinityConfig =
            if gftyUltimateConfig == null then
              null
            else
              package.writeExportText "${name}-gridfinity.json" (builtins.toJSON gftyUltimateConfig);
          command = lib.escapeShellArgs (
            [ "${package}/bin/gfty" ]
            ++ fontArguments fonts
            ++ arguments
            ++ lib.optionals (gridfinityConfig != null) [
              "--gridfinity-config"
              (toString gridfinityConfig)
            ]
            ++ [
              "--onshape-model"
              modelUrl
            ]
          );
          defaultOutputName = lib.escapeShellArg "${name}.step";
          script = package.writeExportScript "export-${name}" ''
            set -euo pipefail
            has_output=false
            for argument in "$@"; do
              if [[ "$argument" == "-o" || "$argument" == -o?* || "$argument" == "--output" || "$argument" == --output=* ]]; then
                has_output=true
                break
              fi
            done
            if [[ "$has_output" == false ]]; then
              set -- --output "$PWD"/${defaultOutputName} "$@"
            fi
            exec ${command} "$@"
          '';
        in
        {
          type = "app";
          program = toString script;
          meta.description = "Export ${name} as a configured Onshape STEP";
        };
      binPackages = lib.mapAttrs (
        binName: definition:
        package.mkBin {
          name = if definition.name == null then binName else definition.name;
          inherit (definition)
            size
            base
            bin
            label
            divider
            easyGrab
            print
            ;
        }
      ) config.gfty.bins;
      allBins = package.mkOutputSet {
        name = "all-bins";
        entries = binPackages;
      };
      binsOutput =
        if builtins.hasAttr "all" binPackages then
          throw "gfty.bins.all is reserved for the combined bin output"
        else
          package.mkOutputSet {
            name = "bins";
            entries = binPackages;
            extra.all = allBins;
          };
      prototypeFor =
        owner: definition:
        if definition.bin != null && definition.gfty-ultimate != null then
          throw "${owner} must define either bin or gfty-ultimate, not both"
        else if definition.bin != null then
          let
            namedDefinition =
              config.gfty.bins.${definition.bin} or (throw "${owner} refers to unknown bin ${definition.bin}");
          in
          {
            size = lib.take 2 namedDefinition.size;
            binPackage = binPackages.${definition.bin};
            legacy = null;
          }
        else if definition.gfty-ultimate != null then
          {
            size =
              map
                (field: definition.gfty-ultimate.${field} or (throw "${owner}.gfty-ultimate must define ${field}"))
                [
                  "size_x_units"
                  "size_y_units"
                ];
            binPackage = null;
            legacy = definition.gfty-ultimate;
          }
        else
          throw "${owner} must define bin or legacy gfty-ultimate";
      labelPackages = lib.mapAttrs (
        labelName: definition:
        let
          prototype = prototypeFor "gfty.labels.${labelName}" definition;
        in
        package.mkLabel {
          name = if definition.name == null then labelName else definition.name;
          inherit (definition)
            template
            filament
            fonts
            text
            icons
            ;
          bin = prototype.binPackage;
        }
      ) config.gfty.labels;
      allLabels = package.mkOutputSet {
        name = "all-labels";
        entries = labelPackages;
      };
      labelsOutput =
        if builtins.hasAttr "all" labelPackages then
          throw "gfty.labels.all is reserved for the combined label output"
        else
          package.mkOutputSet {
            name = "labels";
            entries = labelPackages;
            extra.all = allLabels;
          };
      platePackages = lib.mapAttrs (
        plateName: definition:
        let
          prototype = prototypeFor "gfty.plates.${plateName}" definition;
          mismatchedLabels = builtins.filter (
            labelName:
            (prototypeFor "gfty.labels.${labelName}" (
              config.gfty.labels.${labelName}
                or (throw "gfty plate ${plateName} refers to unknown label ${labelName}")
            )).size != prototype.size
          ) (lib.unique definition.labels);
        in
        assert lib.assertMsg (mismatchedLabels == [ ]) (
          "gfty plate ${plateName} has a different Gridfinity base size than labels: "
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
            labelPackages.${labelName} or (throw "gfty plate ${plateName} refers to unknown label ${labelName}")
          ) definition.labels;
        }
      ) config.gfty.plates;
      platesOutput = package.mkOutputSet {
        name = "plates";
        entries = platePackages;
      };
      labelExportApps = lib.mapAttrs' (
        labelName: output:
        let
          definition = config.gfty.labels.${labelName};
          prototype = prototypeFor "gfty.labels.${labelName}" definition;
        in
        lib.nameValuePair "export-label-${labelName}" (makeExportApp {
          name = labelName;
          arguments = [
            "export"
            (toString output.labelConfig)
          ];
          fonts = output.labelFonts;
          modelUrl = config.gfty.labelModelUrl;
          gftyUltimateConfig = prototype.legacy;
        })
      ) labelPackages;
      plateExportApps = lib.mapAttrs' (
        plateName: output:
        let
          definition = config.gfty.plates.${plateName};
          prototype = prototypeFor "gfty.plates.${plateName}" definition;
        in
        lib.nameValuePair "export-plate-${plateName}" (makeExportApp {
          name = plateName;
          arguments = [
            "label"
            "plate"
            "export"
            "--dimensions"
            (builtins.elemAt definition.dimensions 0)
            (builtins.elemAt definition.dimensions 1)
            "--column-gap"
            definition.columnGap
            "--row-gap"
            definition.rowGap
          ]
          ++ lib.optionals (prototype.binPackage != null) [
            "--bin"
            (toString prototype.binPackage.binConfig)
          ]
          ++ map (label: toString label.labelConfig) output.plateLabels;
          fonts = output.labelFonts;
          modelUrl = config.gfty.labelModelUrl;
          gftyUltimateConfig = prototype.legacy;
        })
      ) platePackages;
      binExportApps = lib.mapAttrs' (
        binName: output:
        lib.nameValuePair "export-bin-${binName}" (makeExportApp {
          name = binName;
          arguments = [
            "export"
            (toString output.binConfig)
          ];
          modelUrl = config.gfty.binModelUrl;
        })
      ) binPackages;
    in
    {
      options.gfty = {
        labelModelUrl = mkOption {
          type = types.str;
          default = defaultLabelModelUrl;
          description = "Immutable Onshape label model version used by generated export apps.";
        };
        binModelUrl = mkOption {
          type = types.str;
          default = defaultBinModelUrl;
          description = "Immutable Gridfinity Ultimate model version used by generated bin apps.";
        };
        bins = mkOption {
          type = types.attrsOf binType;
          default = { };
          description = "Declarative Gridfinity bin definitions.";
        };
        labels = mkOption {
          type = types.attrsOf labelType;
          default = { };
          description = "Declarative label definitions.";
        };
        plates = mkOption {
          type = types.attrsOf plateType;
          default = { };
          description = "Declarative label plate definitions.";
        };
      };

      config = {
        apps = labelExportApps // plateExportApps // binExportApps;
        packages = {
          bins = binsOutput;
          labels = labelsOutput;
          plates = platesOutput;
        };
      };
    }
  );
}
