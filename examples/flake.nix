{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    gfty-label.url = "path:../";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.gfty-label.flakeModules.default
        ./labels.nix
      ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
    };
}
