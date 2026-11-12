//! Update Field (F9) on a LINK field to a saved Word document (ECMA-376 §17.16.5.32, the field
//! Paste link inserts): the source file is read again and the field's result becomes the
//! bookmarked text as it is now. A link that can't be followed keeps its stored result.

use std::path::{Path, PathBuf};

use serde_json::json;
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{CharProps, Document, InlineObject, Pos, StoryRef};

use crate::Session;

/// A fresh folder under the temp dir for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wc-link-update-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A field code path as Word writes it: backslashes doubled.
fn code_path(p: &Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

/// Save a source document at `path` (format by extension): a paragraph "Intro text", then
/// "Keep <linked> only" with the bookmark OLE_LINK1 around `linked`, which is bold when `bold`.
fn save_source(path: &Path, linked: &str, bold: bool) {
    let rpr = if bold { "<w:rPr><w:b/></w:rPr>" } else { "" };
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
<w:p><w:r><w:t xml:space="preserve">Keep </w:t></w:r><w:bookmarkStart w:id="0" w:name="OLE_LINK1"/><w:r>{rpr}<w:t>{linked}</w:t></w:r><w:bookmarkEnd w:id="0"/><w:r><w:t xml:space="preserve"> only</w:t></w:r></w:p>
</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    let mut source = Session::new(wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads"));
    source.run("file.saveAs", &json!({"path": path.display().to_string()})).expect("save source");
}

/// The LINK code Paste link writes for `path`, with the switches `sw` (`\a \t`, `\r`...).
fn link(path: &Path, sw: &str) -> String {
    format!("LINK Word.Document.12 \"{}\" \"OLE_LINK1\" {sw}", code_path(path))
}

/// "Here: ", a range field `instr` whose stored result is "OLD", then " tail".
fn host(instr: &str, locked: bool) -> Session {
    let mut doc = Document::from_text("Here: ");
    let props = CharProps::default();
    let at = doc.insert_object(&Pos::body(0, 6), InlineObject::FieldStart { instr: instr.into(), locked, dirty: false }, &props).unwrap();
    let at = doc.insert_text(&at, "OLD", &props).unwrap();
    let at = doc.insert_object(&at, InlineObject::FieldEnd, &props).unwrap();
    doc.insert_text(&at, " tail", &props).unwrap();
    Session::new(doc)
}

/// One paragraph "Here: " ending in a simple field `instr` with the stored result "OLD".
fn simple_host(instr: &str) -> Session {
    let mut doc = Document::from_text("Here: ");
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let p = doc.para_mut(StoryRef::Body, &path).unwrap();
    let field = InlineObject::Field { instr: instr.into(), result: "OLD".into(), locked: false, dirty: false, code: false, form: String::new() };
    p.insert_object(p.len(), field, &CharProps::default()).unwrap();
    Session::new(doc)
}

fn update(s: &mut Session) {
    s.run("references.updateFields", &json!({"all": true})).expect("F9");
}

/// Where the first range field's result starts and ends.
fn result_span(s: &Session) -> (Pos, Pos) {
    let r = s.doc.field_ranges(StoryRef::Body).into_iter().next().expect("the field is still a range field");
    (Pos { off: r.start.off.saturating_add(OBJ.len_utf8()), ..r.start.clone() }, r.end)
}

/// The text between the first range field's markers.
fn result(s: &Session) -> String {
    let (from, to) = result_span(s);
    s.doc.copy_range(&from, &to).plain_text()
}

/// Is the first character of the first range field's result bold?
fn result_is_bold(s: &Session) -> bool {
    let (from, _) = result_span(s);
    s.doc.para_at(&from).expect("the result's paragraph").props_of_char(from.off).bold == Some(true)
}

fn simple_result(s: &Session) -> String {
    for path in s.doc.para_paths(StoryRef::Body) {
        for o in &s.doc.para(StoryRef::Body, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                return result.clone();
            }
        }
    }
    panic!("no field")
}

/// The body's letters and spaces: field and bookmark markers left out.
fn letters(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body).chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').collect()
}

fn instrs(s: &Session) -> Vec<String> {
    s.doc.field_ranges(StoryRef::Body).into_iter().map(|f| f.instr.trim().to_string()).collect()
}

#[test]
fn f9_shows_the_sources_text_for_text_links_automatic_or_manual() {
    let dir = scratch("text");
    let src = dir.join("source.docx");
    save_source(&src, "this part", true);
    for sw in [r"\a \t", r"\a \u", r"\t", r"\u"] {
        let instr = link(&src, sw);
        let mut s = host(&instr, false);
        update(&mut s);
        assert_eq!(result(&s), "this part", "{sw}");
        assert_eq!(letters(&s), "Here this part tail", "{sw}: the text around the field stays");
        assert_eq!(instrs(&s), std::slice::from_ref(&instr), "{sw}: the field and its code stay");
        assert!(!result_is_bold(&s), "{sw}: plain text takes the result's formatting, not the source's");
    }
}

#[test]
fn a_formatted_link_keeps_the_sources_formatting() {
    let dir = scratch("formatted");
    let src = dir.join("source.docx");
    save_source(&src, "this part", true);
    for sw in [r"\a \r", r"\r"] {
        let instr = link(&src, sw);
        let mut s = host(&instr, false);
        update(&mut s);
        assert_eq!(result(&s), "this part", "{sw}");
        assert!(result_is_bold(&s), "{sw}: the source's bold comes along");
        assert_eq!(letters(&s), "Here this part tail", "{sw}");
        assert_eq!(instrs(&s), std::slice::from_ref(&instr), "{sw}");
    }
}

#[test]
fn the_result_follows_the_source_each_time_it_is_updated() {
    let dir = scratch("follows");
    let src = dir.join("source.docx");
    save_source(&src, "first words", false);
    let mut s = host(&link(&src, r"\a \t"), false);
    update(&mut s);
    assert_eq!(result(&s), "first words");
    save_source(&src, "second longer words", false);
    update(&mut s);
    assert_eq!(result(&s), "second longer words");
    assert_eq!(letters(&s), "Here second longer words tail");
    assert_eq!(std::fs::read(&src).unwrap(), {
        update(&mut s);
        std::fs::read(&src).unwrap()
    });
}

#[test]
fn a_simple_link_field_gets_the_text() {
    let dir = scratch("simple");
    let src = dir.join("source.docx");
    save_source(&src, "this part", false);
    let mut s = simple_host(&link(&src, r"\a \t"));
    update(&mut s);
    assert_eq!(simple_result(&s), "this part");
}

#[test]
fn an_rtf_source_with_the_old_class_name_is_read_through_its_bookmark() {
    let dir = scratch("rtf");
    let src = dir.join("source.rtf");
    save_source(&src, "this part", false);
    // The fixture is sound: INCLUDETEXT reads the same bookmark from it.
    let mut include = simple_host(&format!("INCLUDETEXT \"{}\" OLE_LINK1", code_path(&src)));
    update(&mut include);
    assert_eq!(simple_result(&include).trim_end(), "this part");

    let mut s = host(&format!("LINK word.document.8 \"{}\" \"OLE_LINK1\" \\a \\t", code_path(&src)), false);
    update(&mut s);
    assert_eq!(result(&s), "this part");
}

#[test]
fn a_relative_path_resolves_against_the_documents_folder() {
    let dir = scratch("relative");
    save_source(&dir.join("source.docx"), "this part", false);
    let mut s = host(r#"LINK Word.Document.12 "source.docx" "OLE_LINK1" \a \t"#, false);
    s.path = Some(dir.join("host.docx"));
    update(&mut s);
    assert_eq!(result(&s), "this part");
}

#[test]
fn locked_fields_other_programs_and_formats_not_built_keep_their_result() {
    let dir = scratch("kept");
    let src = dir.join("source.docx");
    save_source(&src, "this part", false);
    let mut locked = host(&link(&src, r"\a \t"), true);
    update(&mut locked);
    assert_eq!(result(&locked), "OLD", "locked");

    let path = code_path(&src);
    let instrs = [
        format!("LINK Excel.Sheet.12 \"{path}\" \"OLE_LINK1\" \\a \\t"),
        link(&src, r"\a \h"),
        link(&src, r"\a \p"),
        link(&src, r"\a \b"),
        link(&src, r"\a"),
    ];
    for instr in instrs {
        let mut s = host(&instr, false);
        update(&mut s);
        assert_eq!(result(&s), "OLD", "{instr}");
        assert_eq!(letters(&s), "Here OLD tail", "{instr}");
    }
}

#[test]
fn a_link_that_cant_be_followed_keeps_its_stored_result() {
    let dir = scratch("failures");
    let src = dir.join("source.docx");
    save_source(&src, "this part", false);
    let instrs = [
        link(&dir.join("absent.docx"), r"\a \t"),
        link(&dir.join("absent.docx"), r"\a \r"),
        r#"LINK Word.Document.12 "\\\\server\\share\\a.docx" "OLE_LINK1" \a \t"#.to_string(),
        r#"LINK Word.Document.12 "//server/share/a.docx" "OLE_LINK1" \a \t"#.to_string(),
        r#"LINK Word.Document.12 "http://example.invalid/a.docx" "OLE_LINK1" \a \t"#.to_string(),
        r#"LINK Word.Document.12 "file://server/share/a.docx" "OLE_LINK1" \a \r"#.to_string(),
        // A relative path with no document folder to resolve it against.
        r#"LINK Word.Document.12 "source.docx" "OLE_LINK1" \a \t"#.to_string(),
        // The file is there; the bookmark is not.
        format!("LINK Word.Document.12 \"{}\" \"NoSuchMark\" \\a \\t", code_path(&src)),
        format!("LINK Word.Document.12 \"{}\" \"NoSuchMark\" \\a \\r", code_path(&src)),
        "LINK Word.Document.12".to_string(),
        "LINK".to_string(),
    ];
    for instr in instrs {
        let mut s = host(&instr, false);
        update(&mut s);
        assert_eq!(result(&s), "OLD", "{instr}");
        assert_eq!(letters(&s), "Here OLD tail", "{instr}");
        let mut simple = simple_host(&instr);
        update(&mut simple);
        assert_eq!(simple_result(&simple), "OLD", "{instr} (simple field)");
    }
}

#[test]
fn automation_reads_a_link_source_only_inside_its_read_root() {
    let dir = scratch("roots");
    let inside = dir.join("in");
    let outside = dir.join("out");
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    save_source(&inside.join("ok.docx"), "allowed words", false);
    save_source(&outside.join("secret.docx"), "secret words", false);
    let roots = crate::file_access::AutomationRoots::new(Some(inside.as_path()), None).unwrap();

    let mut ok = host(&link(&inside.join("ok.docx"), r"\a \t"), false);
    crate::file_access::confine(roots.as_ref(), || update(&mut ok));
    assert_eq!(result(&ok), "allowed words");

    for sw in [r"\a \t", r"\a \r"] {
        let mut refused = host(&link(&outside.join("secret.docx"), sw), false);
        crate::file_access::confine(roots.as_ref(), || update(&mut refused));
        assert_eq!(result(&refused), "OLD", "{sw}");
        assert!(!letters(&refused).contains("secret"), "{sw}");
    }
}
