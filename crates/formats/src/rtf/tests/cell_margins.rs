//! A cell's own margins (`\clpadlN`, `\clpadtN`, `\clpadbN`, `\clpadrN`, each with its unit word
//! `\clpadfl3` … `\clpadfr3`) and Wrap text off (`\clNoWrap`): read per cell and written back.
//! Word reads `\clpadl` as the cell's top margin and `\clpadt` as its left margin.

use super::*;
use wordcraft_doc::Document;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}\fs24{body}}}").as_bytes()).unwrap()
}

/// A one-row, two-cell table whose first cell's definition carries `words`.
fn row(words: &str) -> String {
    format!(r"\trowd\trgaph0{words}\cellx3000\cellx6000\pard\intbl CELLA\cell CELLB\cell\row\pard AFTER\par")
}

fn table(d: &Document) -> &wordcraft_doc::Table {
    d.body.iter().find_map(|b| b.as_table()).expect("a table")
}

/// The margins cell `i` of the first row sets itself: [top, left, bottom, right] in points.
fn sides(d: &Document, i: usize) -> [Option<f32>; 4] {
    let p = &table(d).rows[0].cells[i].props;
    let own = p.margins.map(|m| m.map(Some)).or(p.partial_margins).unwrap_or([None; 4]);
    own.map(|s| s.map(|v| (v * 100.0).round() / 100.0))
}

fn no_wraps(d: &Document) -> Vec<bool> {
    table(d).rows[0].cells.iter().map(|c| c.props.no_wrap).collect()
}

#[test]
fn clpadl_is_the_cells_top_margin() {
    let d = open(&row(r"\clpadl720\clpadfl3"));
    assert_eq!(sides(&d, 0), [Some(36.0), None, None, None]);
    assert_eq!(sides(&d, 1), [None; 4]);
}

#[test]
fn clpadt_is_the_cells_left_margin() {
    let d = open(&row(r"\clpadt720\clpadft3"));
    assert_eq!(sides(&d, 0), [None, Some(36.0), None, None]);
    assert_eq!(sides(&d, 1), [None; 4]);
}

#[test]
fn clpadb_and_clpadr_are_bottom_and_right() {
    let d = open(&row(r"\clpadb720\clpadfb3"));
    assert_eq!(sides(&d, 0), [None, None, Some(36.0), None]);
    let d = open(&row(r"\clpadr720\clpadfr3"));
    assert_eq!(sides(&d, 0), [None, None, None, Some(36.0)]);
}

#[test]
fn a_side_without_the_twips_unit_is_not_set() {
    for words in [r"\clpadl720", r"\clpadl720\clpadfl0"] {
        let d = open(&row(words));
        assert_eq!(sides(&d, 0), [None; 4], "{words}");
        let p = &table(&d).rows[0].cells[0].props;
        assert_eq!((p.margins, p.partial_margins), (None, None), "{words}");
    }
}

#[test]
fn all_four_sides_in_any_order_with_the_unit_first() {
    let d = open(&row(r"\clpadfr3\clpadr80\clpadfb3\clpadb60\clpadft3\clpadt40\clpadfl3\clpadl20"));
    assert_eq!(sides(&d, 0), [Some(1.0), Some(2.0), Some(3.0), Some(4.0)]);
    assert_eq!(table(&d).rows[0].cells[0].props.margins, Some([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(sides(&d, 1), [None; 4]);
}

#[test]
fn each_cell_keeps_its_own_margins_and_the_next_row_starts_clean() {
    let d = open(
        r"\trowd\trgaph0\clpadt360\clpadft3\cellx3000\clpadr100\clpadfr3\cellx6000\pard\intbl a\cell b\cell\row\trowd\trgaph0\cellx3000\cellx6000\pard\intbl c\cell d\cell\row\pard after\par",
    );
    assert_eq!(sides(&d, 0), [None, Some(18.0), None, None]);
    assert_eq!(sides(&d, 1), [None, None, None, Some(5.0)]);
    let second: Vec<_> = table(&d).rows[1].cells.iter().map(|c| (c.props.margins, c.props.partial_margins)).collect();
    assert_eq!(second, [(None, None), (None, None)]);
}

#[test]
fn a_huge_margin_is_capped_at_22_inches() {
    let d = open(&row(r"\clpadl99999999\clpadfl3"));
    assert_eq!(sides(&d, 0), [Some(1584.0), None, None, None]);
}

#[test]
fn cell_margins_are_written_back_with_words_swapped_as_word_does() {
    let d = open(&row(r"\clpadl720\clpadfl3\clpadt360\clpadft3"));
    let rtf = export(&d);
    for word in [r"\clpadl720", r"\clpadfl3", r"\clpadt360", r"\clpadft3"] {
        assert!(rtf.contains(word), "{word} missing in {rtf}");
    }
    assert!(!rtf.contains(r"\clpadb") && !rtf.contains(r"\clpadr"), "sides the cell doesn't set are not written: {rtf}");
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!(sides(&back, 0), [Some(36.0), Some(18.0), None, None]);
    assert_eq!(sides(&back, 1), [None; 4]);
}

#[test]
fn a_table_without_cell_margins_writes_no_cell_padding_words() {
    let rtf = export(&open(&row("")));
    assert!(!rtf.contains(r"\clpad"), "{rtf}");
    assert!(!rtf.contains(r"\clNoWrap"), "{rtf}");
}

#[test]
fn wrap_text_off_is_read_and_written_back() {
    let d = open(&row(r"\clNoWrap"));
    assert_eq!(no_wraps(&d), [true, false]);
    let rtf = export(&d);
    assert!(rtf.contains(r"\clNoWrap"), "{rtf}");
    assert_eq!(no_wraps(&import(rtf.as_bytes()).unwrap()), [true, false]);
}
