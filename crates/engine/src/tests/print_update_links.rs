//! File › Options › Display › Update linked data before printing: links to other files (LINK fields,
//! ECMA-376 §17.16.5.32) are updated from their sources just before the job prints, as in Word.
//! Off by default. `fields.updateLinks` is Update Now on every link.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{CharProps, Document, InlineObject, Pos, StoryRef};

use crate::Session;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wc-print-links-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn code_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

/// A saved source: "Intro text", then "Keep <linked> only" with the bookmark OLE_LINK1 around `linked`.
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
<w:p><w:r><w:t>Intro text</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Keep </w:t></w:r><w:bookmarkStart w:id="0" w:name="OLE_LINK1"/><w:r><w:t>{linked}</w:t></w:r><w:bookmarkEnd w:id="0"/><w:r><w:t xml:space="preserve"> only</w:t></w:r></w:p>
</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    let mut source = Session::new(wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads"));
    source.run("file.saveAs", &json!({"path": path.display().to_string()})).expect("save source");
}

fn link(path: &Path) -> String {
    format!("LINK Word.Document.12 \"{}\" \"OLE_LINK1\" \\t", code_path(path))
}

/// "Here: ", then one range field per (instr, locked) with the stored result "OLD", each followed by " tail".
fn host(links: &[(&str, bool)]) -> Session {
    let mut doc = Document::from_text("Here: ");
    let props = CharProps::default();
    let mut at = Pos::body(0, 6);
    for (instr, locked) in links {
        at = doc.insert_object(&at, InlineObject::FieldStart { instr: (*instr).into(), locked: *locked, dirty: false }, &props).unwrap();
        at = doc.insert_text(&at, "OLD", &props).unwrap();
        at = doc.insert_object(&at, InlineObject::FieldEnd, &props).unwrap();
        at = doc.insert_text(&at, " tail", &props).unwrap();
    }
    Session::new(doc)
}

/// Each range field's result text, in order.
fn results(s: &Session) -> Vec<String> {
    s.doc
        .field_ranges(StoryRef::Body)
        .into_iter()
        .map(|r| {
            let from = Pos { off: r.start.off.saturating_add(OBJ.len_utf8()), ..r.start.clone() };
            s.doc.copy_range(&from, &r.end).plain_text()
        })
        .collect()
}

fn update_links(s: &mut Session) -> Value {
    s.run("fields.updateLinks", &json!({})).expect("fields.updateLinks runs")
}

#[test]
fn the_option_is_off_by_default_and_file_options_sets_it() {
    let mut s = Session::new(Document::from_text("x"));
    let r = s.run("file.options", &json!({"printReverse": false})).unwrap();
    assert_eq!(r["printUpdateLinks"], json!(false), "Word's default is off");
    let r = s.run("file.options", &json!({"printUpdateLinks": true})).unwrap();
    assert_eq!(r["printUpdateLinks"], json!(true));
    let saved = serde_json::to_value(&s.prefs).unwrap();
    assert_eq!(saved["printUpdateLinks"], json!(true), "saved under printUpdateLinks");
    let mut old = saved.clone();
    old.as_object_mut().unwrap().remove("printUpdateLinks");
    let old: crate::prefs::Prefs = serde_json::from_value(old).unwrap();
    assert_eq!(serde_json::to_value(&old).unwrap()["printUpdateLinks"], json!(false), "old prefs load with it off");
}

#[test]
fn update_links_runs_update_now_on_every_unlocked_link() {
    let dir = scratch("all");
    let (a, b) = (dir.join("a.docx"), dir.join("b.docx"));
    save_source(&a, "fresh text");
    save_source(&b, "second source");
    let (la, lb) = (link(&a), link(&b));
    let mut s = host(&[(&la, false), (&lb, false)]);
    assert_eq!(update_links(&mut s)["updated"], json!(2));
    assert_eq!(results(&s), vec!["fresh text".to_string(), "second source".to_string()]);

    let mut s = host(&[(&la, true), (&lb, false)]);
    assert_eq!(update_links(&mut s)["updated"], json!(1), "a locked link is skipped");
    assert_eq!(results(&s), vec!["OLD".to_string(), "second source".to_string()]);
}

#[test]
fn a_link_to_a_missing_file_keeps_its_result() {
    let dir = scratch("missing");
    let l = link(&dir.join("gone.docx"));
    let mut s = host(&[(&l, false)]);
    update_links(&mut s);
    assert_eq!(results(&s), vec!["OLD".to_string()]);
}

/// Print `s` to a PDF in `dir` with `extra` params; the PDF must be written.
fn print(s: &mut Session, dir: &Path, extra: Value) -> Result<Value, crate::CmdError> {
    let out = dir.join("out.pdf");
    let _ = std::fs::remove_file(&out);
    let mut v = json!({"path": out.display().to_string()});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let r = s.run("file.printPdf", &v);
    if r.is_ok() {
        assert!(out.exists(), "the job still prints");
    }
    r
}

#[test]
fn printing_updates_links_only_when_the_option_or_the_job_says_so() {
    let dir = scratch("print");
    let src = dir.join("source.docx");
    save_source(&src, "fresh text");
    let l = link(&src);

    let mut s = host(&[(&l, false)]);
    print(&mut s, &dir, json!({})).unwrap();
    assert_eq!(results(&s), vec!["OLD".to_string()], "off by default: the stored result prints");

    let mut s = host(&[(&l, false)]);
    s.run("file.options", &json!({"printUpdateLinks": true})).unwrap();
    print(&mut s, &dir, json!({})).unwrap();
    assert_eq!(results(&s), vec!["fresh text".to_string()], "on: updated before printing, and it stays");

    let mut s = host(&[(&l, false)]);
    s.run("file.options", &json!({"printUpdateLinks": true})).unwrap();
    print(&mut s, &dir, json!({"updateLinks": false})).unwrap();
    assert_eq!(results(&s), vec!["OLD".to_string()], "the job's updateLinks:false wins");

    let mut s = host(&[(&l, false)]);
    print(&mut s, &dir, json!({"updateLinks": true})).unwrap();
    assert_eq!(results(&s), vec!["fresh text".to_string()], "the job's updateLinks:true wins");

    let mut s = host(&[(&l, false)]);
    assert!(print(&mut s, &dir, json!({"updateLinks": "yes"})).is_err(), "updateLinks must be a bool");
}

#[test]
fn a_missing_source_never_stops_the_print() {
    let dir = scratch("print-missing");
    let l = link(&dir.join("gone.docx"));
    let mut s = host(&[(&l, false)]);
    print(&mut s, &dir, json!({"updateLinks": true})).unwrap();
    assert_eq!(results(&s), vec!["OLD".to_string()]);
}
