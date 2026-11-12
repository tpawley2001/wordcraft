//! Insert › Text from File › Range brings only the comments, replies and footnotes of the
//! bookmarked range; the other comments and notes of that file stay out of the document (they
//! used to come along with no anchor and were saved with it). The whole file still brings all.

use serde_json::{Value, json};

use crate::Session;
use wordcraft_doc::{Document, PartKind, Path, Pos, StoryRef};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn caret(s: &mut Session, block: usize, off: usize) {
    run(s, "caret.set", json!({"pos": {"block": block, "off": off}}));
}

fn story_text(d: &Document, part: u32) -> String {
    d.plain_text(StoryRef::Part(part)).trim().to_string()
}

/// The text of every comment in `d`, sorted.
fn comment_texts(d: &Document) -> Vec<String> {
    let mut out: Vec<String> = d.comments.values().map(|c| story_text(d, c.part)).collect();
    out.sort();
    out
}

/// The text of every footnote story in `d`, sorted.
fn footnote_texts(d: &Document) -> Vec<String> {
    let mut out: Vec<String> = d.parts.iter().filter(|(_, p)| p.kind == PartKind::Footnote).map(|(id, _)| story_text(d, *id)).collect();
    out.sort();
    out
}

/// A footnote `note` at the end of paragraph `block` and a comment `comment` on its word.
fn annotate(s: &mut Session, block: usize, note: &str, comment: &str) {
    let end = s.doc.para_at(&Pos { story: StoryRef::Body, path: Path::top(block), off: 0 }).expect("paragraph").len();
    caret(s, block, end);
    run(s, "references.footnote", json!({"text": note}));
    caret(s, block, 3);
    run(s, "review.newComment", json!({"text": comment}));
}

/// Paragraphs `outside` (comment "outer comment", footnote "outer note"), `[Mid]inside[/Mid]`
/// (comment "inner comment" with the reply "inner reply", footnote "inner note") and `tail`,
/// saved as a .docx in a fresh temp dir; returns the file's path.
fn source_file(tag: &str) -> std::path::PathBuf {
    let mut s = Session::new(Document::from_text("outside\ninside\ntail"));
    annotate(&mut s, 0, "outer note", "outer comment");
    annotate(&mut s, 1, "inner note", "inner comment");
    let inner = s.doc.comments.iter().find(|(_, c)| story_text(&s.doc, c.part) == "inner comment").map(|(id, _)| *id).expect("inner comment");
    run(&mut s, "review.reply", json!({"id": inner, "text": "inner reply"}));
    let end = s.doc.para_at(&Pos { story: StoryRef::Body, path: Path::top(1), off: 0 }).expect("paragraph").len();
    run(&mut s, "select.range", json!({"anchor": {"block": 1, "off": 0}, "focus": {"block": 1, "off": end}}));
    run(&mut s, "insert.bookmark", json!({"name": "Mid"}));
    assert_eq!(comment_texts(&s.doc), ["inner comment", "inner reply", "outer comment"], "the source's comments");
    assert_eq!(footnote_texts(&s.doc), ["inner note", "outer note"], "the source's footnotes");
    let dir = std::env::temp_dir().join(format!("wordcraft-text-from-file-stories-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("source.docx");
    std::fs::write(&path, crate::io::save_bytes("source.docx", &s.doc).unwrap()).unwrap();
    path
}

/// A document holding "x" with the caret at its end.
fn target() -> Session {
    let mut s = Session::new(Document::new());
    run(&mut s, "document.setText", json!({"text": "x"}));
    run(&mut s, "caret.docEnd", json!({}));
    s
}

fn insert_range(s: &mut Session, path: &std::path::Path) {
    run(s, "insert.textFromFile", json!({"path": path.to_str().unwrap(), "range": "Mid"}));
}

#[test]
fn a_range_brings_only_the_comments_anchored_in_it() {
    let path = source_file("comments");
    let mut s = target();
    insert_range(&mut s, &path);
    assert_eq!(s.doc.plain_text(StoryRef::Body).lines().next(), Some("xinside"));
    assert_eq!(comment_texts(&s.doc), ["inner comment", "inner reply"], "the comment outside the bookmark stays in its file");
}

#[test]
fn a_reply_comes_with_its_comment_and_keeps_its_parent() {
    let path = source_file("reply");
    let mut s = target();
    insert_range(&mut s, &path);
    let id_of = |text: &str| s.doc.comments.iter().find(|(_, c)| story_text(&s.doc, c.part) == text).map(|(id, _)| *id);
    let inner = id_of("inner comment").expect("the comment in the range came along");
    let reply = id_of("inner reply").expect("its reply came along");
    assert_eq!(s.doc.comments.get(&reply).and_then(|c| c.parent), Some(inner));
    assert_eq!(s.doc.comments.get(&inner).and_then(|c| c.parent), None);
}

#[test]
fn a_range_brings_only_the_footnotes_referenced_in_it() {
    let path = source_file("notes");
    let mut s = target();
    insert_range(&mut s, &path);
    assert_eq!(footnote_texts(&s.doc), ["inner note"], "the footnote outside the bookmark stays in its file");
}

#[test]
fn the_saved_document_holds_only_the_ranges_comments_and_footnotes() {
    let path = source_file("saved");
    let mut s = target();
    insert_range(&mut s, &path);
    let bytes = crate::io::save_bytes("host.docx", &s.doc).expect("saves");
    let back = crate::io::open_bytes("host.docx", &bytes).expect("reopens");
    assert_eq!(comment_texts(&back), ["inner comment", "inner reply"]);
    assert_eq!(footnote_texts(&back), ["inner note"]);
}

#[test]
fn the_whole_file_still_brings_every_comment_and_footnote() {
    let path = source_file("whole");
    let mut s = target();
    run(&mut s, "insert.textFromFile", json!({"path": path.to_str().unwrap()}));
    assert_eq!(comment_texts(&s.doc), ["inner comment", "inner reply", "outer comment"]);
    assert_eq!(footnote_texts(&s.doc), ["inner note", "outer note"]);
}

#[test]
fn inserting_the_range_twice_brings_its_stories_twice_and_nothing_else() {
    let path = source_file("twice");
    let mut s = target();
    insert_range(&mut s, &path);
    insert_range(&mut s, &path);
    assert_eq!(comment_texts(&s.doc), ["inner comment", "inner comment", "inner reply", "inner reply"]);
    assert_eq!(footnote_texts(&s.doc), ["inner note", "inner note"]);
}

#[test]
fn the_documents_own_comments_and_footnotes_stay() {
    let path = source_file("own");
    let mut s = Session::new(Document::from_text("host"));
    annotate(&mut s, 0, "host note", "host comment");
    run(&mut s, "caret.docEnd", json!({}));
    insert_range(&mut s, &path);
    assert_eq!(comment_texts(&s.doc), ["host comment", "inner comment", "inner reply"]);
    assert_eq!(footnote_texts(&s.doc), ["host note", "inner note"]);
}
