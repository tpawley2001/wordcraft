//! `file.save` of a .txt file with Word's File Conversion choices (`encoding`, `lineEnd`,
//! `allowSubstitution`); without them a .txt file is UTF-8 with CRLF line ends as before.

use serde_json::json;

use crate::Session;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wc-txt-save-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn session(text: &str) -> Session {
    Session::new(wordcraft_doc::Document::from_text(text))
}

#[test]
fn plain_save_stays_utf8_crlf() {
    let dir = scratch("plain");
    let path = dir.join("a.txt");
    let mut s = session("Caf\u{e9}\nb");
    s.run("file.save", &json!({"path": path})).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), "Caf\u{e9}\r\nb".as_bytes());
}

#[test]
fn file_conversion_choices_shape_the_bytes() {
    let dir = scratch("choices");
    let path = dir.join("a.txt");
    let mut s = session("Caf\u{e9} \u{201c}x\u{201d}\nb");
    s.run("file.save", &json!({"path": path, "encoding": "msdos", "lineEnd": "lf", "allowSubstitution": true})).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"Caf\x82 \"x\"\nb");
    s.run("file.saveAs", &json!({"path": path, "encoding": "utf-16be", "lineEnd": "cr"})).unwrap();
    assert_eq!(std::fs::read(&path).unwrap()[..4], [0xFE, 0xFF, 0, b'C']);
    s.run("file.save", &json!({"path": path, "encoding": "windows-1251"})).unwrap();
    // windows-1251 has the curly quotes but no e-acute: one question mark.
    assert_eq!(std::fs::read(&path).unwrap(), b"Caf? \x93x\x94\r\nb");
}
