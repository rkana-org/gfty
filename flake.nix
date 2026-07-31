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
          projectName = "gfty";
        in
        {
          devshells.default = {
            packages = [
              config.treefmt.build.wrapper
              pkgs.esbuild
              pkgs.live-server
              pkgs.rust-analyzer
            ];
            commands = [
              {
                name = "designer-dev";
                help = "serve the Gridfinity Ultimate designer with live reload";
                command = ''exec live-server --port 8080 "$PRJ_ROOT/gridfinity-ultimate/designer"'';
              }
              {
                name = "designer-preview";
                help = "build and serve the production designer";
                command = ''
                  nix build "$PRJ_ROOT#designer"
                  exec live-server --port 8081 "$PRJ_ROOT/result"
                '';
              }
            ];
            devshell.startup.pre-commit.text = config.pre-commit.installationScript;
            env = [
              {
                name = "RUST_SRC_PATH";
                value = "${pkgs.rustPlatform.rustLibSrc}";
              }
              {
                name = "GFTY_FONT_DIRS";
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

          checks.bin-designer-conformance =
            pkgs.runCommand "bin-designer-conformance"
              {
                nativeBuildInputs = [ pkgs.nodejs ];
              }
              ''
                node ${./tests/bin-designer-conformance.js} \
                  ${./gridfinity-ultimate/designer/logic.js} \
                  ${./tests/fixtures/bin/default.json}
                touch "$out"
              '';

          packages = {
            default = pkgs.callPackage ./package.nix { };
            designer = pkgs.callPackage ./gridfinity-ultimate/nix/designer.nix { };
          };
          overlayAttrs = {
            gfty = config.packages.default;
            gfty-label = config.packages.default;
          };
        };
    };
}
