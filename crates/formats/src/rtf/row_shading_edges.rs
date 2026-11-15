//! Row-definition shading, as Microsoft 365 Word opens it: `\trcbpat`/`\trcfpat`/`\trshdng`
//! shade every cell of the row that has no shading of its own, exactly as the same values written
//! as `\clcbpat`/`\clcfpat`/`\clshdng` would (RTF 1.9.1, "Table row definitions").

use super::*;
use wordcraft_doc::{Document, Rgb, Table};

const COLORS: &str = r"{\colortbl;\red255\green0\blue0;\red255\green204\blue0;}";

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}{COLORS}{body}}}").as_bytes()).unwrap()
}

fn first_table(d: &Document) -> &Table {
    d.body.iter().find_map(|b| b.as_table()).unwrap()
}

fn fills(t: &Table) -> Vec<Vec<Option<Rgb>>> {
    t.rows.iter().map(|r| r.cells.iter().map(|c| c.props.shading).collect()).collect()
}

/// Two 2-cell rows; `row` is put after each `\trowd`, `cell` before each `\cellx`.
fn two_rows(row: &str, cell: &str) -> Document {
    open(&format!(
        r"\trowd\trgaph108{row}{cell}\cellx1440{cell}\cellx2880\pard\intbl A\cell B\cell\row\trowd\trgaph108{row}{cell}\cellx1440{cell}\cellx2880\pard\intbl C\cell D\cell\row\pard AFTER\par"
    ))
}

#[test]
fn row_shading_shades_every_cell_as_cell_shading_would() {
    // (row words, the same pattern as cell words)
    let cases = [
        (r"\trcbpat2\trcfpat1\trshdng0", r"\clcbpat2\clcfpat1\clshdng0"),
        (r"\trcbpat2\trcfpat1\trshdng10000", r"\clcbpat2\clcfpat1\clshdng10000"),
        (r"\trcbpat2", r"\clcbpat2"),
    ];
    for (row, cell) in cases {
        let by_row = fills(first_table(&two_rows(row, "")));
        let by_cell = fills(first_table(&two_rows("", cell)));
        assert_eq!(by_row, by_cell, "{row} vs {cell}");
        assert!(by_row.iter().flatten().all(|f| f.is_some()), "{row}: {by_row:?}");
    }
    // A clear pattern shows the background colour.
    let d = two_rows(r"\trcbpat2\trcfpat1\trshdng0", "");
    assert_eq!(fills(first_table(&d))[0][0], Some(Rgb(0xFF, 0xCC, 0x00)));
}

#[test]
fn a_cells_own_shading_beats_the_rows() {
    // Word: the first cell keeps its own FF0000, the second takes the row's FFCC00.
    let d = open(r"\trowd\trgaph108\trcbpat2\clcbpat1\cellx1440\cellx2880\pard\intbl A\cell B\cell\row\pard AFTER\par");
    let t = first_table(&d);
    assert_eq!(fills(t), vec![vec![Some(Rgb(0xFF, 0x00, 0x00)), Some(Rgb(0xFF, 0xCC, 0x00))]]);
    assert_eq!(t.props.shading, None, "row shading is not table shading");
}

#[test]
fn trowd_resets_the_row_shading() {
    let d = open(r"\trowd\trgaph108\trcbpat2\cellx1440\pard\intbl A\cell\row\trowd\trgaph108\cellx1440\pard\intbl B\cell\row\pard AFTER\par");
    assert_eq!(fills(first_table(&d)), vec![vec![Some(Rgb(0xFF, 0xCC, 0x00))], vec![None]]);
}
