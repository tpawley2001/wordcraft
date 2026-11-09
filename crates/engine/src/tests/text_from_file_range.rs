//! Insert › Text from File › Range: a bookmark name inserts only the text that bookmark marks in
//! the chosen file, as Word's Insert File dialog does. An unknown bookmark is an error and leaves
//! the document as it was.

use serde_json::{Value, json};

use crate::Session;
use wordcraft_doc::StoryRef;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn lines(s: &Session) -> Vec<String> {
    s.doc.plain_text(StoryRef::Body).lines().map(str::to_string).collect()
}

/// Paragraphs `alpha [Mid]beta[/Mid] gamma` (beta bold), `one`, `[Two]two`, `three[/Two]`, `four`,
/// saved as a .docx in a fresh temp dir; returns the file's path.
fn source_file(tag: &str) -> std::path::PathBuf {
    let body = r#"<w:p><w:r><w:t xml:space="preserve">alpha </w:t></w:r><w:bookmarkStart w:id="1" w:name="Mid"/><w:r><w:rPr><w:b/></w:rPr><w:t>beta</w:t></w:r><w:bookmarkEnd w:id="1"/><w:r><w:t xml:space="preserve"> gamma</w:t></w:r></w:p>
<w:p><w:r><w:t>one</w:t></w:r></w:p>
<w:p><w:bookmarkStart w:id="2" w:name="Two"/><w:r><w:t>two</w:t></w:r></w:p>
<w:p><w:r><w:t>three</w:t></w:r><w:bookmarkEnd w:id="2"/></w:p>
<w:p><w:r><w:t>four</w:t></w:r></w:p>"#;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    let doc = wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads");
    let dir = std::env::temp_dir().join(format!("wordcraft-text-from-file-range-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("source.docx");
    std::fs::write(&path, crate::io::save_bytes("source.docx", &doc).unwrap()).unwrap();
    path
}

/// A document holding "x" with the caret at its end.
fn target() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "document.setText", json!({"text": "x"}));
    run(&mut s, "caret.docEnd", json!({}));
    s
}

#[test]
fn without_range_the_whole_file_comes_in() {
    let path = source_file("whole");
    let mut s = target();
    run(&mut s, "insert.textFromFile", json!({"path": path.to_str().unwrap()}));
    let l = lines(&s);
    assert_eq!(l.first().map(String::as_str), Some("xalpha beta gamma"));
    assert!(l.iter().any(|t| t == "four"), "{l:?}");
}

#[test]
fn range_inside_one_paragraph_inserts_only_the_bookmarked_text() {
    let path = source_file("mid");
    let mut s = target();
    run(&mut s, "insert.textFromFile", json!({"path": path.to_str().unwrap(), "range": "Mid"}));
    assert_eq!(lines(&s), ["xbeta"]);
    let pos = wordcraft_doc::Pos { story: StoryRef::Body, path: wordcraft_doc::Path::top(0), off: 2 };
    let bold = s.doc.para_at(&pos).and_then(|p| p.props_of_char(2).bold);
    assert_eq!(bold, Some(true), "the bookmarked run keeps its bold");
}

#[test]
fn range_names_match_without_regard_to_case() {
    let path = source_file("case");
    let mut s = target();
    run(&mut s, "insert.textFromFile", json!({"path": path.to_str().unwrap(), "range": "mid"}));
    assert_eq!(lines(&s), ["xbeta"]);
}

#[test]
fn range_over_several_paragraphs_inserts_those_paragraphs() {
    let path = source_file("two");
    let mut s = target();
    run(&mut s, "insert.textFromFile", json!({"path": path.to_str().unwrap(), "range": "Two"}));
    assert_eq!(lines(&s), ["xtwo", "three"]);
}

#[test]
fn unknown_range_is_an_error_and_changes_nothing() {
    let path = source_file("nope");
    let mut s = target();
    let err = s.run("insert.textFromFile", &json!({"path": path.to_str().unwrap(), "range": "Nope"})).unwrap_err();
    assert!(err.to_string().contains("Nope"), "{err}");
    assert_eq!(lines(&s), ["x"]);
    let err = s.run("insert.textFromFile", &json!({"path": path.to_str().unwrap(), "range": "  "}));
    assert!(err.is_err(), "an empty range names no bookmark");
    assert_eq!(lines(&s), ["x"]);
}

#[test]
fn range_insert_is_one_undo_step() {
    let path = source_file("undo");
    let mut s = target();
    run(&mut s, "insert.textFromFile", json!({"path": path.to_str().unwrap(), "range": "Mid"}));
    assert_eq!(lines(&s), ["xbeta"]);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(lines(&s), ["x"]);
}
