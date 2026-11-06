//! A package without `w:docDefaults` (no styles part, or a styles part without the element) opens
//! with Word's current built-in defaults: Aptos 12 pt, line spacing 278/240 (auto), 8 pt after.
//! Measured from Microsoft 365 Word's PDFs of such files: 14.65 pt glyph boxes (12 pt Aptos),
//! 16.97 pt line pitch, 24.99 pt between one-line paragraphs.
use std::io::Write;
use wordcraft_doc::Document;
use wordcraft_doc::props::{CharProps, LineSpacing, ParaProps};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn package(styles: Option<&str>, settings: Option<&str>, theme: bool) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body><w:p><w:r><w:t>one</w:t></w:r></w:p><w:p><w:r><w:t>two</w:t></w:r></w:p></w:body></w:document>"#
    );
    let mut rels = String::new();
    let mut parts: Vec<(String, String)> = vec![("_rels/.rels".into(), root.into()), ("word/document.xml".into(), doc)];
    if let Some(s) = styles {
        rels.push_str(&format!(r#"<Relationship Id="rId1" Type="{REL}/styles" Target="styles.xml"/>"#));
        parts.push((
            "word/styles.xml".into(),
            format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W}">{s}</w:styles>"#),
        ));
    }
    if let Some(s) = settings {
        rels.push_str(&format!(r#"<Relationship Id="rId2" Type="{REL}/settings" Target="settings.xml"/>"#));
        parts.push((
            "word/settings.xml".into(),
            format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="{W}">{s}</w:settings>"#),
        ));
    }
    if theme {
        rels.push_str(&format!(r#"<Relationship Id="rId3" Type="{REL}/theme" Target="theme/theme1.xml"/>"#));
        parts.push((
            "word/theme/theme1.xml".into(),
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="T"><a:themeElements><a:clrScheme name="T"><a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="000000"/></a:dk2><a:lt2><a:srgbClr val="FFFFFF"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="T"><a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Cambria"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="T"><a:fillStyleLst/><a:lnStyleLst/><a:effectStyleLst/><a:bgFillStyleLst/></a:fmtScheme></a:themeElements></a:theme>"#.into(),
        ));
    }
    if !rels.is_empty() {
        parts.push((
            "word/_rels/document.xml.rels".into(),
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#),
        ));
    }
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in parts {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

const NORMAL_ONLY: &str = r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>"#;

fn line_multiple(d: &Document) -> Option<f32> {
    match d.styles.resolve_para(&ParaProps::default()).line_spacing {
        LineSpacing::Multiple(m) => Some(m),
        _ => None,
    }
}

fn assert_word_defaults(d: &Document) {
    let c = d.styles.resolve_char(None, &CharProps::default());
    assert_eq!(c.size, 12.0);
    assert_eq!(c.font, "Aptos");
    let p = d.styles.resolve_para(&ParaProps::default());
    assert_eq!(p.space_before, 0.0);
    assert_eq!(p.space_after, 8.0);
    let m = line_multiple(d).expect("auto line spacing");
    assert!((m - 278.0 / 240.0).abs() < 1e-3, "line multiple {m}");
}

#[test]
fn a_package_without_a_styles_part_gets_words_defaults() {
    assert_word_defaults(&wordcraft_docx::read(&package(None, None, false)).unwrap());
}

#[test]
fn a_styles_part_without_doc_defaults_gets_words_defaults() {
    assert_word_defaults(&wordcraft_docx::read(&package(Some(NORMAL_ONLY), None, false)).unwrap());
}

#[test]
fn compatibility_mode_15_gets_words_defaults_too() {
    let settings =
        r#"<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#;
    assert_word_defaults(&wordcraft_docx::read(&package(Some(NORMAL_ONLY), Some(settings), false)).unwrap());
}

#[test]
fn the_defaults_survive_save_and_reopen() {
    let d = wordcraft_docx::read(&package(None, None, false)).unwrap();
    let again = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    assert_word_defaults(&again);
}

#[test]
fn a_style_still_overrides_the_defaults() {
    let styles = r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/></w:rPr></w:style>"#;
    let d = wordcraft_docx::read(&package(Some(styles), None, false)).unwrap();
    let c = d.styles.resolve_char(Some("Normal"), &CharProps::default());
    assert_eq!(c.size, 10.0);
    assert_eq!(c.font, "Aptos");
    let p = d.styles.resolve_para(&ParaProps::default());
    assert_eq!(p.space_after, 0.0);
}

#[test]
fn present_doc_defaults_are_unchanged() {
    // Guard: a file that has docDefaults, even without pPrDefault, keeps its own reading.
    let styles = format!(
        r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman"/></w:rPr></w:rPrDefault></w:docDefaults>{NORMAL_ONLY}"#
    );
    let d = wordcraft_docx::read(&package(Some(&styles), None, false)).unwrap();
    let c = d.styles.resolve_char(None, &CharProps::default());
    assert_eq!(c.size, 10.0);
    assert_eq!(c.font, "Times New Roman");
    assert_eq!(d.styles.resolve_para(&ParaProps::default()).space_after, 0.0);
}

#[test]
fn a_theme_part_or_an_older_compatibility_mode_is_out_of_scope() {
    // Guard: not measured, so the established reading stays.
    let theme = wordcraft_docx::read(&package(Some(NORMAL_ONLY), None, true)).unwrap();
    assert_eq!(theme.styles.resolve_char(None, &CharProps::default()).size, 10.0);
    let settings =
        r#"<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="14"/></w:compat>"#;
    let older = wordcraft_docx::read(&package(Some(NORMAL_ONLY), Some(settings), false)).unwrap();
    assert_eq!(older.styles.resolve_char(None, &CharProps::default()).size, 10.0);
    assert_eq!(older.styles.resolve_para(&ParaProps::default()).space_after, 0.0);
}
