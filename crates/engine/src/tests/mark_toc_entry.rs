//! Mark Table of Contents Entry (`references.markTocEntry`, Alt+Shift+O): a hidden TC field
//! (ECMA-376 §17.16.5.70) that a table of contents built with `\f` lists.

use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_doc::para::InlineObject;

use crate::Session;

/// A body of "Intro", "Overview" with "Overview" selected.
fn selected() -> Session {
    let mut s = Session::new(Document::from_text("Intro\nOverview"));
    s.run("select.text", &json!({"text": "Overview"})).unwrap();
    s
}

/// Every field instruction in the body, with whether the field is hidden.
fn fields(s: &Session) -> Vec<(String, bool)> {
    let mut v = Vec::new();
    for b in &s.doc.body {
        let Some(p) = b.as_para() else { continue };
        for off in p.object_offsets() {
            if let Some(InlineObject::Field { instr, .. }) = p.object_at(off) {
                v.push((instr.clone(), p.props_of_char(off).hidden == Some(true)));
            }
        }
    }
    v
}

#[test]
fn marks_the_selection_with_a_hidden_tc_field() {
    for (params, want) in [
        (json!({}), r#"TC "Overview" \f C \l 1"#),
        (json!({"level": 3}), r#"TC "Overview" \f C \l 3"#),
        (json!({"identifier": "f", "level": 2}), r#"TC "Overview" \f F \l 2"#),
        (json!({"text": "Say \"hi\"\n"}), r#"TC "Say hi" \f C \l 1"#),
    ] {
        let mut s = selected();
        s.run("references.markTocEntry", &params).unwrap();
        assert_eq!(fields(&s), vec![(want.to_string(), true)], "{params}");
        let text: Vec<String> = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect();
        assert_eq!(text, ["Intro", "Overview"], "the marked text itself is unchanged");
    }
}

#[test]
fn bad_params_change_nothing() {
    for params in [json!({"level": 0}), json!({"level": 10}), json!({"identifier": "AB"}), json!({"identifier": "1"})] {
        let mut s = selected();
        assert!(s.run("references.markTocEntry", &params).is_err(), "{params}");
        assert!(fields(&s).is_empty(), "{params}");
    }
    let mut s = Session::new(Document::from_text("Intro"));
    assert!(s.run("references.markTocEntry", &json!({})).is_err(), "empty selection, no text");
    assert!(fields(&s).is_empty());
}

#[test]
fn undo_removes_the_mark() {
    let mut s = selected();
    s.run("references.markTocEntry", &json!({})).unwrap();
    s.run("edit.undo", &json!({})).unwrap();
    assert!(fields(&s).is_empty());
}

#[test]
fn default_key_outside_macos() {
    let reg = crate::cmd::registry();
    let spec = reg.get("references.markTocEntry").expect("command registered");
    if cfg!(target_os = "macos") {
        assert_eq!(spec.shortcut, "");
    } else {
        assert_eq!(spec.shortcut, "Alt+Shift+O");
    }
}

#[test]
fn table_of_contents_lists_the_marked_entry() {
    let mut s = selected();
    s.run("references.markTocEntry", &json!({"level": 2})).unwrap();
    s.run("caret.docStart", &json!({})).unwrap();
    s.run("references.toc", &json!({"tcFields": true, "headingStyles": false, "outlineLevels": false, "pageNumbers": false})).unwrap();
    let entries: Vec<(String, String)> = s
        .doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .filter_map(|p| {
            let st = p.props.style.clone()?;
            (st.starts_with("TOC") && st != "TOCHeading").then(|| (st, p.plain_text().trim().to_string()))
        })
        .collect();
    assert!(entries.iter().any(|(st, t)| st == "TOC2" && t.contains("Overview")), "{entries:?}");
}
