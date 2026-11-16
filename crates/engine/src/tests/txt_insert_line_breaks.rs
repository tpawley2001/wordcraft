//! File Conversion's Insert line breaks (`insertLineBreaks` on `file.save` / `file.saveAs` of a
//! .txt file): every wrapped line of the main text ends with the chosen line end, as Word writes it.

use serde_json::json;

use crate::Session;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wc-txt-lb-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A paragraph long enough to wrap onto several lines at the default page width.
fn long_para() -> String {
    (0..90).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ")
}

/// The saved UTF-8 text without its byte order mark.
fn saved(path: &std::path::Path) -> String {
    let b = std::fs::read(path).unwrap();
    let b = b.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&b);
    String::from_utf8(b.to_vec()).unwrap()
}

fn layout_lines(s: &mut Session) -> usize {
    let v = s.run("review.wordCount", &json!({})).unwrap();
    usize::try_from(v["lines"].as_u64().unwrap()).unwrap()
}

#[test]
fn each_wrapped_line_ends_with_a_line_end() {
    let dir = scratch("wrap");
    let path = dir.join("a.txt");
    let text = long_para();
    let mut s = Session::new(wordcraft_doc::Document::from_text(&text));
    let lines = layout_lines(&mut s);
    assert!(lines >= 3, "fixture must wrap: {lines} lines");
    s.run("file.save", &json!({"path": path, "encoding": "utf-8", "lineEnd": "lf", "insertLineBreaks": true})).unwrap();
    let out = saved(&path);
    let got: Vec<&str> = out.split('\n').collect();
    assert_eq!(got.len(), lines, "{out:?}");
    // No word is split or lost; the space where a line wraps is not written.
    assert!(got.iter().all(|l| !l.is_empty() && !l.ends_with(' ') && !l.starts_with(' ')), "{got:?}");
    assert_eq!(got.join(" "), text);
}

#[test]
fn without_insert_line_breaks_a_paragraph_is_one_line() {
    let dir = scratch("plain");
    let path = dir.join("a.txt");
    let text = long_para();
    let mut s = Session::new(wordcraft_doc::Document::from_text(&text));
    s.run("file.save", &json!({"path": path, "encoding": "utf-8", "lineEnd": "lf"})).unwrap();
    assert_eq!(saved(&path), text);
    s.run("file.save", &json!({"path": path, "encoding": "utf-8", "lineEnd": "lf", "insertLineBreaks": false})).unwrap();
    assert_eq!(saved(&path), text);
}

#[test]
fn paragraph_ends_and_empty_paragraphs_stay_and_take_the_chosen_line_end() {
    let dir = scratch("paras");
    let path = dir.join("a.txt");
    let long = long_para();
    let doc_text = format!("Short\n\n{long}\nEnd");
    let mut s = Session::new(wordcraft_doc::Document::from_text(&doc_text));
    let lines = layout_lines(&mut s);
    s.run("file.saveAs", &json!({"path": path, "encoding": "utf-8", "lineEnd": "crlf", "insertLineBreaks": true})).unwrap();
    let out = saved(&path);
    assert!(!out.replace("\r\n", "").contains('\n') && !out.replace("\r\n", "").contains('\r'), "only CRLF line ends: {out:?}");
    let got: Vec<&str> = out.split("\r\n").collect();
    assert_eq!(got.first(), Some(&"Short"));
    assert_eq!(got.get(1), Some(&""), "the empty paragraph is an empty line");
    assert_eq!(got.last(), Some(&"End"));
    let middle = got.get(2..got.len() - 1).unwrap();
    assert!(middle.len() >= 3, "{got:?}");
    assert_eq!(middle.join(" "), long);
    // Word Count's Lines counts the empty paragraph's line too.
    assert_eq!(got.len(), lines, "{got:?}");
}

#[test]
fn insert_line_breaks_alone_turns_on_the_file_conversion_options() {
    let o = crate::cmd::file::txt_options(&json!({"insertLineBreaks": true})).expect("options");
    assert!(o.insert_line_breaks);
    assert!(!crate::cmd::file::txt_options(&json!({"encoding": "utf-8"})).unwrap().insert_line_breaks);
}
