//! A document's own styles through RTF (RTF 1.9.1, "Style sheet"): custom paragraph (`\sN`) and
//! character (`\*\csN`) styles become document styles with their based-on, next and linked styles
//! and their flags (`\sqformat`, `\spriority`, `\ssemihidden`, `\slocked`), paragraphs and runs keep
//! their style, and export writes them back.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::styles::StyleKind;

fn open(stylesheet: &str, body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{{\stylesheet{{\s0 Normal;}}{stylesheet}}}{body}}}").as_bytes()).unwrap()
}

/// The text of each run of `p` in character style `id`.
fn styled_text<'a>(p: &'a wordcraft_doc::Paragraph, id: &str) -> Vec<&'a str> {
    p.run_ranges().filter(|(_, c)| c.style.as_deref() == Some(id)).map(|(r, _)| &p.text[r]).collect()
}

const BODY_TEXT_2: &str = r"{\s15\sbasedon0\snext15\sqformat\spriority9\li720\i Body Text 2;}";

#[test]
fn a_custom_paragraph_style_becomes_a_document_style() {
    let d = open(BODY_TEXT_2, r"\pard\s15\li720\i Hello\par");
    let s = d.styles.get("BodyText2").expect("style BodyText2");
    assert_eq!(s.name, "Body Text 2");
    assert_eq!(s.kind, StyleKind::Paragraph);
    assert_eq!(s.based_on.as_deref(), Some("Normal"));
    assert_eq!(s.next.as_deref(), Some("BodyText2"));
    assert!(s.quick);
    assert_eq!(s.priority, Some(9));
    assert!(!s.hidden);
    let p = d.body[0].as_para().unwrap();
    assert_eq!(p.props.style.as_deref(), Some("BodyText2"));
    assert_eq!(d.styles.resolve_para(&p.props).indent_left, 36.0);
}

#[test]
fn a_custom_character_style_becomes_a_document_style_and_marks_its_runs() {
    let d = open(r"{\*\cs10\additive\b Strong Word;}", r"\pard plain {\cs10\b bold} plain\par");
    let s = d.styles.get("StrongWord").expect("style StrongWord");
    assert_eq!(s.name, "Strong Word");
    assert_eq!(s.kind, StyleKind::Character);
    let p = d.body[0].as_para().unwrap();
    assert_eq!(styled_text(p, "StrongWord"), vec!["bold"]);
}

#[test]
fn linked_hidden_and_locked_styles_keep_their_flags() {
    let d =
        open(r"{\s16\sbasedon0\snext0\slink17\ssemihidden\slocked My Quote;}{\*\cs17\additive\slink16\i My Quote Char;}", r"\pard\s16 Quoted\par");
    let q = d.styles.get("MyQuote").expect("style MyQuote");
    assert_eq!(q.linked.as_deref(), Some("MyQuoteChar"));
    assert_eq!(q.next.as_deref(), Some("Normal"));
    assert!(q.hidden);
    assert!(q.locked);
    let c = d.styles.get("MyQuoteChar").expect("style MyQuoteChar");
    assert_eq!(c.kind, StyleKind::Character);
    assert_eq!(c.linked.as_deref(), Some("MyQuote"));
}

#[test]
fn custom_styles_survive_export_and_import() {
    let d = open(BODY_TEXT_2, r"\pard\s15\li720\i Hello\par\pard\s0 Plain\par");
    let rtf = export(&d);
    assert!(rtf.contains("Body Text 2;"), "{rtf}");
    let back = import(rtf.as_bytes()).unwrap();
    let s = back.styles.get("BodyText2").expect("style BodyText2 after round trip");
    assert_eq!(s.kind, StyleKind::Paragraph);
    assert_eq!(s.based_on.as_deref(), Some("Normal"));
    assert_eq!(s.next.as_deref(), Some("BodyText2"));
    assert!(s.quick);
    assert_eq!(s.priority, Some(9));
    let p = back.body[0].as_para().unwrap();
    assert_eq!(p.props.style.as_deref(), Some("BodyText2"));
    assert_eq!(back.styles.resolve_para(&p.props).indent_left, 36.0);
    let plain = back.body[1].as_para().unwrap();
    assert_ne!(plain.props.style.as_deref(), Some("BodyText2"));
}

#[test]
fn a_custom_character_style_survives_export_and_import() {
    let d = open(r"{\*\cs10\additive\b Strong Word;}", r"\pard plain {\cs10\b bold} plain\par");
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(back.styles.get("StrongWord").map(|s| s.kind), Some(StyleKind::Character));
    let p = back.body[0].as_para().unwrap();
    assert_eq!(styled_text(p, "StrongWord"), vec!["bold"]);
}

#[test]
fn built_in_heading_entries_still_map_to_the_heading_styles() {
    let d = open(r"{\s1\sbasedon0\snext0 heading 1;}", r"\pard\s1 Title line\par");
    let p = d.body[0].as_para().unwrap();
    assert_eq!(p.props.style.as_deref(), Some("Heading1"));
    assert!(d.styles.get("heading1").is_none());
}
