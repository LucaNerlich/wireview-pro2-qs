use serde::Serialize;

/// The rendered state of the WireView Pro II.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// App is running and reports a power reading.
    Live,
    /// App is running but reports no reading (e.g. tray power display off).
    Na,
    /// App is not running.
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
}

impl Status {
    pub fn off() -> Self {
        Self {
            state: State::Off,
            watts: None,
            title: None,
        }
    }

    /// Parse a status from the app's SNI `Title` property.
    ///
    /// Returns `None` when the title does not belong to the WireView app.
    pub fn from_title(title: Option<&str>) -> Option<Self> {
        let title = title.map(str::trim).filter(|t| !t.is_empty())?;

        if let Some(rest) = title.strip_prefix("WireView Pro II - ") {
            if let Some(number) = rest.strip_suffix(" W") {
                if let Ok(watts) = number.parse::<f64>() {
                    // "NaN" / "inf" parse as f64 but are not real readings;
                    // accepting them would serialize NaN as null and make
                    // the watch stream re-emit the line every poll.
                    if watts.is_finite() {
                        return Some(Self {
                            state: State::Live,
                            watts: Some(watts),
                            title: Some(title.to_string()),
                        });
                    }
                }
            }
        }

        if title.starts_with("WireView Pro II") {
            return Some(Self {
                state: State::Na,
                watts: None,
                title: Some(title.to_string()),
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
        let s = Status::from_title(Some("WireView Pro II - 43 W")).unwrap();
        let json = serde_json::to_string(&s).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["state"], "live");
        assert_eq!(parsed["watts"], 43.0);
        assert_eq!(parsed["title"], "WireView Pro II - 43 W");
    }

    #[test]
    fn serializes_off_without_optional_fields() {
        let json = serde_json::to_string(&Status::off()).unwrap();
        assert_eq!(json, r#"{"state":"off"}"#);
    }
}
