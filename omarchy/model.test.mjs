import { strict as assert } from "node:assert";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const Model = require("./Model.js");

function eq(actual, expected, label) {
  assert.deepStrictEqual(actual, expected, label);
}

// parseLine

eq(Model.parseLine(""), null, "empty line");
eq(Model.parseLine("not json"), null, "garbage line");
eq(Model.parseLine('{"state":"live"}'), {
  state: "live",
  watts: NaN,
  title: ""
}, "live without watts stays live");
eq(Model.parseLine('{"state":"live","watts":43.2,"title":"WireView Pro II - 43.2 W"}'), {
  state: "live",
  watts: 43.2,
  title: "WireView Pro II - 43.2 W"
}, "live with watts");
eq(Model.parseLine('{"state":"na","title":"WireView Pro II"}'), {
  state: "na",
  watts: NaN,
  title: "WireView Pro II"
}, "na carries title");
eq(Model.parseLine('{"state":"off"}'), {
  state: "off",
  watts: NaN,
  title: ""
}, "off");
eq(Model.parseLine('{"state":"weird"}'), null, "unknown state rejected");
eq(Model.parseLine('{"state":"live","watts":"43"}'), {
  state: "live",
  watts: NaN,
  title: ""
}, "non-numeric watts ignored");

// formatWatts

eq(Model.formatWatts(43.2), "43.2", "one decimal");
eq(Model.formatWatts(43.25), "43.3", "rounds");
eq(Model.formatWatts(NaN), "?", "nan placeholder");

// labelText

eq(Model.labelText(null), "⚡ …", "null label");
eq(Model.labelText({ state: "live", watts: 43.2 }), "⚡ 43.2 W", "live label");
eq(Model.labelText({ state: "na" }), "⚡ — W", "na label");
eq(Model.labelText({ state: "off" }), "⚡ off", "off label");

// tooltipText

eq(Model.tooltipText(null), "WireView Pro II", "null tooltip");
assert.ok(
  Model.tooltipText({ state: "live", watts: 43.2 }).includes("43.2 W"),
  "live tooltip carries watts"
);
assert.ok(
  Model.tooltipText({ state: "off" }).includes("not running"),
  "off tooltip mentions app"
);

// stateLine

assert.ok(Model.stateLine({ state: "live", watts: 7 }).includes("7 W"), "live state line");
assert.ok(Model.stateLine({ state: "na" }).includes("no reading"), "na state line");
assert.ok(Model.stateLine({ state: "off" }).includes("not running"), "off state line");

console.log("model.test.mjs: all assertions passed");
