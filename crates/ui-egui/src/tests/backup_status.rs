//! File › Options › Advanced › Save › Always create backup copy: a save that went through but
//! couldn't keep the previous version as `Backup of <name>.wbk` says so on the status bar. The
//! save itself never fails because of the backup.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::WordApp;

fn app() -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), crate::Services::default())
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wordcraft-ui-wbkstatus-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(a: &mut WordApp, id: &str, v: Value) -> Value {
    a.run(id, v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// What the status bar says now, lower-cased ("" when it says nothing).
fn status(a: &WordApp) -> String {
    a.status_msg.as_ref().map(|(m, _)| m.to_lowercase()).unwrap_or_default()
}

/// An app whose document was typed in and saved once as `dir/Report.docx`, with Always create
/// backup copy set to `backup`.
fn saved_once(dir: &Path, backup: bool) -> (WordApp, PathBuf) {
    let mut a = app();
    a.session.prefs.always_create_backup = backup;
    let path = dir.join("Report.docx");
    run(&mut a, "text.insert", json!({"text": "First version. "}));
    let r = run(&mut a, "file.save", json!({"path": path.to_string_lossy()}));
    assert_eq!(r["saved"], json!(true), "{r}");
    (a, path)
}

/// Type something and save over the document's own file with a clean status bar.
fn edit_and_save(a: &mut WordApp, text: &str) -> Value {
    run(a, "text.insert", json!({"text": text}));
    a.status_msg = None;
    run(a, "file.save", json!({}))
}

#[test]
fn a_backup_that_cannot_be_written_is_reported_on_the_status_bar() {
    let dir = scratch("blocked");
    let (mut a, path) = saved_once(&dir, true);
    let first = std::fs::read(&path).unwrap();
    // A folder already holds the backup's name, so the backup can't be written.
    std::fs::create_dir(dir.join("Backup of Report.wbk")).unwrap();
    let r = edit_and_save(&mut a, "Second version. ");
    assert_eq!(r["saved"], json!(true), "the save itself goes through: {r}");
    assert!(r["backupError"].as_str().is_some_and(|e| !e.is_empty()), "{r}");
    assert!(!a.session.dirty, "the document is saved");
    assert_ne!(std::fs::read(&path).unwrap(), first, "the file has the new version");
    let said = status(&a);
    assert!(said.contains("saved"), "the message starts with the good news: {said:?}");
    assert!(said.contains("backup copy"), "the status bar says the backup copy wasn't made: {said:?}");
    assert!(said.contains("backup of report.wbk"), "with the engine's reason, which names the backup file: {said:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_backup_that_was_written_says_nothing_about_backups() {
    let dir = scratch("written");
    let (mut a, _) = saved_once(&dir, true);
    let r = edit_and_save(&mut a, "Second version. ");
    assert_eq!(r["saved"], json!(true), "{r}");
    assert!(r["backup"].as_str().is_some_and(|b| b.ends_with("Backup of Report.wbk")), "{r}");
    assert!(dir.join("Backup of Report.wbk").is_file());
    assert!(!status(&a).contains("backup"), "{:?}", a.status_msg);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_backup_message_when_the_option_is_off_or_the_file_is_new() {
    // Option off: no backup is tried, even with the backup's name taken.
    let dir = scratch("off");
    let (mut a, _) = saved_once(&dir, false);
    std::fs::create_dir(dir.join("Backup of Report.wbk")).unwrap();
    let r = edit_and_save(&mut a, "Second version. ");
    assert_eq!(r["saved"], json!(true), "{r}");
    assert!(r.get("backupError").is_none(), "{r}");
    assert!(!status(&a).contains("backup"), "{:?}", a.status_msg);

    // Option on, Save As under a new name: there is no previous version to keep.
    let (mut b, _) = saved_once(&dir, true);
    std::fs::create_dir(dir.join("Backup of Copy.wbk")).unwrap();
    b.status_msg = None;
    let r = run(&mut b, "file.saveAs", json!({"path": dir.join("Copy.docx").to_string_lossy()}));
    assert_eq!(r["saved"], json!(true), "{r}");
    assert!(r.get("backupError").is_none(), "{r}");
    assert!(!status(&b).contains("backup"), "{:?}", b.status_msg);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_later_good_save_does_not_repeat_the_message() {
    let dir = scratch("later");
    let (mut a, _) = saved_once(&dir, true);
    let blocker = dir.join("Backup of Report.wbk");
    std::fs::create_dir(&blocker).unwrap();
    edit_and_save(&mut a, "Second version. ");
    assert!(status(&a).contains("backup copy"), "{:?}", a.status_msg);
    // The folder is gone: the next save keeps its backup and has nothing to report.
    std::fs::remove_dir(&blocker).unwrap();
    let r = edit_and_save(&mut a, "Third version. ");
    assert!(r["backup"].as_str().is_some(), "{r}");
    assert!(blocker.is_file());
    assert!(!status(&a).contains("backup"), "{:?}", a.status_msg);
    let _ = std::fs::remove_dir_all(&dir);
}
