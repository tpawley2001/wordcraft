//! `w:noLeading` (Don't add leading (extra space) between rows of text, a legacy `w:compat`
//! option) is read into `Settings::no_leading` and written back, in CT_Compat's order: before
//! `w:doNotExpandShiftReturn` and the `w:compatSetting`s.

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

#[test]
fn no_leading_round_trips() {
    let mut d = Document::new();
    assert!(!rt(&d).settings.no_leading);
    assert!(!settings_xml(&d).contains("noLeading"));
    d.settings.no_leading = true;
    assert!(rt(&d).settings.no_leading);
    assert_eq!(settings_xml(&d).matches("<w:noLeading").count(), 1);
    // Saved twice, still once.
    assert_eq!(settings_xml(&rt(&d)).matches("<w:noLeading").count(), 1);
}

#[test]
fn no_leading_keeps_the_compatibility_mode() {
    let mut d = Document::new();
    d.settings.no_leading = true;
    d.settings.compat_mode = 11;
    let back = rt(&d);
    assert!(back.settings.no_leading);
    assert_eq!(back.settings.compat_mode, 11);
}

#[test]
fn no_leading_is_written_in_ct_compat_order() {
    let mut d = Document::new();
    d.settings.no_leading = true;
    d.settings.do_not_expand_shift_return = true;
    d.settings.wrap_trail_spaces = true;
    d.settings.sum_paragraph_spacing = true;
    let xml = settings_xml(&d);
    let at = |e: &str| xml.find(e).unwrap_or_else(|| panic!("{e} missing in {xml}"));
    assert!(at("<w:compat>") < at("<w:noLeading"));
    assert!(at("<w:noLeading") < at("<w:doNotExpandShiftReturn"));
    assert!(at("<w:noLeading") < at("<w:wrapTrailSpaces"));
    assert!(at("<w:noLeading") < at("<w:doNotUseHTMLParagraphAutoSpacing"));
    assert!(at("<w:noLeading") < at("<w:compatSetting"));
}
