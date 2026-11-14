//! Insert › Drop Cap › In margin when the first-line or hanging indent comes from the paragraph's
//! style (review of #5992): the effective indent moves to the cap's frame paragraph
//! (`w:framePr w:dropCap="margin"`, ECMA-376 Part 1 §17.3.1.11) exactly as a direct one does, and
//! Dropped / None give the paragraph back its inherited indent.

use std::io::Read;

use serde_json::{Value, json};
use wordcraft_doc::props::ParaProps;

use crate::Session;

fn run(s: &mut Session, cmd: &str, v: Value) -> Value {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd} {v}: {e:?}"))
}

/// One paragraph in style `IndPara` (based on Normal) whose paragraph props are `style`, with
/// `direct` as the paragraph's own props.
fn session(style: ParaProps, direct: ParaProps) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    let mut st = s.doc.styles.get("Normal").cloned().unwrap();
    st.id = "IndPara".into();
    st.name = "Ind Para".into();
    st.based_on = Some("Normal".into());
    st.para = style;
    std::sync::Arc::make_mut(&mut s.doc.styles).upsert(st);
    run(&mut s, "text.insert", json!({"text": "Once upon a time there was a letter"}));
    let p = s.doc.para_mut(wordcraft_doc::StoryRef::Body, &wordcraft_doc::Path::top(0)).unwrap();
    p.props = ParaProps { style: Some("IndPara".into()), ..direct };
    s
}

fn first_line(v: f32) -> ParaProps {
    ParaProps { indent_first: Some(v), ..ParaProps::default() }
}

fn props(s: &Session) -> ParaProps {
    s.doc.body.first().and_then(|b| b.as_para()).unwrap().props.clone()
}

/// The text paragraph's resolved (left, first-line) indents, points.
fn resolved(s: &Session) -> (f32, f32) {
    let r = s.doc.styles.resolve_para(&props(s));
    (r.indent_left, r.indent_first)
}

fn document_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    xml
}

/// Each saved body paragraph's `w:pPr`, as written.
fn ppr_of_paragraphs(xml: &str) -> Vec<String> {
    xml.split("<w:p ")
        .chain(xml.split("<w:p>"))
        .filter(|chunk| chunk.contains("</w:p>"))
        .map(|chunk| chunk.split("</w:p>").next().unwrap_or(""))
        .map(|p| p.split("<w:pPr>").nth(1).and_then(|r| r.split("</w:pPr>").next()).unwrap_or("").to_string())
        .collect()
}

/// (cap paragraph's pPr, text paragraph's pPr) in the saved file.
fn saved(s: &Session) -> (String, String) {
    let xml = document_xml(&wordcraft_docx::write(&s.doc).unwrap());
    let pprs = ppr_of_paragraphs(&xml);
    let cap = pprs.iter().find(|p| p.contains("w:dropCap=\"margin\"")).cloned().unwrap_or_else(|| panic!("no margin cap paragraph in {xml}"));
    let text = pprs.iter().find(|p| !p.contains("w:framePr") && !p.contains("w:sectPr")).cloned().unwrap_or_default();
    (cap, text)
}

/// The `w:ind` element of a pPr (empty when there is none).
fn ind(ppr: &str) -> String {
    ppr.split("<w:ind ").nth(1).and_then(|r| r.split("/>").next()).map(|a| a.trim().to_string()).unwrap_or_default()
}

#[test]
fn margin_moves_a_first_line_indent_from_the_style() {
    let mut s = session(first_line(36.0), ParaProps::default());
    run(&mut s, "insert.dropCap", json!({"position": "margin"}));
    assert_eq!(resolved(&s).1, 0.0, "the text paragraph no longer has the style's first-line indent");
    assert_eq!(props(&s).drop_cap_indent_first, Some(36.0));
    let (cap, text) = saved(&s);
    assert!(ind(&cap).split_whitespace().any(|a| a == "w:firstLine=\"720\""), "cap ind: {}", ind(&cap));
    assert!(!ind(&text).contains("w:firstLine=\"720\""), "text ind: {}", ind(&text));
}

#[test]
fn margin_moves_a_hanging_indent_from_the_style() {
    let mut s = session(ParaProps { indent_left: Some(36.0), indent_first: Some(-36.0), ..ParaProps::default() }, ParaProps::default());
    run(&mut s, "insert.dropCap", json!({"position": "margin"}));
    assert_eq!(resolved(&s), (36.0, 0.0), "all text lines start at the left indent");
    let (cap, _) = saved(&s);
    assert!(ind(&cap).split_whitespace().any(|a| a == "w:hanging=\"720\""), "cap ind: {}", ind(&cap));
}

#[test]
fn dropped_after_margin_inherits_the_style_indent_again() {
    let mut s = session(first_line(36.0), ParaProps::default());
    run(&mut s, "insert.dropCap", json!({"position": "margin"}));
    run(&mut s, "insert.dropCap", json!({"position": "dropped"}));
    assert_eq!(resolved(&s).1, 36.0);
    assert_eq!(props(&s).indent_first, None, "no direct copy of the style's indent");
    assert_eq!(props(&s).drop_cap_indent_first, None);
}

#[test]
fn none_after_margin_inherits_the_style_indent_again() {
    let mut s = session(first_line(36.0), ParaProps::default());
    run(&mut s, "insert.dropCap", json!({"position": "margin"}));
    run(&mut s, "insert.dropCap", json!({"position": "none"}));
    assert_eq!(resolved(&s).1, 36.0);
    assert_eq!(props(&s).indent_first, None);
    assert_eq!(props(&s).drop_cap_indent_first, None);
}

#[test]
fn undo_restores_the_paragraph_in_one_step() {
    let mut s = session(first_line(36.0), ParaProps::default());
    let before = props(&s);
    run(&mut s, "insert.dropCap", json!({"position": "margin"}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(props(&s), before);
}

#[test]
fn no_inherited_indent_writes_no_explicit_zero() {
    let mut s = session(ParaProps::default(), ParaProps::default());
    run(&mut s, "insert.dropCap", json!({"position": "margin"}));
    assert_eq!(props(&s).indent_first, None);
    assert_eq!(props(&s).drop_cap_indent_first, None);
}

#[test]
fn a_direct_indent_over_the_style_comes_back_direct() {
    let mut s = session(first_line(36.0), first_line(18.0));
    run(&mut s, "insert.dropCap", json!({"position": "margin"}));
    assert_eq!(resolved(&s).1, 0.0);
    assert_eq!(props(&s).drop_cap_indent_first, Some(18.0));
    run(&mut s, "insert.dropCap", json!({"position": "dropped"}));
    assert_eq!(resolved(&s).1, 18.0);
    assert_eq!(props(&s).indent_first, Some(18.0));
}
