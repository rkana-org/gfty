{ inputs, ... }: {
  imports = [
    inputs.devshell.flakeModule
    inputs.pre-commit-hooks.flakeModule
  ];

  perSystem =
    {
      config,
      pkgs,
      ...
    }:
    {
      pre-commit.settings.hooks = {
        nixfmt-rfc-style = {
          enable = true;
          package = pkgs.nixfmt;
        };
        deadnix.enable = true;
        statix.enable = true;
      };

      devshells.default = {
        packages = [
          # Used by `nix build`; available here for one-off experiments.
          pkgs.esbuild
          pkgs.live-server
          # Allow running `pre-commit run -a` manually.
          config.pre-commit.settings.package
        ];

        commands = [
          {
            name = "dev";
            help = "serve designer/ with live reload (in-browser JSX, no build step)";
            command = ''exec live-server --port 8080 "$PRJ_ROOT/designer"'';
          }
          {
            name = "preview";
            help = "build the production site and serve it";
            command = ''
              nix build "$PRJ_ROOT"
              exec live-server --port 8081 "$PRJ_ROOT/result"
            '';
          }
        ];

        devshell.startup.pre-commit.text = config.pre-commit.installationScript;
      };

      # `nix fmt`
      formatter = pkgs.nixfmt;
    };
}
