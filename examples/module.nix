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
      bins.bin-only = {
        size = [
          1
          1
          6
        ];
        rimInterface.mode = "off";
        labelInterface.mode = "off";
        divider = {
          columns = [ "auto" ];
          rows = [ "auto" ];
        };
      };
      bases.module-example = {
        size = [
          1
          1
        ];
        magnets.enabled = true;
        magnets.connectorCutouts = true;
      };
      rims.module-example = {
        size = [
          1
          1
        ];
        springCompensation = true;
      };
      swappableLabels.module-example = {
        bin = "module-example";
      };
      binSets.module-example = {
        bin = "module-example";
        base = "module-example";
        rim = "module-example";
        swappableLabel = "module-example";
        connectorPin = true;
      };
      labels.module-example = {
        bin = "module-example";
        template = ./templates/1x1.svg;
        text.top = "Hi";
        text.bottom = "Hi";
        icons.main = [
          ./icons/square.svg
          { spacer = "1mm"; }
          { icon = ./icons/square.svg; }
        ];
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
