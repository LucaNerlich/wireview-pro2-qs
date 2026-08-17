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
    title: typeof parsed.title === "string" ? parsed.title : "",
    sensors: parsed.sensors || null
  };
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
  if (status.state === "live")
    return "WireView Pro II \u2014 " + formatWatts(status.watts) + " W\nClick to open the app";
  if (status.state === "na") return "WireView Pro II \u2014 no reading";
  return "WireView Pro II \u2014 app not running\nClick to start it";
}

function stateLine(status) {
  if (!status) return "unknown";
  if (status.state === "live") return "Power draw: " + formatWatts(status.watts) + " W";
  if (status.state === "na") return "App running \u2014 no reading";
  return "App not running";
}

// True when the backend supplied a full hwmon sensor snapshot.
function hasSensors(status) {
  var s = status && status.sensors;
  return !!s && typeof s === "object"
    && Array.isArray(s.voltageV) && Array.isArray(s.currentA);
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

function fmtFault(bits) {
  if (bits === null || bits === undefined) return "none";
  var n = Number(bits);
  if (!isFinite(n) || n === 0) return "none";
  return "0x" + n.toString(16).toUpperCase().padStart(4, "0");
}

if (typeof module !== "undefined" && module.exports) {
  module.exports = {
    parseLine, formatWatts, labelText, tooltipText, stateLine,
    hasSensors, fmt, fmtTemp, fmtFault
  };
}
