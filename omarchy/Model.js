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
    title: typeof parsed.title === "string" ? parsed.title : ""
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

if (typeof module !== "undefined" && module.exports) {
  module.exports = { parseLine, formatWatts, labelText, tooltipText, stateLine };
}
