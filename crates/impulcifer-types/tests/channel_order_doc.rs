#![forbid(unsafe_code)]
//! docs/brir-channel-order.md must match the 3.x tables row by row: a wrong
//! row sends a speaker to the wrong track for whoever trusts the document
//! (the 2.x tables are checked by tests/test_channel_order_doc.py).

use impulcifer_types::{
    constants::{HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER},
    layouts::{IMMERSIVE_LAYOUTS, Layer, SLOTS},
};

fn table(name: &str) -> Vec<Vec<String>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/brir-channel-order.md"
    );
    // A Windows checkout may turn the document's line ends into CRLF.
    let text = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let start = format!("<!-- channel-table:{name} -->\n");
    let end = format!("\n<!-- /channel-table:{name} -->");
    let body = text
        .split_once(&start)
        .and_then(|(_, rest)| rest.split_once(&end))
        .unwrap_or_else(|| panic!("table marker {name} missing"))
        .0;
    body.lines()
        .skip(2)
        .map(|line| {
            line.trim()
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect()
        })
        .collect()
}
fn number(order: &[&str], name: &str) -> usize {
    order.iter().position(|n| *n == name).unwrap() + 1
}

#[test]
fn added_slot_table_matches_the_track_orders() {
    let rows = table("slots-3x");
    assert_eq!(rows.len(), SLOTS.len() - 15);
    for (row, slot) in rows.iter().zip(&SLOTS[15..]) {
        assert_eq!(row[0], slot.code);
        let pair = |order: &[&str]| {
            format!(
                "{}/{}",
                number(order, &format!("{}-left", slot.code)),
                number(order, &format!("{}-right", slot.code))
            )
        };
        assert_eq!(row[5], pair(&HEXADECAGONAL_TRACK_ORDER), "{}", slot.code);
        assert_eq!(row[6], pair(&HESUVI_TRACK_ORDER), "{}", slot.code);
        let users: Vec<_> = row[4].split(", ").collect();
        for (short, id) in [
            ("22.2", "22.2"),
            ("Auro", "13.1"),
            ("DTS", "30.2"),
            ("Atmos", "24.1.10"),
        ] {
            let layout = IMMERSIVE_LAYOUTS.iter().find(|l| l.id == id).unwrap();
            for c in layout.channels.iter().filter(|c| c.slot == slot.code) {
                assert!(
                    users.contains(&format!("{short} {}", c.label).as_str()),
                    "{} misses {short} {}",
                    slot.code,
                    c.label
                );
            }
        }
    }
}

#[test]
fn format_tables_match_the_official_orders() {
    for l in IMMERSIVE_LAYOUTS {
        let rows = table(&format!("layout-{}", l.id));
        assert_eq!(rows.len(), l.channels.len(), "{}", l.id);
        for (i, (row, c)) in rows.iter().zip(l.channels).enumerate() {
            let slot = if c.layer == Layer::Lfe {
                "— (무음)"
            } else {
                c.slot
            };
            assert_eq!(
                row[..5],
                [
                    (i + 1).to_string(),
                    c.label.to_owned(),
                    slot.to_owned(),
                    (2 * i + 1).to_string(),
                    (2 * i + 2).to_string(),
                ],
                "{} row {}",
                l.id,
                i + 1
            );
        }
    }
}
