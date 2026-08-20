//! Read the WireView Pro II from the `wireview` hwmon chip.
//!
//! On Linux the wireview-linux project pairs the GUI with a headless
//! `wireviewd` daemon that exposes the device as a standard hwmon chip under
//! `/sys/class/hwmon/hwmon*/` (`name` == `wireview`). Reading it here gives
//! the full per-pin electrical data (voltage/current for all six 12VHPWR
//! pins), the four temperature channels, fault bitmasks, and the PSU rating —
//! far more than the app's StatusNotifierItem `Title` (watts only).
//!
//! When the chip is absent (e.g. the GUI is talking to the device directly
//! over the serial port, which is exclusive), callers fall back to the SNI
//! title. The file layout and unit conversions mirror `HwmonDevice.cs`.

use std::ffi::CString;
use std::fs::{self, File};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Full sensor snapshot read from the `wireview` hwmon chip.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sensors {
    /// Per-pin voltage for the six 12VHPWR power pins, in volts.
    pub voltage_v: [f64; 6],
    /// Per-pin current for the six 12VHPWR power pins, in amperes.
    pub current_a: [f64; 6],
    /// Per-pin power for the six 12VHPWR power pins, in watts.
    pub power_w: [f64; 6],
    /// Total current, in amperes (`curr7_input` when present, else the pin sum).
    pub sum_current_a: f64,
    /// Total power, in watts (`power1_input` when present, else Σ V×I).
    pub sum_power_w: f64,
    /// Onboard intake temperature, in °C (`null` when not exposed).
    pub temp_in_c: Option<f64>,
    /// Onboard exhaust temperature, in °C (`null` when not exposed).
    pub temp_out_c: Option<f64>,
    /// External probe 1 temperature, in °C (`null` when not exposed).
    pub ext1_c: Option<f64>,
    /// External probe 2 temperature, in °C (`null` when not exposed).
    pub ext2_c: Option<f64>,
    /// Device fan duty cycle, 0–100 (`null` when not exposed).
    pub fan_duty: Option<u8>,
    /// Average of the six pin voltages, in volts (`null` when not exposed).
    pub voltage_avg_v: Option<f64>,
    /// Live fault status bitmask (`0` = no faults).
    pub fault_status: u16,
    /// Latched fault log bitmask (`0` = nothing logged).
    pub fault_log: u16,
    /// Rated PSU wattage selected on the device (`null` when not exposed).
    pub psu_cap_w: Option<u16>,
}

/// The sysfs location of the WireView hwmon chip, or `None` when no daemon
/// has registered one.
pub fn find_chip(base: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("hwmon") {
            continue;
        }
        let Ok(ident) = fs::read_to_string(path.join("name")) else {
            continue;
        };
        if ident.trim().eq_ignore_ascii_case("wireview") {
            return Some(path);
        }
    }
    None
}

fn open_dir(path: &Path) -> Option<OwnedFd> {
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: path is a valid C string; open fails with -1 on error.
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return None;
    }
    // SAFETY: fd is a newly opened directory we own.
    Some(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn read_entry(dirfd: &OwnedFd, name: &str) -> Option<Vec<u8>> {
    let name = CString::new(name).ok()?;
    // SAFETY: dirfd is an open directory; name is a valid C string.
    let fd = unsafe {
        libc::openat(
            dirfd.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return None;
    }
    // SAFETY: fd is a newly opened file we own.
    let mut file = unsafe { File::from_raw_fd(fd) };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn chip_name(dirfd: &OwnedFd) -> Option<String> {
    let buf = read_entry(dirfd, "name")?;
    Some(std::str::from_utf8(&buf).ok()?.trim().to_string())
}

/// Read every sensor file from a WireView hwmon chip. Returns `None` when the
/// chip has no live reading yet (the daemon exposes `in0_input` as the gate).
pub fn read_chip(chip: &Path) -> Option<Sensors> {
    read_chip_dir(&open_dir(chip)?)
}

fn read_chip_dir(dirfd: &OwnedFd) -> Option<Sensors> {
    // The app gates "connected" on `in0_input`; treat its absence as "the
    // daemon owns the device but is not feeding this chip right now".
    read_i64_at(dirfd, "in0_input")?;

    let mut voltage_v = [0.0f64; 6];
    let mut current_a = [0.0f64; 6];
    let mut power_w = [0.0f64; 6];
    for i in 0..6 {
        voltage_v[i] = read_int_or_at(dirfd, &format!("in{i}_input"), 0) as f64 / 1000.0;
        current_a[i] = read_int_or_at(dirfd, &format!("curr{}_input", i + 1), 0) as f64 / 1000.0;
        // power2..power7 are per-pin microwatts; fall back to V×I.
        power_w[i] = read_watts_at(dirfd, &format!("power{}_input", i + 2))
            .unwrap_or(voltage_v[i] * current_a[i]);
    }

    let sum_current_a =
        read_milli_at(dirfd, "curr7_input").unwrap_or_else(|| current_a.iter().sum());
    let sum_power_w = read_watts_at(dirfd, "power1_input")
        .unwrap_or_else(|| voltage_v.iter().zip(&current_a).map(|(v, a)| v * a).sum());

    Some(Sensors {
        voltage_v,
        current_a,
        power_w,
        sum_current_a,
        sum_power_w,
        temp_in_c: read_milli_at(dirfd, "temp1_input"),
        temp_out_c: read_milli_at(dirfd, "temp2_input"),
        ext1_c: read_milli_at(dirfd, "temp3_input"),
        ext2_c: read_milli_at(dirfd, "temp4_input"),
        fan_duty: read_fan_duty_at(dirfd, "fan1_input"),
        voltage_avg_v: read_milli_at(dirfd, "in6_input"),
        fault_status: read_int_or_at(dirfd, "fault_status_raw", 0) as u16,
        fault_log: read_int_or_at(dirfd, "fault_log_raw", 0) as u16,
        psu_cap_w: read_i64_at(dirfd, "psu_cap").map(map_psu_cap),
    })
}

/// Discover and read the WireView hwmon chip under `/sys/class/hwmon`.
///
/// After locating a `hwmon*` node whose `name` is `wireview`, the chip
/// directory is opened and `name` is re-read through that fd so a reused
/// `hwmonN` slot cannot feed another chip's sensors.
pub fn discover() -> Option<Sensors> {
    let path = find_chip(Path::new("/sys/class/hwmon"))?;
    let dirfd = open_dir(&path)?;
    let ident = chip_name(&dirfd)?;
    if !ident.eq_ignore_ascii_case("wireview") {
        return None;
    }
    read_chip_dir(&dirfd)
}

/// The `psu_cap` sysfs value encodes the rated wattage as a small enum.
fn map_psu_cap(value: i64) -> u16 {
    match value {
        0 => 600,
        1 => 450,
        2 => 300,
        3 => 150,
        _ => 0,
    }
}

fn read_i64_at(dirfd: &OwnedFd, name: &str) -> Option<i64> {
    let buf = read_entry(dirfd, name)?;
    std::str::from_utf8(&buf).ok()?.trim().parse().ok()
}

fn read_int_or_at(dirfd: &OwnedFd, name: &str, default: i64) -> i64 {
    read_i64_at(dirfd, name).unwrap_or(default)
}

/// Millidegree / millivolt / milliamp input to the SI unit, `None` when absent.
fn read_milli_at(dirfd: &OwnedFd, name: &str) -> Option<f64> {
    read_i64_at(dirfd, name).map(|v| v as f64 / 1000.0)
}

/// Microwatt hwmon power input to watts, `None` when absent/invalid.
fn read_watts_at(dirfd: &OwnedFd, name: &str) -> Option<f64> {
    read_i64_at(dirfd, name).map(|v| v as f64 / 1_000_000.0)
}

/// Fan duty 0–100 as reported by `fan1_input`. Out-of-range values are clamped.
fn read_fan_duty_at(dirfd: &OwnedFd, name: &str) -> Option<u8> {
    let v = read_i64_at(dirfd, name)?;
    Some(v.clamp(0, 100) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(base: &Path, files: &[(&str, &str)]) {
        fs::create_dir_all(base).unwrap();
        for (name, content) in files {
            fs::write(base.join(name), content).unwrap();
        }
    }

    fn chip(base: &Path) -> PathBuf {
        write(
            &base.join("hwmon0"),
            &[
                ("name", "wireview\n"),
                ("in0_input", "12050\n"),
                ("in1_input", "12080\n"),
                ("in2_input", "12020\n"),
                ("in3_input", "12100\n"),
                ("in4_input", "12040\n"),
                ("in5_input", "12060\n"),
                ("curr1_input", "1500\n"),
                ("curr2_input", "1520\n"),
                ("curr3_input", "1480\n"),
                ("curr4_input", "1550\n"),
                ("curr5_input", "1490\n"),
                ("curr6_input", "1510\n"),
                ("curr7_input", "9050\n"),
                ("power1_input", "109123000\n"),
                ("power2_input", "18075000\n"),
                ("power3_input", "18361600\n"),
                ("power4_input", "17789600\n"),
                ("power5_input", "18755000\n"),
                ("power6_input", "17939600\n"),
                ("power7_input", "18210600\n"),
                ("in6_input", "12058\n"),
                ("fan1_input", "75\n"),
                ("temp1_input", "34500\n"),
                ("temp2_input", "41200\n"),
                ("temp3_input", "27800\n"),
                ("fault_status_raw", "0\n"),
                ("fault_log_raw", "0\n"),
                ("psu_cap", "0\n"),
            ],
        );
        base.join("hwmon0")
    }

    #[test]
    fn finds_wireview_chip_among_others() {
        let base = std::env::temp_dir().join("wv-hwmon-find");
        let _ = fs::remove_dir_all(&base);
        write(&base.join("hwmon0"), &[("name", "coretemp\n")]);
        write(&base.join("hwmon1"), &[("name", "wireview\n")]);
        write(&base.join("hwmon2"), &[("name", "nvme\n")]);

        assert_eq!(find_chip(&base), Some(base.join("hwmon1")));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn reads_and_converts_every_channel() {
        let base = std::env::temp_dir().join("wv-hwmon-read");
        let _ = fs::remove_dir_all(&base);
        let chip = chip(&base);
        let s = read_chip(&chip).unwrap();

        assert!((s.voltage_v[0] - 12.05).abs() < 1e-9);
        assert!((s.current_a[5] - 1.51).abs() < 1e-9);
        assert!((s.sum_current_a - 9.05).abs() < 1e-9);
        assert!((s.sum_power_w - 109.123).abs() < 1e-9);
        assert!((s.power_w[0] - 18.075).abs() < 1e-9);
        assert_eq!(s.fan_duty, Some(75));
        assert_eq!(s.voltage_avg_v, Some(12.058));
        assert_eq!(s.temp_in_c, Some(34.5));
        assert_eq!(s.temp_out_c, Some(41.2));
        assert_eq!(s.ext1_c, Some(27.8));
        assert_eq!(s.ext2_c, None);
        assert_eq!(s.fault_status, 0);
        assert_eq!(s.fault_log, 0);
        assert_eq!(s.psu_cap_w, Some(600));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn maps_psu_cap_enum() {
        assert_eq!(map_psu_cap(0), 600);
        assert_eq!(map_psu_cap(1), 450);
        assert_eq!(map_psu_cap(2), 300);
        assert_eq!(map_psu_cap(3), 150);
        assert_eq!(map_psu_cap(7), 0);
    }

    #[test]
    fn falls_back_to_pin_sum_when_totals_are_missing() {
        let base = std::env::temp_dir().join("wv-hwmon-fallback");
        let _ = fs::remove_dir_all(&base);
        write(
            &base.join("hwmon0"),
            &[
                ("name", "wireview\n"),
                ("in0_input", "12000\n"),
                ("in1_input", "12000\n"),
                ("in2_input", "12000\n"),
                ("in3_input", "12000\n"),
                ("in4_input", "12000\n"),
                ("in5_input", "12000\n"),
                ("curr1_input", "1000\n"),
                ("curr2_input", "1000\n"),
                ("curr3_input", "1000\n"),
                ("curr4_input", "1000\n"),
                ("curr5_input", "1000\n"),
                ("curr6_input", "1000\n"),
            ],
        );
        let s = read_chip(&base.join("hwmon0")).unwrap();
        assert!((s.sum_current_a - 6.0).abs() < 1e-9);
        assert!((s.sum_power_w - 72.0).abs() < 1e-9);
        assert!((s.power_w[0] - 12.0).abs() < 1e-9);
        assert_eq!(s.fan_duty, None);
        assert_eq!(s.voltage_avg_v, None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn chip_without_reading_is_none() {
        let base = std::env::temp_dir().join("wv-hwmon-empty");
        let _ = fs::remove_dir_all(&base);
        write(&base.join("hwmon0"), &[("name", "wireview\n")]);
        assert_eq!(read_chip(&base.join("hwmon0")), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn serializes_camel_case() {
        let base = std::env::temp_dir().join("wv-hwmon-json");
        let _ = fs::remove_dir_all(&base);
        let s = read_chip(&chip(&base)).unwrap();
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["voltageV"][0], 12.05);
        assert_eq!(json["sumCurrentA"], 9.05);
        assert_eq!(json["sumPowerW"], 109.123);
        assert_eq!(json["powerW"][0], 18.075);
        assert_eq!(json["fanDuty"], 75);
        assert_eq!(json["voltageAvgV"], 12.058);
        assert!(json["ext2C"].is_null());
        assert_eq!(json["psuCapW"], 600);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn opened_chip_dir_does_not_follow_a_replaced_node() {
        let base = std::env::temp_dir().join("wv-hwmon-reuse");
        let _ = fs::remove_dir_all(&base);
        write(
            &base.join("hwmon0"),
            &[("name", "wireview\n"), ("in0_input", "12000\n")],
        );
        let dirfd = open_dir(&base.join("hwmon0")).expect("open chip dir");
        assert_eq!(chip_name(&dirfd).as_deref(), Some("wireview"));

        fs::remove_dir_all(base.join("hwmon0")).expect("replace chip node");
        write(
            &base.join("hwmon0"),
            &[("name", "coretemp\n"), ("in0_input", "999\n")],
        );

        assert_ne!(
            chip_name(&dirfd).as_deref(),
            Some("coretemp"),
            "a reused hwmonN path must not be visible through the original dir fd"
        );
        let _ = fs::remove_dir_all(&base);
    }
}
