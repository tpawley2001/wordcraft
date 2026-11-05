//! Section page numbering in Word 97-2003 files ([MS-DOC] §2.6.4): sprmSNfcPgn (0x300E, the
//! number format), sprmSFPgnRestart (0x3011) and sprmSPgnStart97 (0x501C, the start value).
//! Word restarts numbering only in a section whose restart flag is set; otherwise numbering
//! continues from the previous section.

use wordcraft_doc::section::{NumFormat, SectionProps};

use super::{StructSpec, grpprl, one_section, read, struct_doc};

/// The sections of a body "s0\rs1\rs2\r" (one paragraph each) with these SEPX grpprls.
fn sections(sepx: [Vec<u8>; 3]) -> Vec<SectionProps> {
    let [a, b, c] = sepx;
    let spec = StructSpec { sections: vec![(3, a), (6, b), (9, c)], ..one_section("s0\rs1\rs2\r", Vec::new(), vec![(0, 0, Vec::new())]) };
    let doc = read(&struct_doc(&spec)).expect("opens");
    let mut out: Vec<SectionProps> = doc.body.iter().filter_map(|b| b.as_para()).filter_map(|p| p.section.as_deref().cloned()).collect();
    out.push(doc.last_section.clone());
    assert_eq!(out.len(), 3, "three sections");
    out
}

fn numbering(s: &SectionProps) -> (NumFormat, Option<u32>) {
    (s.page_num_format, s.page_num_start)
}

#[test]
fn format_restart_and_start_are_read_per_section() {
    // A real government report: front matter i, ii, iii...; the body restarts at 1; an annex at 30.
    let s = sections([
        grpprl(&[(0x300E, &[2]), (0x3011, &[1]), (0x501C, &1u16.to_le_bytes())]),
        grpprl(&[(0x3011, &[1]), (0x501C, &1u16.to_le_bytes())]),
        grpprl(&[(0x3011, &[1]), (0x501C, &30u16.to_le_bytes())]),
    ]);
    assert_eq!(numbering(&s[0]), (NumFormat::LowerRoman, Some(1)));
    assert_eq!(numbering(&s[1]), (NumFormat::Decimal, Some(1)));
    assert_eq!(numbering(&s[2]), (NumFormat::Decimal, Some(30)));
}

#[test]
fn page_number_formats_map_as_in_word() {
    // nfc 0 Arabic, 1 upper Roman, 2 lower Roman, 3 upper letter, 4 lower letter.
    let want = [NumFormat::Decimal, NumFormat::UpperRoman, NumFormat::LowerRoman, NumFormat::UpperLetter, NumFormat::LowerLetter];
    for (nfc, want) in want.into_iter().enumerate() {
        let spec = one_section("Text\r", grpprl(&[(0x300E, &[nfc as u8])]), vec![(0, 0, Vec::new())]);
        let doc = read(&struct_doc(&spec)).expect("opens");
        assert_eq!(doc.last_section.page_num_format, want, "sprmSNfcPgn {nfc}");
        assert_eq!(doc.last_section.page_num_start, None, "a format alone does not restart numbering");
    }
}

#[test]
fn a_start_value_without_the_restart_flag_continues_numbering() {
    // Word ignores sprmSPgnStart97 in a later section whose fPgnRestart is 0.
    let s = sections([Vec::new(), grpprl(&[(0x3011, &[0]), (0x501C, &5u16.to_le_bytes())]), grpprl(&[(0x501C, &7u16.to_le_bytes())])]);
    assert_eq!(numbering(&s[1]), (NumFormat::Decimal, None));
    assert_eq!(numbering(&s[2]), (NumFormat::Decimal, None));
}

#[test]
fn sections_without_page_number_sprms_are_unchanged() {
    let s = sections([Vec::new(), Vec::new(), Vec::new()]);
    assert!(s.iter().all(|s| numbering(s) == (NumFormat::Decimal, None)));
}
