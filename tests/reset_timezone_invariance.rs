use agent_meter::claude_usage::parse_claude_usage;
use agent_meter::countdown::countdown_label;
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};

fn snapshot_with_reset(resets_at: &str) -> String {
    format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"p","plan_label":null,
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":1.0,"resets_at":"{resets_at}",
        "source":"provider_reported"}}],"bonus_resets":null}}"#
    )
}

#[test]
fn offsets_in_source_timestamps_normalize_to_the_same_instant() {
    let utc = parse_snapshot(&snapshot_with_reset("2026-10-14T05:00:00Z")).unwrap();
    for same in ["2026-10-14T15:00:00+10:00", "2026-10-13T22:00:00-07:00", "2026-10-14T10:30:00+05:30"] {
        let other = parse_snapshot(&snapshot_with_reset(same)).unwrap();
        assert_eq!(other.windows[0].resets_at, utc.windows[0].resets_at, "{same}");
    }
}

#[test]
fn named_zones_resolve_to_one_instant_across_dst_changes() {
    // Sydney is UTC+11 in November (DST) and UTC+10 in June.
    let text = |zone: &str, date: &str| {
        format!("Current session: 10% used · resets {date}, 7:40pm ({zone})\n")
    };
    let nov = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();
    let at = parse_claude_usage(&text("Australia/Sydney", "Nov 1"), nov, "p").windows[0].resets_at;
    assert_eq!(at, Some(Utc.with_ymd_and_hms(2026, 11, 1, 8, 40, 0).unwrap()));
    let jun = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
    let at = parse_claude_usage(&text("Australia/Sydney", "Jun 1"), jun, "p").windows[0].resets_at;
    assert_eq!(at, Some(Utc.with_ymd_and_hms(2026, 6, 1, 9, 40, 0).unwrap()));
}

#[test]
fn a_countdown_is_elapsed_time_even_across_a_dst_change() {
    // US clocks fall back on 2026-11-01: 24 real hours still read "1d 0h".
    let now = Utc.with_ymd_and_hms(2026, 10, 31, 12, 0, 0).unwrap();
    let reset = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
    assert_eq!(countdown_label(reset, now), "1d 0h");
}

/// Runs the checks in `child` in a fresh process per host timezone, so the
/// runner's own environment is never mutated.
#[test]
fn every_check_gives_the_same_result_under_any_host_timezone() {
    for tz in ["UTC", "Australia/Sydney", "America/Los_Angeles", "Asia/Kathmandu"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "child", "--nocapture", "--test-threads=1"])
            .env("TZ", tz)
            .env("AGENT_METER_TZ_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "TZ={tz}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Only runs inside the per-timezone child process.
#[test]
fn child() {
    if std::env::var_os("AGENT_METER_TZ_CHILD").is_none() {
        return;
    }
    offsets_in_source_timestamps_normalize_to_the_same_instant();
    named_zones_resolve_to_one_instant_across_dst_changes();
    a_countdown_is_elapsed_time_even_across_a_dst_change();
}
