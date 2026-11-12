//! `.doc` vertical merges: each row's `sprmTDefTable` carries one `TC80` per cell after the cell
//! boundaries, and bits 5-6 of `TC80.tcgrf` (its first two bytes) say whether the cell is merged
//! with the cell above (1) or is the first cell of a vertically merged set (3) ([MS-DOC]
//! `TDefTableOperand`, `TC80`, `TCGRF`). Word saves merges there, with no `sprmTVertMerge`, and
//! honours them: a merged-away cell shows no content (and no list number for its empty
//! paragraph), and a merged cell's text runs on beside the rows it spans.

use wordcraft_doc::props::{CharProps, VMerge};
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];

/// `tcgrf` with a twips cell width (ftsWidth 3) and nothing else: an ordinary cell.
const PLAIN: u16 = 0x0600;
/// First cell of a vertically merged set (vertMerge 3).
const RESTART: u16 = 0x0660;
/// Merged with the cell above (vertMerge 1); Word also sets vertAlign 1 on such cells.
const CONTINUE: u16 = 0x06A0;

/// A depth-1 mark with extra PAPX sprms; `cell_mark` says it was 0x07.
fn mark(text: &str, extra: &[u8], cell_mark: bool) -> ParaOut {
    let mut papx = vec![0x16, 0x24, 1]; // sprmPFInTable
    papx.extend([0x49, 0x66]); // sprmPItap
    papx.extend(1i32.to_le_bytes());
    papx.extend(extra);
    let mut row = table::decode(&papx);
    row.cell_mark = cell_mark;
    ParaOut { para: Paragraph::with_text(text, CharProps::default()), end_cp: 0, row: Some(Box::new(row)) }
}

/// A `sprmTDefTable` Prl: cb (bytes after it, plus one), itcMac, the boundaries, then one
/// 20-byte TC80 per entry of `tcgrf` (flags, a zero wWidth and four zero Brc80s) followed by
/// `stray` extra bytes. `tcgrf` may be shorter than the cell count.
fn def_table(edges: &[i16], tcgrf: &[u16], stray: usize) -> Vec<u8> {
    let mut body = vec![edges.len() as u8 - 1];
    for e in edges {
        body.extend_from_slice(&e.to_le_bytes());
    }
    for flags in tcgrf {
        body.extend_from_slice(&flags.to_le_bytes());
        body.extend_from_slice(&[0u8; 18]);
    }
    body.extend(std::iter::repeat_n(0xA5u8, stray));
    let cb = (body.len() + 1) as u16;
    let mut v = (0xD608u16).to_le_bytes().to_vec();
    v.extend_from_slice(&cb.to_le_bytes());
    v.extend_from_slice(&body);
    v
}

/// `sprmTVertMerge`: cb = 2, itc, vertMergeFlags.
fn vert_merge(itc: u8, flag: u8) -> Vec<u8> {
    super::grpprl(&[(0xD62B, &[2, itc, flag])])
}

/// One table; each row is its cell count and the sprms of its row-end TAP, in order.
fn rows(rows: &[(usize, &[&[u8]])]) -> Blocks {
    let mut marks: Vec<ParaOut> = Vec::new();
    for (r, (cells, tap)) in rows.iter().enumerate() {
        let mut end = TTP.to_vec();
        for part in *tap {
            end.extend_from_slice(part);
        }
        marks.extend((0..*cells).map(|i| mark(&format!("r{r}c{i}"), &[], true)));
        marks.push(mark("", &end, true));
    }
    table::assemble(marks)
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

fn vmerges(t: &Table, row: usize) -> Vec<VMerge> {
    t.rows[row].cells.iter().map(|c| c.props.vmerge).collect()
}

#[test]
fn tc80_vertical_merge_makes_restart_and_continue_cells() {
    // Three rows as a Word document stores them: a heading row whose first and last cells start
    // a merge, the row under it whose first and last cells are merged away (the middle cell of
    // the heading row stands over two cells here), then an ordinary row.
    let head = def_table(&[-360, 4200, 6367, 8530], &[RESTART, PLAIN, RESTART], 0);
    let under = def_table(&[-360, 4200, 5284, 6367, 8530], &[CONTINUE, PLAIN, PLAIN, CONTINUE], 0);
    let plain = def_table(&[-360, 4200, 5284, 6367, 8530], &[PLAIN; 4], 0);
    let blocks = rows(&[(3, &[&head]), (4, &[&under]), (4, &[&plain])]);
    let t = table(&blocks);
    assert_eq!(t.rows.len(), 3);
    assert_eq!(vmerges(t, 0), [VMerge::Restart, VMerge::None, VMerge::Restart]);
    assert_eq!(vmerges(t, 1), [VMerge::Continue, VMerge::None, VMerge::None, VMerge::Continue]);
    assert_eq!(vmerges(t, 2), [VMerge::None; 4]);
    // The merged-away cells keep their own paragraph in the model (Word keeps it in the file).
    assert_eq!(t.rows[1].cells[0].blocks.len(), 1);
}

#[test]
fn staggered_merges_in_neighbouring_columns() {
    // Three rows of another Word document: cells 0-1 are merged over rows 0-1 and cells 2-3 over
    // rows 1-2 (Word lets the right-hand pair start while the left-hand pair is still running).
    let edges = [-90, 1170, 4680, 5940, 9450];
    let x = def_table(&edges, &[RESTART, RESTART, PLAIN, PLAIN], 0);
    let y = def_table(&edges, &[0x0620, 0x0620, RESTART, RESTART], 0);
    let z = def_table(&edges, &[PLAIN, PLAIN, 0x0620, 0x0620], 0);
    let blocks = rows(&[(4, &[&x]), (4, &[&y]), (4, &[&z])]);
    let t = table(&blocks);
    assert_eq!(vmerges(t, 0), [VMerge::Restart, VMerge::Restart, VMerge::None, VMerge::None]);
    assert_eq!(vmerges(t, 1), [VMerge::Continue, VMerge::Continue, VMerge::Restart, VMerge::Restart]);
    assert_eq!(vmerges(t, 2), [VMerge::None, VMerge::None, VMerge::Continue, VMerge::Continue]);
}

#[test]
fn plain_tc80s_leave_cells_unmerged() {
    let plain = def_table(&[-360, 4200, 5284, 6367, 8530], &[PLAIN; 4], 0);
    let zero = def_table(&[0, 2880, 5760], &[0, 0], 0);
    let blocks = rows(&[(4, &[&plain])]);
    assert_eq!(vmerges(table(&blocks), 0), [VMerge::None; 4]);
    let blocks = rows(&[(2, &[&zero])]);
    assert_eq!(vmerges(table(&blocks), 0), [VMerge::None; 2]);
}

#[test]
fn vert_merge_sprm_still_works_with_plain_tc80s() {
    let row = def_table(&[0, 2880, 5760], &[PLAIN, PLAIN], 0);
    let blocks = rows(&[(2, &[&row, &vert_merge(1, 3)]), (2, &[&row, &vert_merge(1, 1)])]);
    let t = table(&blocks);
    assert_eq!(vmerges(t, 0), [VMerge::None, VMerge::Restart]);
    assert_eq!(vmerges(t, 1), [VMerge::None, VMerge::Continue]);
}

#[test]
fn short_or_cut_tc80_array_applies_only_complete_cells() {
    // itcMac says three cells; the array holds one complete TC80 and ten stray bytes (which
    // would read as vertMerge 1 if taken as a TC80).
    let cut = def_table(&[0, 1440, 2880, 4320], &[RESTART], 10);
    let blocks = rows(&[(3, &[&cut])]);
    let t = table(&blocks);
    assert_eq!(vmerges(t, 0), [VMerge::Restart, VMerge::None, VMerge::None]);

    // No TC80s at all.
    let bare = def_table(&[0, 1440, 2880], &[], 0);
    let blocks = rows(&[(2, &[&bare])]);
    assert_eq!(vmerges(table(&blocks), 0), [VMerge::None; 2]);
}
