//! Where a table sits (RTF 1.9.1, "Table Definitions"): `\trleftN` is the row's left edge from the left margin and
//! `\cellxN` each cell's right edge from the same margin, so the first column is `cellx - trleft` wide; `\tblindN` with
//! `\tblindtype3` states the indent of the cell text, `\trgaph` right of the edge; `\trqc` and `\trqr` centre the table
//! or put it at the right margin. The positions are what Word shows for the same rows.

use super::*;
use wordcraft_doc::{Align, Document};

fn open(rows: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}\pard P\par {rows}\pard Q\par}}").as_bytes()).unwrap()
}

fn table(d: &Document) -> &wordcraft_doc::Table {
    d.body.iter().find_map(|b| b.as_table()).expect("a table")
}

/// One row of two cells, "A" and "B", after the row definition `def`.
fn row(def: &str) -> String {
    format!(r"\trowd{def}\pard\intbl A\cell B\cell\row")
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// The table's left edge from the margin in points (0 when unset), its alignment and its column widths.
fn placed(d: &Document) -> (f32, Option<Align>, Vec<f32>) {
    let t = table(d);
    (t.props.indent.unwrap_or(0.0), t.props.align, t.grid.clone())
}

fn assert_placed(def: &str, indent: f32, align: Option<Align>, grid: [f32; 2]) {
    let (i, a, g) = placed(&open(&row(def)));
    assert!(close(i, indent), "{def}: indent {i}, want {indent}");
    assert_eq!(a, align, "{def}: alignment");
    assert!(g.len() == 2 && close(g[0], grid[0]) && close(g[1], grid[1]), "{def}: grid {g:?}, want {grid:?}");
}

#[test]
fn a_row_at_the_margin_is_unchanged() {
    assert_placed(r"\trgaph108\trleft0\cellx2880\cellx5760", 0.0, None, [144.0, 144.0]);
    assert_placed(r"\cellx2880\cellx5760", 0.0, None, [144.0, 144.0]);
}

#[test]
fn the_left_edge_is_trleft_and_the_first_column_starts_there() {
    // Word's usual row: the edge half a gap left of the margin, so the cell text lines up with the body text.
    assert_placed(r"\trgaph108\trleft-108\cellx2880\cellx5760", -5.4, None, [149.4, 144.0]);
    assert_placed(r"\trgaph108\trleft720\cellx3600\cellx6480", 36.0, None, [144.0, 144.0]);
    assert_placed(r"\trgaph108\trleft-1440\cellx2880\cellx5760", -72.0, None, [216.0, 144.0]);
}

#[test]
fn tblind_states_the_text_indent_and_wins_over_trleft() {
    assert_placed(r"\trgaph108\trleft-108\tblind720\tblindtype3\cellx2880\cellx5760", 30.6, None, [149.4, 144.0]);
    assert_placed(r"\trleft0\tblind720\tblindtype3\cellx2880\cellx5760", 36.0, None, [144.0, 144.0]);
    // Only twips (`\tblindtype3`) state an indent.
    assert_placed(r"\trgaph108\trleft-108\tblind720\tblindtype2\cellx2880\cellx5760", -5.4, None, [149.4, 144.0]);
    assert_placed(r"\trgaph108\trleft-108\tblind720\cellx2880\cellx5760", -5.4, None, [149.4, 144.0]);
}

#[test]
fn centred_and_right_aligned_tables_keep_their_alignment_and_no_indent() {
    assert_placed(r"\trqc\trgaph108\trleft720\cellx3600\cellx6480", 0.0, Some(Align::Center), [144.0, 144.0]);
    assert_placed(r"\trqr\trgaph108\trleft-108\cellx2880\cellx5760", 0.0, Some(Align::Right), [149.4, 144.0]);
    assert_placed(r"\trql\trgaph108\trleft0\cellx2880\cellx5760", 0.0, None, [144.0, 144.0]);
}

#[test]
fn the_first_row_places_the_table_and_every_row_shares_its_edge() {
    let def = r"\trgaph108\trleft720\cellx3600\cellx6480";
    let d = open(&format!("{}{}", row(def), row(def)));
    let (i, a, g) = placed(&d);
    assert!(close(i, 36.0) && a.is_none(), "indent {i}, alignment {a:?}");
    assert!(g.len() == 2 && close(g[0], 144.0) && close(g[1], 144.0), "grid {g:?}");
    assert_eq!(table(&d).rows.len(), 2);
}

#[test]
fn a_new_row_definition_forgets_the_last_rows_position() {
    // The second table's row states no `\trleft` and no `\trqc`: it sits at the margin.
    let d = open(&format!(r"{}\pard between\par {}", row(r"\trqc\trgaph108\trleft720\cellx3600\cellx6480"), row(r"\cellx2880\cellx5760")));
    let tables: Vec<_> = d.body.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].props.align, Some(Align::Center));
    assert_eq!(tables[1].props.align, None);
    assert!(close(tables[1].props.indent.unwrap_or(0.0), 0.0));
}

#[test]
fn a_position_alone_leaves_the_tables_style_as_it_was() {
    let plain = open(&row(r"\trgaph108\trleft0\cellx2880\cellx5760"));
    let moved = open(&row(r"\trgaph108\trleft-108\cellx2880\cellx5760"));
    let centred = open(&row(r"\trqc\trgaph108\trleft0\cellx2880\cellx5760"));
    assert_eq!(table(&moved).props.style, table(&plain).props.style);
    assert_eq!(table(&centred).props.style, table(&plain).props.style);
}

#[test]
fn hostile_positions_leave_a_finite_table() {
    for def in [
        r"\trleft2147483647\cellx2880\cellx5760",
        r"\trleft-2147483647\cellx2880\cellx5760",
        r"\trleft5000\cellx2880\cellx5760",
        r"\trgaph2147483647\tblind-2147483647\tblindtype3\cellx2880\cellx5760",
        r"\trqc\trqr\trleft2880\cellx2880\cellx5760",
    ] {
        let d = open(&row(def));
        let (i, _, g) = placed(&d);
        assert!(i.is_finite() && (-1584.0..=1584.0).contains(&i), "{def}: indent {i}");
        assert!(g.iter().all(|w| w.is_finite() && *w > 0.0), "{def}: grid {g:?}");
        assert_eq!(table(&d).rows.len(), 1, "{def}");
    }
}

#[test]
fn the_left_edge_round_trips() {
    let out = export(&open(&row(r"\trgaph108\trleft720\cellx3600\cellx6480")));
    assert!(out.contains(r"\trleft720") && out.contains(r"\cellx3600") && out.contains(r"\cellx6480"), "{out}");
    assert!(!out.contains(r"\trqc") && !out.contains(r"\trqr") && !out.contains(r"\tblind"), "{out}");
    let (i, a, g) = placed(&import(out.as_bytes()).unwrap());
    assert!(close(i, 36.0) && a.is_none(), "indent {i}, alignment {a:?}");
    assert!(g.len() == 2 && close(g[0], 144.0) && close(g[1], 144.0), "grid {g:?}");

    let out = export(&open(&row(r"\trgaph108\trleft-108\cellx2880\cellx5760")));
    assert!(out.contains(r"\trleft-108") && out.contains(r"\cellx2880") && out.contains(r"\cellx5760"), "{out}");
}

#[test]
fn the_alignment_round_trips() {
    for (word, align) in [(r"\trqc", Align::Center), (r"\trqr", Align::Right)] {
        let out = export(&open(&row(&format!(r"{word}\trgaph108\trleft0\cellx2880\cellx5760"))));
        assert!(out.contains(&format!(r"\trowd{word}")), "{word} not written after \\trowd:\n{out}");
        assert!(out.contains(r"\trleft0"), "{out}");
        let (i, a, g) = placed(&import(out.as_bytes()).unwrap());
        assert!(close(i, 0.0), "{word}: indent {i}");
        assert_eq!(a, Some(align));
        assert!(g.len() == 2 && close(g[0], 144.0) && close(g[1], 144.0), "{word}: grid {g:?}");
    }
}

#[test]
fn a_table_at_the_margin_is_written_as_before() {
    let out = export(&open(&row(r"\trgaph108\trleft0\cellx2880\cellx5760")));
    assert!(out.contains(r"\trowd\trgaph108\trleft0"), "{out}");
    assert!(!out.contains(r"\trqc") && !out.contains(r"\trqr"), "{out}");
}
