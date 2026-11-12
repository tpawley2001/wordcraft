//! Update Table › Update page numbers only on tables the acceptance tests don't build: a table
//! still held as a range field (as opened from DOCX), and an entry whose number was retyped with
//! Track Changes on.

use std::sync::Arc;

use serde_json::{Value, json};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Block, Document, Paragraph, Path, Pos, StoryRef};

use crate::{Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
}

fn field(instr: &str, result: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked, dirty: false, code: false, form: String::new(), code_paras: None }
}

/// The `Field` results of body block `i`.
fn results(s: &Session, i: usize) -> Vec<String> {
    let p = s.doc.para(StoryRef::Body, &Path::top(i)).unwrap();
    p.objects.iter().filter_map(|o| if let InlineObject::Field { result, .. } = o { Some(result.clone()) } else { None }).collect()
}

#[test]
fn toc_page_numbers_only_updates_pageref_fields_of_a_range_field_toc() {
    let props = CharProps::default();
    // Entries: label, the PAGEREF code, locked.
    let entries = [
        ("Alpha\t", r"PAGEREF _Toc1 \h", false),
        ("Beta\t", r"PAGEREF _Toc2 \h", false),
        ("Gamma\t", r"PAGEREF _Toc3 \h", true),
        ("Delta\t", r"PAGEREF _Toc4 \p", false),
    ];
    let mut body = Vec::new();
    let last = entries.len() - 1;
    for (k, (label, instr, locked)) in entries.iter().enumerate() {
        let mut p = Paragraph::with_text(label, props.clone()).styled("TOC1");
        p.insert_object(p.len(), field(instr, "9", *locked), &props).unwrap();
        if k == last {
            p.insert_object(p.len(), InlineObject::FieldEnd, &props).unwrap();
        }
        if k == 0 {
            p.insert_object(0, InlineObject::FieldStart { instr: r#"TOC \o "1-3" \h"#.into(), locked: false, dirty: false }, &props).unwrap();
        }
        body.push(p);
    }
    for (k, h) in ["Alpha", "Beta", "Gamma", "Delta"].iter().enumerate() {
        let mut p = Paragraph::with_text(h, props.clone()).styled("Heading1");
        p.props.page_break_before = (k > 0).then_some(true);
        let name = format!("_Toc{}", k + 1);
        p.insert_object(p.len(), InlineObject::BookmarkEnd { name: name.clone() }, &props).unwrap();
        p.insert_object(0, InlineObject::BookmarkStart { name }, &props).unwrap();
        body.push(p);
    }
    let mut d = Document::new();
    d.body = body.into_iter().map(|p| Arc::new(Block::Para(p))).collect();
    let mut s = Session::new(d);
    assert_eq!(s.doc.field_ranges(StoryRef::Body).len(), 1);
    let texts: Vec<String> = (0..4).map(|i| s.doc.para(StoryRef::Body, &Path::top(i)).unwrap().text.clone()).collect();
    let steps = s.undo_depth();
    run(&mut s, "references.updateToc", json!({"pageNumbersOnly": true}));
    // Alpha on page 1 and Beta on page 2 follow their headings; the locked field and the `\p`
    // one ("above"/"below") keep their results.
    assert_eq!([results(&s, 0), results(&s, 1), results(&s, 2), results(&s, 3)], [["1"], ["2"], ["9"], ["9"]]);
    let after: Vec<String> = (0..4).map(|i| s.doc.para(StoryRef::Body, &Path::top(i)).unwrap().text.clone()).collect();
    assert_eq!(after, texts, "the entries are not rebuilt");
    assert_eq!(s.doc.field_ranges(StoryRef::Body).len(), 1, "still the range field");
    assert_eq!(s.undo_depth(), steps + 1);
    // Up to date: a second update is no edit.
    run(&mut s, "references.updateToc", json!({"pageNumbersOnly": true}));
    assert_eq!(s.undo_depth(), steps + 1);
}

#[test]
fn toc_page_numbers_only_ignores_tracked_deletions_in_the_entry() {
    let mut d = Document::new();
    let mut body = Vec::new();
    for (k, h) in ["Intro", "Middle"].iter().enumerate() {
        let mut p = Paragraph::with_text(h, CharProps::default()).styled("Heading1");
        p.props.page_break_before = (k > 0).then_some(true);
        body.push(p);
    }
    d.body = body.into_iter().map(|p| Arc::new(Block::Para(p))).collect();
    let mut s = Session::new(d);
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "references.toc", json!({"hyperlinks": false}));
    let entry = |s: &Session| {
        s.doc.body.iter().position(|b| b.as_para().is_some_and(|p| p.props.style.as_deref() == Some("TOC1") && p.text.starts_with("Intro"))).unwrap()
    };
    let i = entry(&s);
    let heading = s.doc.body.iter().position(|b| b.as_para().is_some_and(|p| p.text == "Intro")).unwrap();
    s.sel = Selection::caret(Pos::body(heading, 0));
    run(&mut s, "para.pageBreakBefore", json!({"value": true}));
    // The entry's "1" deleted and "7" typed with Track Changes on.
    let len = s.doc.para(StoryRef::Body, &Path::top(i)).unwrap().len();
    assert_eq!(s.doc.para(StoryRef::Body, &Path::top(i)).unwrap().plain_text(), "Intro\t1");
    run(&mut s, "review.trackChanges", json!({"value": true}));
    s.sel = Selection { anchor: Pos::body(i, len - 1), focus: Pos::body(i, len) };
    run(&mut s, "text.delete", json!({}));
    s.sel = Selection::caret(Pos::body(i, len));
    run(&mut s, "text.insert", json!({"text": "7"}));
    run(&mut s, "review.trackChanges", json!({"value": false}));
    let p = s.doc.para(StoryRef::Body, &Path::top(i)).unwrap();
    assert_eq!((p.plain_text().as_str(), p.final_text().as_str()), ("Intro\t17", "Intro\t7"));
    run(&mut s, "references.updateToc", json!({"pageNumbersOnly": true}));
    let p = s.doc.para(StoryRef::Body, &Path::top(entry(&s))).unwrap();
    assert_eq!(p.final_text(), "Intro\t2", "the live number is replaced");
    let (at, _) = p.text.char_indices().last().unwrap();
    let props = p.props_of_char(at);
    assert_eq!((props.ins, props.del), (None, None), "the new number is no tracked change");
    assert_eq!(p.deleted_ranges().len(), 1, "the tracked deletion stays as it was");
}
