{
  perSystem = {
    gfty = {
      bins.module-example = {
        size = [
          1
          1
          6
        ];
        divider = {
          columns = [ "auto" ];
          rows = [ "auto" ];
        };
      };
      labels.module-example = {
        bin = "module-example";
        template = ./templates/1x1.svg;
        text.top = "Hi";
        text.bottom = "Hi";
      };
      plates.module-example = {
        bin = "module-example";
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
