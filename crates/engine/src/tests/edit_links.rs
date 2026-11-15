//! Edit Links to Files (Word's Links dialog): `fields.links` lists the main text's LINK fields
//! (ECMA-376 §17.16.5.32) and `fields.editLink` sets one link's update method (`\a`), lock,
//! preserve-formatting switch (`\* MERGEFORMAT`) and source, updates it now or breaks it.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{CharProps, Document, InlineObject, Pos, StoryRef};

use crate::Session;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wc-edit-links-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A field code path as Word writes it: backslashes doubled.
fn code_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

/// Save a .docx at `path` whose bookmark OLE_LINK1 holds `linked`.
fn save_source(path: &Path, linked: &str) {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:r><w:t xml:space="preserve">Keep </w:t></w:r><w:bookmarkStart w:id="0" w:name="OLE_LINK1"/><w:r><w:t>{linked}</w:t></w:r><w:bookmarkEnd w:id="0"/></w:p>
</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    let mut source = Session::new(wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads"));
    source.run("file.saveAs", &json!({"path": path.display().to_string()})).expect("save source");
}

fn link_to(path: &Path, sw: &str) -> String {
    format!("LINK Word.Document.12 \"{}\" \"OLE_LINK1\" {sw}", code_path(path))
}

/// "Here: ", then each `(instr, locked)` as a range field with the stored result "OLD", then " tail".
fn host(fields: &[(&str, bool)]) -> Session {
    let mut doc = Document::from_text("Here: ");
    let props = CharProps::default();
    let mut at = Pos::body(0, 6);
    for (instr, locked) in fields {
        at = doc.insert_object(&at, InlineObject::FieldStart { instr: (*instr).into(), locked: *locked, dirty: false }, &props).unwrap();
        at = doc.insert_text(&at, "OLD", &props).unwrap();
        at = doc.insert_object(&at, InlineObject::FieldEnd, &props).unwrap();
    }
    doc.insert_text(&at, " tail", &props).unwrap();
    Session::new(doc)
}

fn links(s: &mut Session) -> Vec<Value> {
    let v = s.run("fields.links", &json!({})).expect("fields.links");
    v["links"].as_array().expect("a links array").clone()
}

fn edit(s: &mut Session, params: Value) {
    s.run("fields.editLink", &params).expect("fields.editLink");
}

fn instrs(s: &Session) -> Vec<String> {
    s.doc.field_ranges(StoryRef::Body).into_iter().map(|f| f.instr.trim().to_string()).collect()
}

/// The text between the first range field's markers.
fn result(s: &Session) -> String {
    let r = s.doc.field_ranges(StoryRef::Body).into_iter().next().expect("a range field");
    let from = Pos { off: r.start.off.saturating_add(OBJ.len_utf8()), ..r.start.clone() };
    s.doc.copy_range(&from, &r.end).plain_text()
}

fn letters(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body).chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == ':').collect()
}

#[test]
fn links_lists_each_link_with_its_source_item_and_settings() {
    let auto = r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1" \a \t"#;
    let manual = r#"LINK Word.Document.12 "C:\\d\\b.docx" "OLE_LINK2" \t \* MERGEFORMAT"#;
    let mut s = host(&[(auto, false), ("PAGE", false), (manual, true)]);
    let l = links(&mut s);
    assert_eq!(l.len(), 2, "PAGE is not a link: {l:?}");
    assert_eq!(l[0]["index"], 0);
    assert_eq!(l[0]["source"], r"C:\d\a.docx", "the path as a path, backslashes not doubled");
    assert_eq!(l[0]["item"], "OLE_LINK1");
    assert_eq!(l[0]["progId"], "Word.Document.12");
    assert_eq!(l[0]["update"], "auto");
    assert_eq!(l[0]["locked"], false);
    assert_eq!(l[0]["preserveFormatting"], false);
    assert_eq!(l[1]["index"], 1);
    assert_eq!(l[1]["source"], r"C:\d\b.docx");
    assert_eq!(l[1]["item"], "OLE_LINK2");
    assert_eq!(l[1]["update"], "manual");
    assert_eq!(l[1]["locked"], true);
    assert_eq!(l[1]["preserveFormatting"], true);
}

#[test]
fn other_file_fields_and_hyperlinks_are_not_listed() {
    let mut s = host(&[(r#"INCLUDETEXT "C:\\d\\a.docx""#, false), (r#"HYPERLINK "https://example.com""#, false)]);
    assert!(links(&mut s).is_empty());
}

#[test]
fn manual_update_removes_only_the_a_switch_and_undoes_in_one_step() {
    let instr = r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1" \a \t"#;
    let mut s = host(&[(instr, false)]);
    edit(&mut s, json!({"index": 0, "update": "manual"}));
    let code = instrs(&s).remove(0);
    assert!(!has_switch(&code, r"\a"), "{code}");
    assert!(code.starts_with(r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1""#), "{code}");
    assert!(has_switch(&code, r"\t"), "{code}");
    assert_eq!(links(&mut s)[0]["update"], "manual");
    edit(&mut s, json!({"index": 0, "update": "auto"}));
    assert_eq!(links(&mut s)[0]["update"], "auto");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(links(&mut s)[0]["update"], "manual", "one undo step per change");
}

#[test]
fn locked_and_preserve_formatting_set_the_lock_flag_and_mergeformat() {
    let instr = r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1" \a \t"#;
    let mut s = host(&[(instr, false)]);
    edit(&mut s, json!({"index": 0, "locked": true, "preserveFormatting": true}));
    assert!(s.doc.field_ranges(StoryRef::Body)[0].locked);
    assert!(instrs(&s)[0].contains(r"\* MERGEFORMAT"), "{:?}", instrs(&s));
    edit(&mut s, json!({"index": 0, "preserveFormatting": false}));
    assert!(!instrs(&s)[0].to_ascii_uppercase().contains("MERGEFORMAT"), "{:?}", instrs(&s));
    assert!(s.doc.field_ranges(StoryRef::Body)[0].locked, "the lock stays");
}

#[test]
fn update_now_updates_an_unlocked_link_and_leaves_a_locked_one() {
    let dir = scratch("update-now");
    let src = dir.join("source.docx");
    save_source(&src, "fresh text");
    let instr = link_to(&src, r"\a \t");
    let mut s = host(&[(&instr, true)]);
    edit(&mut s, json!({"index": 0, "action": "updateNow"}));
    assert_eq!(result(&s), "OLD", "a locked link is not updated");
    edit(&mut s, json!({"index": 0, "locked": false}));
    edit(&mut s, json!({"index": 0, "action": "updateNow"}));
    assert_eq!(result(&s), "fresh text");
    assert_eq!(instrs(&s), vec![instr.trim().to_string()], "the code stays");
}

#[test]
fn change_source_rewrites_the_path_and_shows_the_new_sources_text() {
    let dir = scratch("change-source");
    let old = dir.join("old.docx");
    let new = dir.join("new.docx");
    save_source(&old, "old source");
    save_source(&new, "new source");
    let mut s = host(&[(&link_to(&old, r"\a \t"), false)]);
    edit(&mut s, json!({"index": 0, "source": new.display().to_string()}));
    let code = instrs(&s).remove(0);
    assert!(code.contains(&format!("\"{}\"", code_path(&new))), "{code}");
    assert!(!code.contains("old.docx"), "{code}");
    assert!(code.contains("Word.Document.12") && code.contains("\"OLE_LINK1\"") && has_switch(&code, r"\a"), "{code}");
    assert_eq!(links(&mut s)[0]["source"], new.display().to_string());
    assert_eq!(result(&s), "new source");
}

#[test]
fn break_link_keeps_the_result_as_text() {
    let instr = r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1" \a \t"#;
    let mut s = host(&[(instr, false), ("PAGE", false)]);
    edit(&mut s, json!({"index": 0, "action": "breakLink"}));
    assert!(links(&mut s).is_empty());
    assert_eq!(instrs(&s), vec!["PAGE".to_string()], "only the broken link's field goes");
    assert_eq!(letters(&s), "Here: OLDOLD tail");
}

#[test]
fn a_bad_index_is_an_error_and_changes_nothing() {
    let instr = r#"LINK Word.Document.12 "C:\\d\\a.docx" "OLE_LINK1" \a \t"#;
    let mut s = host(&[(instr, false)]);
    assert!(s.run("fields.editLink", &json!({"index": 5, "update": "manual"})).is_err());
    assert!(s.run("fields.editLink", &json!({"update": "manual"})).is_err());
    assert_eq!(instrs(&s), vec![instr.to_string()]);
}

/// True when `sw` appears as a whole switch token (not as text inside a quoted path).
fn has_switch(code: &str, sw: &str) -> bool {
    code.split_whitespace().any(|t| t == sw)
}
