//! Section footnote and endnote options, chapter page numbering, form protection, text flow and the
//! 32-bit page-number start in Word 97-2003 files ([MS-DOC] §2.6.4): sprmSFpc (0x303B), sprmSRncFtn
//! (0x303C), sprmSRncEdn (0x303E), sprmSNFtn (0x503F), sprmSNfcFtnRef (0x5040), sprmSNEdn (0x5041),
//! sprmSNfcEdnRef (0x5042), sprmScnsPgn (0x3000), sprmSiHeadingPgn (0x3001), sprmSFProtected (0x3006),
//! sprmSTextFlow (0x5033) and sprmSPgnStart (0x7044).

use wordcraft_doc::props::TextDirection;
use wordcraft_doc::section::{ChapterSep, NotePos, NoteProps, NoteRestart, NumFormat, SectionProps};

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

#[test]
fn footnote_options_are_read_per_section() {
    let s = sections([
        // Beneath text, restart each page, symbols (MSONFC 9 = chicago).
        grpprl(&[(0x303B, &[2]), (0x303C, &[2]), (0x5040, &9u16.to_le_bytes())]),
        // Bottom of page, restart each section, lower Roman, start at 5.
        grpprl(&[(0x303B, &[1]), (0x303C, &[1]), (0x5040, &2u16.to_le_bytes()), (0x503F, &5u16.to_le_bytes())]),
        Vec::new(),
    ]);
    assert_eq!(
        s[0].footnote_pr,
        NoteProps { pos: Some(NotePos::BeneathText), num_fmt: Some(NumFormat::Chicago), num_start: None, num_restart: Some(NoteRestart::EachPage) }
    );
    assert_eq!(
        s[1].footnote_pr,
        NoteProps { pos: Some(NotePos::PageBottom), num_fmt: Some(NumFormat::LowerRoman), num_start: Some(5), num_restart: Some(NoteRestart::EachSect) }
    );
    assert_eq!(s[2].footnote_pr, NoteProps::default(), "a section without the sprms keeps the defaults");
    assert_eq!(s[2].endnote_pr, NoteProps::default());
}

#[test]
fn endnote_options_are_read() {
    let s = sections([
        grpprl(&[(0x303E, &[1]), (0x5041, &3u16.to_le_bytes()), (0x5042, &1u16.to_le_bytes())]),
        grpprl(&[(0x303E, &[0])]),
        Vec::new(),
    ]);
    assert_eq!(s[0].endnote_pr.num_restart, Some(NoteRestart::EachSect));
    assert_eq!(s[0].endnote_pr.num_start, Some(3));
    assert_eq!(s[0].endnote_pr.num_fmt, Some(NumFormat::UpperRoman));
    assert_eq!(s[0].footnote_pr, NoteProps::default(), "endnote sprms leave the footnote options alone");
    assert_eq!(s[1].endnote_pr.num_restart, Some(NoteRestart::Continuous));
}

#[test]
fn out_of_range_note_operands_are_ignored() {
    let s = sections([
        grpprl(&[(0x303B, &[7]), (0x303C, &[9]), (0x503F, &0u16.to_le_bytes())]),
        // Truncated operands: one byte where two are due.
        grpprl(&[(0x503F, &[4]), (0x5040, &[2])]),
        Vec::new(),
    ]);
    assert_eq!(s[0].footnote_pr, NoteProps::default());
    assert_eq!(s[1].footnote_pr.num_start, None);
}

#[test]
fn chapter_numbering_is_read() {
    // Heading 1 with an em dash ("2—1"); Heading 3 with a period; a separator alone.
    let s = sections([grpprl(&[(0x3001, &[1]), (0x3000, &[3])]), grpprl(&[(0x3001, &[3]), (0x3000, &[1])]), grpprl(&[(0x3000, &[4])])]);
    assert_eq!((s[0].chapter_style, s[0].chapter_sep), (Some(1), Some(ChapterSep::EmDash)));
    assert_eq!((s[1].chapter_style, s[1].chapter_sep), (Some(3), Some(ChapterSep::Period)));
    assert_eq!((s[2].chapter_style, s[2].chapter_sep), (None, Some(ChapterSep::EnDash)));
}

#[test]
fn chapter_separators_map_in_order() {
    let want = [ChapterSep::Hyphen, ChapterSep::Period, ChapterSep::Colon, ChapterSep::EmDash, ChapterSep::EnDash];
    for (v, w) in want.into_iter().enumerate() {
        let s = sections([grpprl(&[(0x3001, &[2]), (0x3000, &[v as u8])]), Vec::new(), Vec::new()]);
        assert_eq!(s[0].chapter_sep, Some(w), "cnsPgn {v}");
    }
    let s = sections([grpprl(&[(0x3001, &[0]), (0x3000, &[5])]), Vec::new(), Vec::new()]);
    assert_eq!((s[0].chapter_style, s[0].chapter_sep), (None, None), "level 0 and separator 5 set nothing");
}

#[test]
fn form_protection_is_read() {
    let s = sections([grpprl(&[(0x3006, &[1])]), grpprl(&[(0x3006, &[0])]), Vec::new()]);
    assert_eq!(s[0].form_prot, Some(true));
    assert_eq!(s[1].form_prot, Some(false));
    assert_eq!(s[2].form_prot, None);
}

#[test]
fn text_flow_is_read() {
    let s = sections([grpprl(&[(0x5033, &1u16.to_le_bytes())]), grpprl(&[(0x5033, &3u16.to_le_bytes())]), grpprl(&[(0x5033, &0u16.to_le_bytes())])]);
    assert_eq!(s[0].text_direction, TextDirection::Down);
    assert_eq!(s[1].text_direction, TextDirection::Up);
    assert_eq!((s[2].text_direction, s[2].text_direction_set), (TextDirection::Horizontal, true), "stated horizontal");
}

#[test]
fn page_number_start_32_bit_form() {
    let s = sections([
        grpprl(&[(0x3011, &[1]), (0x7044, &40_000u32.to_le_bytes())]),
        // Both forms: the later one wins.
        grpprl(&[(0x3011, &[1]), (0x501C, &7u16.to_le_bytes()), (0x7044, &9u32.to_le_bytes())]),
        grpprl(&[(0x3011, &[1]), (0x7044, &9u32.to_le_bytes()), (0x501C, &7u16.to_le_bytes())]),
    ]);
    assert_eq!(s[0].page_num_start, Some(40_000));
    assert_eq!(s[1].page_num_start, Some(9));
    assert_eq!(s[2].page_num_start, Some(7));
}
