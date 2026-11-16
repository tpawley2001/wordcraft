//! File Conversion: Save As Plain Text asks for the encoding, line ends and character
//! substitution before writing; Cancel writes nothing; the choice is remembered for the session.

use serde_json::json;

use crate::WordApp;

fn app(text: &str) -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text(text)), crate::Services::default())
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wordcraft-ui-fileconv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn dialog(a: &WordApp) -> Option<&'static str> {
    a.dialog.as_ref().map(|d| d.name())
}

#[test]
fn save_as_plain_text_opens_file_conversion_and_cancel_writes_nothing() {
    let dir = scratch("cancel");
    let picked = dir.join("notes.txt").to_string_lossy().to_string();
    let mut a = app("Caf\u{e9}");
    let to = picked.clone();
    a.services.pick_save = Some(Box::new(move |_| Some(to.clone())));
    let _ = a.save_as(crate::file_dialogs::AfterSave::Nothing);
    assert_eq!(dialog(&a), Some("fileConversion"), "the Save As of a .txt asks first");
    assert!(!std::path::Path::new(&picked).exists(), "nothing is written before OK");
    a.run("ui.fileConversion", json!({"answer": "cancel"})).unwrap();
    assert_eq!(dialog(&a), None);
    assert!(!std::path::Path::new(&picked).exists(), "Cancel writes nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ok_saves_with_the_choice_and_a_later_save_reuses_it() {
    let dir = scratch("ok");
    let path = dir.join("notes.txt");
    let mut a = app("Caf\u{e9} \u{201c}x\u{201d}\nb");
    // A document whose own file is plain text (one opened from notes.txt): its first Save asks.
    a.session.path = Some(path.clone());
    let r = a.run("file.save", json!({})).unwrap();
    assert_eq!(r["pending"], "fileConversion");
    assert!(!path.exists());
    a.run("ui.fileConversion", json!({"answer": "ok", "encoding": "msdos", "lineEnd": "lf", "allowSubstitution": true})).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"Caf\x82 \"x\"\nb");
    // Save again: the same file, the same choice, no question.
    a.run("text.insert", json!({"text": "!"})).unwrap();
    a.run("file.save", json!({})).unwrap();
    assert_eq!(dialog(&a), None);
    assert!(std::fs::read(&path).unwrap().contains(&b'!'), "the Save wrote the change");
    assert!(!std::fs::read(&path).unwrap().contains(&b'\r'));
    // Save As again: asks, starting from the remembered choice.
    a.run("file.saveAs", json!({"path": dir.join("other.txt")})).unwrap();
    let Some(crate::dialogs::Dialog::FileConversion { form }) = &a.dialog else { panic!("File Conversion is open") };
    assert_eq!((form.kind.as_str(), form.line_end.as_str(), form.allow_substitution), ("msdos", "lf", true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scripts_that_say_how_never_see_the_dialog() {
    let dir = scratch("script");
    let path = dir.join("notes.txt");
    let mut a = app("a\nb");
    a.run("file.saveAs", json!({"path": path, "encoding": "utf-8"})).unwrap();
    assert_eq!(dialog(&a), None);
    assert_eq!(std::fs::read(&path).unwrap(), b"\xEF\xBB\xBFa\r\nb");
    // The control channel and MCP run commands without asking: UTF-8 as before.
    let plain = dir.join("plain.txt");
    a.execute("file.saveAs", json!({"path": plain})).unwrap();
    assert_eq!(std::fs::read(&plain).unwrap(), b"a\r\nb");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_dialog_draws_its_preview() {
    let mut a = app("\u{2265}");
    let dir = scratch("draw");
    a.run("file.saveAs", json!({"path": dir.join("n.txt")})).unwrap();
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
    for t in 1..4 {
        let input = egui::RawInput { time: Some(f64::from(t) / 60.0), screen_rect: Some(screen), ..Default::default() };
        ctx.run_ui(input, |ui| {
            a.logic(ui.ctx());
            a.ui(ui);
        })
        .drop_without_applying_deltas();
    }
    assert_eq!(dialog(&a), Some("fileConversion"), "the dialog stays open until answered");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_answer_without_the_dialog_leaves_other_dialogs_alone() {
    let mut a = app("x");
    a.dialog = Some(crate::dialogs::Dialog::MarkAsFinal);
    assert!(a.run("ui.fileConversion", json!({"answer": "ok"})).is_err());
    assert!(a.run("ui.fileConversion", json!({"answer": "maybe"})).is_err());
    assert_eq!(dialog(&a), Some("markAsFinal"));
}
