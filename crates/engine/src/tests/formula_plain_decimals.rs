//! A formula field (`= …`, ECMA-376 §17.16.5.1) with plain operands and no `\#` picture: Word
//! rounds the result to two decimals and drops trailing zeros (`4.5`, not `4.50`). Every row
//! below was measured in Microsoft 365 Word (16.0.20527, en-US) on 2026-11-12: the table rows in
//! one five-column table, the plain-number rows in paragraphs outside a table.

use wordcraft_doc::{Paragraph, Table};

use crate::cmd::table_formula::evaluate_field;

/// The measured table, cells as typed; rows 3 and 4 hold the fields, row 5 the LEFT run.
///
/// |   | A     | B      | C      | D      | E     |
/// | 1 | `1.5` | `1.50` | `2.50` | `1.25` | `0.1` |
/// | 2 | `3`   | `3`    | `2.50` | `3.5`  | `0.2` |
/// | 5 | text  | `1.5`  | `3`    |        |       |
fn measured_table() -> Table {
    let data: [(usize, [&str; 5]); 3] = [
        (0, ["1.5", "1.50", "2.50", "1.25", "0.1"]),
        (1, ["3", "3", "2.50", "3.5", "0.2"]),
        (4, ["!Table Index Cannot be Zero", "1.5", "3", "", ""]),
    ];
    let mut t = Table::new(5, 5, 468.0);
    for (r, row) in data {
        for (c, text) in row.iter().enumerate() {
            if let Some(cell) = t.rows.get_mut(r).and_then(|x| x.cells.get_mut(c)) {
                cell.blocks = vec![wordcraft_doc::para_block(Paragraph::with_text(text, Default::default()))];
            }
        }
    }
    t
}

/// (row, column, field code, what Word shows), rows and columns counted from 0; the rows that
/// show something else, as "code in row R column C: got X, Word shows Y".
fn mismatches(rows: &[(usize, usize, &str, &str)]) -> Vec<String> {
    let t = measured_table();
    rows.iter()
        .filter_map(|(r, c, instr, want)| {
            let got = evaluate_field(&t, *r, *c, instr).unwrap_or_else(|e| format!("error: {e}"));
            (got != *want).then(|| format!("{instr} in row {} column {}: got {got}, Word shows {want}", r + 1, c + 1))
        })
        .collect()
}

#[test]
fn sum_above_over_plain_decimals_drops_trailing_zeros() {
    let wrong = mismatches(&[
        (2, 0, "= SUM(ABOVE)", "4.5"),
        (2, 1, "= SUM(ABOVE)", "4.5"),
        (2, 2, "= SUM(ABOVE)", "5"),
        (2, 3, "= SUM(ABOVE)", "4.75"),
        (2, 4, "= SUM(ABOVE)", "0.3"),
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn sum_left_over_plain_decimals_drops_trailing_zeros() {
    let wrong = mismatches(&[(4, 3, "= SUM(LEFT)", "4.5")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn ranges_and_cell_maths_show_at_most_two_decimals() {
    let wrong = mismatches(&[
        (3, 0, "= SUM(A1:A2)", "4.5"),
        (3, 1, "= SUM(B1:B2)", "4.5"),
        (3, 2, "= C1+C2", "5"),
        (3, 3, "= D1*D2", "4.38"),
        (3, 4, "= E1/3", "0.03"),
        (4, 4, "= AVERAGE(B1:B2)", "2.25"),
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Measured in paragraphs outside a table; plain numbers evaluate the same way in a cell.
#[test]
fn plain_number_maths_drops_trailing_zeros() {
    let wrong = mismatches(&[(3, 0, "= 10 / 4", "2.5"), (3, 0, "= 2 * 3.10", "6.2"), (3, 0, "= 1 / 3", "0.33")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A numeric picture still decides the decimals.
#[test]
fn a_numeric_picture_keeps_its_zeros() {
    let wrong = mismatches(&[(3, 0, r#"= 10 / 4 \# "0.00""#, "2.50"), (2, 0, r#"= SUM(ABOVE) \# "0.00""#, "4.50")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
