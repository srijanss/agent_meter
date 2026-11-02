use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{max_scroll, use_links_scrolled, use_links_viewed, AccountEntry};
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
fn a_notice_hides_the_use_link_on_the_last_body_row() {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let last_body_row = AREA.bottom() - 2;
    let mut hidden_somewhere = false;
    for scroll in 0..=max_scroll(AREA, &entries(), now, false) {
        let plain = use_links_scrolled(AREA, &entries(), now, scroll);
        let noticed = use_links_viewed(AREA, &entries(), now, scroll, true);
        let expected: Vec<_> =
            plain.iter().filter(|l| l.area.y < last_body_row).cloned().collect();
        hidden_somewhere |= expected.len() != plain.len();
        assert_eq!(noticed, expected, "scroll {scroll}");
        assert_eq!(use_links_viewed(AREA, &entries(), now, scroll, false), plain);
    }
    assert!(hidden_somewhere, "some offset puts a link under the notice");
}
