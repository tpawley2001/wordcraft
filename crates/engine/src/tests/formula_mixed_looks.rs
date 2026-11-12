//! A formula field (`= …`, ECMA-376 §17.16.5.1) whose operands look different, and the look a
//! range, a function or ABOVE/LEFT carries, as Microsoft Word shows them. Every table row below was
//! measured in Microsoft 365 Word (16.0.20527, en-US): the bookmark rows with the text in a
//! bookmark and the field in the page header, the table rows in one five-column table. Word
//! keeps a stored result until the field is updated, so these tests update the fields.

use serde_json::{Value, json};
use wordcraft_doc::{CharProps, InlineObject, Paragraph, StoryRef, Table};

use crate::Session;
use crate::cmd::table_formula::evaluate_field;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// (bookmark name, the text it marks): one paragraph each, at the start of the body. The names
/// are letters only, so none reads as a cell reference or a function.
const OPERANDS: [(&str, &str); 17] = [
    ("Dollar", "$5"),
    ("Dollartwo", "$2.50"),
    ("Plain", "3"),
    ("Neg", "(5)"),
    ("Comma", "1,234.50"),
    ("Euro", "\u{20ac}5"),
    ("Pound", "\u{a3}5"),
    ("Pct", "50%"),
    ("Dectwo", "2.50"),
    ("Big", "1234567"),
    ("Negd", "($5)"),
    ("Minusd", "-$5"),
    ("Dsix", "$600"),
    ("Dfive", "$500"),
    ("Dbig", "$1234"),
    ("Dc", "$5.5"),
    ("Cents", "$0.125"),
];

/// (field code, what Word shows after Update Field): a `$` on any operand makes the result a
/// currency (two decimals, a negative one in parentheses), a thousands comma on any operand
/// groups the result, and neither appears without an operand that has it.
const MIXED_ROWS: [(&str, &str); 25] = [
    ("= Dollar + Plain", "$8.00"),
    ("= Plain + Dollar", "$8.00"),
    ("= Dollar + Dollartwo", "$7.50"),
    ("= Dollar - 10", "($5.00)"),
    ("= Dollar * 2", "$10.00"),
    ("= Dollar / 3", "$1.67"),
    ("= Dollar + 1.25", "$6.25"),
    ("= Neg + 10", "5"),
    ("= Neg * Neg", "25"),
    ("= Comma + Dollar", "$1,239.50"),
    ("= Comma + Plain", "1,237.5"),
    ("= Euro + 1", "6"),
    ("= Pound + 1", "6"),
    ("= Pct + Pct", "1"),
    ("= Dectwo + 1", "3.5"),
    ("= Dectwo", "2.5"),
    ("= Big + 1", "1234568"),
    ("= Negd", "($5.00)"),
    ("= Negd + 1", "($4.00)"),
    ("= Minusd", "($5.00)"),
    ("= Minusd + 1", "($4.00)"),
    ("= Dsix + Dfive", "$1100.00"),
    ("= Dbig + 1", "$1235.00"),
    ("= Dc", "$5.50"),
    ("= Cents", "$0.125"),
];

/// (field code, what Word shows): a function's result carries its operands' look, COUNT too.
const FUNCTION_ROWS: [(&str, &str); 6] = [
    ("= SUM(Dollar,Plain)", "$8.00"),
    ("= MAX(Dollar,Plain)", "$5.00"),
    ("= MIN(Dollar,Plain)", "$3.00"),
    ("= PRODUCT(Dollar,Plain)", "$15.00"),
    ("= AVERAGE(Dollar,Plain)", "$4.00"),
    ("= COUNT(Dollar,Plain)", "$2.00"),
];

/// The operand paragraphs, each wrapped in its bookmark, then one paragraph per row ending in
/// its field with the stale result "OLD".
fn session(rows: &[(&str, &str)]) -> Session {
    let mut text: Vec<String> = OPERANDS.iter().map(|b| b.1.to_string()).collect();
    text.extend((0..rows.len()).map(|i| format!("F{:02}", i + 1)));
    let mut doc = wordcraft_doc::Document::from_text(&text.join("\n"));
    for (i, path) in doc.para_paths(StoryRef::Body).into_iter().enumerate() {
        let p = doc.para_mut(StoryRef::Body, &path).unwrap();
        if let Some((name, _)) = OPERANDS.get(i) {
            p.insert_object(p.len(), InlineObject::BookmarkEnd { name: (*name).into() }, &CharProps::default()).unwrap();
            p.insert_object(0, InlineObject::BookmarkStart { name: (*name).into() }, &CharProps::default()).unwrap();
            continue;
        }
        let field = InlineObject::Field {
            instr: rows[i - OPERANDS.len()].0.to_string(),
            result: "OLD".into(),
            locked: false,
            dirty: false,
            code: false,
            form: String::new(),
            code_paras: None,
        };
        p.insert_object(p.len(), field, &CharProps::default()).unwrap();
    }
    Session::new(doc)
}

/// The rows whose field shows something else than Word after Update Field, as
/// "code: got X, Word shows Y".
fn mismatches(rows: &[(&str, &str)]) -> Vec<String> {
    let mut s = session(rows);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.updateFields", json!({"all": true}));
    let mut got = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { instr, result, .. } = o {
                got.push((instr.clone(), result.clone()));
            }
        }
    }
    assert_eq!(got.len(), rows.len(), "one field per row");
    got.iter()
        .zip(rows)
        .filter(|((_, result), (_, want))| result != want)
        .map(|((code, result), (_, want))| format!("{code}: got {result}, Word shows {want}"))
        .collect()
}

#[test]
fn the_operands_are_bookmarked_as_written() {
    let s = session(&MIXED_ROWS);
    for (name, text) in OPERANDS {
        assert_eq!(crate::cmd::references::bookmark_text(&s.doc, name).as_deref(), Some(text), "{name}");
    }
}

#[test]
fn operands_with_different_looks_give_words_result() {
    let wrong = mismatches(&MIXED_ROWS);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_function_carries_its_operands_look() {
    let wrong = mismatches(&FUNCTION_ROWS);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Not measured row by row: it follows from the measured rule that a numeric picture replaces
/// the operand's look altogether.
#[test]
fn a_numeric_picture_still_wins_over_mixed_looks() {
    let rows = [(r#"= Dollar + Plain \# "0.0""#, "8.0"), (r#"= Dollar - 10 \# "0.00""#, "-5.00")];
    let wrong = mismatches(&rows);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The measured table: two rows of data, three empty rows the formulas sit in.
///
/// |   | A    | B            | C     | D       | E |
/// | 1 | `$5` | `3 apples 4` | `(5)` | `1,000` |   |
/// | 2 | `$3` | `2`          | `10`  | `234`   |   |
fn measured_table() -> Table {
    let data: [[&str; 4]; 2] = [["$5", "3 apples 4", "(5)", "1,000"], ["$3", "2", "10", "234"]];
    let mut t = Table::new(5, 5, 468.0);
    for (r, row) in data.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            if let Some(cell) = t.rows.get_mut(r).and_then(|x| x.cells.get_mut(c)) {
                cell.blocks = vec![wordcraft_doc::para_block(Paragraph::with_text(text, Default::default()))];
            }
        }
    }
    t
}

/// (row, column, field code, what Word shows), rows and columns counted from 0.
fn table_mismatches(rows: &[(usize, usize, &str, &str)]) -> Vec<String> {
    let t = measured_table();
    rows.iter()
        .filter_map(|(r, c, instr, want)| {
            let got = evaluate_field(&t, *r, *c, instr).unwrap_or_else(|e| format!("error: {e}"));
            (got != *want).then(|| format!("{instr} in row {} column {}: got {got}, Word shows {want}", r + 1, c + 1))
        })
        .collect()
}

#[test]
fn cell_references_with_different_looks_give_words_result() {
    let wrong = table_mismatches(&[(3, 0, "= A1 + A2", "$8.00"), (3, 1, "= A1 + B2", "$7.00"), (3, 2, "= C1 + C2", "5"), (4, 0, "= B1", "7"), (4, 3, "= D1 + D2", "1,234")]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_range_reads_each_cell_like_a_single_reference_and_carries_the_look() {
    let wrong = table_mismatches(&[
        (3, 3, "= SUM(A1:A2)", "$8.00"),
        (3, 4, "= SUM(A1:D1)", "$1,007.00"),
        (4, 1, "= SUM(B1:B2)", "9"),
        (4, 2, "= AVERAGE(A1:A2)", "$4.00"),
        (4, 4, "= MAX(A1:D1)", "$1,000.00"),
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn above_and_left_carry_the_look_and_stop_at_text_or_parentheses() {
    let wrong = table_mismatches(&[
        (2, 0, "= SUM(ABOVE)", "$8.00"),
        (2, 1, "= SUM(ABOVE)", "2"),
        (2, 2, "= SUM(ABOVE)", "10"),
        (2, 3, "= SUM(ABOVE)", "1,234"),
        (0, 4, "= SUM(LEFT)", "1,000"),
        (1, 4, "= SUM(LEFT)", "$249.00"),
    ]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
