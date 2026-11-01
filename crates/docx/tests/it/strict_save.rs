//! A file opened from Strict Open XML (ISO/IEC 29500 Strict, `http://purl.oclc.org/ooxml/...`) saves as a
//! clean Transitional package: parts carried verbatim and their relationship types are translated, so
//! Word opens the file without repair. Hand-written fixtures, no files produced by Word.

use std::collections::BTreeMap;
use std::io::{Read, Write};

const S_W: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";
const S_R: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";
const S_REL: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";
const T_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const T_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn zip(entries: &[(&str, String)]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, s) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(s.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn unzip(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        assert!(out.insert(f.name().to_string(), b).is_none(), "duplicate part {}", f.name());
    }
    out
}

fn text(files: &BTreeMap<String, Vec<u8>>, name: &str) -> String {
    String::from_utf8_lossy(files.get(name).unwrap_or_else(|| panic!("no {name} in {:?}", files.keys()))).into_owned()
}

fn rels(list: &[(&str, String, &str)]) -> String {
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (id, ty, target) in list {
        s.push_str(&format!(r#"<Relationship Id="{id}" Type="{ty}" Target="{target}"/>"#));
    }
    s.push_str("</Relationships>");
    s
}

const CT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/><Override PartName="/word/webSettings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.webSettings+xml"/><Override PartName="/word/stylesWithEffects.xml" ContentType="application/vnd.ms-word.stylesWithEffects+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#;

/// A Strict package with the parts a Word 2010 "Strict Open XML" save has.
fn strict_package() -> Vec<u8> {
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{S_W}" xmlns:r="{S_R}" conformance="strict"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{S_W}"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style></w:styles>"#
    );
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="{S_W}" xmlns:r="{S_R}" xmlns:m="http://purl.oclc.org/ooxml/officeDocument/math"><w:zoom w:percent="100%"/><w:defaultTabStop w:val="720"/><m:mathPr><m:mathFont m:val="Cambria Math"/></m:mathPr></w:settings>"#
    );
    let fonts = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:fonts xmlns:w="{S_W}" xmlns:r="{S_R}"><w:font w:name="Times New Roman"><w:family w:val="roman"/></w:font></w:fonts>"#
    );
    let web = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:webSettings xmlns:w="{S_W}" xmlns:r="{S_R}"><w:optimizeForBrowser/></w:webSettings>"#
    );
    let swe = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{S_W}" xmlns:r="{S_R}"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style></w:styles>"#
    );
    let app = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://purl.oclc.org/ooxml/officeDocument/extendedProperties" xmlns:vt="http://purl.oclc.org/ooxml/officeDocument/docPropsVTypes"><Template>Normal.dotm</Template><Pages>1</Pages><Application>Microsoft Office Word</Application></Properties>"#.to_string();
    let root = rels(&[
        ("rId1", format!("{S_REL}/extendedProperties"), "docProps/app.xml"),
        ("rId2", format!("{S_REL}/officeDocument"), "word/document.xml"),
    ]);
    let doc_rels = rels(&[
        ("rId1", format!("{S_REL}/styles"), "styles.xml"),
        ("rId2", format!("{S_REL}/settings"), "settings.xml"),
        ("rId3", format!("{S_REL}/fontTable"), "fontTable.xml"),
        ("rId4", format!("{S_REL}/webSettings"), "webSettings.xml"),
        ("rId5", "http://schemas.microsoft.com/office/2007/relationships/stylesWithEffects".to_string(), "stylesWithEffects.xml"),
    ]);
    zip(&[
        ("[Content_Types].xml", CT.to_string()),
        ("_rels/.rels", root),
        ("word/document.xml", doc),
        ("word/_rels/document.xml.rels", doc_rels),
        ("word/styles.xml", styles),
        ("word/settings.xml", settings),
        ("word/fontTable.xml", fonts),
        ("word/webSettings.xml", web),
        ("word/stylesWithEffects.xml", swe),
        ("docProps/app.xml", app),
    ])
}

fn save_strict() -> (Vec<u8>, BTreeMap<String, Vec<u8>>) {
    let doc = wordcraft_docx::read(&strict_package()).expect("read strict");
    let out = wordcraft_docx::write(&doc).expect("write");
    let files = unzip(&out);
    (out, files)
}

#[test]
fn no_part_keeps_a_strict_namespace_or_relationship_type() {
    let (_, files) = save_strict();
    for (name, bytes) in &files {
        let s = String::from_utf8_lossy(bytes);
        assert!(!s.contains("purl.oclc.org/ooxml"), "{name} still has a Strict URI:\n{s}");
        assert!(!s.contains(r#"conformance="strict""#), "{name} still says conformance=strict");
    }
}

#[test]
fn the_strict_extended_properties_part_is_not_kept_twice() {
    let (_, files) = save_strict();
    let root = text(&files, "_rels/.rels");
    assert_eq!(root.matches("/extended-properties\"").count(), 1, "{root}");
    assert!(!root.contains("extendedProperties"), "{root}");
    assert!(!files.contains_key("word/embeddings/part.xml"), "parts: {:?}", files.keys());
}

#[test]
fn web_settings_stay_related_with_the_transitional_type_and_namespace() {
    let (_, files) = save_strict();
    let rels = text(&files, "word/_rels/document.xml.rels");
    let ty = format!(r#"Type="{T_REL}/webSettings""#);
    assert_eq!(rels.matches(&ty).count(), 1, "{rels}");
    let target = rels.split("<Relationship ").find(|r| r.contains(&ty)).unwrap();
    let target = target.split("Target=\"").nth(1).unwrap().split('"').next().unwrap();
    let web = text(&files, &format!("word/{target}"));
    assert!(web.contains(&format!(r#"xmlns:w="{T_W}""#)), "{web}");
    assert!(web.contains("optimizeForBrowser"), "{web}");
    for name in ["word/settings.xml", "word/fontTable.xml"] {
        let s = text(&files, name);
        assert!(s.contains(&format!(r#"xmlns:w="{T_W}""#)), "{name}: {s}");
    }
}

#[test]
fn the_saved_file_reads_back_with_its_text() {
    let (out, files) = save_strict();
    assert!(text(&files, "word/document.xml").contains(">Hello<"));
    let again = wordcraft_docx::read(&out).expect("re-read");
    let files2 = unzip(&wordcraft_docx::write(&again).expect("write again"));
    assert!(text(&files2, "word/document.xml").contains(">Hello<"));
    assert_eq!(text(&files2, "_rels/.rels").matches("/extended-properties\"").count(), 1);
}

#[test]
fn transitional_web_settings_are_still_copied_byte_for_byte() {
    let web = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:webSettings xmlns:w="{T_W}"><w:optimizeForBrowser/><w:allowPNG/></w:webSettings>"#
    );
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{T_W}" xmlns:r="{T_REL}"><w:body><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:body></w:document>"#
    );
    let bytes = zip(&[
        ("[Content_Types].xml", CT.to_string()),
        ("_rels/.rels", rels(&[("rId1", format!("{T_REL}/officeDocument"), "word/document.xml")])),
        ("word/document.xml", doc),
        ("word/_rels/document.xml.rels", rels(&[("rId1", format!("{T_REL}/webSettings"), "webSettings.xml")])),
        ("word/webSettings.xml", web.clone()),
    ]);
    let files = unzip(&wordcraft_docx::write(&wordcraft_docx::read(&bytes).unwrap()).unwrap());
    assert_eq!(text(&files, "word/webSettings.xml"), web);
}
