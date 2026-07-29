{
  inputs = {
    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    nci = {
      url = "github:90-008/nix-cargo-integration";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.devshell.flakeModule
        inputs.flake-parts.flakeModules.easyOverlay
        inputs.nci.flakeModule
        inputs.pre-commit-hooks.flakeModule
        inputs.treefmt-nix.flakeModule
      ];

      flake.flakeModules.default = import ./flake-module.nix { inherit (inputs) self; };

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      perSystem =
        {
          config,
          pkgs,
          ...
        }:
        let
          projectName = "gfty-label";
        in
        {
          devshells.default = {
            packages = [
              config.treefmt.build.wrapper
              pkgs.rust-analyzer
            ];
            devshell.startup.pre-commit.text = config.pre-commit.installationScript;
            env = [
              {
                name = "RUST_SRC_PATH";
                value = "${pkgs.rustPlatform.rustLibSrc}";
              }
              {
                name = "GFTY_LABEL_FONT_DIRS";
                value = "${pkgs.dejavu_fonts}:${pkgs.liberation_ttf}:${pkgs.jetbrains-mono}";
              }
            ];
          };

          pre-commit.settings.hooks.treefmt.enable = true;
          treefmt = {
            projectRootFile = "flake.nix";
            programs = {
              deadnix.enable = true;
              statix.enable = true;
              nixfmt.enable = true;
              rustfmt.enable = true;
            };
          };

          nci.projects.${projectName} = {
            path = ./.;
            numtideDevshell = "default";
          };

          packages.default = pkgs.callPackage ./package.nix { };
          overlayAttrs.gfty-label = config.packages.default;
        };
    };
}
