//! A table's cell padding in RTF (RTF 1.9.1, "Table Definitions"): `\trgaphN` is the left and right padding of a
//! row's cells, 0 when the row has none; `\trpaddlN`, `\trpaddtN`, `\trpaddbN` and `\trpaddrN` state each side when
//! its unit word (`\trpaddflN`, `\trpaddftN`, `\trpaddfbN`, `\trpaddfrN`) is 3 (twips). The values are what Word
//! shows for the same rows; the table's first row decides.

use super::*;
use wordcraft_doc::{Block, Document};

/// Word's default cell padding (top, left, bottom, right), points: what a table with none of its own gets.
const WORD_DEFAULT: [f32; 4] = [0.0, 5.4, 0.0, 5.4];

fn open(rows: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}\pard P\par {rows}\pard Q\par}}").as_bytes()).unwrap()
}

fn tables(d: &Document) -> Vec<&wordcraft_doc::Table> {
    d.body.iter().filter_map(|b| b.as_table()).collect()
}

fn table(d: &Document) -> &wordcraft_doc::Table {
    tables(d).first().copied().expect("a table")
}

/// One row of two cells, "A" and "B", after the row definition `def` (which ends before its `\cellx` words).
fn row(def: &str) -> String {
    format!(r"\trowd{def}\cellx2880\cellx5760\pard\intbl A\cell B\cell\row")
}

/// The table's default cell padding (top, left, bottom, right), points: its own, else Word's default.
fn padding(d: &Document) -> [f32; 4] {
    table(d).props.cell_margins.unwrap_or(WORD_DEFAULT)
}

fn same(got: [f32; 4], want: [f32; 4]) -> bool {
    got.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01)
}

fn assert_padding(def: &str, want: [f32; 4]) {
    let got = padding(&open(&row(def)));
    assert!(same(got, want), "{def}: padding (top, left, bottom, right) {got:?}, want {want:?}");
}

#[test]
fn words_usual_row_keeps_the_default_padding() {
    let d = open(&row(r"\trgaph108\trleft-108"));
    assert_eq!(table(&d).props.cell_margins, None, "a row with Word's own gap sets nothing");
    assert_padding(r"\trgaph108\trleft0", WORD_DEFAULT);
    // Word's own rows state the same padding twice.
    let d = open(&row(r"\trgaph108\trleft-108\trpaddl108\trpaddr108\trpaddfl3\trpaddfr3"));
    assert_eq!(table(&d).props.cell_margins, None, "the default stated in full sets nothing");
}

#[test]
fn a_row_without_a_gap_has_no_padding() {
    assert_padding(r"\trleft0", [0.0; 4]);
    assert_padding("", [0.0; 4]);
    assert_padding(r"\trgaph0\trleft0", [0.0; 4]);
}

#[test]
fn the_gap_is_the_left_and_right_padding() {
    assert_padding(r"\trgaph200\trleft0", [0.0, 10.0, 0.0, 10.0]);
    assert_padding(r"\trgaph20", [0.0, 1.0, 0.0, 1.0]);
}

#[test]
fn a_side_stated_in_twips_wins_over_the_gap() {
    assert_padding(r"\trgaph108\trleft720\trpaddl300\trpaddfl3", [0.0, 15.0, 0.0, 5.4]);
    assert_padding(r"\trgaph108\trleft0\trpaddr300\trpaddfr3", [0.0, 5.4, 0.0, 15.0]);
    assert_padding(r"\trgaph108\trleft0\trpaddl0\trpaddfl3", [0.0, 0.0, 0.0, 5.4]);
    assert_padding(r"\trleft0\trpaddl300\trpaddfl3", [0.0, 15.0, 0.0, 0.0]);
    // The unit may come before its value.
    assert_padding(r"\trpaddfl3\trpaddl300\trgaph108", [0.0, 15.0, 0.0, 5.4]);
}

#[test]
fn top_and_bottom_padding_come_only_from_their_own_words() {
    assert_padding(r"\trgaph108\trleft0\trpaddt200\trpaddft3\trpaddb400\trpaddfb3", [10.0, 5.4, 20.0, 5.4]);
    assert_padding(r"\trgaph108\trpaddl60\trpaddt20\trpaddb40\trpaddr80\trpaddfl3\trpaddft3\trpaddfb3\trpaddfr3", [1.0, 3.0, 2.0, 4.0]);
}

#[test]
fn a_side_without_the_twips_unit_is_ignored() {
    assert_padding(r"\trgaph108\trleft0\trpaddl300", WORD_DEFAULT);
    assert_padding(r"\trgaph108\trleft0\trpaddl300\trpaddfl0", WORD_DEFAULT);
    assert_padding(r"\trgaph108\trleft0\trpaddt200\trpaddb400", WORD_DEFAULT);
    assert_padding(r"\trpaddr300\trpaddfr0", [0.0; 4]);
}

#[test]
fn the_first_row_decides_and_a_new_row_definition_forgets_the_last_ones_padding() {
    let d = open(&format!("{}{}", row(r"\trgaph108\trpaddl300\trpaddfl3"), row(r"\trgaph108")));
    assert_eq!(tables(&d).len(), 1);
    assert_eq!(table(&d).rows.len(), 2);
    assert!(same(padding(&d), [0.0, 15.0, 0.0, 5.4]), "{:?}", padding(&d));

    // The second table's row states only Word's gap: nothing is left of the first table's padding.
    let d = open(&format!(r"{}\pard between\par {}", row(r"\trpaddt200\trpaddft3\trpaddl300\trpaddfl3"), row(r"\trgaph108")));
    let t = tables(&d);
    assert_eq!(t.len(), 2);
    assert!(same(t[0].props.cell_margins.unwrap_or(WORD_DEFAULT), [10.0, 15.0, 0.0, 0.0]), "{:?}", t[0].props.cell_margins);
    assert!(same(t[1].props.cell_margins.unwrap_or(WORD_DEFAULT), WORD_DEFAULT), "{:?}", t[1].props.cell_margins);
}

#[test]
fn padding_alone_leaves_the_tables_style_and_position_as_they_were() {
    let plain = open(&row(r"\trgaph108\trleft0"));
    let padded = open(&row(r"\trgaph108\trleft0\trpaddl300\trpaddfl3"));
    let bare = open(&row(r"\trleft0"));
    for d in [&padded, &bare] {
        assert_eq!(table(d).props.style, table(&plain).props.style);
        assert_eq!(table(d).props.align, None);
        assert!(table(d).props.indent.unwrap_or(0.0).abs() < 0.01);
        assert_eq!(table(d).grid.len(), 2);
        assert!(table(d).grid.iter().all(|w| (w - 144.0).abs() < 0.01), "{:?}", table(d).grid);
        assert_eq!(table(d).props.cell_margin_sides, None, "all four sides are set");
    }
}

#[test]
fn hostile_padding_stays_finite_and_within_22_inches() {
    for def in [
        r"\trpaddl2147483647\trpaddfl3",
        r"\trpaddl-500\trpaddfl3\trpaddt-2147483647\trpaddft3",
        r"\trgaph2147483647",
        r"\trgaph-2147483647\trpaddb99999999\trpaddfb3",
        r"\trpaddfl2147483647\trpaddl300\trpaddfr-3\trpaddr300",
    ] {
        let d = open(&row(def));
        let p = padding(&d);
        assert!(p.iter().all(|v| v.is_finite() && (0.0..=1584.0).contains(v)), "{def}: padding {p:?}");
        assert_eq!(table(&d).rows.len(), 1, "{def}");
    }
    // A negative value is no padding.
    assert_padding(r"\trgaph108\trpaddl-500\trpaddfl3", [0.0, 0.0, 0.0, 5.4]);
}

#[test]
fn the_padding_round_trips() {
    let out = export(&open(&row(r"\trgaph108\trleft0\trpaddl300\trpaddfl3\trpaddt200\trpaddft3")));
    assert!(out.contains(r"\trgaph300\trleft0"), "the gap is the left padding:\n{out}");
    for word in [r"\trpaddl300", r"\trpaddt200", r"\trpaddb0", r"\trpaddr108", r"\trpaddfl3", r"\trpaddft3", r"\trpaddfb3", r"\trpaddfr3"] {
        assert!(out.contains(word), "{word} not written:\n{out}");
    }
    let back = padding(&import(out.as_bytes()).unwrap());
    assert!(same(back, [10.0, 15.0, 0.0, 5.4]), "{back:?}");
}

#[test]
fn a_table_without_padding_round_trips_without_any() {
    let out = export(&open(&row(r"\trleft0")));
    assert!(out.contains(r"\trgaph0\trleft0"), "{out}");
    assert!(out.contains(r"\trpaddl0") && out.contains(r"\trpaddr0") && out.contains(r"\trpaddfl3") && out.contains(r"\trpaddfr3"), "{out}");
    let back = padding(&import(out.as_bytes()).unwrap());
    assert!(same(back, [0.0; 4]), "{back:?}");
}

#[test]
fn every_row_of_the_table_states_the_padding() {
    let def = r"\trgaph108\trpaddl300\trpaddfl3";
    let out = export(&open(&format!("{}{}{}", row(def), row(def), row(def))));
    assert_eq!(out.matches(r"\trowd").count(), 3, "{out}");
    assert_eq!(out.matches(r"\trpaddl300").count(), 3, "{out}");
    assert_eq!(out.matches(r"\trgaph300").count(), 3, "{out}");
}

#[test]
fn a_table_with_the_default_padding_is_written_as_before() {
    let out = export(&open(&row(r"\trgaph108\trleft0")));
    assert!(out.contains(r"\trowd\trgaph108\trleft0"), "{out}");
    assert!(!out.contains(r"\trpadd"), "{out}");
}

#[test]
fn a_documents_default_cell_margins_are_written() {
    // Table Properties › Options… › Default cell margins: top 1 pt, left 2 pt, bottom 3 pt, right 4 pt.
    let mut d = open(&row(r"\trgaph108\trleft0"));
    for b in d.body.iter_mut() {
        if let Block::Table(t) = std::sync::Arc::make_mut(b) {
            t.props.cell_margins = Some([1.0, 2.0, 3.0, 4.0]);
        }
    }
    let out = export(&d);
    assert!(out.contains(r"\trgaph40\trleft0"), "{out}");
    assert!(out.contains(r"\trpaddl40\trpaddt20\trpaddb60\trpaddr80\trpaddfl3\trpaddft3\trpaddfb3\trpaddfr3"), "{out}");
    let back = padding(&import(out.as_bytes()).unwrap());
    assert!(same(back, [1.0, 2.0, 3.0, 4.0]), "{back:?}");
}
