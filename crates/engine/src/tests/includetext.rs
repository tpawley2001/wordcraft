//! INCLUDETEXT (ECMA-376 §17.16.5.30): on F9 the field shows the text of another file, or of one
//! bookmark in it; network paths and URLs are refused without being read, and automation's read
//! root applies.

use std::path::PathBuf;

use serde_json::json;
use wordcraft_doc::{CharProps, InlineObject, StoryRef};

use crate::Session;

/// A fresh folder under the temp dir for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wc-includetext-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A field code path as Word writes it: backslashes doubled.
fn code_path(p: &std::path::Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

/// One paragraph "Here: " ending in a field `instr` with the stale result "OLD".
fn session(instr: &str, locked: bool) -> Session {
    let mut doc = wordcraft_doc::Document::from_text("Here: ");
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let p = doc.para_mut(StoryRef::Body, &path).unwrap();
    let field = InlineObject::Field { instr: instr.into(), result: "OLD".into(), locked, dirty: false, code: false, form: String::new(), code_paras: None };
    p.insert_object(p.len(), field, &CharProps::default()).unwrap();
    Session::new(doc)
}

fn update(s: &mut Session) {
    s.run("references.updateFields", &json!({"all": true})).expect("F9");
}

fn result(s: &Session) -> String {
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                return result.clone();
            }
        }
    }
    panic!("no field")
}

#[test]
fn a_text_file_is_included_on_f9() {
    let dir = scratch("txt");
    let src = dir.join("note.txt");
    std::fs::write(&src, "Hello include").unwrap();
    for instr in [format!("INCLUDETEXT \"{}\"", code_path(&src)), format!("INCLUDETEXT \"{}\" \\! \\c Text", code_path(&src))] {
        let mut s = session(&instr, false);
        update(&mut s);
        assert_eq!(result(&s).trim_end(), "Hello include", "{instr}");
    }
}

#[test]
fn a_bookmark_argument_includes_only_that_range_of_a_docx() {
    let dir = scratch("docx");
    let src = dir.join("source.docx");
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:r><w:t>Intro text</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Keep </w:t></w:r><w:bookmarkStart w:id="0" w:name="Part"/><w:r><w:t>this part</w:t></w:r><w:bookmarkEnd w:id="0"/><w:r><w:t xml:space="preserve"> only</w:t></w:r></w:p>
</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#;
    let mut source = Session::new(wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads"));
    source.run("file.saveAs", &json!({"path": src.display().to_string()})).expect("save source");

    let mut s = session(&format!("INCLUDETEXT \"{}\" Part", code_path(&src)), false);
    update(&mut s);
    assert_eq!(result(&s).trim_end(), "this part");

    let mut whole = session(&format!("INCLUDETEXT \"{}\"", code_path(&src)), false);
    update(&mut whole);
    let text = result(&whole);
    let (a, b) = (text.find("Intro text"), text.find("Keep this part only"));
    assert!(a.is_some() && b.is_some() && a < b, "{text:?}");

    let mut missing = session(&format!("INCLUDETEXT \"{}\" NoSuchMark", code_path(&src)), false);
    update(&mut missing);
    assert!(result(&missing).starts_with("Error!"), "{:?}", result(&missing));
}

#[test]
fn unreadable_network_and_url_paths_give_an_error_result() {
    let dir = scratch("errors");
    let instrs = [
        format!("INCLUDETEXT \"{}\"", code_path(&dir.join("absent.txt"))),
        r#"INCLUDETEXT "\\\\server\\share\\a.txt""#.to_string(),
        r#"INCLUDETEXT "//server/share/a.txt""#.to_string(),
        r#"INCLUDETEXT "http://example.invalid/a.txt""#.to_string(),
        r#"INCLUDETEXT "file://server/share/a.txt""#.to_string(),
        "INCLUDETEXT".to_string(),
        // A relative path with no document folder to resolve it against.
        r#"INCLUDETEXT "a.txt""#.to_string(),
    ];
    for instr in instrs {
        let mut s = session(&instr, false);
        update(&mut s);
        assert!(result(&s).starts_with("Error!"), "{instr}: {:?}", result(&s));
    }
}

#[test]
fn a_relative_path_resolves_against_the_documents_folder() {
    let dir = scratch("relative");
    std::fs::write(dir.join("near.txt"), "Beside the document").unwrap();
    let mut s = session(r#"INCLUDETEXT "near.txt""#, false);
    s.path = Some(dir.join("host.docx"));
    update(&mut s);
    assert_eq!(result(&s).trim_end(), "Beside the document");
}

#[test]
fn a_locked_field_keeps_its_result() {
    let dir = scratch("locked");
    let src = dir.join("note.txt");
    std::fs::write(&src, "New text").unwrap();
    let mut s = session(&format!("INCLUDETEXT \"{}\"", code_path(&src)), true);
    update(&mut s);
    assert_eq!(result(&s), "OLD");
}

#[test]
fn automation_reads_only_inside_its_read_root() {
    let dir = scratch("roots");
    let inside = dir.join("in");
    let outside = dir.join("out");
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(inside.join("ok.txt"), "Allowed").unwrap();
    std::fs::write(outside.join("secret.txt"), "Secret").unwrap();
    let roots = crate::file_access::AutomationRoots::new(Some(inside.as_path()), None).unwrap();

    let mut ok = session(&format!("INCLUDETEXT \"{}\"", code_path(&inside.join("ok.txt"))), false);
    crate::file_access::confine(roots.as_ref(), || update(&mut ok));
    assert_eq!(result(&ok).trim_end(), "Allowed");

    let mut refused = session(&format!("INCLUDETEXT \"{}\"", code_path(&outside.join("secret.txt"))), false);
    crate::file_access::confine(roots.as_ref(), || update(&mut refused));
    let r = result(&refused);
    assert!(r.starts_with("Error!") && !r.contains("Secret"), "{r:?}");
}
