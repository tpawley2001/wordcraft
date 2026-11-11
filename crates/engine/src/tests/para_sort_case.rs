//! Home › Paragraph › Sort › Options… › Case sensitive (`para.sort` `caseSensitive`): off, keys that
//! differ only in letter case are equal and keep document order; on, such a tie is broken with
//! lowercase first, as in Word. Letter order always decides first.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;

/// Paragraphs from `lines` (one per line), English (US), all selected, sorted with `v`.
fn sorted(lines: &[&str], v: Value) -> Vec<String> {
    let mut s = Session::new(Document::from_text(&lines.join("\n")));
    s.run("review.language", &json!({"lang": "en-US", "default": true})).unwrap();
    s.run("select.all", &json!({})).unwrap();
    s.run("para.sort", &v).unwrap_or_else(|e| panic!("para.sort {v}: {e}"));
    s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()
}

const FRUIT: &[&str] = &["banana", "Apple", "apple", "Banana"];

#[test]
fn off_by_default_ties_keep_document_order() {
    assert_eq!(sorted(FRUIT, json!({})), ["Apple", "apple", "banana", "Banana"]);
    assert_eq!(sorted(FRUIT, json!({"caseSensitive": false})), ["Apple", "apple", "banana", "Banana"]);
    assert_eq!(sorted(FRUIT, json!({"caseSensitive": false, "keys": [{"type": "text"}]})), ["Apple", "apple", "banana", "Banana"]);
}

#[test]
fn on_puts_lowercase_first_among_equal_words() {
    assert_eq!(sorted(FRUIT, json!({"caseSensitive": true})), ["apple", "Apple", "banana", "Banana"]);
    assert_eq!(sorted(FRUIT, json!({"caseSensitive": true, "keys": [{"type": "text"}]})), ["apple", "Apple", "banana", "Banana"]);
}

#[test]
fn on_keeps_letter_order_first() {
    // Not a code-point compare: a capital does not sort before every lowercase letter.
    assert_eq!(sorted(&["b", "A", "c"], json!({"caseSensitive": true})), ["A", "b", "c"]);
    assert_eq!(sorted(&["b", "A", "c"], json!({"caseSensitive": true, "keys": [{"type": "text"}]})), ["A", "b", "c"]);
}

#[test]
fn descending_reverses_the_case_order_too() {
    assert_eq!(sorted(FRUIT, json!({"caseSensitive": true, "descending": true})), ["Banana", "banana", "Apple", "apple"]);
    assert_eq!(
        sorted(FRUIT, json!({"caseSensitive": true, "keys": [{"type": "text", "descending": true}]})),
        ["Banana", "banana", "Apple", "apple"]
    );
}

#[test]
fn a_field_key_is_case_sensitive() {
    let v = json!({"caseSensitive": true, "separator": "tab", "keys": [{"field": 1, "type": "text"}]});
    assert_eq!(sorted(&["1\tDelta", "2\tdelta", "3\tAlpha"], v), ["3\tAlpha", "2\tdelta", "1\tDelta"]);
}

#[test]
fn case_of_the_first_key_decides_before_the_second_key() {
    let v = json!({"caseSensitive": true, "separator": "tab", "keys": [{"field": 0, "type": "text"}, {"field": 1, "type": "text"}]});
    assert_eq!(sorted(&["Smith\ta", "smith\tb", "Smith\tc"], v.clone()), ["smith\tb", "Smith\ta", "Smith\tc"]);
    // Without the option the first key ties on all three and the second key decides.
    let mut off = v;
    off["caseSensitive"] = json!(false);
    assert_eq!(sorted(&["Smith\ta", "smith\tb", "Smith\tc"], off), ["Smith\ta", "smith\tb", "Smith\tc"]);
}

#[test]
fn number_keys_are_not_affected() {
    assert_eq!(sorted(&["10", "9", "100"], json!({"caseSensitive": true, "keys": [{"type": "number"}]})), ["9", "10", "100"]);
}
