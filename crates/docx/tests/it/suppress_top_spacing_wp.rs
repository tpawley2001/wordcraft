//! `w:suppressTopSpacingWP` (ECMA-376 Part 1 §17.15.3, CT_Compat; Word's "Suppress extra line spacing like
//! WordPerfect 5.x", the .doc `Copts80.f2ptExtLeadingOnly`) is read into `Settings::ext_leading_2pt_only` and
//! written back once, in CT_Compat's order: after `w:noLeading`, before `w:wrapTrailSpaces`,
//! `w:footnoteLayoutLikeWW8`, `w:splitPgBreakAndParaMark` and the `w:compatSetting`s. Microsoft 365 Word lays a
//! 12 pt Times New Roman line out 14.00 pt tall with it and 13.80 pt without it.

use wordcraft_doc::Document;

fn settings_xml(d: &Document) -> String {
    let bytes = wordcraft_docx::write(d).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/settings.xml").unwrap(), &mut xml).unwrap();
    xml
}

fn rt(d: &Document) -> Document {
    wordcraft_docx::read(&wordcraft_docx::write(d).unwrap()).unwrap()
}

/// A saved document whose `w:compat` holds `compat` (inserted as its first children).
fn with_compat(compat: &str) -> Document {
    let mut d = Document::new();
    d.settings.compat_mode = 11;
    let bytes = wordcraft_docx::write(&d).unwrap();
    let mut zin = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut data).unwrap();
        if name == "word/settings.xml" {
            let xml = String::from_utf8(data).unwrap();
            let xml = if xml.contains("<w:compat>") {
                xml.replacen("<w:compat>", &format!("<w:compat>{compat}"), 1)
            } else {
                xml.replacen("</w:settings>", &format!("<w:compat>{compat}</w:compat></w:settings>"), 1)
            };
            data = xml.into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut out, &data).unwrap();
    }
    wordcraft_docx::read(&out.finish().unwrap().into_inner()).unwrap()
}

const E: &str = "<w:suppressTopSpacingWP";

#[test]
fn read_on_off_and_absent() {
    assert!(!with_compat("").settings.ext_leading_2pt_only, "absent = off");
    assert!(with_compat("<w:suppressTopSpacingWP/>").settings.ext_leading_2pt_only, "bare = on");
    assert!(with_compat(r#"<w:suppressTopSpacingWP w:val="1"/>"#).settings.ext_leading_2pt_only);
    assert!(!with_compat(r#"<w:suppressTopSpacingWP w:val="0"/>"#).settings.ext_leading_2pt_only);
    assert!(!with_compat(r#"<w:suppressTopSpacingWP w:val="false"/>"#).settings.ext_leading_2pt_only);
}

#[test]
fn model_flag_round_trips_once() {
    let mut d = Document::new();
    assert!(!settings_xml(&d).contains("suppressTopSpacingWP"));
    assert!(!rt(&d).settings.ext_leading_2pt_only);
    d.settings.ext_leading_2pt_only = true;
    d.settings.compat_mode = 11;
    let back = rt(&d);
    assert!(back.settings.ext_leading_2pt_only);
    assert_eq!(settings_xml(&d).matches(E).count(), 1);
    assert_eq!(settings_xml(&back).matches(E).count(), 1, "a reopened file saves it once");
}

#[test]
fn opened_flag_is_saved_once_and_cleared_when_turned_off() {
    let mut d = with_compat("<w:suppressTopSpacingWP/>");
    assert_eq!(settings_xml(&d).matches(E).count(), 1);
    assert!(rt(&d).settings.ext_leading_2pt_only);
    d.settings.ext_leading_2pt_only = false;
    assert!(!settings_xml(&d).contains("suppressTopSpacingWP"));
    assert!(!rt(&d).settings.ext_leading_2pt_only, "turning it off sticks after a save");
}

#[test]
fn written_in_ct_compat_order() {
    let mut d = Document::new();
    d.settings.ext_leading_2pt_only = true;
    d.settings.no_leading = true;
    d.settings.wrap_trail_spaces = true;
    d.settings.footnote_layout_like_ww8 = true;
    d.settings.split_pg_break_and_para_mark = true;
    let xml = settings_xml(&d);
    let at = |e: &str| xml.find(e).unwrap_or_else(|| panic!("{e} missing in {xml}"));
    assert!(at("<w:noLeading") < at(E));
    assert!(at(E) < at("<w:wrapTrailSpaces"));
    assert!(at(E) < at("<w:footnoteLayoutLikeWW8"));
    assert!(at(E) < at("<w:splitPgBreakAndParaMark"));
    assert!(at(E) < at("<w:compatSetting"));
}
