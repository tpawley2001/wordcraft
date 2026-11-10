//! Table row height (`\trrhN`), rows that can't break across pages (`\trkeep`) and turned cell
//! text (`\cltxtbrl`, `\cltxbtlr`, `\cltxlrtb`…), RTF 1.9.1 table row and cell properties: read
//! per row / per cell and written back.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{HeightRule, TextDirection};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

fn table(d: &Document) -> &wordcraft_doc::Table {
    d.body.iter().find_map(|b| b.as_table()).expect("a table")
}

/// Each row's (height in points, rule).
fn heights(d: &Document) -> Vec<(Option<f32>, HeightRule)> {
    table(d).rows.iter().map(|r| (r.props.height.map(|h| (h * 100.0).round() / 100.0), r.props.height_rule)).collect()
}

fn keeps(d: &Document) -> Vec<bool> {
    table(d).rows.iter().map(|r| r.props.cant_split).collect()
}

fn directions(d: &Document) -> Vec<TextDirection> {
    table(d).rows[0].cells.iter().map(|c| c.props.text_direction).collect()
}

const HEIGHTS: &str = r"\trowd\trrh400\cellx3000\pard\intbl at least\cell\row\trowd\trrh-567\cellx3000\pard\intbl exact\cell\row\trowd\trrh0\cellx3000\pard\intbl zero\cell\row\trowd\cellx3000\pard\intbl none\cell\row\pard after\par";

#[test]
fn row_heights_are_read_per_row() {
    let d = open(HEIGHTS);
    assert_eq!(
        heights(&d),
        [(Some(20.0), HeightRule::AtLeast), (Some(28.35), HeightRule::Exact), (None, HeightRule::Auto), (None, HeightRule::Auto)]
    );
}

#[test]
fn rows_kept_from_breaking_are_read_per_row() {
    let d = open(
        r"\trowd\trkeep\cellx3000\pard\intbl kept\cell\row\trowd\cellx3000\pard\intbl free\cell\row\trowd\trkeep\cellx3000\pard\intbl kept\cell\row\pard after\par",
    );
    assert_eq!(keeps(&d), [true, false, true]);
}

#[test]
fn cell_text_direction_is_read_per_cell() {
    let d = open(
        r"\trowd\cltxtbrl\cellx2000\cltxbtlr\cellx4000\cltxlrtb\cellx6000\cellx8000\cltxtbrlv\cellx10000\pard\intbl a\cell b\cell c\cell d\cell e\cell\row\pard after\par",
    );
    assert_eq!(directions(&d), [TextDirection::Down, TextDirection::Up, TextDirection::Horizontal, TextDirection::Horizontal, TextDirection::Down]);
}

#[test]
fn a_table_without_them_keeps_the_defaults() {
    let d = open(r"\trowd\cellx3000\cellx6000\pard\intbl a\cell b\cell\row\pard after\par");
    assert_eq!(heights(&d), [(None, HeightRule::Auto)]);
    assert_eq!(keeps(&d), [false]);
    assert_eq!(directions(&d), [TextDirection::Horizontal, TextDirection::Horizontal]);
}

#[test]
fn row_heights_keeps_and_directions_round_trip() {
    let d = open(
        r"\trowd\trrh400\trkeep\cltxtbrl\cellx3000\cltxbtlr\cellx6000\pard\intbl a\cell b\cell\row\trowd\trrh-567\cellx3000\cellx6000\pard\intbl c\cell d\cell\row\pard after\par",
    );
    let rtf = export(&d);
    assert!(rtf.contains(r"\trrh400"), "{rtf}");
    assert!(rtf.contains(r"\trrh-567"), "{rtf}");
    assert!(rtf.contains(r"\trkeep"), "{rtf}");
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!(heights(&back), [(Some(20.0), HeightRule::AtLeast), (Some(28.35), HeightRule::Exact)]);
    assert_eq!(keeps(&back), [true, false]);
    assert_eq!(directions(&back), [TextDirection::Down, TextDirection::Up]);
}

#[test]
fn a_huge_row_height_is_ignored() {
    let d = open(r"\trowd\trrh999999\cellx3000\pard\intbl a\cell\row\trowd\trrh-40000\cellx3000\pard\intbl b\cell\row\pard after\par");
    assert_eq!(heights(&d), [(None, HeightRule::Auto), (None, HeightRule::Auto)]);
}
