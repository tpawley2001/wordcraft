//! Read-only recommended and Password to modify (`w:writeProtection`, ECMA-376 Part 1
//! §17.15.1.93) and Lock Tracking, as in Word. A document opened read-only can be edited but
//! not saved over its file; saving it under another name makes an ordinary document.
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
    let dir = std::env::temp_dir().join(format!("wordcraft-engine-writeprot-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn p(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A Word XML (Flat OPC) document whose settings hold `write_protection` (the whole element).
fn write_flat(path: &Path, write_protection: &str) {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/_rels/document.xml.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p></w:body></w:document></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/settings.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml">
    <pkg:xmlData><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{write_protection}<w:defaultTabStop w:val="720"/></w:settings></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    std::fs::write(path, xml).unwrap();
}

/// `w:writeProtection` with a modify password, in the form `protection_hash` writes.
fn with_password(password: &str, recommended: bool) -> String {
    let attrs = wordcraft_docx::protection_hash(password, 1000).unwrap();
    let mut el = String::from("<w:writeProtection");
    if recommended {
        el.push_str(r#" w:recommended="1""#);
    }
    for (k, v) in attrs {
        el.push_str(&format!(r#" {k}="{v}""#));
    }
    el + "/>"
}

fn err(r: Result<Value, CmdError>) -> String {
    match r {
        Ok(v) => panic!("expected an error, got {v}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_modify_password_must_be_given_or_read_only_chosen() {
    let dir = temp_dir("pw");
    let file = dir.join("locked.xml");
    write_flat(&file, &with_password("open sesame", false));
    let mut s = s();
    let e = err(s.run("file.open", &json!({"path": p(&file)})));
    assert!(e.contains("password to modify"), "{e}");
    let e = err(s.run("file.open", &json!({"path": p(&file), "modifyPassword": "Open Sesame"})));
    assert!(e.contains("password is incorrect"), "{e}");
    let r = run(&mut s, "file.open", json!({"path": p(&file), "modifyPassword": "open sesame"}));
    assert_eq!(r["readOnly"], json!(false));
    run(&mut s, "text.insert", json!({"text": "x"}));
    run(&mut s, "file.save", json!({}));
}

#[test]
fn read_only_edits_but_saves_only_under_another_name() {
    let dir = temp_dir("ro");
    let file = dir.join("locked.xml");
    write_flat(&file, &with_password("pw", false));
    let before = std::fs::read(&file).unwrap();
    let mut s = s();
    let r = run(&mut s, "file.open", json!({"path": p(&file), "readOnly": true}));
    assert_eq!(r["readOnly"], json!(true));
    assert_eq!(run(&mut s, "file.readOnly", json!({})), json!({"readOnly": true}));
    // Editing is allowed.
    run(&mut s, "text.insert", json!({"text": "New "}));
    // Saving over the file is not.
    let e = err(s.run("file.save", &json!({})));
    assert!(e.contains("read-only"), "{e}");
    let e = err(s.run("file.save", &json!({"path": p(&file)})));
    assert!(e.contains("read-only"), "{e}");
    assert_eq!(std::fs::read(&file).unwrap(), before, "the file is untouched");
    // Under another name it saves, and the copy is an ordinary document.
    let copy = dir.join("copy.docx");
    run(&mut s, "file.saveAs", json!({"path": p(&copy)}));
    assert!(copy.exists());
    assert_eq!(run(&mut s, "file.readOnly", json!({})), json!({"readOnly": false}));
    run(&mut s, "text.insert", json!({"text": "more "}));
    run(&mut s, "file.save", json!({}));
}

#[test]
fn read_only_recommended_is_reported_on_open() {
    let dir = temp_dir("rec");
    let file = dir.join("rec.xml");
    write_flat(&file, r#"<w:writeProtection w:recommended="1"/>"#);
    let mut s = s();
    let r = run(&mut s, "file.open", json!({"path": p(&file)}));
    assert_eq!(r["readOnlyRecommended"], json!(true));
    assert_eq!(r["readOnly"], json!(false), "the user chooses; the engine doesn't");
    let plain = dir.join("plain.xml");
    write_flat(&plain, "");
    let r = run(&mut s, "file.open", json!({"path": p(&plain)}));
    assert_eq!(r["readOnlyRecommended"], json!(false));
}

#[test]
fn general_options_write_and_remove_write_protection() {
    let dir = temp_dir("opts");
    let file = dir.join("opts.docx");
    let mut s = s();
    let r = run(&mut s, "file.generalOptions", json!({"modifyPassword": "pw", "readOnlyRecommended": true}));
    assert_eq!(r, json!({"modifyPassword": true, "readOnlyRecommended": true}));
    run(&mut s, "file.saveAs", json!({"path": p(&file)}));
    let flat = String::from_utf8(wordcraft_docx::write_flat(&s.doc).unwrap()).unwrap();
    assert!(flat.contains("<w:writeProtection"), "written");
    assert!(flat.contains(r#"w:recommended="1""#));
    // Reopened: the password is asked for, and the right one opens it for editing.
    let mut t = self::s();
    assert!(err(t.run("file.open", &json!({"path": p(&file)}))).contains("password to modify"));
    let r = run(&mut t, "file.open", json!({"path": p(&file), "modifyPassword": "pw"}));
    assert_eq!(r["readOnlyRecommended"], json!(true));
    // Removing both removes the element.
    let r = run(&mut t, "file.generalOptions", json!({"modifyPassword": "", "readOnlyRecommended": false}));
    assert_eq!(r, json!({"modifyPassword": false, "readOnlyRecommended": false}));
    let flat = String::from_utf8(wordcraft_docx::write_flat(&t.doc).unwrap()).unwrap();
    assert!(!flat.contains("writeProtection"));
}

/// `w:writeProtection` is modelled, not passed through: a file's own element survives a save
/// unchanged in meaning.
#[test]
fn a_files_write_protection_survives_open_and_save() {
    let dir = temp_dir("keep");
    let file = dir.join("keep.xml");
    write_flat(&file, &with_password("pw", true));
    let out = dir.join("out.docx");
    let mut s = s();
    run(&mut s, "file.open", json!({"path": p(&file), "modifyPassword": "pw"}));
    run(&mut s, "file.saveAs", json!({"path": p(&out)}));
    let mut t = self::s();
    assert!(err(t.run("file.open", &json!({"path": p(&out)}))).contains("password to modify"));
    let r = run(&mut t, "file.open", json!({"path": p(&out), "modifyPassword": "pw"}));
    assert_eq!(r["readOnlyRecommended"], json!(true));
}

#[test]
fn lock_tracking_keeps_track_changes_on_until_unlocked() {
    let mut s = s();
    assert_eq!(run(&mut s, "review.lockTracking", json!({"password": "pw"})), json!({"locked": true}));
    assert!(s.doc.settings.track_changes, "tracking is on");
    assert_eq!(s.doc.settings.protection.as_deref(), Some("trackedChanges"));
    assert!(s.run("review.trackChanges", &json!({"value": false})).is_err(), "can't turn it off while locked");
    assert!(s.doc.settings.track_changes);
    assert!(s.run("review.lockTracking", &json!({"unlock": true, "password": "nope"})).is_err());
    assert_eq!(s.doc.settings.protection.as_deref(), Some("trackedChanges"), "a wrong password leaves it locked");
    assert_eq!(run(&mut s, "review.lockTracking", json!({"unlock": true, "password": "pw"})), json!({"locked": false}));
    assert!(s.doc.settings.protection.is_none());
    run(&mut s, "review.trackChanges", json!({"value": false}));
    assert!(!s.doc.settings.track_changes);
}
