//! Keeps the finished device setting changes of the last 30 minutes in a plain
//! text file, so they can be traced: `~/.local/state/micdeck/device-writes.log`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use micdeck_core::device_settings::{phase_summary, spec, value_label};
use micdeck_core::WriteReport;

fn log_path() -> Option<PathBuf> {
    let state_home = match std::env::var_os("XDG_STATE_HOME").map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => Some(dir),
        _ => std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("state")),
    };
    state_home.map(|d| d.join("micdeck").join("device-writes.log"))
}

/// `unix_ms` as `YYYY-MM-DDTHH:MM:SSZ`.
fn utc(unix_ms: u64) -> String {
    let secs = unix_ms / 1000;
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// One line for a finished change of the receiver at `key`.
pub fn line(key: &str, from: Option<&str>, report: &WriteReport) -> String {
    let setting = report.setting.as_str();
    let label = spec(setting).map_or(setting, |s| s.label);
    let from = from.map_or_else(|| "?".to_string(), |v| value_label(setting, v));
    let mut line = format!(
        "{} {key} {label} ({setting}): {from} -> {} | {}",
        utc(report.unix_ms),
        value_label(setting, &report.value),
        phase_summary(setting, &report.phase),
    );
    if let Some(seq) = report.seq {
        line.push_str(&format!(" | seq {seq}"));
    }
    if let Some(packet) = &report.packet {
        line.push_str(&format!(" | {packet}"));
    }
    line
}

/// Lines older than this are dropped whenever a new one is written.
const KEEP_MS: u64 = 30 * 60 * 1_000;

/// The lines of `text` written at `cutoff` or later (lines start with their
/// UTC time, which sorts like text).
fn recent(text: &str, cutoff: &str) -> String {
    text.lines().filter(|l| l.get(..cutoff.len()).is_some_and(|t| t >= cutoff)).map(|l| format!("{l}\n")).collect()
}

pub fn append(line: &str) {
    let Some(path) = log_path() else { return };
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let kept = fs::read_to_string(&path).map(|t| recent(&t, &utc(now_ms.saturating_sub(KEEP_MS)))).unwrap_or_default();
    let written = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| OpenOptions::new().create(true).write(true).truncate(true).open(&path))
        .and_then(|mut file| writeln!(file, "{kept}{line}"));
    if let Err(e) = written {
        log::warn!("cannot write {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use micdeck_core::WritePhase;

    #[test]
    fn formats_utc_dates() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_709_210_096_000), "2024-02-29T12:34:56Z");
    }

    #[test]
    fn keeps_only_recent_lines() {
        let text = "2026-11-08T10:00:00Z old\n2026-11-08T10:40:00Z new\n";
        assert_eq!(recent(text, "2026-11-08T10:30:00Z"), "2026-11-08T10:40:00Z new\n");
    }

    #[test]
    fn a_line_says_what_changed_and_how_it_ended() {
        let report = WriteReport {
            request: 1,
            setting: "low-cut".into(),
            value: "on".into(),
            phase: WritePhase::Confirmed,
            unix_ms: 0,
            seq: Some(3),
            packet: Some("55 16".into()),
        };
        assert_eq!(
            line("1-4", Some("off"), &report),
            "1970-01-01T00:00:00Z 1-4 Hochpassfilter (low-cut): Aus -> An | Übernommen und zurückgelesen | seq 3 | 55 16"
        );
    }
}
