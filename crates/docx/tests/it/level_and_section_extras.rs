//! What Word writes on list levels and sections is kept on save: `w:lvl/@w:tplc`, `@w:tentative` and
//! `w:legacy` (ECMA-376 Part 1 §17.9.6, §17.9.10), `w:numIdMacAtCleanup` (§17.9.21), `w:rtlGutter`
//! (§17.6.16) and the paper code `w:pgSz/@w:code` (§17.6.13). Fixtures are hand-written from the spec.

use std::io::{Read, Write};

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

fn docx(sect: &str, numbering: &str) -> Vec<u8> {
    let root = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{R}officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let doc_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{R}numbering" Target="numbering.xml"/></Relationships>"#
    );
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W}><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>one</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>two</w:t></w:r></w:p>{sect}</w:body></w:document>"#
    );
    let numbering = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering {W}>{numbering}</w:numbering>"#);
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("_rels/.rels", root.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/numbering.xml", numbering.as_bytes()),
    ] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

fn save(bytes: &[u8]) -> Vec<u8> {
    wordcraft_docx::write(&wordcraft_docx::read(bytes).expect("read")).expect("write")
}

const SECT: &str = r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>"#;

fn numbering(lvl0: &str, lvl0_attrs: &str, tail: &str) -> String {
    format!(
        r#"<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="multilevel"/><w:lvl w:ilvl="0"{lvl0_attrs}><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/>{lvl0}<w:lvlJc w:val="left"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl><w:lvl w:ilvl="1" w:tplc="04090019" w:tentative="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%2)"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="1440" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>{tail}"#
    )
}

/// The saved `w:lvl` element with `w:ilvl="n"` (first abstract list).
fn lvl(xml: &str, n: u8) -> String {
    let key = format!(r#"<w:lvl w:ilvl="{n}""#);
    let start = xml.find(&key).unwrap_or_else(|| panic!("level {n} missing from {xml}"));
    let end = start + xml[start..].find("</w:lvl>").unwrap();
    xml[start..end].to_string()
}

#[test]
fn level_template_code_and_tentative_flag_are_kept() {
    let out = part(&save(&docx(SECT, &numbering("", r#" w:tplc="0409000f""#, ""))), "word/numbering.xml");
    let l0 = lvl(&out, 0);
    assert!(l0.starts_with(r#"<w:lvl w:ilvl="0" w:tplc="0409000F""#), "{l0}");
    assert!(!l0.contains("w:tentative"), "{l0}");
    let l1 = lvl(&out, 1);
    assert!(l1.starts_with(r#"<w:lvl w:ilvl="1" w:tplc="04090019" w:tentative="1""#), "{l1}");
}

#[test]
fn a_bad_template_code_is_dropped() {
    let out = part(&save(&docx(SECT, &numbering("", r#" w:tplc="nothex!""#, ""))), "word/numbering.xml");
    assert!(!lvl(&out, 0).contains("w:tplc"), "{out}");
}

#[test]
fn a_legacy_level_is_kept_before_lvl_jc() {
    let legacy = r#"<w:legacy w:legacy="1" w:legacySpace="120" w:legacyIndent="360"/>"#;
    let out = part(&save(&docx(SECT, &numbering(legacy, "", ""))), "word/numbering.xml");
    let l0 = lvl(&out, 0);
    let at = l0.find(r#"<w:legacy w:legacy="1" w:legacySpace="120" w:legacyIndent="360"/>"#).unwrap_or_else(|| panic!("{l0}"));
    assert!(at > l0.find("<w:lvlText").unwrap() && at < l0.find("<w:lvlJc").unwrap(), "{l0}");
    assert!(!lvl(&out, 1).contains("w:legacy"), "{out}");
}

#[test]
fn a_legacy_level_with_only_the_flag_writes_no_measures() {
    let out = part(&save(&docx(SECT, &numbering(r#"<w:legacy w:legacy="1"/>"#, "", ""))), "word/numbering.xml");
    let l0 = lvl(&out, 0);
    assert!(l0.contains(r#"<w:legacy w:legacy="1"/>"#), "{l0}");
}

#[test]
fn num_id_mac_at_cleanup_is_kept_last() {
    let out = part(&save(&docx(SECT, &numbering("", "", r#"<w:numIdMacAtCleanup w:val="2"/>"#))), "word/numbering.xml");
    let at = out.find(r#"<w:numIdMacAtCleanup w:val="2"/>"#).unwrap_or_else(|| panic!("{out}"));
    assert!(at > out.rfind("</w:num>").unwrap(), "{out}");
}

#[test]
fn rtl_gutter_and_paper_code_are_kept_in_schema_order() {
    let sect = concat!(
        r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840" w:code="1"/>"#,
        r#"<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="360"/>"#,
        r#"<w:bidi/><w:rtlGutter/><w:docGrid w:linePitch="360"/></w:sectPr>"#
    );
    let out = part(&save(&docx(sect, &numbering("", "", ""))), "word/document.xml");
    let s = &out[out.rfind("<w:sectPr").unwrap()..];
    assert!(s.contains(r#"<w:pgSz w:w="12240" w:h="15840" w:code="1"/>"#), "{s}");
    let rg = s.find("<w:rtlGutter/>").unwrap_or_else(|| panic!("{s}"));
    assert!(rg > s.find("<w:bidi/>").unwrap(), "{s}");
    if let Some(g) = s.find("<w:docGrid") {
        assert!(rg < g, "{s}");
    }
}

#[test]
fn landscape_keeps_the_paper_code_after_orient() {
    let sect = r#"<w:sectPr><w:pgSz w:w="15840" w:h="12240" w:orient="landscape" w:code="5"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>"#;
    let out = part(&save(&docx(sect, &numbering("", "", ""))), "word/document.xml");
    assert!(out.contains(r#"<w:pgSz w:w="15840" w:h="12240" w:orient="landscape" w:code="5"/>"#), "{out}");
}

#[test]
fn without_them_nothing_new_is_written() {
    let bytes = save(&docx(SECT, &numbering("", "", "")));
    let doc = part(&bytes, "word/document.xml");
    assert!(!doc.contains("w:rtlGutter") && !doc.contains("w:code="), "{doc}");
    let num = part(&bytes, "word/numbering.xml");
    assert!(!lvl(&num, 0).contains("w:tplc") && !num.contains("w:legacy") && !num.contains("numIdMacAtCleanup"), "{num}");
}

#[test]
fn a_second_save_is_identical() {
    let sect = r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840" w:code="1"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="360"/><w:rtlGutter/></w:sectPr>"#;
    let num = numbering(
        r#"<w:legacy w:legacy="1" w:legacySpace="0" w:legacyIndent="-90"/>"#,
        r#" w:tplc="04090001""#,
        r#"<w:numIdMacAtCleanup w:val="1"/>"#,
    );
    let once = save(&docx(sect, &num));
    let twice = save(&once);
    for p in ["word/document.xml", "word/numbering.xml"] {
        assert_eq!(part(&once, p), part(&twice, p), "{p}");
    }
    assert!(part(&once, "word/numbering.xml").contains(r#"w:legacyIndent="-90""#));
}

/// A level's own number size and underline (`w:lvl/w:rPr`, §17.9.24 with §17.3.2.38 `w:sz` and
/// §17.3.2.40 `w:u`) are saved inside that level's `w:rPr`.
#[test]
fn a_levels_number_size_and_underline_are_kept() {
    let rpr = r#"<w:rPr><w:sz w:val="40"/><w:szCs w:val="40"/><w:u w:val="double"/></w:rPr>"#;
    let nums = numbering("", "", "").replacen("</w:pPr></w:lvl>", &format!("</w:pPr>{rpr}</w:lvl>"), 1);
    let saved = part(&save(&docx(SECT, &nums)), "word/numbering.xml");
    let l0 = lvl(&saved, 0);
    let start = l0.find("<w:rPr>").unwrap_or_else(|| panic!("level 0 has no w:rPr: {l0}"));
    let end = start + l0[start..].find("</w:rPr>").unwrap();
    let rpr = &l0[start..end];
    assert!(rpr.contains(r#"<w:sz w:val="40"/>"#), "{rpr}");
    assert!(rpr.contains(r#"<w:u w:val="double"/>"#), "{rpr}");
    assert!(!lvl(&saved, 1).contains("<w:u "), "level 1 has no underline");
}
