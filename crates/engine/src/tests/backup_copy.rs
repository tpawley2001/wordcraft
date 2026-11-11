//! File › Options › Advanced › Save › Always create backup copy, as in Word: with it on, a save
//! over the document's own file first keeps the previous version beside it as
//! `Backup of <name>.wbk`; each save replaces the backup. The first save of a new document and
//! Save As under a new name make none. Off by default.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{CmdError, Session};

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wordcraft-engine-backupcopy-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn p(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The names of the `.wbk` files in `dir`, sorted.
fn backups(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".wbk"))
        .collect();
    names.sort();
    names
}

/// A session with the setting on whose document was typed in and saved once as `file`.
fn saved_once(dir: &Path, file: &str) -> (Session, PathBuf) {
    let mut s = s();
    run(&mut s, "file.options", json!({"alwaysCreateBackup": true}));
    let path = dir.join(file);
    run(&mut s, "text.insert", json!({"text": "First version. "}));
    let r = run(&mut s, "file.save", json!({"path": p(&path)}));
    assert_eq!(r["saved"], json!(true));
    (s, path)
}

#[test]
fn the_setting_is_off_by_default_and_file_options_sets_it() {
    let mut s = s();
    assert_eq!(run(&mut s, "file.options", json!({"typingReplacesSelection": true}))["alwaysCreateBackup"], json!(false));
    assert_eq!(run(&mut s, "file.options", json!({"alwaysCreateBackup": true}))["alwaysCreateBackup"], json!(true));
    assert_eq!(run(&mut s, "file.options", json!({"overtype": false}))["alwaysCreateBackup"], json!(true));
    assert!(matches!(s.run("file.options", &json!({"alwaysCreateBackup": "yes"})), Err(CmdError::Params(_))));
    assert_eq!(run(&mut s, "file.options", json!({"alwaysCreateBackup": false}))["alwaysCreateBackup"], json!(false));
}

#[test]
fn with_the_setting_off_saves_make_no_backup() {
    let dir = temp_dir("off");
    let mut s = s();
    let path = dir.join("Report.docx");
    run(&mut s, "text.insert", json!({"text": "One. "}));
    run(&mut s, "file.save", json!({"path": p(&path)}));
    run(&mut s, "text.insert", json!({"text": "Two. "}));
    let r = run(&mut s, "file.save", json!({}));
    assert_eq!(r["saved"], json!(true));
    assert!(r.get("backup").is_none() && r.get("backupError").is_none(), "{r}");
    assert!(backups(&dir).is_empty(), "{:?}", backups(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_first_save_of_a_new_document_makes_no_backup() {
    let dir = temp_dir("first");
    let (_s, path) = saved_once(&dir, "Report.docx");
    assert!(path.is_file());
    assert!(backups(&dir).is_empty(), "{:?}", backups(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_again_keeps_the_previous_version_and_each_save_replaces_it() {
    let dir = temp_dir("again");
    let (mut s, path) = saved_once(&dir, "Report.docx");
    let backup = dir.join("Backup of Report.wbk");

    let first = std::fs::read(&path).unwrap();
    run(&mut s, "text.insert", json!({"text": "Second version. "}));
    let r = run(&mut s, "file.save", json!({}));
    assert_eq!(r["saved"], json!(true));
    assert_eq!(r["backup"], json!(p(&backup)), "{r}");
    assert!(r.get("backupError").is_none(), "{r}");
    assert_eq!(std::fs::read(&backup).unwrap(), first, "the backup is the file as it was before this save");
    let second = std::fs::read(&path).unwrap();
    assert_ne!(second, first);

    run(&mut s, "text.insert", json!({"text": "Third version. "}));
    run(&mut s, "file.save", json!({}));
    assert_eq!(std::fs::read(&backup).unwrap(), second, "the next save replaces the backup");
    assert_eq!(backups(&dir), ["Backup of Report.wbk"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_backup_name_drops_only_the_last_extension() {
    let dir = temp_dir("stem");
    let (mut s, path) = saved_once(&dir, "My.Notes.v2.rtf");
    let first = std::fs::read(&path).unwrap();
    run(&mut s, "text.insert", json!({"text": "More. "}));
    run(&mut s, "file.save", json!({}));
    assert_eq!(backups(&dir), ["Backup of My.Notes.v2.wbk"]);
    assert_eq!(std::fs::read(dir.join("Backup of My.Notes.v2.wbk")).unwrap(), first);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_as_under_a_new_name_makes_no_backup() {
    let dir = temp_dir("saveas");
    let (mut s, _path) = saved_once(&dir, "Report.docx");
    run(&mut s, "text.insert", json!({"text": "Second version. "}));
    run(&mut s, "file.save", json!({}));
    let report_backup = std::fs::read(dir.join("Backup of Report.wbk")).unwrap();

    run(&mut s, "text.insert", json!({"text": "Third version. "}));
    let r = run(&mut s, "file.save", json!({"path": p(&dir.join("Other.docx"))}));
    assert_eq!(r["saved"], json!(true));
    assert!(r.get("backup").is_none(), "{r}");
    assert_eq!(backups(&dir), ["Backup of Report.wbk"]);
    assert_eq!(std::fs::read(dir.join("Backup of Report.wbk")).unwrap(), report_backup, "the old document's backup is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_save_that_asks_for_no_backup_makes_none() {
    let dir = temp_dir("nobackup");
    let (mut s, _path) = saved_once(&dir, "Report.docx");
    run(&mut s, "text.insert", json!({"text": "Second version. "}));
    let r = run(&mut s, "file.save", json!({"backup": false}));
    assert_eq!(r["saved"], json!(true));
    assert!(r.get("backup").is_none() && r.get("backupError").is_none(), "{r}");
    assert!(backups(&dir).is_empty(), "{:?}", backups(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_backup_that_cannot_be_written_does_not_stop_the_save() {
    let dir = temp_dir("blocked");
    let (mut s, path) = saved_once(&dir, "Report.docx");
    let first = std::fs::read(&path).unwrap();
    // A folder already holds the backup's name, so the backup can't be written.
    std::fs::create_dir(dir.join("Backup of Report.wbk")).unwrap();
    run(&mut s, "text.insert", json!({"text": "Second version. "}));
    let r = run(&mut s, "file.save", json!({}));
    assert_eq!(r["saved"], json!(true), "{r}");
    assert!(r.get("backup").is_none(), "{r}");
    assert!(r["backupError"].as_str().is_some_and(|e| !e.is_empty()), "{r}");
    assert_ne!(std::fs::read(&path).unwrap(), first, "the document itself is saved");
    assert!(dir.join("Backup of Report.wbk").is_dir());
    let _ = std::fs::remove_dir_all(&dir);
}
