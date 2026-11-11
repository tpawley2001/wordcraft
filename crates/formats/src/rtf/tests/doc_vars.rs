//! Document variables (`{\*\docvar {name}{value}}`, RTF 1.9.1 "Document Variables"; the RTF form
//! of DOCX `w:docVars`, ECMA-376 Part 1 §17.15.1.32): read into the model and written back.
//! Microsoft Word never shows these groups as text (measured).

use super::*;
use wordcraft_doc::{DocVar, StoryRef};

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

fn var(name: &str, value: &str) -> DocVar {
    DocVar { name: name.into(), value: value.into() }
}

fn with_vars(vars: &str) -> String {
    format!(r"{{\rtf1\ansi\ansicpg1252\uc1{{\info{{\title T}}}}{vars}\pard Xyzzy\par}}")
}

#[test]
fn variables_are_read_in_file_order_and_stay_out_of_the_text() {
    let d = doc(&with_vars(r"{\*\docvar {Client}{Acme}}{\*\docvar {Two Words}{Second value}}"));
    assert_eq!(d.settings.doc_vars, [var("Client", "Acme"), var("Two Words", "Second value")]);
    assert_eq!(d.doc_var("CLIENT"), Some("Acme"));
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Xyzzy");
    assert_eq!(d.core.title, "T", "the information group is untouched");
}

#[test]
fn names_and_values_decode_escapes_like_other_text() {
    // \'e9 is é in code page 1252; 舑 is an en dash with a one-byte fallback; \{ \} \\ are literal.
    let d = doc(&with_vars(r"{\*\docvar {Caf\'e9}{a舑?b \{x\} \\ y}}"));
    assert_eq!(d.settings.doc_vars, [var("Café", "a\u{2013}b {x} \\ y")]);
}

#[test]
fn a_variable_with_no_name_is_dropped_and_a_missing_value_is_empty() {
    let d = doc(&with_vars(r"{\*\docvar {}{lost}}{\*\docvar }{\*\docvar {OnlyName}}{\*\docvar {Kept}{1}}"));
    assert_eq!(d.settings.doc_vars, [var("OnlyName", ""), var("Kept", "1")]);
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Xyzzy");
}

#[test]
fn a_docvar_group_inside_a_header_is_not_a_document_variable() {
    let d = doc(r"{\rtf1\ansi{\header \pard Head{\*\docvar {InHeader}{no}}\par}\pard Xyzzy\par}");
    assert!(d.settings.doc_vars.is_empty(), "{:?}", d.settings.doc_vars);
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Xyzzy");
}

#[test]
fn over_long_names_and_values_are_skipped() {
    let long_name = "n".repeat(256);
    let long_value = "v".repeat(65_281);
    let d = doc(&with_vars(&format!(r"{{\*\docvar {{{long_name}}}{{x}}}}{{\*\docvar {{Big}}{{{long_value}}}}}{{\*\docvar {{Ok}}{{fits}}}}")));
    assert_eq!(d.settings.doc_vars, [var("Ok", "fits")]);
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Xyzzy");
}

#[test]
fn variables_are_written_before_the_body_and_read_back() {
    let mut d = doc(&with_vars(""));
    assert!(!export(&d).contains(r"\docvar"), "no variables: no group");
    d.settings.doc_vars = vec![var("Client", "Acme"), var("Caf\u{e9} {1}", "a\u{2013}b \\ {x}"), var("Blank", "")];
    let out = export(&d);
    assert_eq!(out.matches(r"{\*\docvar ").count(), 3, "{out}");
    assert!(out.contains(r"{\*\docvar {Client}{Acme}}"), "{out}");
    assert!(out.find(r"{\*\docvar ").unwrap() < out.find("Xyzzy").unwrap(), "{out}");
    let again = doc(&out);
    assert_eq!(again.settings.doc_vars, d.settings.doc_vars);
    assert_eq!(again.plain_text(StoryRef::Body).trim(), "Xyzzy");
}
