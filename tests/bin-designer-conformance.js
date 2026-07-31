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
