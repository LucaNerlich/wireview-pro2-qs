use serde::Serialize;

use crate::hwmon::Sensors;

/// The rendered state of the WireView Pro II.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Device (or SNI title) reports a power reading.
    Live,
    /// App is running but reports no reading (e.g. tray power display off).
    Na,
    /// No device reading and no SNI title.
    Off,
}

/// A single status report, serialized as one JSON line for the QML frontend.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watts: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Whether `/usr/bin/wireview-linux` is running for this user.
    /// Independent of [`State`]: a live hwmon chip can exist without the GUI.
    #[serde(rename = "appRunning")]
    pub app_running: bool,
    /// Full per-pin/temperature/fault data, present only when read from the
    /// `wireview` hwmon chip rather than the app's SNI title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensors: Option<Sensors>,
}

impl Status {
    /// Creates a status indicating that no device reading or SNI title is available.
    ///
    /// # Examples
    ///
    /// ```
    /// let status = Status::off();
    /// assert!(matches!(status.state, State::Off));
    /// ```
    pub fn off() -> Self {
        Self {
            state: State::Off,
            watts: None,
            title: None,
            app_running: false,
            sensors: None,
        }
    }

    /// Sets the application-running status while preserving the device reading.
    ///
    /// # Examples
    ///
    /// ```
    /// let status = Status::off().with_app_running(true);
    /// assert!(status.app_running);
    /// ```
    pub fn with_app_running(mut self, running: bool) -> Self {
        self.app_running = running;
        self
    }

    /// Builds a live status from a hardware-monitoring sensor snapshot.
    ///
    /// # Examples
    ///
    /// ```
    /// let sensors = Sensors::default();
    /// let status = Status::from_sensors(&sensors);
    ///
    /// assert!(matches!(status.state, State::Live));
    /// ```
    pub fn from_sensors(sensors: &Sensors) -> Self {
        let watts = sensors.sum_power_w;
        Self {
            state: State::Live,
            watts: Some(watts),
            title: Some(format!("WireView Pro II - {watts:.1} W")),
            app_running: false,
            sensors: Some(sensors.clone()),
        }
    }

    /// Creates a status from the application's SNI title.
    ///
    /// Titles in the `WireView Pro II - <watts> W` format produce a live status.
    /// Other titles beginning with `WireView Pro II` produce a status without a
    /// reading. Missing, empty, unrelated, malformed, or non-finite readings are
    /// rejected.
    ///
    /// # Arguments
    ///
    /// * `title` - The optional SNI title to parse.
    ///
    /// # Returns
    ///
    /// `Some` status when the title identifies the application, or `None` when it
    /// does not.
    ///
    /// # Examples
    ///
    /// ```
    /// let status = Status::from_title(Some("WireView Pro II - 12.5 W"));
    ///
    /// assert!(status.is_some());
    /// ```
    pub fn from_title(title: Option<&str>) -> Option<Self> {
        let title = title.map(str::trim).filter(|t| !t.is_empty())?;

        // "NaN" / "inf" parse as f64 but are not real readings; accepting
        // them would serialize NaN as null and make the watch stream re-emit
        // the line every poll.
        if let Some(rest) = title.strip_prefix("WireView Pro II - ")
            && let Some(number) = rest.strip_suffix(" W")
            && let Ok(watts) = number.parse::<f64>()
            && watts.is_finite()
        {
            return Some(Self {
                state: State::Live,
                watts: Some(watts),
                title: Some(title.to_string()),
                app_running: false,
                sensors: None,
            });
        }

        if title.starts_with("WireView Pro II") {
            return Some(Self {
                state: State::Na,
                watts: None,
                title: Some(title.to_string()),
                app_running: false,
                sensors: None,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_live_reading() {
        let s = Status::from_title(Some("WireView Pro II - 43 W")).unwrap();
        assert_eq!(s.state, State::Live);
        assert_eq!(s.watts, Some(43.0));
        assert_eq!(s.title.as_deref(), Some("WireView Pro II - 43 W"));
        assert!(!s.app_running);
    }

    #[test]
    fn parses_fractional_reading() {
        let s = Status::from_title(Some("WireView Pro II - 12.5 W")).unwrap();
        assert_eq!(s.state, State::Live);
        assert_eq!(s.watts, Some(12.5));
    }

    #[test]
    fn parses_plain_title_as_na() {
        let s = Status::from_title(Some("WireView Pro II")).unwrap();
        assert_eq!(s.state, State::Na);
        assert_eq!(s.watts, None);
    }

    #[test]
    fn rejects_other_titles() {
        assert!(Status::from_title(Some("Steam")).is_none());
        assert!(Status::from_title(None).is_none());
        assert!(Status::from_title(Some("")).is_none());
        assert!(Status::from_title(Some("   ")).is_none());
    }

    #[test]
    fn rejects_malformed_reading() {
        let s = Status::from_title(Some("WireView Pro II - abc W")).unwrap();
        assert_eq!(s.state, State::Na);
    }

    #[test]
    fn rejects_non_finite_reading() {
        for title in [
            "WireView Pro II - NaN W",
            "WireView Pro II - inf W",
            "WireView Pro II - -inf W",
            "WireView Pro II - 1e999 W",
        ] {
            let s = Status::from_title(Some(title)).unwrap();
            assert_eq!(s.state, State::Na, "{title}");
            assert_eq!(s.watts, None, "{title}");
        }
    }

    #[test]
    fn serializes_live_as_json() {
        let s = Status::from_title(Some("WireView Pro II - 43 W"))
            .unwrap()
            .with_app_running(true);
        let json = serde_json::to_string(&s).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["state"], "live");
        assert_eq!(parsed["watts"], 43.0);
        assert_eq!(parsed["title"], "WireView Pro II - 43 W");
        assert_eq!(parsed["appRunning"], true);
    }

    #[test]
    fn serializes_off_without_optional_fields() {
        let json = serde_json::to_string(&Status::off()).unwrap();
        assert_eq!(json, r#"{"state":"off","appRunning":false}"#);
    }

    #[test]
    fn overlays_app_running_on_a_hwmon_reading() {
        let sensors = crate::hwmon::Sensors {
            voltage_v: [12.0; 6],
            current_a: [1.5; 6],
            power_w: [18.0; 6],
            sum_current_a: 9.0,
            sum_power_w: 108.0,
            temp_in_c: Some(34.5),
            temp_out_c: None,
            ext1_c: None,
            ext2_c: None,
            fan_duty: Some(75),
            voltage_avg_v: Some(12.0),
            fault_status: 0,
            fault_log: 1,
            psu_cap_w: Some(600),
        };
        let s = Status::from_sensors(&sensors).with_app_running(false);
        assert_eq!(s.state, State::Live);
        assert!(!s.app_running);
        assert_eq!(s.watts, Some(108.0));
    }

    #[test]
    fn from_sensors_builds_live_status_with_sensors() {
        let sensors = crate::hwmon::Sensors {
            voltage_v: [12.0; 6],
            current_a: [1.5; 6],
            power_w: [18.0; 6],
            sum_current_a: 9.0,
            sum_power_w: 108.0,
            temp_in_c: Some(34.5),
            temp_out_c: None,
            ext1_c: None,
            ext2_c: None,
            fan_duty: None,
            voltage_avg_v: None,
            fault_status: 0,
            fault_log: 1,
            psu_cap_w: Some(600),
        };
        let s = Status::from_sensors(&sensors);
        assert_eq!(s.state, State::Live);
        assert_eq!(s.watts, Some(108.0));

        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["state"], "live");
        assert_eq!(json["watts"], 108.0);
        assert_eq!(json["appRunning"], false);
        assert_eq!(json["sensors"]["sumPowerW"], 108.0);
        assert_eq!(json["sensors"]["tempInC"], 34.5);
        assert!(json["sensors"]["tempOutC"].is_null());
        assert_eq!(json["sensors"]["faultLog"], 1);
        assert_eq!(json["sensors"]["psuCapW"], 600);
        assert_eq!(json["sensors"]["powerW"][0], 18.0);
    }
}
