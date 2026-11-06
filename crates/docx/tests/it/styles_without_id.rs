//! A `w:style` written without a `w:styleId` (SQL Server Reporting Services writes its 1 pt
//! `EmptyCellLayoutStyle` this way, with no `w:type` either) is found by its name, as in Word:
//! paragraphs naming it in `w:pStyle` take its formatting.
use std::io::{Read, Write};
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Block, Document, StyleKind};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn package(styles_body: &str, para_style: &str) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body><w:p><w:pPr><w:pStyle w:val="{para_style}"/></w:pPr></w:p><w:p><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"#
    );
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>{styles_body}</w:styles>"#
    );
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in
        [("_rels/.rels", root), ("word/document.xml", doc.as_str()), ("word/_rels/document.xml.rels", rels), ("word/styles.xml", styles.as_str())]
    {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

const ID_LESS: &str = r#"<w:style><w:name w:val="EmptyCellLayoutStyle" /><w:basedOn w:val="Normal" /><w:rPr><w:sz w:val="2" /></w:rPr></w:style>"#;

fn size(d: &Document, style: &str) -> f32 {
    d.styles.resolve_char(Some(style), &CharProps::default()).size
}

fn first_para_style(d: &Document) -> Option<&str> {
    match d.body.first().map(|b| &**b) {
        Some(Block::Para(p)) => p.props.style.as_deref(),
        _ => panic!("the body starts with a paragraph"),
    }
}

#[test]
fn a_style_without_an_id_is_found_by_its_name() {
    let d = wordcraft_docx::read(&package(ID_LESS, "EmptyCellLayoutStyle")).unwrap();
    let st = d.styles.get("EmptyCellLayoutStyle").expect("the id-less style is read under its name");
    assert_eq!(st.kind, StyleKind::Paragraph, "no w:type means a paragraph style");
    assert_eq!(size(&d, "EmptyCellLayoutStyle"), 1.0);
    assert_eq!(first_para_style(&d), Some("EmptyCellLayoutStyle"));
}

#[test]
fn the_derived_id_is_written_on_save() {
    let d = wordcraft_docx::read(&package(ID_LESS, "EmptyCellLayoutStyle")).unwrap();
    let bytes = wordcraft_docx::write(&d).unwrap();
    let mut xml = String::new();
    zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap().by_name("word/styles.xml").unwrap().read_to_string(&mut xml).unwrap();
    assert!(xml.contains(r#"w:styleId="EmptyCellLayoutStyle""#), "{xml}");
    let again = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(size(&again, "EmptyCellLayoutStyle"), 1.0);
    assert_eq!(first_para_style(&again), Some("EmptyCellLayoutStyle"));
}

#[test]
fn a_name_with_spaces_gives_an_id_without_them() {
    let body = r#"<w:style><w:name w:val="Empty Cell" /><w:rPr><w:sz w:val="2" /></w:rPr></w:style>"#;
    let d = wordcraft_docx::read(&package(body, "EmptyCell")).unwrap();
    assert!(d.styles.get("EmptyCell").is_some());
    assert_eq!(size(&d, "EmptyCell"), 1.0);
}

#[test]
fn an_explicit_id_wins_over_an_id_less_style_of_the_same_name() {
    let explicit = r#"<w:style w:type="paragraph" w:styleId="Gap"><w:name w:val="Gap"/><w:rPr><w:sz w:val="40"/></w:rPr></w:style>"#;
    let id_less = r#"<w:style><w:name w:val="Gap"/><w:rPr><w:sz w:val="2"/></w:rPr></w:style>"#;
    for body in [format!("{explicit}{id_less}"), format!("{id_less}{explicit}")] {
        let d = wordcraft_docx::read(&package(&body, "Gap")).unwrap();
        assert_eq!(size(&d, "Gap"), 20.0, "{body}");
    }
}

#[test]
fn an_id_less_nameless_style_is_ignored() {
    let body = r#"<w:style><w:rPr><w:sz w:val="2"/></w:rPr></w:style><w:style w:styleId=""><w:name w:val="  "/></w:style>"#;
    let d = wordcraft_docx::read(&package(body, "Normal")).unwrap();
    // Only Normal (and whatever the reader always adds) is there; nothing came from the two.
    assert!(d.styles.styles.iter().all(|s| !s.id.is_empty()));
    assert_eq!(size(&d, "Normal"), 12.0);
}

#[test]
fn omitted_defaults_remain_implicit_across_repeated_saves() {
    use wordcraft_doc::props::{LineSpacing, ParaProps};
    let mut d = wordcraft_docx::read(&package("", "Normal")).unwrap();
    for _ in 0..3 {
        assert!(d.styles.builtin_word_defaults);
        assert_eq!(d.styles.default_chr, CharProps::default());
        assert_eq!(d.styles.default_para, ParaProps::default());
        let c = d.styles.effective_default_chr();
        assert_eq!(c.font_ea.as_deref(), Some("Aptos"));
        assert_eq!(c.font_cs.as_deref(), Some("Aptos"));
        assert_eq!(c.size_cs, Some(12.0));
        let p = d.styles.resolve_para(&ParaProps::default());
        assert_eq!(p.space_after, 8.0);
        assert_eq!(p.line_spacing, LineSpacing::Multiple(278.0 / 240.0));
        d = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    }
}

#[test]
fn editing_implicit_defaults_preserves_the_other_effective_values() {
    use wordcraft_doc::props::{LineSpacing, ParaProps};
    let mut d = wordcraft_docx::read(&package("", "Normal")).unwrap();
    std::sync::Arc::make_mut(&mut d.styles).default_para.space_after = Some(4.0);
    let d = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    assert_eq!(size(&d, "Normal"), 12.0);
    assert_eq!(d.styles.resolve_char(None, &CharProps::default()).font, "Aptos");
    let p = d.styles.resolve_para(&ParaProps::default());
    assert_eq!(p.space_after, 4.0);
    assert_eq!(p.line_spacing, LineSpacing::Multiple(278.0 / 240.0));
}

#[test]
fn an_explicitly_empty_defaults_element_keeps_the_established_fallback() {
    let d = wordcraft_docx::read(&package("<w:docDefaults/>", "Normal")).unwrap();
    assert!(!d.styles.builtin_word_defaults);
    assert_eq!(size(&d, "Normal"), 10.0);
}
