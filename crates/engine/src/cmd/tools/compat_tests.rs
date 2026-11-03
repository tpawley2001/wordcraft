//! Compatibility Mode: `file.compatibility` reports the layout mode, `file.convert` upgrades it.

use super::*;
use std::path::PathBuf;

fn doc_in_mode(mode: u32) -> Session {
    let mut s = Session::new(Document::from_text("Some text."));
    s.doc.settings.compat_mode = mode;
    s
}

#[test]
fn compatibility_reports_mode_version_and_flag() {
    for (mode, version, compat) in [
        (11, "Word 97-2003", true),
        (12, "Word 2007", true),
        (13, "Word 2007", true),
        (14, "Word 2010", true),
        (15, "Word 2013 and later", false),
        (16, "Word 2013 and later", false),
    ] {
        let mut s = doc_in_mode(mode);
        let r = s.run("file.compatibility", &json!({})).unwrap();
        assert_eq!(r["mode"], mode, "mode {mode}");
        assert_eq!(r["version"], version, "mode {mode}");
        assert_eq!(r["compatibilityMode"], compat, "mode {mode}");
        assert!(r["issues"].is_array(), "mode {mode}: {r}");
    }
}

#[test]
fn word_97_2003_files_are_always_in_compatibility_mode() {
    for name in ["Old.doc", "OLD.DOC", "Template.dot"] {
        let mut s = doc_in_mode(15);
        s.path = Some(PathBuf::from("/files").join(name));
        let r = s.run("file.compatibility", &json!({})).unwrap();
        assert_eq!(r["compatibilityMode"], true, "{name}");
    }
    let mut s = doc_in_mode(15);
    s.path = Some(PathBuf::from("/files/New.docx"));
    assert_eq!(s.run("file.compatibility", &json!({})).unwrap()["compatibilityMode"], false);
}

#[test]
fn convert_upgrades_to_current_mode_as_one_undo_step() {
    let mut s = doc_in_mode(12);
    s.path = Some(PathBuf::from("/files/Report.docx"));
    s.dirty = false;
    let r = s.run("file.convert", &json!({})).unwrap();
    assert_eq!(r["mode"], 15);
    assert_eq!(r["previous"], 12);
    assert!(r.get("path").is_none(), "a .docx keeps its path: {r}");
    assert_eq!(s.doc.settings.compat_mode, 15);
    assert!(s.dirty);
    assert_eq!(s.path, Some(PathBuf::from("/files/Report.docx")));
    let after = s.run("file.compatibility", &json!({})).unwrap();
    assert_eq!(after["compatibilityMode"], false);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc.settings.compat_mode, 12);
}

#[test]
fn convert_is_disabled_for_a_current_document() {
    let mut s = doc_in_mode(15);
    s.path = Some(PathBuf::from("/files/New.docx"));
    let before = s.doc.settings.compat_mode;
    match s.run("file.convert", &json!({})) {
        Err(CmdError::Disabled(_)) => {}
        other => panic!("expected Disabled, got {other:?}"),
    }
    assert_eq!(s.doc.settings.compat_mode, before);
}

#[test]
fn convert_moves_a_binary_file_to_the_new_format() {
    for (from, to) in [("/files/Old.doc", "/files/Old.docx"), ("/files/Letter.DOT", "/files/Letter.dotx")] {
        let mut s = doc_in_mode(15);
        s.path = Some(PathBuf::from(from));
        let r = s.run("file.convert", &json!({})).unwrap();
        assert_eq!(r["mode"], 15, "{from}");
        assert_eq!(r["path"], to, "{from}: {r}");
        assert_eq!(s.path, Some(PathBuf::from(to)), "{from}");
        assert_eq!(s.run("file.compatibility", &json!({})).unwrap()["compatibilityMode"], false, "{from}");
    }
}
