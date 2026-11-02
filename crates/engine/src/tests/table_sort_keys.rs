//! Table › Sort with up to three keys (Sort by, Then by, Then by), each with a Type (Text, Number
//! or Date) and its own order, as Word's Sort dialog does.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;

/// A table of `rows` with the caret in it, English (US) text.
fn table(rows: &[&[&str]]) -> Session {
    let mut s = Session::new(Document::new());
    s.run("review.language", &json!({"lang": "en-US", "default": true})).unwrap();
    s.run("insert.table", &json!({"rows": rows.len(), "cols": rows[0].len()})).unwrap();
    for (k, c) in rows.iter().flat_map(|r| r.iter()).enumerate() {
        if k > 0 {
            s.run("text.tab", &json!({})).unwrap();
        }
        if !c.is_empty() {
            s.run("text.insert", &json!({"text": c})).unwrap();
        }
    }
    s
}

fn sort(s: &mut Session, keys: Value) {
    s.run("table.sort", &json!({"keys": keys, "header": false})).unwrap_or_else(|e| panic!("table.sort: {e}"));
}

/// The table's rows, each joined with `|`.
fn rows(s: &Session) -> Vec<String> {
    let t = s.doc.body.iter().find_map(|b| b.as_table()).unwrap();
    t.rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect::<String>())
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect()
}

const NAMES: &[&[&str]] = &[&["Smith", "Bob"], &["Adams", "Zoe"], &["Smith", "Al"], &["Adams", "Amy"]];

#[test]
fn then_by_breaks_ties_of_the_first_key() {
    let mut s = table(NAMES);
    sort(&mut s, json!([{"column": 0, "type": "text"}, {"column": 1, "type": "text"}]));
    assert_eq!(rows(&s), ["Adams|Amy", "Adams|Zoe", "Smith|Al", "Smith|Bob"]);
}

#[test]
fn each_key_has_its_own_order() {
    let mut s = table(NAMES);
    sort(&mut s, json!([{"column": 0, "type": "text"}, {"column": 1, "type": "text", "descending": true}]));
    assert_eq!(rows(&s), ["Adams|Zoe", "Adams|Amy", "Smith|Bob", "Smith|Al"]);
    let mut s = table(NAMES);
    sort(&mut s, json!([{"column": 0, "type": "text", "descending": true}, {"column": 1, "type": "text"}]));
    assert_eq!(rows(&s), ["Smith|Al", "Smith|Bob", "Adams|Amy", "Adams|Zoe"]);
}

#[test]
fn three_keys_and_equal_rows_keep_their_order() {
    let mut s = table(&[
        &["Sales", "2", "Kim", "a"],
        &["Ops", "10", "Lee", "b"],
        &["Sales", "10", "Ann", "c"],
        &["Sales", "10", "Ann", "d"],
        &["Ops", "10", "Bo", "e"],
        &["Sales", "10", "Zed", "f"],
    ]);
    sort(&mut s, json!([{"column": 0, "type": "text"}, {"column": 1, "type": "number", "descending": true}, {"column": 2, "type": "text"}]));
    assert_eq!(rows(&s), ["Ops|10|Bo|e", "Ops|10|Lee|b", "Sales|10|Ann|c", "Sales|10|Ann|d", "Sales|10|Zed|f", "Sales|2|Kim|a"]);
}

#[test]
fn text_type_compares_digits_as_characters_and_number_type_by_value() {
    let mut s = table(&[&["10"], &["9"], &["2"]]);
    sort(&mut s, json!([{"column": 0, "type": "text"}]));
    assert_eq!(rows(&s), ["10", "2", "9"]);
    sort(&mut s, json!([{"column": 0, "type": "number"}]));
    assert_eq!(rows(&s), ["2", "9", "10"]);
}

#[test]
fn number_type_puts_cells_without_a_number_after_the_numbers() {
    let mut s = table(&[&["n/a"], &["7"], &["-"], &["3"]]);
    sort(&mut s, json!([{"column": 0, "type": "number"}]));
    assert_eq!(rows(&s), ["3", "7", "n/a", "-"]);
    sort(&mut s, json!([{"column": 0, "type": "number", "descending": true}]));
    assert_eq!(rows(&s), ["n/a", "-", "7", "3"]);
}

#[test]
fn date_type_orders_us_and_iso_dates_by_day() {
    let mut s = table(&[&["3/1/2026"], &["12/5/2025"], &["2026-01-20"], &["1/15/2026"]]);
    sort(&mut s, json!([{"column": 0, "type": "date"}]));
    assert_eq!(rows(&s), ["12/5/2025", "1/15/2026", "2026-01-20", "3/1/2026"]);
    sort(&mut s, json!([{"column": 0, "type": "date", "descending": true}]));
    assert_eq!(rows(&s), ["3/1/2026", "2026-01-20", "1/15/2026", "12/5/2025"]);
    // An impossible date is not a date: it goes after the dates.
    let mut s = table(&[&["2/30/2026"], &["2/3/2026"], &["13/1/2026"], &["1/31/2026"]]);
    sort(&mut s, json!([{"column": 0, "type": "date"}]));
    assert_eq!(rows(&s), ["1/31/2026", "2/3/2026", "2/30/2026", "13/1/2026"]);
}

#[test]
fn the_header_row_stays_on_top() {
    let mut s = table(&[&["Last", "First"], &["Smith", "Bob"], &["Adams", "Zoe"], &["Smith", "Al"]]);
    s.run("table.sort", &json!({"keys": [{"column": 0, "type": "text"}, {"column": 1, "type": "text"}], "header": true})).unwrap();
    assert_eq!(rows(&s), ["Last|First", "Adams|Zoe", "Smith|Al", "Smith|Bob"]);
}

#[test]
fn bad_keys_are_refused_and_change_nothing() {
    for keys in [
        json!([]),
        json!([{"column": 0}, {"column": 1}, {"column": 0}, {"column": 1}]),
        json!([{"column": 2}]),
        json!([{"column": 0, "type": "colour"}]),
    ] {
        let mut s = table(NAMES);
        assert!(s.run("table.sort", &json!({"keys": keys, "header": false})).is_err(), "{keys}");
        assert_eq!(rows(&s), ["Smith|Bob", "Adams|Zoe", "Smith|Al", "Adams|Amy"], "{keys}");
    }
}

#[test]
fn the_one_column_form_still_works() {
    let mut s = table(&[&["10"], &["b"], &["9"], &["a"]]);
    s.run("table.sort", &json!({"column": 0, "header": false})).unwrap();
    assert_eq!(rows(&s), ["9", "10", "a", "b"]);
}
