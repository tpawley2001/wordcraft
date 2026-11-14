//! `w:balanceSingleByteDoubleByteWidth` (ECMA-376 Part 1 §17.15.3.1) and `w:useWord97LineBreakRules`
//! (§17.15.3.57) are read into `Settings::balance_sbcs_dbcs_width` and
//! `Settings::use_word97_line_break_rules` and written back once each, in CT_Compat's order
//! (`balanceSingleByteDoubleByteWidth` before `useWord97LineBreakRules`). Together they switch off
//! the proportional-font space squeeze of compatibility mode 11 with Compress punctuation (Microsoft
//! 365 Word 16.0.20527, synthetic .docx files, 2026-11-13).

use wordcraft_doc::{CharSpacing, Document};

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

const BALANCE: &str = "balanceSingleByteDoubleByteWidth";
const WORD97: &str = "useWord97LineBreakRules";

#[test]
fn read_on_off_and_absent() {
    let d = with_compat("");
    assert!(!d.settings.balance_sbcs_dbcs_width && !d.settings.use_word97_line_break_rules, "absent = off");
    let d = with_compat("<w:balanceSingleByteDoubleByteWidth/>");
    assert!(d.settings.balance_sbcs_dbcs_width && !d.settings.use_word97_line_break_rules);
    let d = with_compat("<w:useWord97LineBreakRules/>");
    assert!(!d.settings.balance_sbcs_dbcs_width && d.settings.use_word97_line_break_rules);
    let d = with_compat(r#"<w:balanceSingleByteDoubleByteWidth w:val="1"/><w:useWord97LineBreakRules w:val="true"/>"#);
    assert!(d.settings.balance_sbcs_dbcs_width && d.settings.use_word97_line_break_rules);
    let d = with_compat(r#"<w:balanceSingleByteDoubleByteWidth w:val="0"/><w:useWord97LineBreakRules w:val="false"/>"#);
    assert!(!d.settings.balance_sbcs_dbcs_width && !d.settings.use_word97_line_break_rules);
}

#[test]
fn written_once_in_schema_order() {
    let mut d = Document::new();
    d.settings.compat_mode = 11;
    let xml = settings_xml(&d);
    assert!(!xml.contains(BALANCE) && !xml.contains(WORD97), "{xml}");
    d.settings.balance_sbcs_dbcs_width = true;
    d.settings.use_word97_line_break_rules = true;
    let xml = settings_xml(&d);
    assert_eq!(xml.matches(&format!("<w:{BALANCE}")).count(), 1, "{xml}");
    assert_eq!(xml.matches(&format!("<w:{WORD97}")).count(), 1, "{xml}");
    assert!(xml.find(BALANCE).unwrap() < xml.find(WORD97).unwrap(), "CT_Compat order: {xml}");
    let compat = &xml[xml.find("<w:compat>").unwrap()..xml.find("</w:compat>").unwrap()];
    assert!(compat.contains(BALANCE) && compat.contains(WORD97), "inside w:compat: {xml}");
    assert!(compat.find(WORD97).unwrap() < compat.find("<w:compatSetting").unwrap_or(usize::MAX), "before the compatSettings: {xml}");
}

#[test]
fn each_flag_round_trips_alone() {
    for (balance, word97) in [(true, false), (false, true), (true, true)] {
        let mut d = Document::new();
        d.settings.compat_mode = 11;
        d.settings.char_spacing = CharSpacing::CompressPunctuation;
        d.settings.balance_sbcs_dbcs_width = balance;
        d.settings.use_word97_line_break_rules = word97;
        let back = rt(&d);
        assert_eq!(back.settings.balance_sbcs_dbcs_width, balance);
        assert_eq!(back.settings.use_word97_line_break_rules, word97);
        assert_eq!(back.settings.squeezes_proportional_spaces(), !(balance && word97));
    }
}

#[test]
fn a_document_opened_from_rtf_keeps_the_pair_when_saved() {
    // As the RTF reader leaves it: RTF justification and both flags on.
    let mut d = Document::new();
    d.settings.rtf_justification = true;
    d.settings.balance_sbcs_dbcs_width = true;
    d.settings.use_word97_line_break_rules = true;
    let xml = settings_xml(&d);
    assert_eq!(xml.matches(&format!("<w:{BALANCE}")).count(), 1, "{xml}");
    assert_eq!(xml.matches(&format!("<w:{WORD97}")).count(), 1, "{xml}");
    let back = rt(&d);
    assert!(back.settings.squeezes_like_rtf(), "compressPunctuation + mode 11 (P0455)");
    assert!(!back.settings.squeezes_proportional_spaces(), "proportional fonts still don't squeeze");
}
