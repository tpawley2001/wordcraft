//! A document opened from RTF (`Settings::rtf_justification`) is saved as DOCX the way Word saves an
//! RTF file: `w:characterSpacingControl` `compressPunctuation` (ECMA-376 Part 1 §17.15.1.18) and
//! compatibility mode 11, so the reopened copy keeps squeezing spaces (`Settings::squeezes_like_rtf`).
//! Microsoft 365 Word 16.0.20527, synthetic RTF files saved as .docx, 2026-11-13.

use wordcraft_doc::{CharSpacing, Document};

fn settings_xml(d: &Document) -> String {
    let bytes = wordcraft_docx::write(d).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/settings.xml").unwrap(), &mut xml).unwrap();
    xml
}

fn doc(rtf: bool, mode: u32, cs: CharSpacing) -> Document {
    let mut d = Document::new();
    d.settings.rtf_justification = rtf;
    d.settings.compat_mode = mode;
    d.settings.char_spacing = cs;
    d
}

/// Saves `d`, checks the saved settings part, reopens it and returns the reopened document.
fn save_check(d: &Document, cs: &str, mode: u32) -> Document {
    let xml = settings_xml(d);
    assert_eq!(xml.matches("<w:characterSpacingControl").count(), 1, "{xml}");
    assert!(xml.contains(&format!(r#"<w:characterSpacingControl w:val="{cs}"/>"#)), "{xml}");
    assert_eq!(xml.matches(r#"w:name="compatibilityMode""#).count(), 1, "{xml}");
    let m = format!(r#"w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="{mode}""#);
    assert!(xml.contains(&m), "{xml}");
    let back = wordcraft_docx::read(&wordcraft_docx::write(d).unwrap()).unwrap();
    assert_eq!(back.settings.compat_mode, mode);
    back
}

#[test]
fn an_rtf_document_saves_compress_punctuation_and_mode_11() {
    let back = save_check(&doc(true, 15, CharSpacing::DoNotCompress), "compressPunctuation", 11);
    assert_eq!(back.settings.char_spacing, CharSpacing::CompressPunctuation);
    assert!(back.settings.squeezes_like_rtf());
}

#[test]
fn an_rtf_document_keeps_a_compressing_value_of_its_own() {
    let back = save_check(&doc(true, 15, CharSpacing::CompressPunctuationAndJapaneseKana), "compressPunctuationAndJapaneseKana", 11);
    assert_eq!(back.settings.char_spacing, CharSpacing::CompressPunctuationAndJapaneseKana);
    assert!(back.settings.squeezes_like_rtf());
}

#[test]
fn an_rtf_document_keeps_a_lower_mode() {
    let back = save_check(&doc(true, 12, CharSpacing::DoNotCompress), "compressPunctuation", 12);
    assert_eq!(back.settings.char_spacing, CharSpacing::CompressPunctuation);
    assert!(back.settings.squeezes_like_rtf());
}

#[test]
fn other_documents_save_as_before() {
    let back = save_check(&doc(false, 15, CharSpacing::DoNotCompress), "doNotCompress", 15);
    assert_eq!(back.settings.char_spacing, CharSpacing::DoNotCompress);
    assert!(!back.settings.squeezes_like_rtf());
    let back = save_check(&doc(false, 11, CharSpacing::DoNotCompress), "doNotCompress", 11);
    assert!(!back.settings.squeezes_like_rtf());
}
