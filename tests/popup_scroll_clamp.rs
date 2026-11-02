use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{max_scroll, use_links_scrolled, AccountEntry};
use chrono::{TimeZone, Utc};
use ratatui::layout::Rect;

const AREA: Rect = Rect { x: 0, y: 0, width: 40, height: 10 };

fn entries() -> Vec<AccountEntry> {
    (1..=8)
        .map(|n| AccountEntry {
            account: Account {
                name: format!("acct-{n}"),
                provider: Provider::Claude,
                home: PathBuf::from("/x"),
                active: n == 1,
            },
            snapshot: None,
        })
        .collect()
}

#[test]
fn use_links_for_an_offset_past_the_end_match_what_is_drawn_at_the_end() {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let max = max_scroll(AREA, &entries(), now, false);
    assert!(max > 0);
    let at_end = use_links_scrolled(AREA, &entries(), now, max);
    assert!(!at_end.is_empty());
    assert_eq!(use_links_scrolled(AREA, &entries(), now, max + 40), at_end);
}
