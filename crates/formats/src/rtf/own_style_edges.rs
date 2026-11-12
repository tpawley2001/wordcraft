//! Edge cases of a document's own styles through RTF: duplicate names, `\sbasedon` loops, names
//! that are built-in styles' ids, and the built-in entries Word writes.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::styles::StyleKind;

fn open(stylesheet: &str, body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{{\stylesheet{{\s0 Normal;}}{stylesheet}}}{body}}}").as_bytes()).unwrap()
}

#[test]
fn entries_with_the_same_id_get_a_number() {
    let d = open(r"{\s15 My Style;}{\s16 My-Style;}", r"\pard\s15 a\par\pard\s16 b\par");
    assert_eq!(d.styles.get("MyStyle").map(|s| s.name.as_str()), Some("My Style"));
    assert_eq!(d.styles.get("MyStyle2").map(|s| s.name.as_str()), Some("My-Style"));
    let ids: Vec<_> = d.body.iter().map(|b| b.as_para().unwrap().props.style.clone()).collect();
    assert_eq!(ids, [Some("MyStyle".to_string()), Some("MyStyle2".to_string())]);
}

#[test]
fn a_based_on_loop_imports_and_resolves() {
    let d = open(r"{\s15\sbasedon16\li720 Alpha;}{\s16\sbasedon15\ri360 Beta;}", r"\pard\s15 a\par");
    assert_eq!(d.styles.get("Alpha").and_then(|s| s.based_on.as_deref()), Some("Beta"));
    assert_eq!(d.styles.get("Beta").and_then(|s| s.based_on.as_deref()), Some("Alpha"));
    // The style resolves through the loop (the paragraph, stating no indent, has none).
    let p = d.body[0].as_para().unwrap();
    assert_eq!(p.props.style.as_deref(), Some("Alpha"));
    let alpha = wordcraft_doc::props::ParaProps { style: Some("Alpha".into()), ..Default::default() };
    let r = d.styles.resolve_para(&alpha);
    assert_eq!((r.indent_left, r.indent_right), (36.0, 18.0));
    let back = import(export(&d).as_bytes()).unwrap();
    assert!(back.styles.get("Alpha").is_some() && back.styles.get("Beta").is_some());
}

#[test]
fn a_character_style_named_like_a_paragraph_kind_keeps_the_kind() {
    let d = open(r"{\*\cs15\additive\b Title;}{\s7 Title;}", r"\pard\s7 Big {\cs15 word}\par");
    assert_eq!(d.styles.get("Title").map(|s| s.kind), Some(StyleKind::Paragraph));
    assert_eq!(d.styles.get("Title2").map(|s| s.kind), Some(StyleKind::Character));
    let p = d.body[0].as_para().unwrap();
    assert_eq!(p.props.style.as_deref(), Some("Title"));
}

#[test]
fn words_built_in_entries_are_not_own_styles() {
    let d = open(
        r"{\*\cs10\additive\ssemihidden Default Paragraph Font;}{\*\ts11\tsrowd\ssemihidden Normal Table;}{\*\cs15\additive\sbasedon10\ul\cf0 Hyperlink;}{\*\cs16\additive\sbasedon10\b Emphasis Plus;}",
        r"\pard {\cs16 x}\par",
    );
    assert!(d.styles.get("NormalTable").is_none_or(|s| s.kind == StyleKind::Table));
    assert_eq!(d.styles.get("Hyperlink").map(|s| s.kind), Some(StyleKind::Character));
    let s = d.styles.get("EmphasisPlus").expect("style EmphasisPlus");
    assert_eq!(s.based_on.as_deref(), Some("DefaultParagraphFont"));
    // Based on Default Paragraph Font: written without `\sbasedon`.
    let rtf = export(&d);
    let entry = rtf.split("{\\*\\cs").find(|e| e.contains("Emphasis Plus;")).expect("entry");
    assert!(!entry.contains("\\sbasedon"), "{entry}");
}

#[test]
fn hostile_style_numbers_do_not_panic() {
    for sheet in [r"{\s-5\sbasedon-3\snext2147483647\slink-2147483648 A;}", r"{\*\cs-2147483648\slink0 B;}", r"{\s15\spriority-1 ;}"] {
        let d = open(sheet, r"\pard\s-5 a {\cs-2147483648 b}\par");
        let _ = import(export(&d).as_bytes()).unwrap();
    }
}
