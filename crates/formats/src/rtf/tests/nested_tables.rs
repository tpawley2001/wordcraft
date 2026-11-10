//! Nested tables, RTF 1.9.1 "Nested tables": a table inside a table cell. The inner table's
//! paragraphs carry `\itapN` (N = nesting depth, 1 = the outer table), end their cells with
//! `\nestcell` and their rows with `\nestrow`; the inner row's properties sit in a
//! `{\*\nesttableprops \trowd ... \nestrow}` group; `{\nonesttables ...}` holds a flat copy for
//! readers without nested tables and is skipped by readers that have them.

use super::*;
use wordcraft_doc::{Block, Document, Table};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

fn tables(d: &Document) -> Vec<&Table> {
    d.body.iter().filter_map(|b| b.as_table()).collect()
}

/// Each block of a cell: "T" for a table, else the paragraph's text.
fn cell_blocks(t: &Table, row: usize, cell: usize) -> Vec<String> {
    t.rows[row].cells[cell]
        .blocks
        .iter()
        .map(|b| match &**b {
            Block::Table(_) => "T".to_string(),
            other => other.as_para().map(|p| p.plain_text()).unwrap_or_default(),
        })
        .collect()
}

fn inner(t: &Table, row: usize, cell: usize) -> &Table {
    t.rows[row].cells[cell].blocks.iter().find_map(|b| b.as_table()).expect("a nested table")
}

fn texts(t: &Table) -> Vec<Vec<String>> {
    t.rows
        .iter()
        .map(|r| r.cells.iter().map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect::<Vec<_>>().join("|")).collect())
        .collect()
}

/// As Word 2016+ writes it: the outer row's `\trowd` before the cells, the inner table in cell 2,
/// then more text in cell 2 after it.
const NESTED: &str = r"\trowd\trgaph108\cellx4000\cellx8000
\pard\intbl\itap1 outer A\cell
\pard\intbl\itap2 inner 1\nestcell inner 2\nestcell
{\*\nesttableprops\trowd\trgaph108\cellx1500\cellx3000\nestrow}{\nonesttables\par}
\pard\intbl\itap2 inner 3\nestcell inner 4\nestcell
{\*\nesttableprops\trowd\trgaph108\cellx1500\cellx3000\nestrow}{\nonesttables\par}
\pard\intbl\itap1 after inner\cell
\trowd\trgaph108\cellx4000\cellx8000\row
\pard\itap0 after table\par";

#[test]
fn a_nested_table_lands_in_its_cell() {
    let d = open(NESTED);
    let ts = tables(&d);
    assert_eq!(ts.len(), 1, "one top-level table");
    let t = ts[0];
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.rows[0].cells.len(), 2);
    assert_eq!(cell_blocks(t, 0, 0), ["outer A"]);
    // The fallback `\nonesttables` paragraph is not read: just the table and the text after it.
    assert_eq!(cell_blocks(t, 0, 1), ["T", "after inner"]);
    assert_eq!(texts(inner(t, 0, 1)), [["inner 1", "inner 2"], ["inner 3", "inner 4"]]);
    let after: Vec<String> = d.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect();
    assert_eq!(after, ["after table"]);
}

#[test]
fn inner_and_outer_tables_keep_their_own_widths() {
    let d = open(NESTED);
    let t = tables(&d)[0];
    assert_eq!(t.grid, [200.0, 200.0]);
    assert_eq!(inner(t, 0, 1).grid, [75.0, 75.0]);
}

#[test]
fn outer_row_properties_given_only_before_the_cells_survive_the_inner_table() {
    // No `\trowd` repeated before `\row`: the outer row must still use 4000/8000, not the
    // inner table's 1500/3000.
    let d = open(
        r"\trowd\trgaph108\cellx4000\cellx8000
\pard\intbl\itap2 x\nestcell{\*\nesttableprops\trowd\cellx1500\nestrow}{\nonesttables\par}
\pard\intbl\itap1 a\cell b\cell\row\pard after\par",
    );
    let t = tables(&d)[0];
    assert_eq!(t.grid, [200.0, 200.0]);
    assert_eq!(cell_blocks(t, 0, 0), ["T", "a"]);
    assert_eq!(cell_blocks(t, 0, 1), ["b"]);
    assert_eq!(inner(t, 0, 0).grid, [75.0]);
}

#[test]
fn three_levels_nest() {
    let d = open(
        r"\trowd\cellx6000
\pard\intbl\itap3 deep\nestcell{\*\nesttableprops\trowd\cellx1000\nestrow}{\nonesttables\par}
\pard\intbl\itap2 mid\nestcell{\*\nesttableprops\trowd\cellx3000\nestrow}{\nonesttables\par}
\pard\intbl\itap1 top\cell\row\pard after\par",
    );
    let t = tables(&d)[0];
    assert_eq!(cell_blocks(t, 0, 0), ["T", "top"]);
    let mid = inner(t, 0, 0);
    assert_eq!(cell_blocks(mid, 0, 0), ["T", "mid"]);
    assert_eq!(texts(inner(mid, 0, 0)), [["deep"]]);
}

#[test]
fn nested_tables_round_trip() {
    let d = open(NESTED);
    let rtf = export(&d);
    for w in [r"\itap2", r"\nestcell", r"\nestrow", r"\*\nesttableprops", r"\nonesttables"] {
        assert!(rtf.contains(w), "{w} missing:\n{rtf}");
    }
    let back = import(rtf.as_bytes()).unwrap();
    let t = tables(&back)[0];
    assert_eq!(cell_blocks(t, 0, 0), ["outer A"]);
    assert_eq!(cell_blocks(t, 0, 1), ["T", "after inner"]);
    let i = inner(t, 0, 1);
    assert_eq!(texts(i), [["inner 1", "inner 2"], ["inner 3", "inner 4"]]);
    assert_eq!(i.grid, [75.0, 75.0]);
}

#[test]
fn hostile_nesting_is_bounded() {
    // A huge \itap, \nestcell / \nestrow with no table around them, and 200 levels: no panic,
    // the text is kept, and the depth stays within the model's limit.
    let mut deep = String::from(r"\trowd\cellx6000");
    for n in (2..=200).rev() {
        deep.push_str(&format!(r"\pard\intbl\itap{n} L{n}\nestcell{{\*\nesttableprops\trowd\cellx1000\nestrow}}"));
    }
    deep.push_str(r"\pard\intbl\itap1 top\cell\row\pard after\par");
    for body in [r"\pard\intbl\itap100000 x\nestcell\nestrow\par", r"\pard stray\nestcell\nestrow\par", deep.as_str()] {
        let d = open(body);
        let text = d.body.iter().map(|b| format!("{b:?}")).collect::<String>();
        assert!(!d.body.is_empty(), "{body}");
        let _ = export(&d);
        assert!(text.len() < 50_000_000);
    }
    let d = open(&deep);
    fn depth(t: &Table) -> usize {
        1 + t.rows.iter().flat_map(|r| &r.cells).flat_map(|c| &c.blocks).filter_map(|b| b.as_table()).map(depth).max().unwrap_or(0)
    }
    let t = tables(&d)[0];
    // The model keeps tables 8 levels deep (formats::model::MAX_DEPTH); deeper levels' text is
    // kept as paragraphs in the deepest table's cell.
    assert!((2..=8).contains(&depth(t)), "depth {}", depth(t));
    let all = format!("{:?}", d.body);
    assert!(all.contains("L200") && all.contains("L2") && all.contains("top"));
}

#[test]
fn a_flat_table_is_unchanged() {
    let d = open(r"\trowd\cellx3000\cellx6000\pard\intbl a\cell b\cell\row\pard after\par");
    let t = tables(&d)[0];
    assert_eq!(texts(t), [["a", "b"]]);
    assert_eq!(t.grid, [150.0, 150.0]);
}
