use agent_meter::snapshot::{parse_snapshot, Health, Source};
use chrono::{TimeZone, Utc};

const SPEC_EXAMPLE: &str = r#"{
  "schema_version": 1,
  "provider": "codex",
  "profile_id": "profile-local-1",
  "plan_label": "Pro",
  "collected_at": "2026-10-14T00:00:00Z",
  "source": "provider_reported",
  "health": "ok",
  "windows": [
    {
      "id": "weekly",
      "label": "Weekly",
      "used_percent": 63.0,
      "resets_at": "2026-10-18T00:00:00Z",
      "source": "provider_reported"
    }
  ],
  "bonus_resets": {
    "remaining": 2,
    "expires_at": "2026-11-03T00:00:00Z",
    "source": "provider_reported"
  }
}"#;

#[test]
fn parses_the_spec_example_snapshot() {
    let snapshot = parse_snapshot(SPEC_EXAMPLE).expect("spec example parses");

    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.provider, "codex");
    assert_eq!(snapshot.profile_id, "profile-local-1");
    assert_eq!(snapshot.plan_label.as_deref(), Some("Pro"));
    assert_eq!(snapshot.collected_at, Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap());
    assert_eq!(snapshot.source, Source::ProviderReported);
    assert_eq!(snapshot.health, Health::Ok);

    assert_eq!(snapshot.windows.len(), 1);
    let weekly = &snapshot.windows[0];
    assert_eq!(weekly.id, "weekly");
    assert_eq!(weekly.label, "Weekly");
    assert_eq!(weekly.used_percent, Some(63.0));
    assert_eq!(weekly.resets_at, Some(Utc.with_ymd_and_hms(2026, 10, 18, 0, 0, 0).unwrap()));
    assert_eq!(weekly.source, Source::ProviderReported);

    let bonus = snapshot.bonus_resets.expect("bonus resets reported");
    assert_eq!(bonus.remaining, Some(2));
    assert_eq!(bonus.expires_at, Some(Utc.with_ymd_and_hms(2026, 11, 3, 0, 0, 0).unwrap()));
    assert_eq!(bonus.source, Source::ProviderReported);
}
