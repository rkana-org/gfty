#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const [logicPath, fixturePath] = process.argv.slice(2);
if (!logicPath || !fixturePath) {
  throw new Error("usage: bin-designer-conformance.js LOGIC.js EXPECTED.json");
}

const context = { window: {} };
vm.createContext(context);
vm.runInContext(fs.readFileSync(logicPath, "utf8"), context, {
  filename: path.basename(logicPath),
});

const actual = JSON.parse(
  context.window.GF.toMinified(
    context.window.GF.defaultFlat(),
    context.window.GF.defaultDivider(),
  ),
);
const expected = JSON.parse(fs.readFileSync(fixturePath, "utf8"));
assert.deepStrictEqual(actual, expected);

const defaults = context.window.GF.defaultFlat();
const defaultDivider = context.window.GF.defaultDivider();
const tomlFiles = context.window.GF.toTomlFiles(defaults, defaultDivider);
assert.deepStrictEqual(
  Array.from(tomlFiles, (file) => file.name),
  ["bin.toml", "base.toml", "rim.toml", "swappable-label.toml", "bin-set.toml"],
);
const toml = Object.fromEntries(Array.from(tomlFiles, (file) => [file.name, file.text]));
assert.match(toml["bin.toml"], /^kind = "bin"\nversion = 2\nsize = \[2, 2, 6\]/);
assert.match(toml["bin.toml"], /\[rim-interface\]\nmode = "swappable"/);
assert.match(toml["bin.toml"], /\[label-interface\]\nmode = "swappable"\ndepth = "10mm"\nsupports = "auto"/);
assert.match(toml["bin.toml"], /\[divider\]\ncolumns = \["auto", "auto", "auto"\]\nrows = \["auto", "auto"\]/);
assert.match(toml["base.toml"], /\[magnets\]\nenabled = true\nconnector-cutouts = true/);
assert.equal(
  toml["bin-set.toml"],
  `kind = "bin-set"
version = 1
bin = "bin.toml"
base = "base.toml"
rim = "rim.toml"
swappable-label = "swappable-label.toml"
connector-pin = true
`,
);

const nix = context.window.GF.toNix(defaults, defaultDivider);
for (const definition of [
  "bins.designed",
  "bases.designed",
  "rims.designed",
  "swappableLabels.designed",
  "binSets.designed",
]) {
  assert.match(nix, new RegExp(definition.replace(".", "\\.")));
}
assert.match(nix, /rimInterface\.mode = "swappable";/);
assert.match(nix, /connectorPin = true;/);

const baseOnly = Object.assign({}, defaults, { bin_enable: false });
assert.deepStrictEqual(
  Array.from(context.window.GF.toTomlFiles(baseOnly, defaultDivider), (file) => file.name),
  ["base.toml"],
);
const integrated = Object.assign({}, defaults, {
  base_enable: false,
  bin_nesting_swappable_rim_enable: false,
  bin_tub_label_is_swappable: false,
});
const integratedFiles = context.window.GF.toTomlFiles(integrated, defaultDivider);
assert.deepStrictEqual(
  Array.from(integratedFiles, (file) => file.name),
  ["bin.toml", "bin-set.toml"],
);
assert.match(integratedFiles[0].text, /\[rim-interface\]\nmode = "integrated"/);
assert.match(integratedFiles[0].text, /\[label-interface\]\nmode = "integrated"/);

const custom = Object.assign({}, defaults, { easygrab_mode: "custom" });
const customDivider = {
  columns: [context.window.GF.track("fixed", "21"), context.window.GF.track("frac", "2")],
  rows: [context.window.GF.track("auto")],
  merges: [],
  easygrab: [{ side: "south", cols: [0, 0], rows: [0, 0], radius: 12.5 }],
};
const customToml = context.window.GF.toTomlFiles(custom, customDivider)[0].text;
assert.match(customToml, /columns = \["21mm", "2fr"\]/);
assert.match(customToml, /\[\[easy-grab\.faces\]\]\nside = "south"\ncolumns = \[0, 0\]\nrows = \[0, 0\]\nradius = "12\.5mm"/);
const customNix = context.window.GF.toNix(custom, customDivider);
assert.match(customNix, /columns = \[ "21mm" "2fr" \];/);
assert.match(customNix, /radius = "12\.5mm";/);

const disabled = Object.assign({}, defaults, { base_enable: false, bin_enable: false });
assert.equal(context.window.GF.toTomlFiles(disabled, defaultDivider).length, 0);
assert.equal(
  context.window.GF.toNix(disabled, defaultDivider),
  "{\n  perSystem = {\n    gfty = { };\n  };\n}\n",
);
