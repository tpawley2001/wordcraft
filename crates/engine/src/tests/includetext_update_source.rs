//! Update Source (Ctrl+Shift+F7, `fields.updateSource`): an INCLUDETEXT field's result written back
//! to the file it includes, the whole file or only the bookmark it names (ECMA-376 §17.16.5.30).
//! Only on this explicit command; network paths and URLs are refused.

use std::path::{Path, PathBuf};

use serde_json::json;
use wordcraft_doc::{CharProps, InlineObject, StoryRef};

use crate::{KeyMap, Session, cmd::registry};

/// A fresh folder under the temp dir for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wc-updsource-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A field code path as Word writes it: backslashes doubled.
fn code_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

/// One paragraph "Here: " ending in the field `instr` whose (edited) result is `result`, the caret
/// just after the field.
fn session(instr: &str, result: &str, locked: bool) -> Session {
    let mut doc = wordcraft_doc::Document::from_text("Here: ");
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let p = doc.para_mut(StoryRef::Body, &path).unwrap();
    let field = InlineObject::Field { instr: instr.into(), result: result.into(), locked, dirty: false, code: false, form: String::new() };
    p.insert_object(p.len(), field, &CharProps::default()).unwrap();
    let mut s = Session::new(doc);
    s.run("caret.end", &json!({})).unwrap();
    s
}

fn body_texts(doc: &wordcraft_doc::Document) -> Vec<String> {
    doc.para_paths(StoryRef::Body).iter().map(|p| doc.para(StoryRef::Body, p).unwrap().plain_text()).collect()
}

fn field_result(s: &Session) -> String {
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                return result.clone();
            }
        }
    }
    panic!("no field")
}

/// A .docx "Before / PART / After" with the bookmark `Part` over the middle paragraph's text.
fn docx_with_bookmark(dir: &Path) -> PathBuf {
    let mut src = Session::new(wordcraft_doc::Document::from_text("Before\nPART\nAfter"));
    src.run("select.text", &json!({"text": "PART"})).unwrap();
    src.run("insert.bookmark", &json!({"name": "Part"})).unwrap();
    let path = dir.join("source.docx");
    crate::io::save_path(&path, &src.doc).unwrap();
    path
}

#[test]
fn ctrl_shift_f7_is_update_source() {
    let reg = registry();
    assert!(reg.get("fields.updateSource").is_some());
    assert_eq!(KeyMap::default().resolve(&reg, "Mod+Shift+F7").map(|c| c.id), Some("fields.updateSource"));
}

#[test]
fn the_whole_text_file_takes_the_edited_result() {
    let dir = scratch("txt");
    let src = dir.join("note.txt");
    std::fs::write(&src, "Old line\n").unwrap();
    let mut s = session(&format!("INCLUDETEXT \"{}\"", code_path(&src)), "New text\rSecond\r", false);
    let out = s.run("fields.updateSource", &json!({})).expect("update source");
    assert_eq!(out["updated"].as_array().map(Vec::len), Some(1), "{out}");
    let back = crate::io::open_path(&src).unwrap();
    assert_eq!(body_texts(&back), ["New text", "Second"]);
    // The document itself is unchanged.
    assert_eq!(field_result(&s), "New text\rSecond\r");
}

#[test]
fn only_the_named_bookmark_changes_in_a_docx_source() {
    let dir = scratch("bm");
    let src = docx_with_bookmark(&dir);
    for instr in [format!("INCLUDETEXT \"{}\" Part", code_path(&src)), format!("INCLUDETEXT \"{}\" \\! part", code_path(&src))] {
        let mut s = session(&instr, "Changed", false);
        s.run("fields.updateSource", &json!({})).expect(&instr);
        let back = crate::io::open_path(&src).unwrap();
        assert_eq!(body_texts(&back), ["Before", "Changed", "After"], "{instr}");
        assert!(back.bookmarks().iter().any(|(n, _)| n == "Part"), "bookmark kept: {instr}");
    }
    // F9 then reads the written text back through the bookmark.
    let mut s = session(&format!("INCLUDETEXT \"{}\" Part", code_path(&src)), "OLD", false);
    s.run("references.updateFields", &json!({"all": true})).unwrap();
    assert_eq!(field_result(&s).trim_end(), "Changed");
}

#[test]
fn a_missing_bookmark_writes_nothing() {
    let dir = scratch("nobm");
    let src = docx_with_bookmark(&dir);
    let before = std::fs::read(&src).unwrap();
    let mut s = session(&format!("INCLUDETEXT \"{}\" Nowhere", code_path(&src)), "Changed", false);
    assert!(s.run("fields.updateSource", &json!({})).is_err());
    assert_eq!(std::fs::read(&src).unwrap(), before);
}

#[test]
fn a_locked_field_writes_nothing() {
    let dir = scratch("locked");
    let src = dir.join("note.txt");
    std::fs::write(&src, "Keep me\n").unwrap();
    let mut s = session(&format!("INCLUDETEXT \"{}\"", code_path(&src)), "Changed", true);
    assert!(s.run("fields.updateSource", &json!({})).is_err());
    assert_eq!(std::fs::read_to_string(&src).unwrap(), "Keep me\n");
}

#[test]
fn network_paths_urls_and_unsaved_relative_paths_are_refused() {
    for instr in [
        r#"INCLUDETEXT "\\\\server\\share\\a.txt""#,
        r#"INCLUDETEXT "//server/share/a.txt""#,
        r#"INCLUDETEXT "http://example.com/a.txt""#,
        r#"INCLUDETEXT "file:///tmp/a.txt""#,
        r#"INCLUDETEXT "relative.txt""#,
        r#"INCLUDETEXT """#,
    ] {
        let mut s = session(instr, "Changed", false);
        assert!(s.run("fields.updateSource", &json!({})).is_err(), "{instr}");
    }
}

#[test]
fn a_missing_source_file_is_not_created() {
    let dir = scratch("missing");
    let src = dir.join("absent.txt");
    let mut s = session(&format!("INCLUDETEXT \"{}\"", code_path(&src)), "Changed", false);
    assert!(s.run("fields.updateSource", &json!({})).is_err());
    assert!(!src.exists());
}

#[test]
fn no_includetext_field_at_the_caret_is_an_error() {
    let dir = scratch("nofield");
    let src = dir.join("note.txt");
    std::fs::write(&src, "Keep me\n").unwrap();
    let mut s = session(&format!("INCLUDETEXT \"{}\"", code_path(&src)), "Changed", false);
    s.run("caret.home", &json!({})).unwrap();
    assert!(s.run("fields.updateSource", &json!({})).is_err());
    let mut other = session("PAGE", "1", false);
    assert!(other.run("fields.updateSource", &json!({})).is_err());
    assert_eq!(std::fs::read_to_string(&src).unwrap(), "Keep me\n");
}
