//! `w:splitPgBreakAndParaMark` (a legacy `w:compat` option Word writes for converted Word 97-2003
//! documents) is read into `Settings::split_pg_break_and_para_mark` and written back, in
//! CT_Compat's order: after `w:doNotUseHTMLParagraphAutoSpacing` and
//! `w:allowSpaceOfSameStyleInTable`, before the `w:compatSetting`s.

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
fn split_break_mark_round_trips() {
    let mut d = Document::new();
    assert!(!rt(&d).settings.split_pg_break_and_para_mark);
    assert!(!settings_xml(&d).contains("splitPgBreakAndParaMark"));
    d.settings.split_pg_break_and_para_mark = true;
    d.settings.compat_mode = 11;
    let back = rt(&d);
    assert!(back.settings.split_pg_break_and_para_mark);
    assert_eq!(back.settings.compat_mode, 11);
    assert_eq!(settings_xml(&d).matches("<w:splitPgBreakAndParaMark").count(), 1);
    assert_eq!(settings_xml(&back).matches("<w:splitPgBreakAndParaMark").count(), 1);
}

#[test]
fn split_break_mark_is_written_in_ct_compat_order() {
    let mut d = Document::new();
    d.settings.split_pg_break_and_para_mark = true;
    d.settings.do_not_expand_shift_return = true;
    d.settings.sum_paragraph_spacing = true;
    d.settings.allow_space_of_same_style_in_table = true;
    let xml = settings_xml(&d);
    let at = |e: &str| xml.find(e).unwrap_or_else(|| panic!("{e} missing in {xml}"));
    assert!(at("<w:doNotExpandShiftReturn") < at("<w:splitPgBreakAndParaMark"));
    assert!(at("<w:doNotUseHTMLParagraphAutoSpacing") < at("<w:splitPgBreakAndParaMark"));
    assert!(at("<w:allowSpaceOfSameStyleInTable") < at("<w:splitPgBreakAndParaMark"));
    assert!(at("<w:splitPgBreakAndParaMark") < at("<w:compatSetting"));
}
