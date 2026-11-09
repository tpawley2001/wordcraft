//! Section paper source ([MS-DOC] §2.6.4 sprmSDmBinFirst 0x5007, sprmSDmBinOther 0x5008): the printer
//! tray for the section's first page and its other pages, the same codes as DOCX `w:paperSrc`.

use super::{StructSpec, grpprl, one_section, read, struct_doc};

const S_DM_BIN_FIRST: u16 = 0x5007;
const S_DM_BIN_OTHER: u16 = 0x5008;

/// (first, other) trays of a one-section document whose Sepx holds `prls`.
fn trays(prls: &[(u16, &[u8])]) -> (Option<u16>, Option<u16>) {
    let spec = one_section("Text\r", grpprl(prls), vec![(0, 0, Vec::new())]);
    let doc = read(&struct_doc(&spec)).expect("opens");
    (doc.last_section.paper_src_first, doc.last_section.paper_src_other)
}

#[test]
fn no_tray_sprms_leave_the_paper_source_unset() {
    assert_eq!(trays(&[]), (None, None));
}

#[test]
fn the_first_page_tray_is_read() {
    assert_eq!(trays(&[(S_DM_BIN_FIRST, &4u16.to_le_bytes())]), (Some(4), None));
}

#[test]
fn the_other_pages_tray_is_read() {
    assert_eq!(trays(&[(S_DM_BIN_OTHER, &15u16.to_le_bytes())]), (None, Some(15)));
}

#[test]
fn both_trays_are_read() {
    let prls: [(u16, &[u8]); 2] = [(S_DM_BIN_FIRST, &1u16.to_le_bytes()), (S_DM_BIN_OTHER, &7u16.to_le_bytes())];
    assert_eq!(trays(&prls), (Some(1), Some(7)));
}

#[test]
fn a_driver_specific_tray_code_is_kept() {
    assert_eq!(trays(&[(S_DM_BIN_FIRST, &257u16.to_le_bytes())]), (Some(257), None));
}

#[test]
fn tray_zero_is_the_default_and_stays_unset() {
    let prls: [(u16, &[u8]); 2] = [(S_DM_BIN_FIRST, &0u16.to_le_bytes()), (S_DM_BIN_OTHER, &0u16.to_le_bytes())];
    assert_eq!(trays(&prls), (None, None));
}

#[test]
fn each_section_keeps_its_own_trays() {
    let spec = StructSpec {
        sections: vec![(6, grpprl(&[(S_DM_BIN_FIRST, &4u16.to_le_bytes())])), (12, grpprl(&[(S_DM_BIN_OTHER, &2u16.to_le_bytes())]))],
        papx_entries: vec![(0, 0, Vec::new()), (6, 0, Vec::new())],
        ..one_section("first\rsecond\r", Vec::new(), Vec::new())
    };
    let doc = read(&struct_doc(&spec)).expect("opens");
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let sec = p0.section.as_ref().expect("section on the first paragraph");
    assert_eq!((sec.paper_src_first, sec.paper_src_other), (Some(4), None));
    assert_eq!((doc.last_section.paper_src_first, doc.last_section.paper_src_other), (None, Some(2)));
}
