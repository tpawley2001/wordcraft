//! A drop cap that comes from the paragraph's style (#4144): Insert > Drop Cap > None turns it off
//! with an explicit zero-line drop cap on the paragraph, and the menu's Dropped / In margin
//! (`keep`) start from the style's lines, distance and font.

use serde_json::{Value, json};
use wordcraft_doc::props::ParaProps;

use crate::Session;

fn run(s: &mut Session, cmd: &str, v: Value) -> Value {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd} {v}: {e:?}"))
}

/// One paragraph in style `CapPara`, whose drop cap is 4 lines, 6 pt from the text, in Liberation Mono.
fn session() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    let mut st = s.doc.styles.get("Normal").cloned().unwrap();
    st.id = "CapPara".into();
    st.name = "Cap Para".into();
    st.based_on = Some("Normal".into());
    st.para = ParaProps { drop_cap: Some(4), drop_cap_distance: Some(6.0), drop_cap_font: Some("Liberation Mono".into()), ..ParaProps::default() };
    std::sync::Arc::make_mut(&mut s.doc.styles).upsert(st);
    run(&mut s, "text.insert", json!({"text": "Once upon a time"}));
    s.doc.para_mut(wordcraft_doc::StoryRef::Body, &wordcraft_doc::Path::top(0)).unwrap().props.style = Some("CapPara".into());
    s
}

fn props(s: &Session) -> ParaProps {
    s.doc.body.first().and_then(|b| b.as_para()).unwrap().props.clone()
}

fn resolved_lines(s: &Session) -> u8 {
    s.doc.styles.resolve_para(&props(s)).drop_cap
}

fn own(s: &Session) -> (Option<u8>, Option<bool>, Option<f32>, Option<String>) {
    let p = props(s);
    (p.drop_cap, p.drop_cap_margin, p.drop_cap_distance, p.drop_cap_font)
}

#[test]
fn none_turns_off_a_style_drop_cap() {
    let mut s = session();
    assert_eq!(resolved_lines(&s), 4, "the style gives the paragraph its drop cap");
    run(&mut s, "insert.dropCap", json!({"position": "none"}));
    assert_eq!(own(&s), (Some(0), None, None, None));
    assert_eq!(resolved_lines(&s), 0);
    assert_eq!(crate::cmd::insert::drop_cap_state(&s).position, "none");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(props(&s).drop_cap, None);
    assert_eq!(resolved_lines(&s), 4);
}

#[test]
fn keep_starts_from_the_style_values() {
    let mut s = session();
    run(&mut s, "insert.dropCap", json!({"position": "margin", "keep": true}));
    assert_eq!(own(&s), (Some(4), Some(true), Some(6.0), Some("Liberation Mono".to_string())));
    let mut s = session();
    run(&mut s, "insert.dropCap", json!({"position": "dropped", "keep": true, "lines": 2}));
    assert_eq!(own(&s), (Some(2), None, Some(6.0), Some("Liberation Mono".to_string())));
    assert_eq!(resolved_lines(&s), 2);
}

#[test]
fn dropped_after_none_gives_a_cap_again() {
    let mut s = session();
    run(&mut s, "insert.dropCap", json!({"position": "none"}));
    run(&mut s, "insert.dropCap", json!({"position": "dropped", "lines": 2}));
    assert_eq!(props(&s).drop_cap, Some(2));
    assert_eq!(resolved_lines(&s), 2);
}

#[test]
fn none_without_a_style_cap_writes_no_off_value() {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "text.insert", json!({"text": "Once upon a time"}));
    run(&mut s, "insert.dropCap", json!({"position": "dropped"}));
    run(&mut s, "insert.dropCap", json!({"position": "none"}));
    assert_eq!(props(&s).drop_cap, None);
}

#[test]
fn an_off_paragraph_saves_without_a_frame() {
    let mut s = session();
    run(&mut s, "insert.dropCap", json!({"position": "none"}));
    let bytes = wordcraft_docx::write(&s.doc).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut xml).unwrap();
    assert!(!xml.contains("w:framePr"), "{xml}");
    assert!(xml.contains("Once upon a time"));
}
