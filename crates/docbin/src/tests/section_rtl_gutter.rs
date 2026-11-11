//! sprmSFRTLGutter (0x322A, Bool8, [MS-DOC] §2.6.4): the gutter is on the right side of the page,
//! read into the same model field as DOCX `w:rtlGutter`.

use super::{grpprl, one_section, read, struct_doc};

const S_DXA_GUTTER: u16 = 0xB025;
const S_F_RTL_GUTTER: u16 = 0x322A;

/// (rtl_gutter, gutter) of a one-section document whose Sepx holds `prls`.
fn gutter(prls: &[(u16, Vec<u8>)]) -> (bool, f32) {
    let prls: Vec<(u16, &[u8])> = prls.iter().map(|(op, v)| (*op, v.as_slice())).collect();
    let spec = one_section("Text\r", grpprl(&prls), vec![(0, 0, Vec::new())]);
    let doc = read(&struct_doc(&spec)).expect("opens");
    (doc.last_section.rtl_gutter, doc.last_section.gutter)
}

#[test]
fn a_gutter_alone_stays_on_the_left() {
    assert_eq!(gutter(&[(S_DXA_GUTTER, 720u16.to_le_bytes().to_vec())]), (false, 36.0));
}

#[test]
fn sprm_s_f_rtl_gutter_puts_the_gutter_on_the_right() {
    assert_eq!(gutter(&[(S_DXA_GUTTER, 720u16.to_le_bytes().to_vec()), (S_F_RTL_GUTTER, vec![1])]), (true, 36.0));
}

#[test]
fn sprm_s_f_rtl_gutter_off_keeps_it_on_the_left() {
    assert_eq!(gutter(&[(S_DXA_GUTTER, 720u16.to_le_bytes().to_vec()), (S_F_RTL_GUTTER, vec![0])]), (false, 36.0));
}
