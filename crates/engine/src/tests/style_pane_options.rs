//! Styles pane › Options… (Style Pane Options): which styles the pane shows (In use, All styles)
//! and how it sorts them, kept in the file as `w:stylePaneFormatFilter` / `w:stylePaneSortMethod`
//! (ECMA-376 Part 1 §17.15.1.85, §17.15.1.86).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wordcraft-engine-stylepane-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn p(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Styles: Normal (paragraph, priority 0), Heading 1 (paragraph, priority 9, based on Normal,
/// Arial), Quote (linked paragraph + QuoteChar, priority 29), Strong (character, priority 22,
/// Courier New), Secret (hidden paragraph), Table Grid (table). The body uses Normal and Heading 1
/// only; a run in the header uses Strong.
const STYLES: &str = r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:uiPriority w:val="0"/><w:qFormat/></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="9"/><w:qFormat/><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/></w:rPr></w:style>
<w:style w:type="paragraph" w:customStyle="1" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:link w:val="QuoteChar"/><w:uiPriority w:val="29"/></w:style>
<w:style w:type="character" w:customStyle="1" w:styleId="QuoteChar"><w:name w:val="Quote Char"/><w:link w:val="Quote"/><w:uiPriority w:val="29"/></w:style>
<w:style w:type="character" w:styleId="Strong"><w:name w:val="Strong"/><w:uiPriority w:val="22"/><w:rPr><w:rFonts w:ascii="Courier New" w:hAnsi="Courier New"/><w:b/></w:rPr></w:style>
<w:style w:type="paragraph" w:customStyle="1" w:styleId="Secret"><w:name w:val="Secret"/><w:semiHidden/></w:style>
<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:uiPriority w:val="39"/></w:style>
</w:styles>"#;

/// A Word XML (Flat OPC) document with [`STYLES`] whose settings hold `settings` (raw children).
fn flat(settings: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/_rels/document.xml.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p><w:p><w:r><w:t>Body</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rId3"/></w:sectPr></w:body></w:document></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/header1.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml">
    <pkg:xmlData><w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:rPr><w:rStyle w:val="Strong"/></w:rPr><w:t>Head</w:t></w:r></w:p></w:hdr></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/styles.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml">
    <pkg:xmlData>{STYLES}</pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/settings.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml">
    <pkg:xmlData><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{settings}<w:defaultTabStop w:val="720"/></w:settings></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    )
}

/// A session with `settings` opened from a flat file.
fn open(name: &str, settings: &str) -> Session {
    let dir = temp_dir(name);
    let file = dir.join("in.xml");
    std::fs::write(&file, flat(settings)).unwrap();
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "file.open", json!({"path": p(&file)}));
    s
}

/// The listed names, lower-cased (built-in names may be shown capitalised).
fn names(s: &mut Session) -> Vec<String> {
    let list = run(s, "styles.paneList", json!({}));
    list.as_array().expect("an array").iter().map(|x| x["name"].as_str().unwrap_or_default().to_lowercase()).collect()
}

/// The written settings part's `<w:stylePane…/>` elements, in order.
fn written(s: &Session) -> Vec<String> {
    let flat = String::from_utf8(wordcraft_docx::write_flat(&s.doc).unwrap()).unwrap();
    let mut out = Vec::new();
    let mut rest = flat.as_str();
    while let Some(i) = rest.find("<w:stylePane") {
        let end = rest[i..].find("/>").map(|e| i + e + 2).expect("an empty element");
        out.push(rest[i..end].to_string());
        rest = &rest[end..];
    }
    out
}

#[test]
fn defaults_are_all_styles_alphabetical_and_nothing_is_written() {
    let mut s = open("defaults", "");
    assert_eq!(run(&mut s, "styles.paneOptions", json!({})), json!({"show": "all", "sort": "alphabetical"}));
    // Hidden and table styles never show; the linked pair shows once, as its paragraph half.
    assert_eq!(names(&mut s), ["heading 1", "normal", "quote", "strong"]);
    let types: Vec<Value> = run(&mut s, "styles.paneList", json!({})).as_array().unwrap().iter().map(|x| x["type"].clone()).collect();
    assert_eq!(types, [json!("paragraph"), json!("paragraph"), json!("linked"), json!("character")]);
    assert!(written(&s).is_empty(), "no options, no elements: {:?}", written(&s));
}

#[test]
fn sort_orders() {
    let mut s = open("sorts", "");
    let cases: [(&str, [&str; 4]); 5] = [
        ("alphabetical", ["heading 1", "normal", "quote", "strong"]),
        // uiPriority: Normal 0, heading 1 9, Strong 22, Quote 29.
        ("recommended", ["normal", "heading 1", "strong", "quote"]),
        // Own or inherited font: heading 1 Arial, Strong Courier New, Normal and Quote none ("").
        ("font", ["normal", "quote", "heading 1", "strong"]),
        // Based on: none (Normal, Strong) first, then Normal (heading 1, Quote).
        ("basedOn", ["normal", "strong", "heading 1", "quote"]),
        // By type: paragraph, linked, character.
        ("type", ["heading 1", "normal", "quote", "strong"]),
    ];
    for (sort, want) in cases {
        let r = run(&mut s, "styles.paneOptions", json!({"sort": sort}));
        assert_eq!(r["sort"], json!(sort));
        assert_eq!(names(&mut s), want, "sort {sort}");
    }
}

#[test]
fn in_use_lists_styles_used_in_any_story() {
    let mut s = open("inuse", "");
    assert_eq!(run(&mut s, "styles.paneOptions", json!({"show": "inUse"})), json!({"show": "inUse", "sort": "alphabetical"}));
    // Body: heading 1 and Normal; header: Strong (a run). Quote is defined but unused.
    assert_eq!(names(&mut s), ["heading 1", "normal", "strong"]);
    run(&mut s, "styles.paneOptions", json!({"show": "all"}));
    assert_eq!(names(&mut s), ["heading 1", "normal", "quote", "strong"]);
}

#[test]
fn bad_values_are_refused() {
    let mut s = open("bad", "");
    assert!(s.run("styles.paneOptions", &json!({"show": "recent"})).is_err());
    assert!(s.run("styles.paneOptions", &json!({"sort": "size"})).is_err());
    assert_eq!(run(&mut s, "styles.paneOptions", json!({})), json!({"show": "all", "sort": "alphabetical"}));
}

#[test]
fn a_files_options_are_read() {
    let cases = [
        // (settings children, show, sort)
        (r#"<w:stylePaneFormatFilter w:val="3F01"/><w:stylePaneSortMethod w:val="0000"/>"#, "all", "alphabetical"),
        (r#"<w:stylePaneFormatFilter w:val="1028"/><w:stylePaneSortMethod w:val="0001"/>"#, "inUse", "recommended"),
        (r#"<w:stylePaneFormatFilter w:val="1f08"/><w:stylePaneSortMethod w:val="0003"/>"#, "inUse", "font"),
        // Strict names.
        (r#"<w:stylePaneSortMethod w:val="basedOn"/>"#, "all", "basedOn"),
        (r#"<w:stylePaneSortMethod w:val="type"/>"#, "all", "type"),
        // The attribute form wins over w:val for the bits it names.
        (
            r#"<w:stylePaneFormatFilter w:val="0001" w:allStyles="0" w:customStyles="0" w:latentStyles="0" w:stylesInUse="1" w:headingStyles="0" w:numberingStyles="0" w:tableStyles="0" w:directFormattingOnRuns="0" w:directFormattingOnParagraphs="0" w:directFormattingOnNumbering="0" w:directFormattingOnTables="0" w:clearFormatting="1" w:top3HeadingStyles="0" w:visibleStyles="0" w:alternateStyleNames="0"/>"#,
            "inUse",
            "alphabetical",
        ),
    ];
    for (i, (xml, show, sort)) in cases.into_iter().enumerate() {
        let mut s = open(&format!("read{i}"), xml);
        assert_eq!(run(&mut s, "styles.paneOptions", json!({})), json!({"show": show, "sort": sort}), "{xml}");
    }
}

#[test]
fn a_files_options_are_written_back_with_the_bits_kept() {
    // Word 2010's attribute form.
    let word = r#"<w:stylePaneFormatFilter w:val="3F01" w:allStyles="1" w:customStyles="0" w:latentStyles="0" w:stylesInUse="0" w:headingStyles="0" w:numberingStyles="0" w:tableStyles="0" w:directFormattingOnRuns="1" w:directFormattingOnParagraphs="1" w:directFormattingOnNumbering="1" w:directFormattingOnTables="1" w:clearFormatting="1" w:top3HeadingStyles="1" w:visibleStyles="0" w:alternateStyleNames="0"/><w:stylePaneSortMethod w:val="0004"/>"#;
    let s = open("keep", word);
    let w = written(&s);
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(w[0].contains(r#"w:val="3F01""#) && w[0].contains(r#"w:allStyles="1""#) && w[0].contains(r#"w:stylesInUse="0""#), "{w:?}");
    assert!(w[0].contains(r#"w:directFormattingOnTables="1""#) && w[0].contains(r#"w:top3HeadingStyles="1""#), "{w:?}");
    assert_eq!(w[1], r#"<w:stylePaneSortMethod w:val="0004"/>"#);

    // Choosing In use flips bits 0x0001 and 0x0008 only.
    let mut s = open("flip", word);
    run(&mut s, "styles.paneOptions", json!({"show": "inUse", "sort": "recommended"}));
    let w = written(&s);
    assert!(w[0].contains(r#"w:val="3F08""#) && w[0].contains(r#"w:allStyles="0""#) && w[0].contains(r#"w:stylesInUse="1""#), "{w:?}");
    assert_eq!(w[1], r#"<w:stylePaneSortMethod w:val="0001"/>"#);

    // The plain hex form stays plain.
    let mut s = open("plain", r#"<w:stylePaneFormatFilter w:val="1028"/>"#);
    run(&mut s, "styles.paneOptions", json!({"show": "all"}));
    assert_eq!(written(&s), [r#"<w:stylePaneFormatFilter w:val="1021"/>"#]);
}

#[test]
fn options_set_on_a_new_document_are_written() {
    let mut s = open("new", "");
    run(&mut s, "styles.paneOptions", json!({"show": "inUse", "sort": "type"}));
    let w = written(&s);
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(w[0].starts_with(r#"<w:stylePaneFormatFilter w:val="0008""#) && w[0].contains(r#"w:stylesInUse="1""#), "{w:?}");
    assert_eq!(w[1], r#"<w:stylePaneSortMethod w:val="0005"/>"#);
}
