//! An IF whose condition holds a nested field with no stored text (as imported from
//! `{ IF { MERGEFIELD Amount } >= 100 "big" "small" }`) keeps its stored result instead of reading
//! the operator as an operand ("Error! Unknown op code for conditional."); Word shows the result.

use wordcraft_doc::para::{CodeParas, NestedCode, code_hash};

use super::conditional::result;

fn with_nested(instr: &str, at: usize, nested: &str, len: usize) -> Option<String> {
    let code =
        CodeParas { code: code_hash(instr), nested: vec![NestedCode { at, instr: nested.into() }], nested_len: vec![len], ..Default::default() };
    result(instr, Some(&code))
}

#[test]
fn an_empty_nested_left_operand_keeps_the_stored_result() {
    assert_eq!(with_nested(r#"IF  >= 100 "big" "small""#, 3, "MERGEFIELD Amount", 0), None);
}

#[test]
fn a_nested_left_operand_with_stored_text_is_compared() {
    assert_eq!(with_nested(r#"IF 150 >= 100 "big" "small""#, 3, "MERGEFIELD Amount", 3).as_deref(), Some("big"));
}

#[test]
fn an_empty_nested_right_operand_keeps_the_stored_result() {
    assert_eq!(with_nested(r#"IF 100 <=  "x" "y""#, 10, "MERGEFIELD Amount", 0), None);
}

#[test]
fn an_empty_nested_field_outside_the_condition_changes_nothing() {
    assert_eq!(with_nested(r#"IF 2 > 1 "big"  "#, 15, "MERGEFIELD X", 0).as_deref(), Some("big"));
}

#[test]
fn ifs_without_nested_fields_are_unchanged() {
    assert_eq!(result(r#"IF 1 = 1 "yes" """#, None).as_deref(), Some("yes"));
    assert_eq!(result("IF a b c", None).as_deref(), Some("Error! Unknown op code for conditional."));
}
