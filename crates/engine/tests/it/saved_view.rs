//! A document opens in the view it was saved in (Web Layout, Outline), as in Word; a document
//! saved in Draft opens in Print Layout (Word's default: "Allow opening a document in Draft view"
//! is off). Switching the view isn't an edit.

use serde_json::json;
use wordcraft_engine::Session;
use wordcraft_layout::ViewMode;

fn run(s: &mut Session, cmd: &str, v: serde_json::Value) {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd}: {e:?}"));
}

#[test]
fn a_document_reopens_in_the_view_it_was_saved_in() {
    let dir = std::env::temp_dir().join(format!("wordcraft-engine-saved-view-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (command, reopened) in [
        ("view.webLayout", ViewMode::Web),
        ("view.outline", ViewMode::Outline),
        ("view.draft", ViewMode::Print),
        ("view.printLayout", ViewMode::Print),
    ] {
        let path = dir.join(format!("{}.docx", command.replace('.', "-")));
        let path = path.to_string_lossy().to_string();
        let mut s = Session::new(wordcraft_doc::Document::new());
        run(&mut s, "text.insert", json!({"text": "Hello"}));
        run(&mut s, "file.save", json!({"path": path}));
        run(&mut s, command, json!({}));
        assert!(!s.dirty, "{command}: switching the view isn't an edit");
        run(&mut s, "file.save", json!({}));

        let mut other = Session::new(wordcraft_doc::Document::new());
        run(&mut other, "file.open", json!({"path": path}));
        assert_eq!(other.view.mode, reopened, "{command}");
        assert!(!other.dirty, "{command}: opening isn't an edit");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_records_the_current_view() {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "view.webLayout", json!({}));
    s.stamp_save();
    assert_eq!(s.doc.settings.view, Some(wordcraft_doc::SavedView::Web));
    run(&mut s, "view.draft", json!({}));
    s.stamp_save();
    assert_eq!(s.doc.settings.view, Some(wordcraft_doc::SavedView::Draft));
    run(&mut s, "view.readMode", json!({}));
    s.stamp_save();
    assert_eq!(s.doc.settings.view, None, "Read Mode is saved as Print Layout");
    run(&mut s, "view.outline", json!({}));
    s.stamp_save();
    assert_eq!(s.doc.settings.view, Some(wordcraft_doc::SavedView::Outline));
    run(&mut s, "view.printLayout", json!({}));
    s.stamp_save();
    assert_eq!(s.doc.settings.view, None);
}

#[test]
fn an_rtf_document_reopens_in_the_view_it_was_saved_in() {
    let dir = std::env::temp_dir().join(format!("wordcraft-engine-saved-view-rtf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let opened = dir.join("opened.rtf");
    std::fs::write(&opened, r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}\viewkind5\pard a\par}").unwrap();
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "file.open", json!({"path": opened.to_string_lossy()}));
    assert_eq!(s.view.mode, ViewMode::Web, "\\viewkind5 opens in Web Layout");

    let saved = dir.join("saved.rtf").to_string_lossy().to_string();
    run(&mut s, "view.outline", json!({}));
    run(&mut s, "file.save", json!({"path": saved}));
    let mut other = Session::new(wordcraft_doc::Document::new());
    run(&mut other, "file.open", json!({"path": saved}));
    assert_eq!(other.view.mode, ViewMode::Outline);
    let _ = std::fs::remove_dir_all(&dir);
}
