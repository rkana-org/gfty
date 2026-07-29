{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    gfty-label = {
      url = "path:../";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.gfty-label.flakeModules.default ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem =
        { system, pkgs, ... }:
        let
          screws = pkgs.gfty-label.mkLabel {
            name = "screws-label";
            template = ./templates/basic-label.svg;
            fonts = [ ];
            filament = 0;
            icons.fasteners = [
              ./icons/bolt.svg
              { spacer = "1mm"; }
              ./icons/nut.svg
            ];
            text.main = "M{3}x[10]";
          };

          nuts = pkgs.gfty-label.mkLabel {
            name = "nuts-label";
            template = ./templates/basic-label.svg;
            filament = 0;
            icons.fasteners = [ ./icons/nut.svg ];
            text.main = "M{4}";
          };
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.gfty-label.overlays.default ];
          };

          gfty-label = {
            labels.module-example = {
              template = ./templates/basic-label.svg;
              filament = 0;
              text.main = "Defined with the flake-parts module";
            };
            plates.module-example = {
              dimensions = [
                "100mm"
                "50mm"
              ];
              labels = [
                "module-example"
                "module-example"
              ];
            };
          };

          packages = {
            inherit screws nuts;
            plate = pkgs.gfty-label.mkPlate {
              name = "fastener-plate";
              dimensions = [
                "200mm"
                "250mm"
              ];
              labels = [
                screws
                screws
                nuts
              ];
            };
            default = screws;
          };
        };
    };
}
