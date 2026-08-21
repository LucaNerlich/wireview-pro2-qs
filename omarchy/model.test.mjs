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
  appRunning: false,
  sensors: null
}, "live without watts stays live");
eq(Model.parseLine('{"state":"live","watts":43.2,"title":"WireView Pro II - 43.2 W"}'), {
  state: "live",
  watts: 43.2,
  title: "WireView Pro II - 43.2 W",
  appRunning: false,
  sensors: null
}, "live with watts");
eq(Model.parseLine('{"state":"live","watts":43.2,"appRunning":true}'), {
  state: "live",
  watts: 43.2,
  title: "",
  appRunning: true,
  sensors: null
}, "appRunning true");
eq(Model.parseLine('{"state":"na","title":"WireView Pro II"}'), {
  state: "na",
  watts: NaN,
  title: "WireView Pro II",
  appRunning: false,
  sensors: null
}, "na carries title");
eq(Model.parseLine('{"state":"off"}'), {
  state: "off",
  watts: NaN,
  title: "",
  appRunning: false,
  sensors: null
}, "off");
eq(Model.parseLine('{"state":"weird"}'), null, "unknown state rejected");
eq(Model.parseLine('{"state":"live","watts":"43"}'), {
  state: "live",
  watts: NaN,
  title: "",
  appRunning: false,
  sensors: null
}, "non-numeric watts ignored");
eq(Model.parseLine('{"state":"live","watts":43,"title":"<img src=x onerror=alert(1)>"}'), {
  state: "live",
  watts: 43,
  title: "",
  appRunning: false,
  sensors: null
}, "html title dropped");
eq(Model.parseLine('{"state":"na","title":"WireView Pro II <b>x</b>"}'), {
  state: "na",
  watts: NaN,
  title: "",
  appRunning: false,
  sensors: null
}, "html na title dropped");
eq(Model.parseLine('{"state":"na","title":"WireView Pro II &amp; tray"}'), {
  state: "na",
  watts: NaN,
  title: "",
  appRunning: false,
  sensors: null
}, "entity title dropped");
eq(Model.safeTitle("WireView Pro II - 43.2 W"), "WireView Pro II - 43.2 W", "plain title kept");
eq(Model.safeTitle("<b>x</b>"), "", "safeTitle strips tags");

const sensorsLine = Model.parseLine(
  '{"state":"live","watts":108.05,"appRunning":false,"sensors":{"voltageV":[12.0,12.1],"currentA":[1.5,1.6],"powerW":[18.0,19.36],"sumCurrentA":3.1,"sumPowerW":108.05,"tempInC":34.5,"tempOutC":null,"ext1C":null,"ext2C":null,"fanDuty":75,"faultStatus":0,"faultLog":0,"psuCapW":600}}'
);
eq(sensorsLine.state, "live", "sensors line keeps state");
eq(sensorsLine.watts, 108.05, "sensors line keeps watts");
eq(sensorsLine.appRunning, false, "hwmon-only is not the GUI");
assert.deepStrictEqual(sensorsLine.sensors.voltageV, [12.0, 12.1], "sensors voltage array");
eq(sensorsLine.sensors.sumCurrentA, 3.1, "sensors sum current");
eq(sensorsLine.sensors.fanDuty, 75, "fan duty");

// hasSensors / hasLiveFault / appLine

eq(Model.hasSensors({ sensors: { voltageV: [], currentA: [] } }), true, "sensors present");
eq(Model.hasSensors({ sensors: null }), false, "null sensors");
eq(Model.hasSensors({}), false, "missing sensors");
eq(Model.hasLiveFault({ sensors: { faultStatus: 0 } }), false, "no live fault");
eq(Model.hasLiveFault({ sensors: { faultStatus: 0x20 } }), true, "imbalance fault");
eq(Model.hasLiveFault({ sensors: { faultStatus: 0, currentA: [9, 1, 1, 1, 1, 1] } }), true, "computed imbalance is a live fault");
eq(Model.appLine({ appRunning: true }), "running", "app running");
eq(Model.appLine({ appRunning: false, sensors: { voltageV: [], currentA: [] } }), "daemon only", "hwmon without GUI");
eq(Model.appLine({ appRunning: false }), "not running", "nothing running");

// fmt / fmtTemp / fmtFan / fmtFault / namedFaults

eq(Model.fmt(9.056, 2), "9.06", "fmt rounds");
eq(Model.fmt(null), "\u2014", "fmt null");
eq(Model.fmt(NaN), "\u2014", "fmt nan");
eq(Model.fmtTemp(34.56), "34.6 \u00B0C", "temp");
eq(Model.fmtTemp(null), "\u2014", "temp null");
eq(Model.fmtFan(75.4), "75 %", "fan");
eq(Model.fmtFan(null), "\u2014", "fan null");
eq(Model.fmtFault(0), "none", "no fault");
eq(Model.fmtFault(0x8001), "0x8001", "fault hex");
eq(Model.fmtFaultNames(0), "none", "named none");
eq(Model.fmtFaultNames(0x04), "Over-Current", "named OCP");
eq(Model.fmtFaultNames(0x21), "Chip Over-Temp, Current Imbalance", "named combo");
eq(Model.fmtFaultNames(0x0040), "0x0040", "unknown bit stays hex");
assert.deepStrictEqual(Model.namedFaults(0x08), ["Wire Over-Current"], "wire OCP");

// pinStats / pinPower / imbalance

const balanced = { currentA: [1.5, 1.52, 1.48, 1.55, 1.49, 1.51], voltageV: [12, 12, 12, 12, 12, 12] };
const statsOk = Model.pinStats(balanced);
eq(statsOk.warn, false, "low load is not an imbalance alarm");
eq(statsOk.maxIndex, 3, "hottest pin index");

const imbalanced = { currentA: [9.0, 1.0, 1.0, 1.0, 1.0, 1.0] };
const statsWarn = Model.pinStats(imbalanced);
eq(statsWarn.warn, true, "6A/40% rule trips");
eq(statsWarn.maxIndex, 0, "pin 1 hottest");
eq(Model.pinIsHot(imbalanced, 0), true, "hot pin flagged");
eq(Model.pinIsHot(imbalanced, 1), false, "other pins not flagged");
assert.ok(Model.imbalanceLine(imbalanced).includes("Imbalance"), "warn line");
assert.ok(Model.imbalanceLine(balanced).includes("Spread"), "ok line");

eq(Model.pinPower({ powerW: [18.1, 19] }, 0), 18.1, "chip pin power");
eq(Model.pinPower({ voltageV: [12, 12], currentA: [1.5, 2] }, 1), 24, "V×I fallback");

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
  Model.tooltipText({ state: "live", watts: 43.2, appRunning: true }).includes("43.2 W"),
  "live tooltip carries watts"
);
assert.ok(
  Model.tooltipText({ state: "off", appRunning: false }).includes("not running"),
  "off tooltip mentions app"
);
assert.ok(
  Model.tooltipText({
    state: "live",
    watts: 200,
    appRunning: false,
    sensors: { faultStatus: 0x20, currentA: [9, 1, 1, 1, 1, 1], fanDuty: 80 }
  }).includes("Current Imbalance"),
  "tooltip names live faults"
);

// stateLine

assert.ok(Model.stateLine({ state: "live", watts: 7 }).includes("7 W"), "live state line");
assert.ok(Model.stateLine({ state: "na" }).includes("no reading"), "na state line");
assert.ok(Model.stateLine({ state: "off" }).includes("not running"), "off state line");
assert.ok(
  Model.stateLine({ state: "live", watts: 50, sensors: { faultStatus: 0x10 } }).includes("Over-Power"),
  "fault overrides the power line"
);
assert.ok(
  Model.stateLine({ state: "live", watts: 50, sensors: { faultStatus: 0, currentA: [9, 1, 1, 1, 1, 1] } }).includes("Current Imbalance"),
  "computed imbalance with faultStatus 0 displays alert text"
);
assert.ok(
  Model.stateLine({ state: "live", watts: 50, sensors: { faultStatus: 0, currentA: [9, 1, 1, 1, 1, 1] } }).includes("pin 1"),
  "computed imbalance stateLine includes pin details"
);

// faultAlert / notifyCommand

eq(Model.faultAlert({}), null, "no sensors is not an alert");
eq(Model.faultAlert({ sensors: { faultStatus: 0 } }), null, "zero mask is not an alert");
eq(Model.faultAlert({ sensors: { faultStatus: 0x04 } }).headline, "WireView Pro II fault", "ocp headline");
eq(Model.faultAlert({ sensors: { faultStatus: 0x04 } }).body, "Over-Current", "ocp body");
eq(Model.faultAlert({ sensors: { faultStatus: 0x04 } }).key, "Over-Current", "ocp key");
eq(Model.faultAlert({ sensors: { faultStatus: 0x20, currentA: [9, 1, 1, 1, 1, 1] } }).key,
  "Current Imbalance@0", "device bit is not duplicated");
assert.ok(
  Model.faultAlert({ sensors: { faultStatus: 0, currentA: [9, 1, 1, 1, 1, 1] } }).body.indexOf("pin 1") !== -1,
  "computed imbalance names the hot pin"
);
assert.ok(
  Model.faultAlert({ sensors: { faultStatus: 0, currentA: [9, 1, 1, 1, 1, 1] } }).key !==
    Model.faultAlert({ sensors: { faultStatus: 0, currentA: [1, 1, 1, 1, 1, 9] } }).key,
  "a moved hot pin changes the alert key"
);
eq(Model.faultAlert({ sensors: { faultStatus: 0x04 } }).key.indexOf("@"), -1,
  "non-imbalance alerts carry no pin suffix");

const notify = Model.notifyCommand(Model.faultAlert({ sensors: { faultStatus: 0x10 } }));
eq(notify[0], "omarchy-notification-send", "goes through omarchy.notifications");
assert.ok(notify.indexOf("critical") !== -1, "critical urgency");
assert.ok(notify.indexOf("omarchy-shell shell summon luca.wireview-pro2 '{}'") !== -1, "click opens panel");
eq(Model.notifyCommand(null), [], "null alert is empty argv");

console.log("model.test.mjs: all assertions passed");
