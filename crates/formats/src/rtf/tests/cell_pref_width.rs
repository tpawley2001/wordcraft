//! A cell's preferred width (`\clftsWidthN` unit + `\clwWidthN`) and Hide end-of-cell mark
//! (`\clhidemark`): read per cell definition and written back. The column grid still comes from
//! the `\cellx` edges.

use super::*;
use wordcraft_doc::Document;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}\fs24{body}}}").as_bytes()).unwrap()
}

/// A one-row, two-cell table: the first cell's definition carries `first`, the second's `second`.
fn row2(first: &str, second: &str) -> String {
    format!(r"\trowd\trgaph0{first}\cellx3000{second}\cellx6000\pard\intbl CELLA\cell CELLB\cell\row\pard AFTER\par")
}

fn row(words: &str) -> String {
    row2(words, "")
}

fn table(d: &Document) -> &wordcraft_doc::Table {
    d.body.iter().find_map(|b| b.as_table()).expect("a table")
}

/// (width points, width percent, hide mark) of each cell in the first row, rounded to 0.01.
fn prefs(d: &Document) -> Vec<(Option<f32>, Option<f32>, bool)> {
    let r = |v: Option<f32>| v.map(|v| (v * 100.0).round() / 100.0);
    table(d).rows[0].cells.iter().map(|c| (r(c.props.width), r(c.props.width_pct), c.props.hide_mark)).collect()
}

const NONE: (Option<f32>, Option<f32>, bool) = (None, None, false);

#[test]
fn twips_width_is_read_in_points() {
    assert_eq!(prefs(&open(&row(r"\clftsWidth3\clwWidth2880"))), [(Some(144.0), None, false), NONE]);
}

#[test]
fn width_word_before_its_unit_word_reads_the_same() {
    assert_eq!(prefs(&open(&row(r"\clwWidth2880\clftsWidth3"))), [(Some(144.0), None, false), NONE]);
}

#[test]
fn fiftieths_of_a_percent_read_as_a_percentage() {
    assert_eq!(prefs(&open(&row(r"\clftsWidth2\clwWidth2500"))), [(None, Some(50.0), false), NONE]);
}

#[test]
fn percentage_over_100_is_capped() {
    assert_eq!(prefs(&open(&row(r"\clftsWidth2\clwWidth6000"))), [(None, Some(100.0), false), NONE]);
}

#[test]
fn no_unit_auto_or_bad_values_give_no_preferred_width() {
    for words in [
        r"\clwWidth2880",
        r"\clftsWidth0\clwWidth2880",
        r"\clftsWidth1\clwWidth2880",
        r"\clftsWidth3\clwWidth0",
        r"\clftsWidth3\clwWidth-200",
        r"\clftsWidth3\clwWidth40000",
        r"\clftsWidth2\clwWidth0",
    ] {
        assert_eq!(prefs(&open(&row(words))), [NONE, NONE], "{words}");
    }
}

#[test]
fn each_cell_keeps_its_own_width_and_the_next_definition_starts_clean() {
    let d = open(&row2(r"\clftsWidth3\clwWidth1440\clhidemark", r"\clftsWidth2\clwWidth1250"));
    assert_eq!(prefs(&d), [(Some(72.0), None, true), (None, Some(25.0), false)]);
}

#[test]
fn hide_mark_is_read_per_cell() {
    assert_eq!(prefs(&open(&row2("", r"\clhidemark"))), [NONE, (None, None, true)]);
}

#[test]
fn preferred_width_does_not_change_the_column_grid() {
    let plain = open(&row(""));
    let wide = open(&row(r"\clftsWidth3\clwWidth5000"));
    assert_eq!(table(&wide).grid, table(&plain).grid);
}

#[test]
fn widths_and_hide_mark_are_written_and_read_back() {
    let d = open(&row2(r"\clftsWidth3\clwWidth2880\clhidemark", r"\clftsWidth2\clwWidth2500"));
    let rtf = export(&d);
    for word in [r"\clftsWidth3\clwWidth2880", r"\clhidemark", r"\clftsWidth2\clwWidth2500"] {
        assert!(rtf.contains(word), "{word} missing in {rtf}");
    }
    assert_eq!(prefs(&import(rtf.as_bytes()).unwrap()), [(Some(144.0), None, true), (None, Some(50.0), false)]);
}

#[test]
fn a_table_without_them_writes_none_of_the_words() {
    let rtf = export(&open(&row("")));
    for word in [r"\clftsWidth", r"\clwWidth", r"\clhidemark"] {
        assert!(!rtf.contains(word), "{word} in {rtf}");
    }
}
