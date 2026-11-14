//! Table of Figures › Options…: a table of figures built from a paragraph style (`\t`) or from
//! table entry fields with an identifier (`\f`), as Word builds them (P05-410, P05-411).

use super::*;
use crate::Session;

/// The body's entry paragraphs (the generated list), as plain text.
fn entries(s: &Session) -> Vec<String> {
    s.doc.body.iter().filter_map(|b| b.as_para()).filter(|p| p.props.style.as_deref() == Some(ENTRY_STYLE)).map(|p| p.plain_text()).collect()
}

/// The field codes of the body's TOC fields.
fn toc_codes(s: &Session) -> Vec<String> {
    let mut out = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { instr, .. } = o
                && instr.trim_start().starts_with("TOC")
            {
                out.push(instr.trim().to_string());
            }
        }
    }
    out
}

fn doc() -> Session {
    Session::new(wordcraft_doc::Document::from_text("Intro\nKiln\nBody\nPress\nEnd\n"))
}

/// Gives the paragraph holding `text` the paragraph style `style`.
fn style_para(s: &mut Session, text: &str, style: &str) {
    s.run("select.text", &json!({"text": text})).unwrap();
    s.run("para.style", &json!({"style": style})).unwrap();
}

/// Puts a field with code `instr` at the end of the paragraph holding `text`.
fn field_after(s: &mut Session, text: &str, instr: &str) {
    s.run("select.text", &json!({"text": text})).unwrap();
    s.run("caret.end", &json!({})).unwrap();
    s.run("insert.field", &json!({"instr": instr})).unwrap();
}

fn insert_tof(s: &mut Session, params: serde_json::Value) {
    s.run("caret.docEnd", &json!({})).unwrap();
    s.run("references.tableOfFigures", &params).unwrap();
}

#[test]
fn options_write_word_s_field_codes() {
    let cases = [
        (json!({"style": "Caption"}), r#"TOC \h \z \t "Caption" \c"#),
        (json!({"entryFields": "f"}), r#"TOC \h \z \f F \c"#),
        (json!({"style": "Heading 3", "entryFields": "T"}), r#"TOC \h \z \t "Heading 3" \f T \c"#),
        (json!({"style": "Caption", "hyperlinks": false}), r#"TOC \t "Caption" \c"#),
        (json!({"style": "Caption", "pageNumbers": false}), r#"TOC \h \z \t "Caption" \c \n"#),
        (json!({"label": "Table"}), r#"TOC \h \z \c "Table""#),
    ];
    for (params, want) in cases {
        let mut s = doc();
        insert_tof(&mut s, params.clone());
        assert_eq!(toc_codes(&s), [want.to_string()], "{params}");
    }
}

#[test]
fn bad_options_are_refused() {
    for params in [json!({"entryFields": "FF"}), json!({"entryFields": "1"}), json!({"entryFields": ""}), json!({"style": "No Such Style"})] {
        let mut s = doc();
        s.run("caret.docEnd", &json!({})).unwrap();
        assert!(s.run("references.tableOfFigures", &params).is_err(), "{params}");
        assert!(toc_codes(&s).is_empty(), "{params}");
    }
}

#[test]
fn a_style_list_holds_that_style_s_paragraphs_and_no_captions() {
    let mut s = doc();
    style_para(&mut s, "Kiln", "Heading 2");
    style_para(&mut s, "Press", "Heading 2");
    s.run("select.text", &json!({"text": "Body"})).unwrap();
    s.run("references.caption", &json!({"label": "Figure", "text": "Body"})).unwrap();
    insert_tof(&mut s, json!({"style": "Heading 2", "hyperlinks": false}));
    assert_eq!(entries(&s), ["Kiln\t1", "Press\t1"]);
    // Update Table keeps building it from the style.
    for _ in 0..2 {
        s.run("references.updateFigures", &json!({})).unwrap();
        assert_eq!(entries(&s), ["Kiln\t1", "Press\t1"]);
    }
}

#[test]
fn a_style_list_matches_a_style_by_name_in_any_case() {
    let mut s = doc();
    style_para(&mut s, "Press", "Heading 2");
    s.run("caret.docEnd", &json!({})).unwrap();
    s.run("insert.field", &json!({"instr": r#"TOC \t "heading 2,1" \c"#})).unwrap();
    s.run("references.updateFigures", &json!({})).unwrap();
    assert_eq!(entries(&s), ["Press\t1"]);
}

#[test]
fn an_entry_field_list_holds_the_tc_fields_with_its_identifier() {
    let mut s = doc();
    field_after(&mut s, "Kiln", r#"TC "Alpha" \f T"#);
    field_after(&mut s, "Body", r#"TC "Other" \f F"#);
    field_after(&mut s, "Press", r#"TC "Beta" \f t"#);
    insert_tof(&mut s, json!({"entryFields": "T", "hyperlinks": false}));
    assert_eq!(toc_codes(&s), [r#"TOC \f T \c"#.to_string()]);
    assert_eq!(entries(&s), ["Alpha\t1", "Beta\t1"]);
    s.run("references.updateFigures", &json!({})).unwrap();
    assert_eq!(entries(&s), ["Alpha\t1", "Beta\t1"]);
}

#[test]
fn both_options_list_in_document_order() {
    let mut s = doc();
    field_after(&mut s, "Intro", r#"TC "First" \f T"#);
    style_para(&mut s, "Kiln", "Heading 2");
    field_after(&mut s, "Press", r#"TC "Last" \f T"#);
    insert_tof(&mut s, json!({"style": "Heading 2", "entryFields": "T", "hyperlinks": false}));
    assert_eq!(entries(&s), ["First\t1", "Kiln\t1", "Last\t1"]);
}

#[test]
fn a_caption_list_is_unchanged() {
    let mut s = doc();
    s.run("select.text", &json!({"text": "Kiln"})).unwrap();
    s.run("references.caption", &json!({"label": "Figure", "text": "Kiln"})).unwrap();
    style_para(&mut s, "Press", "Heading 2");
    insert_tof(&mut s, json!({"hyperlinks": false}));
    assert_eq!(toc_codes(&s), [r#"TOC \c "Figure""#.to_string()]);
    assert_eq!(entries(&s), ["Figure 1: Kiln\t1"]);
}
