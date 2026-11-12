//! A formula field (`= …`, ECMA-376 §17.16.5.1) that cannot be worked out shows `!` and one of
//! Word's fixed error texts. Every row below was measured in Microsoft 365 Word (16.0.20527,
//! en-US) on 2026-11-12: the plain rows in paragraphs outside a table, the table rows in one
//! four-column table. The `!` is added where the field's result is written, so the evaluator's
//! error message is the text after it.

use wordcraft_doc::{Document, Paragraph, Table};

use crate::cmd::table_formula::{evaluate_field, evaluate_text_field};

/// What the field shows: its result, or `!` and the error message.
fn shown<E: std::fmt::Display>(result: Result<String, E>) -> String {
    match result {
        Ok(text) => text,
        Err(e) => format!("!{e}"),
    }
}

/// (field code, what Word shows) for fields in a paragraph outside a table; the rows that show
/// something else, as "code: got X, Word shows Y".
fn plain_mismatches(rows: &[(&str, &str)]) -> Vec<String> {
    let doc = Document::from_text("x");
    rows.iter()
        .filter_map(|(instr, want)| {
            let got = shown(evaluate_text_field(&doc, None, instr));
            (got != *want).then(|| format!("{instr}: got {got}, Word shows {want}"))
        })
        .collect()
}

/// The measured table, cells as typed (the other cells hold the fields; A1 holds the error text
/// its own field shows).
///
/// |   | A     | B   | C   | D |
/// | 1 | error |     | `2` |   |
/// | 2 | `3`   |     |     |   |
/// | 3 |       | `x` |     |   |
/// | 4 | `abc` |     |     |   |
fn measured_table() -> Table {
    let data: [(usize, usize, &str); 5] = [(0, 0, "!Table Index Cannot be Zero"), (0, 2, "2"), (1, 0, "3"), (2, 1, "x"), (3, 0, "abc")];
    let mut t = Table::new(4, 4, 468.0);
    for (r, c, text) in data {
        if let Some(cell) = t.rows.get_mut(r).and_then(|x| x.cells.get_mut(c)) {
            cell.blocks = vec![wordcraft_doc::para_block(Paragraph::with_text(text, Default::default()))];
        }
    }
    t
}

/// (row, column, field code, what Word shows), rows and columns counted from 0; the rows that
/// show something else.
fn table_mismatches(rows: &[(usize, usize, &str, &str)]) -> Vec<String> {
    let t = measured_table();
    rows.iter()
        .filter_map(|(r, c, instr, want)| {
            let got = shown(evaluate_field(&t, *r, *c, instr));
            (got != *want).then(|| format!("{instr} in row {} column {}: got {got}, Word shows {want}", r + 1, c + 1))
        })
        .collect()
}

#[test]
fn division_by_zero_shows_zero_divide() {
    let wrong = plain_mismatches(&[
        ("= 1 / 0", "!Zero Divide"),
        ("= 1 / (2 - 2)", "!Zero Divide"),
        ("= INT(1/0)", "!Zero Divide"),
        ("= 1 / 0 + nosuch", "!Zero Divide"),
        (r#"= 1 / 0 \# "0.00""#, "!Zero Divide"),
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let wrong = table_mismatches(&[(2, 2, "= C1/0", "!Zero Divide")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_formula_that_stops_early_shows_unexpected_end_of_formula() {
    let wrong = plain_mismatches(&[("= 2 +", "!Unexpected End of Formula"), ("= (1", "!Unexpected End of Formula")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_token_in_the_wrong_place_shows_syntax_error_and_the_token() {
    let wrong = plain_mismatches(&[
        ("= 5 ** 2", "!Syntax Error, *"),
        ("= 1 +* 2", "!Syntax Error, *"),
        (r#"= "abc""#, "!Syntax Error, \""),
        ("= 1 )", "!Syntax Error, )"),
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_function_closed_without_its_arguments_shows_syntax_error_at_the_parenthesis() {
    let wrong = plain_mismatches(&[
        ("= SUM()", "!Syntax Error, )"),
        ("= AVERAGE()", "!Syntax Error, )"),
        ("= ABS()", "!Syntax Error, )"),
        ("= IF(1>0,5)", "!Syntax Error, )"),
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn two_numbers_side_by_side_show_missing_operator() {
    let wrong = plain_mismatches(&[("= 2 3", "!Missing Operator")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn cells_and_sides_outside_a_table_show_the_formula_not_in_table() {
    let wrong = plain_mismatches(&[
        ("= SUM(ABOVE)", "!The Formula Not In Table"),
        ("= SUM(LEFT)", "!The Formula Not In Table"),
        ("= SUM(A1:A2)", "!The Formula Not In Table"),
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn left_in_the_first_cell_and_above_in_the_first_row_show_table_index_cannot_be_zero() {
    let wrong = table_mismatches(&[(0, 0, "= SUM(LEFT)", "!Table Index Cannot be Zero"), (0, 1, "= SUM(ABOVE)", "!Table Index Cannot be Zero")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn results_and_texts_word_and_we_already_agree_on_stay() {
    let wrong = plain_mismatches(&[("= nosuch", "!Undefined Bookmark, NOSUCH"), ("= 1 + 1", "2")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // RIGHT in the last cell, LEFT over text without digits, ABOVE over `3` and a cell showing an error.
    let wrong = table_mismatches(&[(0, 3, "= SUM(RIGHT)", "0"), (3, 1, "= SUM(LEFT)", "0"), (2, 0, "= SUM(ABOVE)", "3")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
