let
  gftyUltimate = import ./gfty-ultimate.nix;
in
{
  perSystem = {
    gfty-label = {
      labels.module-example = {
        gfty-ultimate = gftyUltimate 1 1;
        template = ./templates/1x1.svg;
        text.top = "Hi";
        text.bottom = "Hi";
      };
      plates.module-example = {
        gfty-ultimate = gftyUltimate 1 1;
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
  };
}
