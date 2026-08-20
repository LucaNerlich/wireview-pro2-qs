// Pure parsing/formatting shared by BarWidget.qml and Panel.qml.
// Kept in plain JS so node can exercise it without a QML engine.

function parseLine(line) {
  var text = String(line || "").trim();
  if (text === "") return null;
  var parsed;
  try {
    parsed = JSON.parse(text);
  } catch (e) {
    return null;
  }
  if (parsed === null || typeof parsed !== "object") return null;

  var state = String(parsed.state || "");
  if (state !== "live" && state !== "na" && state !== "off") return null;

  var watts = NaN;
  if (state === "live" && typeof parsed.watts === "number" && isFinite(parsed.watts))
    watts = parsed.watts;

  return {
    state: state,
    watts: state === "live" ? watts : NaN,
    title: safeTitle(parsed.title),
    appRunning: parsed.appRunning === true,
    sensors: parsed.sensors || null
  };
}

// Qt Text defaults can treat a string that looks like HTML as rich text
// (Text.AutoText). The watch line is untrusted from QML's point of view
// (PATH fallback binary, a swapped SNI Title), so drop markup rather than
// let it reach PanelHero / Text.
function safeTitle(value) {
  if (typeof value !== "string") return "";
  if (value.indexOf("<") !== -1 || value.indexOf(">") !== -1 || value.indexOf("&") !== -1)
    return "";
  return value;
}

function formatWatts(watts) {
  var number = Number(watts);
  if (!isFinite(number)) return "?";
  return String(Math.round(number * 10) / 10);
}

function labelText(status) {
  if (!status) return "\u26A1 \u2026";
  if (status.state === "live") return "\u26A1 " + formatWatts(status.watts) + " W";
  if (status.state === "na") return "\u26A1 \u2014 W";
  return "\u26A1 off";
}

function tooltipText(status) {
  if (!status) return "WireView Pro II";
  var lines = [];
  if (status.state === "live")
    lines.push("WireView Pro II \u2014 " + formatWatts(status.watts) + " W");
  else if (status.state === "na")
    lines.push("WireView Pro II \u2014 no reading");
  else
    lines.push("WireView Pro II \u2014 app not running");

  var faults = namedFaults(status.sensors && status.sensors.faultStatus);
  if (faults.length)
    lines.push("Fault: " + faults.join(", "));

  var stats = pinStats(status.sensors);
  if (stats && stats.warn)
    lines.push("Pin imbalance " + fmt(stats.spreadPct, 0) + "% (pin " + (stats.maxIndex + 1) + ")");

  if (status.sensors && status.sensors.fanDuty !== null && status.sensors.fanDuty !== undefined)
    lines.push("Fan " + fmtFan(status.sensors.fanDuty));

  if (status.state === "off" || !status.appRunning)
    lines.push("Click to start the app");
  else
    lines.push("Click to open the app");
  return lines.join("\n");
}

function stateLine(status) {
  if (!status) return "unknown";
  var alert = faultAlert(status);
  if (alert)
    return "Fault: " + alert.body;
  if (status.state === "live") return "Power draw: " + formatWatts(status.watts) + " W";
  if (status.state === "na") return "App running \u2014 no reading";
  return "App not running";
}

function appLine(status) {
  if (!status) return "unknown";
  if (status.appRunning) return "running";
  if (hasSensors(status)) return "daemon only";
  return "not running";
}

// True when the backend supplied a full hwmon sensor snapshot.
function hasSensors(status) {
  var s = status && status.sensors;
  return !!s && typeof s === "object"
    && Array.isArray(s.voltageV) && Array.isArray(s.currentA);
}

function hasLiveFault(status) {
  return faultAlert(status) !== null;
}

// High-importance toast for omarchy.notifications (via omarchy-notification-send).
// Live device fault bits, plus the firmware v03 pin-imbalance heuristic when
// that bit is not already set. Returns null when nothing is alarming.
function faultAlert(status) {
  var sensors = status && status.sensors;
  var names = namedFaults(sensors && sensors.faultStatus);
  var stats = pinStats(sensors);
  if (stats && stats.warn && names.indexOf("Current Imbalance") === -1)
    names = names.concat(["Current Imbalance"]);
  if (!names.length) return null;

  var body = names.join(", ");
  if (stats && stats.warn)
    body += " — pin " + (stats.maxIndex + 1) + " at " + fmt(stats.maxA, 1) + " A";

  return {
    key: names.join("|"),
    headline: "WireView Pro II fault",
    body: body
  };
}

// Argv for `omarchy-notification-send`. Critical urgency stays on screen
// (omarchy.notifications duration 0) and omarchy-action bypasses DND.
function notifyCommand(alert) {
  if (!alert) return [];
  return [
    "omarchy-notification-send",
    "-u", "critical",
    "-g", "\u26A1",
    "--exec", "omarchy-shell shell summon luca.wireview-pro2 '{}'",
    String(alert.headline || "WireView Pro II fault"),
    String(alert.body || "")
  ];
}

function fmt(value, digits) {
  if (value === null || value === undefined) return "\u2014";
  var n = Number(value);
  if (!isFinite(n)) return "\u2014";
  var factor = Math.pow(10, digits || 0);
  return String(Math.round(n * factor) / factor);
}

function fmtTemp(c) {
  if (c === null || c === undefined) return "\u2014";
  var n = Number(c);
  if (!isFinite(n)) return "\u2014";
  return fmt(n, 1) + " \u00B0C";
}

function fmtFan(duty) {
  if (duty === null || duty === undefined) return "\u2014";
  var n = Number(duty);
  if (!isFinite(n)) return "\u2014";
  return String(Math.round(n)) + " %";
}

function fmtFault(bits) {
  if (bits === null || bits === undefined) return "none";
  var n = Number(bits);
  if (!isFinite(n) || n === 0) return "none";
  return "0x" + n.toString(16).toUpperCase().padStart(4, "0");
}

// Bit names match WireViewPro2Device.FAULT in wireview-linux
// (1 << enum value). Unknown bits keep their hex remainder.
var FAULT_NAMES = [
  { bit: 0, name: "Chip Over-Temp" },
  { bit: 1, name: "Sensor Over-Temp" },
  { bit: 2, name: "Over-Current" },
  { bit: 3, name: "Wire Over-Current" },
  { bit: 4, name: "Over-Power" },
  { bit: 5, name: "Current Imbalance" }
];

function namedFaults(bits) {
  var n = Number(bits);
  if (!isFinite(n) || n === 0) return [];
  var names = [];
  var known = 0;
  for (var i = 0; i < FAULT_NAMES.length; i++) {
    var mask = 1 << FAULT_NAMES[i].bit;
    known |= mask;
    if (n & mask) names.push(FAULT_NAMES[i].name);
  }
  var extra = n & ~known;
  if (extra) names.push(fmtFault(extra));
  return names;
}

function fmtFaultNames(bits) {
  var names = namedFaults(bits);
  if (names.length === 0) return "none";
  return names.join(", ");
}

// Firmware v03 imbalance alarm: a pin at >= 6 A and (max-min)/max > 40%.
var IMBALANCE_MIN_A = 6;
var IMBALANCE_SPREAD_PCT = 40;

function pinStats(sensors) {
  if (!sensors || !Array.isArray(sensors.currentA) || sensors.currentA.length === 0)
    return null;
  var currents = sensors.currentA;
  var maxI = -Infinity;
  var minI = Infinity;
  var maxIndex = 0;
  var minIndex = 0;
  for (var i = 0; i < currents.length; i++) {
    var a = Number(currents[i]);
    if (!isFinite(a)) continue;
    if (a > maxI) { maxI = a; maxIndex = i; }
    if (a < minI) { minI = a; minIndex = i; }
  }
  if (!isFinite(maxI) || !isFinite(minI)) return null;
  var spreadPct = maxI > 0 ? ((maxI - minI) / maxI) * 100 : 0;
  return {
    maxIndex: maxIndex,
    minIndex: minIndex,
    maxA: maxI,
    minA: minI,
    spreadPct: spreadPct,
    warn: maxI >= IMBALANCE_MIN_A && spreadPct > IMBALANCE_SPREAD_PCT
  };
}

function pinPower(sensors, index) {
  if (sensors && Array.isArray(sensors.powerW) && index < sensors.powerW.length)
    return sensors.powerW[index];
  if (!sensors || !Array.isArray(sensors.voltageV) || !Array.isArray(sensors.currentA))
    return NaN;
  var v = Number(sensors.voltageV[index]);
  var a = Number(sensors.currentA[index]);
  if (!isFinite(v) || !isFinite(a)) return NaN;
  return v * a;
}

function imbalanceLine(sensors) {
  var stats = pinStats(sensors);
  if (!stats) return "";
  var pin = String(stats.maxIndex + 1);
  var pct = fmt(stats.spreadPct, 0);
  if (stats.warn) return "Imbalance " + pct + "% \u2014 pin " + pin + " hottest";
  return "Spread " + pct + "% \u2014 pin " + pin + " highest";
}

function pinIsHot(sensors, index) {
  var stats = pinStats(sensors);
  return !!(stats && stats.warn && index === stats.maxIndex);
}

if (typeof module !== "undefined" && module.exports) {
  module.exports = {
    parseLine, formatWatts, labelText, tooltipText, stateLine, appLine,
    hasSensors, hasLiveFault, fmt, fmtTemp, fmtFan, fmtFault, fmtFaultNames,
    namedFaults,     pinStats, pinPower, imbalanceLine, pinIsHot, safeTitle,
    faultAlert, notifyCommand
  };
}
