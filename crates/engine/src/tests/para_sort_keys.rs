//! Home › Paragraph › Sort (Word's Sort Text): up to three keys, each the whole paragraph or one
//! field (split at tabs, commas or another character), with a Type (Text, Number or Date) and its
//! own order; a header paragraph stays on top.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::{CmdError, Session};

/// Paragraphs from `lines` (one per line), English (US), all selected.
fn paras(lines: &[&str]) -> Session {
    let mut s = Session::new(Document::from_text(&lines.join("\n")));
    s.run("review.language", &json!({"lang": "en-US", "default": true})).unwrap();
    s.run("select.all", &json!({})).unwrap();
    s
}

fn sort(s: &mut Session, v: Value) {
    s.run("para.sort", &v).unwrap_or_else(|e| panic!("para.sort {v}: {e}"));
}

fn texts(s: &Session) -> Vec<String> {
    s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()
}

const PEOPLE: &[&str] = &["Smith\tBob", "Adams\tZoe", "Smith\tAl", "Adams\tAmy"];

#[test]
fn fields_split_at_tabs_and_then_by_breaks_ties() {
    // By first name (field 1), then surname (field 0); the whole paragraphs would sort Adams first.
    let mut s = paras(&["Smith\tAl", "Adams\tZoe", "Jones\tAl", "Adams\tAmy"]);
    sort(&mut s, json!({"separator": "tab", "keys": [{"field": 1, "type": "text"}, {"field": 0, "type": "text"}]}));
    assert_eq!(texts(&s), ["Jones\tAl", "Smith\tAl", "Adams\tAmy", "Adams\tZoe"]);
}

#[test]
fn each_key_has_its_own_order() {
    let mut s = paras(PEOPLE);
    sort(&mut s, json!({"separator": "tab", "keys": [{"field": 0, "type": "text", "descending": true}, {"field": 1, "type": "text"}]}));
    assert_eq!(texts(&s), ["Smith\tAl", "Smith\tBob", "Adams\tAmy", "Adams\tZoe"]);
}

#[test]
fn number_type_on_a_comma_field() {
    // As text, "10" < "100" < "9"; as numbers 9 < 10 < 100. Spaces around a field are ignored.
    let lines = ["apples, 10", "pears, 9", "figs, 100"];
    let mut s = paras(&lines);
    sort(&mut s, json!({"separator": "comma", "keys": [{"field": 1, "type": "number"}]}));
    assert_eq!(texts(&s), ["pears, 9", "apples, 10", "figs, 100"]);
    let mut s = paras(&lines);
    sort(&mut s, json!({"separator": "comma", "keys": [{"field": 1, "type": "text"}]}));
    assert_eq!(texts(&s), ["apples, 10", "figs, 100", "pears, 9"]);
}

#[test]
fn date_type_orders_by_date_not_text() {
    let lines = ["Launch\t2/14/2026", "Kickoff\t12/1/2025", "Review\t10/1/2026"];
    let mut s = paras(&lines);
    sort(&mut s, json!({"separator": "tab", "keys": [{"field": 1, "type": "date"}]}));
    assert_eq!(texts(&s), ["Kickoff\t12/1/2025", "Launch\t2/14/2026", "Review\t10/1/2026"]);
    let mut s = paras(&lines);
    sort(&mut s, json!({"separator": "tab", "keys": [{"field": 1, "type": "text"}]}));
    assert_eq!(texts(&s), ["Review\t10/1/2026", "Kickoff\t12/1/2025", "Launch\t2/14/2026"]);
}

#[test]
fn a_key_without_a_field_sorts_by_the_whole_paragraph() {
    let mut s = paras(&["10 apples", "9 pears", "100 figs"]);
    sort(&mut s, json!({"keys": [{"type": "number", "descending": true}]}));
    assert_eq!(texts(&s), ["100 figs", "10 apples", "9 pears"]);
}

#[test]
fn another_character_separates_fields() {
    let mut s = paras(&["b;2", "a;3", "c;1"]);
    sort(&mut s, json!({"separator": ";", "keys": [{"field": 1, "type": "number"}]}));
    assert_eq!(texts(&s), ["c;1", "b;2", "a;3"]);
}

#[test]
fn a_header_paragraph_stays_on_top() {
    let mut s = paras(&["Name\tScore", "Bea\t10", "Al\t7", "Cy\t12"]);
    sort(&mut s, json!({"separator": "tab", "header": true, "keys": [{"field": 1, "type": "number"}]}));
    assert_eq!(texts(&s), ["Name\tScore", "Al\t7", "Bea\t10", "Cy\t12"]);
    let mut s = paras(&["Name\tScore", "Bea\t10", "Al\t7", "Cy\t12"]);
    sort(&mut s, json!({"separator": "tab", "header": false, "keys": [{"field": 0, "type": "text"}]}));
    assert_eq!(texts(&s), ["Al\t7", "Bea\t10", "Cy\t12", "Name\tScore"]);
}

#[test]
fn a_missing_field_reads_as_empty_and_sorts_first_as_text() {
    let mut s = paras(&["a\tz", "c", "b\ty"]);
    sort(&mut s, json!({"separator": "tab", "keys": [{"field": 1, "type": "text"}]}));
    assert_eq!(texts(&s), ["c", "b\ty", "a\tz"]);
}

#[test]
fn bad_keys_are_refused_and_change_nothing() {
    for v in [
        json!({"keys": []}),
        json!({"keys": [{}, {}, {}, {}]}),
        json!({"keys": [{"type": "colour"}]}),
        json!({"separator": "", "keys": [{"field": 0}]}),
        json!({"separator": "ab", "keys": [{"field": 0}]}),
    ] {
        let mut s = paras(PEOPLE);
        let r = s.run("para.sort", &v);
        assert!(matches!(r, Err(CmdError::Params(_))), "{v}: {r:?}");
        assert_eq!(texts(&s), PEOPLE, "{v}");
    }
}

#[test]
fn the_one_key_form_is_unchanged() {
    // No `keys`: today's order (paragraphs that start with a number first, numerically, then the
    // rest in collation order), whole paragraphs.
    let mut s = paras(&["item 10", "banana", "10 figs", "2 apples"]);
    sort(&mut s, json!({}));
    assert_eq!(texts(&s), ["2 apples", "10 figs", "banana", "item 10"]);
}
