//! OpenType number forms, number spacing, stylistic sets and contextual alternates
//! (`w14:numForm`, `w14:numSpacing`, `w14:stylisticSets`, `w14:cntxtAlts`, [MS-DOCX]) in document
//! defaults, styles and runs. Synthetic packages, written here; no external documents are needed.

use std::io::Write;
use wordcraft_doc::props::{NumForm, NumSpacing, StyleSet};
use wordcraft_doc::{Document, Paragraph};

fn package(defaults: &str, style: &str, run: &str) -> Vec<u8> {
    let ns = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="w14""#;
    let root = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="s1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let document = format!(
        r#"<w:document {ns}><w:body><w:p><w:r><w:rPr><w:rStyle w:val="Fancy"/>{run}</w:rPr><w:t>office affinity</w:t></w:r></w:p></w:body></w:document>"#
    );
    let styles = format!(
        r#"<w:styles {ns}><w:docDefaults><w:rPrDefault><w:rPr>{defaults}</w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="character" w:styleId="Fancy"><w:name w:val="Fancy"/><w:rPr>{style}</w:rPr></w:style></w:styles>"#
    );
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, data) in [
        ("_rels/.rels", root),
        ("word/_rels/document.xml.rels", rels),
        ("word/document.xml", document.as_str()),
        ("word/styles.xml", styles.as_str()),
    ] {
        z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(data.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn paragraph(d: &Document) -> &Paragraph {
    d.body.iter().find_map(|b| b.as_para()).unwrap()
}

fn saved_xml(d: &Document, name: &str) -> String {
    let out = wordcraft_docx::write(d).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(out)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut z.by_name(name).unwrap(), &mut xml).unwrap();
    xml
}

/// Word stores the Font dialog's Number forms and Number spacing as `w14:numForm` and
/// `w14:numSpacing`; each level keeps its own value through reading and saving.
#[test]
fn number_forms_and_spacing_survive_saving() {
    let bytes = package(
        r#"<w14:numForm w14:val="lining"/><w14:numSpacing w14:val="proportional"/>"#,
        r#"<w14:numForm w14:val="default"/>"#,
        r#"<w14:numForm w14:val="oldStyle"/><w14:numSpacing w14:val="tabular"/>"#,
    );
    let mut d = wordcraft_docx::read(&bytes).unwrap();
    for generation in 0..3 {
        assert_eq!(d.styles.default_chr.num_form, Some(NumForm::Lining), "defaults, generation {generation}");
        assert_eq!(d.styles.default_chr.num_spacing, Some(NumSpacing::Proportional), "defaults, generation {generation}");
        let style = &d.styles.get("Fancy").unwrap().chr;
        assert_eq!(style.num_form, Some(NumForm::Default), "style, generation {generation}");
        assert_eq!(style.num_spacing, None, "style, generation {generation}");
        let run = &paragraph(&d).runs[0].props;
        assert_eq!(run.num_form, Some(NumForm::OldStyle), "run, generation {generation}");
        assert_eq!(run.num_spacing, Some(NumSpacing::Tabular), "run, generation {generation}");
        d = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    }
}

#[test]
fn ooxml_names() {
    for (v, name) in [(NumForm::Default, "default"), (NumForm::Lining, "lining"), (NumForm::OldStyle, "oldStyle")] {
        assert_eq!(v.ooxml(), name);
        assert_eq!(NumForm::from_ooxml(name), Some(v));
    }
    for (v, name) in [(NumSpacing::Default, "default"), (NumSpacing::Proportional, "proportional"), (NumSpacing::Tabular, "tabular")] {
        assert_eq!(v.ooxml(), name);
        assert_eq!(NumSpacing::from_ooxml(name), Some(v));
    }
    assert_eq!(NumForm::from_ooxml("fancy"), None);
    assert_eq!(NumSpacing::from_ooxml("Tabular"), None);
}

/// Stylistic sets are read sorted by id; a set without `w14:val` is on; ids outside 1..=20 are
/// dropped. The saved file lists each set with its id and value.
#[test]
fn stylistic_sets_survive_saving() {
    let run = r#"<w14:stylisticSets><w14:styleSet w14:id="3" w14:val="true"/><w14:styleSet w14:id="1"/><w14:styleSet w14:id="21" w14:val="true"/><w14:styleSet w14:id="20" w14:val="false"/></w14:stylisticSets>"#;
    let mut d = wordcraft_docx::read(&package("", "", run)).unwrap();
    let want = vec![StyleSet { id: 1, on: true }, StyleSet { id: 3, on: true }, StyleSet { id: 20, on: false }];
    for generation in 0..3 {
        assert_eq!(paragraph(&d).runs[0].props.stylistic_sets.as_ref(), Some(&want), "generation {generation}");
        d = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    }
    let xml = saved_xml(&d, "word/document.xml");
    assert!(xml.contains(r#"w14:id="1""#) && xml.contains(r#"w14:id="20""#), "{xml}");
    assert!(!xml.contains(r#"w14:id="21""#), "{xml}");
    let empty = wordcraft_docx::read(&package("", "", "<w14:stylisticSets/>")).unwrap();
    assert_eq!(paragraph(&empty).runs[0].props.stylistic_sets, Some(vec![]));
}

/// `w14:cntxtAlts` is an on/off property: no value is on.
#[test]
fn contextual_alternates_survive_saving() {
    for (el, want) in [
        ("<w14:cntxtAlts/>", Some(true)),
        (r#"<w14:cntxtAlts w14:val="1"/>"#, Some(true)),
        (r#"<w14:cntxtAlts w14:val="true"/>"#, Some(true)),
        (r#"<w14:cntxtAlts w14:val="0"/>"#, Some(false)),
        (r#"<w14:cntxtAlts w14:val="false"/>"#, Some(false)),
    ] {
        let mut d = wordcraft_docx::read(&package("", "", el)).unwrap();
        for generation in 0..3 {
            assert_eq!(paragraph(&d).runs[0].props.contextual_alternates, want, "{el}, generation {generation}");
            d = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
        }
    }
}

/// Values [MS-DOCX] doesn't define are ignored: the run keeps inheriting.
#[test]
fn unknown_values_are_ignored() {
    let d = wordcraft_docx::read(&package("", "", r#"<w14:numForm w14:val="fancy"/><w14:numSpacing/>"#)).unwrap();
    let run = &paragraph(&d).runs[0].props;
    assert_eq!(run.num_form, None);
    assert_eq!(run.num_spacing, None);
}

/// The elements are written in [MS-DOCX]'s order, after `w14:ligatures`.
#[test]
fn written_in_ms_docx_order() {
    let run = r#"<w14:cntxtAlts/><w14:stylisticSets><w14:styleSet w14:id="2"/></w14:stylisticSets><w14:numSpacing w14:val="tabular"/><w14:numForm w14:val="lining"/><w14:ligatures w14:val="all"/>"#;
    let d = wordcraft_docx::read(&package("", "", run)).unwrap();
    let xml = saved_xml(&d, "word/document.xml");
    let at = |tag: &str| xml.find(tag).unwrap_or_else(|| panic!("{tag} missing: {xml}"));
    let order = ["<w14:ligatures", "<w14:numForm", "<w14:numSpacing", "<w14:stylisticSets", "<w14:cntxtAlts"].map(at);
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?} in {xml}");
}

/// Without values nothing is written, so documents that never asked stay as they were.
#[test]
fn nothing_written_without_values() {
    let d = wordcraft_docx::read(&package("", "", "")).unwrap();
    for name in ["word/document.xml", "word/styles.xml"] {
        let xml = saved_xml(&d, name);
        for tag in ["numForm", "numSpacing", "stylisticSets", "cntxtAlts"] {
            assert!(!xml.contains(tag), "{tag} in {name}");
        }
    }
}
