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
  title: "",
  sensors: null
}, "live without watts stays live");
eq(Model.parseLine('{"state":"live","watts":43.2,"title":"WireView Pro II - 43.2 W"}'), {
  state: "live",
  watts: 43.2,
  title: "WireView Pro II - 43.2 W",
  sensors: null
}, "live with watts");
eq(Model.parseLine('{"state":"na","title":"WireView Pro II"}'), {
  state: "na",
  watts: NaN,
  title: "WireView Pro II",
  sensors: null
}, "na carries title");
eq(Model.parseLine('{"state":"off"}'), {
  state: "off",
  watts: NaN,
  title: "",
  sensors: null
}, "off");
eq(Model.parseLine('{"state":"weird"}'), null, "unknown state rejected");
eq(Model.parseLine('{"state":"live","watts":"43"}'), {
  state: "live",
  watts: NaN,
  title: "",
  sensors: null
}, "non-numeric watts ignored");
eq(Model.parseLine('{"state":"live","watts":43,"title":"<img src=x onerror=alert(1)>"}'), {
  state: "live",
  watts: 43,
  title: "",
  sensors: null
}, "html title dropped");
eq(Model.parseLine('{"state":"na","title":"WireView Pro II <b>x</b>"}'), {
  state: "na",
  watts: NaN,
  title: "",
  sensors: null
}, "html na title dropped");
eq(Model.parseLine('{"state":"na","title":"WireView Pro II &amp; tray"}'), {
  state: "na",
  watts: NaN,
  title: "",
  sensors: null
}, "entity title dropped");
eq(Model.safeTitle("WireView Pro II - 43.2 W"), "WireView Pro II - 43.2 W", "plain title kept");
eq(Model.safeTitle("<b>x</b>"), "", "safeTitle strips tags");

const sensorsLine = Model.parseLine(
  '{"state":"live","watts":108.05,"sensors":{"voltageV":[12.0,12.1],"currentA":[1.5,1.6],"sumCurrentA":3.1,"sumPowerW":108.05,"tempInC":34.5,"tempOutC":null,"ext1C":null,"ext2C":null,"faultStatus":0,"faultLog":0,"psuCapW":600}}'
);
eq(sensorsLine.state, "live", "sensors line keeps state");
eq(sensorsLine.watts, 108.05, "sensors line keeps watts");
assert.deepStrictEqual(sensorsLine.sensors.voltageV, [12.0, 12.1], "sensors voltage array");
eq(sensorsLine.sensors.sumCurrentA, 3.1, "sensors sum current");

// hasSensors

eq(Model.hasSensors({ sensors: { voltageV: [], currentA: [] } }), true, "sensors present");
eq(Model.hasSensors({ sensors: null }), false, "null sensors");
eq(Model.hasSensors({}), false, "missing sensors");

// fmt / fmtTemp / fmtFault

eq(Model.fmt(9.056, 2), "9.06", "fmt rounds");
eq(Model.fmt(null), "\u2014", "fmt null");
eq(Model.fmt(NaN), "\u2014", "fmt nan");
eq(Model.fmtTemp(34.56), "34.6 \u00B0C", "temp");
eq(Model.fmtTemp(null), "\u2014", "temp null");
eq(Model.fmtFault(0), "none", "no fault");
eq(Model.fmtFault(0x8001), "0x8001", "fault hex");

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
