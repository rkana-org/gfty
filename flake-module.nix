{ self }:
{ lib, flake-parts-lib, ... }:
let
  inherit (flake-parts-lib) mkPerSystemOption;
  inherit (lib) mkOption types;

  defaultLabelModelUrl = "https://cad.onshape.com/documents/089ad0a2edf08cd2cfdc9875/v/02d1ce92af09ce405aff8f7d/e/5bba513a46b691f2bf439aaa";
  defaultBinModelUrl = "https://cad.onshape.com/documents/044aa38d921c6673acd89aef/v/793cbd4a9bdd57cb44baa08a/e/47f09ccd9b344504691f98d4";

  positiveUnsigned = types.addCheck types.ints.unsigned (value: value > 0);
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

  rimInterfaceType = types.submodule {
    options.mode = mkOption {
      type = types.enum [
        "off"
        "integrated"
        "swappable"
      ];
      default = "swappable";
    };
  };

  labelInterfaceType = types.submodule {
    options = {
      mode = mkOption {
        type = types.enum [
          "off"
          "integrated"
          "swappable"
        ];
        default = "swappable";
      };
      depth = mkOption {
        type = types.str;
        default = "10mm";
      };
      supports = mkOption {
        type = types.enum [
          "always"
          "auto"
          "off"
        ];
        default = "auto";
      };
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
      tub = mkOption {
        type = types.bool;
        default = true;
        description = "Generate a tub cavity in the bin body.";
      };
      rimInterface = mkOption {
        type = rimInterfaceType;
        default = { };
      };
      labelInterface = mkOption {
        type = labelInterfaceType;
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
      maxPrintOverhang = mkOption {
        type = types.addCheck types.number (value: value >= 0 && value <= 90);
        default = 60;
        description = "Maximum printable overhang angle in degrees.";
      };
    };
  };

  baseType = types.submodule {
    options = {
      name = mkOption {
        type = types.nullOr types.str;
        default = null;
      };
      size = mkOption {
        type = types.addCheck (types.listOf positiveUnsigned) (values: builtins.length values == 2);
      };
      roundedCorners = mkOption {
        type = types.bool;
        default = false;
      };
      magnets = mkOption {
        type = types.submodule {
          options = {
            enabled = mkOption {
              type = types.bool;
              default = true;
            };
            connectorCutouts = mkOption {
              type = types.bool;
              default = true;
            };
          };
        };
        default = { };
      };
    };
  };

  rimType = types.submodule {
    options = {
      name = mkOption {
        type = types.nullOr types.str;
        default = null;
      };
      size = mkOption {
        type = types.addCheck (types.listOf positiveUnsigned) (values: builtins.length values == 2);
      };
      springCompensation = mkOption {
        type = types.bool;
        default = true;
      };
      additionalExpansion = mkOption {
        type = types.str;
        default = "0mm";
      };
    };
  };

  swappableLabelType = types.submodule {
    options = {
      name = mkOption {
        type = types.nullOr types.str;
        default = null;
      };
      bin = mkOption {
        type = types.str;
        description = "Named bin used to derive the normalized label interface.";
      };
      embossing = mkOption {
        type = types.submodule {
          options = {
            clearance = mkOption {
              type = types.str;
              default = "0.4mm";
            };
            inset = mkOption {
              type = types.str;
              default = "0mm";
            };
          };
        };
        default = { };
      };
    };
  };

  colorOverridesType =
    colors:
    builtins.isAttrs colors
    && lib.all (value: builtins.isInt value && value >= 0) (builtins.attrValues colors);

  scaleFactorType = value: (builtins.isInt value || builtins.isFloat value) && value > 0;

  iconPlacementType = types.either types.path (
    types.addCheck types.attrs (
      value:
      let
        names = builtins.attrNames value;
        iconNames = [
          "colors"
          "icon"
          "scale"
          "scaleX"
          "scaleY"
        ];
      in
      (
        (value ? icon)
        && lib.all (name: builtins.elem name iconNames) names
        && builtins.isPath value.icon
        && (!(value ? colors) || colorOverridesType value.colors)
        && (!(value ? scale) || scaleFactorType value.scale)
        && (!(value ? scaleX) || scaleFactorType value.scaleX)
        && (!(value ? scaleY) || scaleFactorType value.scaleY)
      )
      || (names == [ "spacer" ] && builtins.isString value.spacer)
    )
  );

  binSetType = types.submodule {
    options = {
      name = mkOption {
        type = types.nullOr types.str;
        default = null;
      };
      bin = mkOption { type = types.str; };
      base = mkOption {
        type = types.nullOr types.str;
        default = null;
      };
      rim = mkOption {
        type = types.nullOr types.str;
        default = null;
      };
      swappableLabel = mkOption {
        type = types.nullOr types.str;
        default = null;
      };
      connectorPin = mkOption {
        type = types.bool;
        default = false;
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
        type = types.attrsOf (types.listOf iconPlacementType);
        default = { };
        description = "Ordered icon paths, per-icon scaling/color attrsets, and explicit spacers for each icon box, keyed without the icons- prefix.";
      };
      bin = mkOption {
        type = types.str;
        description = "Named bin from gfty.bins used as the label prototype.";
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
        type = types.str;
        description = "Named bin used by the plate's shared base model.";
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
        }:
        let
          command = lib.escapeShellArgs (
            [ "${package}/bin/gfty" ]
            ++ fontArguments fonts
            ++ arguments
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
            tub
            rimInterface
            labelInterface
            divider
            easyGrab
            maxPrintOverhang
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
      basePackages = lib.mapAttrs (
        baseName: definition:
        package.mkBase {
          name = if definition.name == null then baseName else definition.name;
          inherit (definition) size roundedCorners magnets;
        }
      ) config.gfty.bases;
      basesOutput = package.mkOutputSet {
        name = "bases";
        entries = basePackages;
      };
      rimPackages = lib.mapAttrs (
        rimName: definition:
        package.mkRim {
          name = if definition.name == null then rimName else definition.name;
          inherit (definition) size springCompensation additionalExpansion;
        }
      ) config.gfty.rims;
      rimsOutput = package.mkOutputSet {
        name = "rims";
        entries = rimPackages;
      };
      swappableLabelPackages = lib.mapAttrs (
        labelName: definition:
        package.mkSwappableLabel {
          name = if definition.name == null then labelName else definition.name;
          bin =
            binPackages.${definition.bin}
              or (throw "gfty.swappableLabels.${labelName} refers to unknown bin ${definition.bin}");
          inherit (definition) embossing;
        }
      ) config.gfty.swappableLabels;
      swappableLabelsOutput = package.mkOutputSet {
        name = "swappable-labels";
        entries = swappableLabelPackages;
      };
      binSetPackages = lib.mapAttrs (
        setName: definition:
        package.mkBinSet {
          name = if definition.name == null then setName else definition.name;
          bin =
            binPackages.${definition.bin}
              or (throw "gfty.binSets.${setName} refers to unknown bin ${definition.bin}");
          base =
            if definition.base == null then
              null
            else
              basePackages.${definition.base}
                or (throw "gfty.binSets.${setName} refers to unknown base ${definition.base}");
          rim =
            if definition.rim == null then
              null
            else
              rimPackages.${definition.rim}
                or (throw "gfty.binSets.${setName} refers to unknown rim ${definition.rim}");
          swappableLabel =
            if definition.swappableLabel == null then
              null
            else
              swappableLabelPackages.${definition.swappableLabel}
                or (throw "gfty.binSets.${setName} refers to unknown swappable label ${definition.swappableLabel}");
          inherit (definition) connectorPin;
        }
      ) config.gfty.binSets;
      binSetsOutput = package.mkOutputSet {
        name = "bin-sets";
        entries = binSetPackages;
      };
      prototypeFor =
        owner: definition:
        let
          namedDefinition =
            config.gfty.bins.${definition.bin} or (throw "${owner} refers to unknown bin ${definition.bin}");
        in
        {
          size = lib.take 2 namedDefinition.size;
          binPackage = binPackages.${definition.bin};
        };
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
        lib.nameValuePair "export-label-${labelName}" (makeExportApp {
          name = labelName;
          arguments = [
            "export"
            (toString output.labelConfig)
          ];
          fonts = output.labelFonts;
          modelUrl = config.gfty.labelModelUrl;
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
          ++ [
            "--bin"
            (toString prototype.binPackage.binConfig)
          ]
          ++ map (label: toString label.labelConfig) output.plateLabels;
          fonts = output.labelFonts;
          modelUrl = config.gfty.labelModelUrl;
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
      baseExportApps = lib.mapAttrs' (
        baseName: output:
        lib.nameValuePair "export-base-${baseName}" (makeExportApp {
          name = baseName;
          arguments = [
            "export"
            (toString output.baseConfig)
          ];
          modelUrl = config.gfty.binModelUrl;
        })
      ) basePackages;
      rimExportApps = lib.mapAttrs' (
        rimName: output:
        lib.nameValuePair "export-rim-${rimName}" (makeExportApp {
          name = rimName;
          arguments = [
            "export"
            (toString output.rimConfig)
          ];
          modelUrl = config.gfty.binModelUrl;
        })
      ) rimPackages;
      swappableLabelExportApps = lib.mapAttrs' (
        labelName: output:
        lib.nameValuePair "export-swappable-label-${labelName}" (makeExportApp {
          name = labelName;
          arguments = [
            "export"
            (toString output.swappableLabelConfig)
          ];
          modelUrl = config.gfty.binModelUrl;
        })
      ) swappableLabelPackages;
      binSetExportApps = lib.mapAttrs' (
        setName: output:
        lib.nameValuePair "export-bin-set-${setName}" (makeExportApp {
          name = setName;
          arguments = [
            "export"
            (toString output.binSetConfig)
          ];
          modelUrl = config.gfty.binModelUrl;
        })
      ) binSetPackages;
      connectorPinExportApp = {
        export-connector-pin = makeExportApp {
          name = "connector-pin";
          arguments = [
            "connector-pin"
            "export"
          ];
          modelUrl = config.gfty.binModelUrl;
        };
      };
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
        bases = mkOption {
          type = types.attrsOf baseType;
          default = { };
          description = "Independent Gridfinity base definitions.";
        };
        rims = mkOption {
          type = types.attrsOf rimType;
          default = { };
          description = "Independent swappable-rim definitions.";
        };
        swappableLabels = mkOption {
          type = types.attrsOf swappableLabelType;
          default = { };
          description = "Swappable label blanks derived from named bins.";
        };
        binSets = mkOption {
          type = types.attrsOf binSetType;
          default = { };
          description = "Compatible sets of constituent Gridfinity parts.";
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
        apps =
          labelExportApps
          // plateExportApps
          // binExportApps
          // baseExportApps
          // rimExportApps
          // swappableLabelExportApps
          // binSetExportApps
          // connectorPinExportApp;
        packages = {
          bins = binsOutput;
          bases = basesOutput;
          rims = rimsOutput;
          swappable-labels = swappableLabelsOutput;
          bin-sets = binSetsOutput;
          labels = labelsOutput;
          plates = platesOutput;
        };
      };
    }
  );
}
