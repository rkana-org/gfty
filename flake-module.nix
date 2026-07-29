{ self }:
{ lib, flake-parts-lib, ... }:
let
  inherit (flake-parts-lib) mkPerSystemOption;
  inherit (lib) mkOption types;
in
{
  options.perSystem = mkPerSystemOption (
    { config, system, ... }:
    let
      package = self.packages.${system}.default;
      labelPackages = lib.mapAttrs (
        name: definition: package.mkLabel (definition // { name = definition.name or name; })
      ) config.gfty-label.labels;
      resolvePlate =
        name: definition:
        package.mkPlate (
          definition
          // {
            name = definition.name or name;
            labels = map (
              label:
              if builtins.isString label && builtins.hasAttr label labelPackages then
                labelPackages.${label}
              else
                label
            ) definition.labels;
          }
        );
      generatedPackages =
        lib.mapAttrs' (name: value: lib.nameValuePair "label-${name}" value) labelPackages
        // lib.mapAttrs' (
          name: value: lib.nameValuePair "plate-${name}" (resolvePlate name value)
        ) config.gfty-label.plates;
    in
    {
      options.gfty-label = {
        labels = mkOption {
          type = types.attrsOf types.attrs;
          default = { };
          description = "Labels built with gfty-label.mkLabel.";
        };
        plates = mkOption {
          type = types.attrsOf types.attrs;
          default = { };
          description = "Plates built with gfty-label.mkPlate; label names resolve from gfty-label.labels.";
        };
      };

      config.packages = generatedPackages;
    }
  );
}
