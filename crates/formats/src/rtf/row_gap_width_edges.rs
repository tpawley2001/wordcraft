//! Rows that start or end short of the table, as Microsoft 365 Word opens them: the grid comes from
//! all rows' cell edges, so such a row gets `grid_before` / `grid_after`, and
//! `\trftsWidthB`/`\trwWidthB`, `\trftsWidthA`/`\trwWidthA` give its `w_before` / `w_after` when it
//! has a gap there (RTF 1.9.1, "Table row definitions").

use super::*;
use wordcraft_doc::{Document, Table};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}{body}}}").as_bytes()).unwrap()
}

fn first_table(d: &Document) -> &Table {
    d.body.iter().find_map(|b| b.as_table()).unwrap()
}

/// A 3-column first row, then `second` (its row words and cell edges).
fn ragged(second: &str) -> Document {
    open(&format!(
        r"\trowd\trgaph108\trleft0\cellx1440\cellx2880\cellx4320\pard\intbl A\cell B\cell C\cell\row\trowd\trgaph108{second}\pard\intbl D\cell E\cell\row\pard AFTER\par"
    ))
}

fn gaps(t: &Table) -> Vec<(u32, Option<f32>, u32, Option<f32>)> {
    t.rows.iter().map(|r| (r.props.grid_before, r.props.w_before, r.props.grid_after, r.props.w_after)).collect()
}

#[test]
fn width_before_and_after_are_kept_with_their_gap() {
    // (second row, expected (grid_before, w_before, grid_after, w_after) of row 2), from Word's .docx.
    let cases = [
        (r"\trleft0\trftsWidthA3\trwWidthA720\cellx1440\cellx2880", (0, None, 1, Some(36.0))),
        (r"\trleft1440\trftsWidthB3\trwWidthB1440\cellx2880\cellx4320", (1, Some(72.0), 0, None)),
        // No gap before: Word drops the width before.
        (r"\trleft0\trftsWidthB3\trwWidthB1440\cellx1440\cellx2880", (0, None, 1, None)),
        // Only twips count; a negative width is dropped.
        (r"\trleft0\trftsWidthA1\trwWidthA720\cellx1440\cellx2880", (0, None, 1, None)),
        (r"\trleft0\trftsWidthA3\trwWidthA-720\cellx1440\cellx2880", (0, None, 1, None)),
    ];
    for (second, want) in cases {
        let d = ragged(second);
        let g = gaps(first_table(&d));
        assert_eq!(g[0], (0, None, 0, None), "{second}: first row");
        assert_eq!(g[1], want, "{second}");
    }
}

#[test]
fn width_before_and_after_are_written_and_read_back() {
    for second in [r"\trleft0\trftsWidthA3\trwWidthA720\cellx1440\cellx2880", r"\trleft1440\trftsWidthB3\trwWidthB1440\cellx2880\cellx4320"] {
        let d = ragged(second);
        let rtf = export(&d);
        let back = import(rtf.as_bytes()).unwrap();
        assert_eq!(gaps(first_table(&back)), gaps(first_table(&d)), "{second}\n{rtf}");
    }
    let rtf = export(&ragged(r"\trleft0\trftsWidthA3\trwWidthA720\cellx1440\cellx2880"));
    assert!(rtf.contains(r"\trftsWidthA3\trwWidthA720"), "{rtf}");
    // A row without a stated width writes none.
    let plain = export(&ragged(r"\trleft0\cellx1440\cellx2880"));
    assert!(!plain.contains(r"\trwWidthA") && !plain.contains(r"\trwWidthB"), "{plain}");
}
