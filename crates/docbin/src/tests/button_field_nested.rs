//! A field nested in the code of a GOTOBUTTON or MACROBUTTON field contributes its result to the
//! button's display text, not its code ([MS-DOC] §2.8.25 PlcFld: 0x13 begin, 0x14 separator,
//! 0x15 end). Equation numbers are written this way: a MACROBUTTON whose text holds SEQ fields.

use super::{read, struct_doc, structured_spec};
use wordcraft_doc::para::InlineObject;

fn open(text: String) -> wordcraft_doc::Document {
    let text: &'static str = Box::leak(text.into_boxed_str());
    let spec = structured_spec(text, text.chars().count() as u32);
    read(&struct_doc(&spec)).expect("opens")
}

/// The codes of the document's inline field objects, in order.
fn codes(doc: &wordcraft_doc::Document) -> Vec<String> {
    doc.body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| &p.objects)
        .filter_map(|o| match o {
            InlineObject::Field { instr, .. } => Some(instr.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_equation_number_keeps_only_the_results_of_its_nested_fields() {
    let doc = open(
        "x\u{13} MACROBUTTON MTPlaceRef \\* MERGEFORMAT \u{13} SEQ MTEqn \\h \\* MERGEFORMAT \u{14}\u{15}(\u{13} SEQ MTEqn \\c \\* Arabic \\* MERGEFORMAT \u{14}1\u{15})\u{15}y\r"
            .to_string(),
    );
    assert_eq!(codes(&doc), [r"MACROBUTTON MTPlaceRef \* MERGEFORMAT (1)"]);
}

#[test]
fn a_nested_field_with_no_result_adds_nothing() {
    let doc = open("\u{13}MACROBUTTON NoMacro x\u{13} QUOTE \"QQ\" \u{15}y\u{15}\r".to_string());
    assert_eq!(codes(&doc), ["MACROBUTTON NoMacro xy"]);
}

#[test]
fn a_gotobutton_takes_the_nested_result_too() {
    let doc = open("\u{13}GOTOBUTTON bm a\u{13} QUOTE q \u{14}RR\u{15}b\u{15}\r".to_string());
    assert_eq!(codes(&doc), ["GOTOBUTTON bm aRRb"]);
    let doc = open("\u{13}macrobutton NoMacro a\u{13} QUOTE q \u{14}RR\u{15}b\u{15}\r".to_string());
    assert_eq!(codes(&doc), ["macrobutton NoMacro aRRb"]);
}

#[test]
fn a_field_in_the_nested_code_is_dropped_and_one_in_the_nested_result_is_kept() {
    let doc = open("\u{13}MACROBUTTON NoMacro a\u{13} QUOTE \u{13} PAGE \u{14}3\u{15} \u{14}R\u{15}b\u{15}\r".to_string());
    assert_eq!(codes(&doc), ["MACROBUTTON NoMacro aRb"]);
    let doc = open("\u{13}MACROBUTTON NoMacro a\u{13} QUOTE \u{14}R\u{13} PAGE \u{14}3\u{15}S\u{15}b\u{15}\r".to_string());
    assert_eq!(codes(&doc), ["MACROBUTTON NoMacro aR3Sb"]);
}

#[test]
fn any_other_field_keeps_the_nested_code() {
    let doc = open("\u{13} IF \u{13} MERGEFIELD a \u{14}1\u{15} = 1 \"y\" \"n\" \u{14}y\u{15}\r".to_string());
    let codes = codes(&doc);
    assert_eq!(codes.len(), 1);
    assert!(codes.first().is_some_and(|c| c.starts_with("IF") && c.contains("MERGEFIELD a")), "{codes:?}");
}

#[test]
fn ten_thousand_nested_begins_are_bounded() {
    let text = format!("\u{13}MACROBUTTON NoMacro a{}{}b\u{15}\r", "\u{13}".repeat(10_000), "\u{15}".repeat(10_000));
    let doc = open(text);
    assert_eq!(codes(&doc), ["MACROBUTTON NoMacro ab"]);
}
