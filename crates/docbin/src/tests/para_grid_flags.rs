//! Automatically adjust right indent (sprmPFAdjustRight 0x2448) and Snap to grid
//! (sprmPFUsePgsuSettings 0x2447), Bool8 PAP sprms ([MS-DOC] §2.6.2), read from direct formatting
//! and from styles. Microsoft 365 Word writes `48 24 00` / `47 24 00` for a paragraph or style that
//! turns the setting off, `48 24 01` for a paragraph that turns it back on over an off style, and
//! nothing when it is on as by default.
//!
//! Also Right-to-left (sprmPFBiDi 0x2441, Bool8). Word stores an RTL paragraph's alignment and
//! indents twice: the old sprms (sprmPJc80 0x2403, sprmPDxaLeft80 0x840F, sprmPDxaRight80 0x840E)
//! are PHYSICAL (mirrored in an RTL paragraph) and the newer ones (sprmPJc 0x2461, sprmPDxaLeft
//! 0x845E, sprmPDxaRight 0x845D) are LOGICAL, like `w:jc` / `w:ind` in the .docx Word saves of the
//! same document. The grpprls below are the ones Microsoft 365 Word wrote (2026-11-14), rsids dropped.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::{Align, ParaProps};

const P_F_USE_PGSU_SETTINGS: u16 = 0x2447;
const P_F_ADJUST_RIGHT: u16 = 0x2448;
const P_F_BIDI: u16 = 0x2441;
const P_JC80: u16 = 0x2403;
const P_JC: u16 = 0x2461;
const P_DXA_RIGHT80: u16 = 0x840E;
const P_DXA_LEFT80: u16 = 0x840F;
const P_DXA_RIGHT: u16 = 0x845D;
const P_DXA_LEFT: u16 = 0x845E;

/// Normal (istd 0) and "Quiet" (istd 1, on Normal, with `style_papx`); one paragraph per entry
/// of `paras` (start CP, istd, grpprl).
fn doc(text: &'static str, style_papx: Vec<u8>, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let spec = RichSpec {
        text,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Quiet", style_papx, Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: paras,
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn para(doc: &Document, i: usize) -> ParaProps {
    doc.body.iter().filter_map(|b| b.as_para()).nth(i).expect("paragraph").props.clone()
}

/// One Normal paragraph "One" with `papx` as its direct formatting.
fn one(papx: Vec<u8>) -> ParaProps {
    para(&doc("One\r", Vec::new(), vec![(0, 0, papx)]), 0)
}

#[test]
fn adjust_right_reads_on_off_and_unset() {
    assert_eq!(one(grpprl(&[(P_F_ADJUST_RIGHT, &[1])])).adjust_right_indent, Some(true));
    assert_eq!(one(grpprl(&[(P_F_ADJUST_RIGHT, &[0])])).adjust_right_indent, Some(false), "0 is an explicit off");
    assert_eq!(one(Vec::new()).adjust_right_indent, None, "no sprm: nothing set (resolves to on)");
}

#[test]
fn snap_to_grid_reads_on_off_and_unset() {
    assert_eq!(one(grpprl(&[(P_F_USE_PGSU_SETTINGS, &[1])])).snap_to_grid, Some(true));
    assert_eq!(one(grpprl(&[(P_F_USE_PGSU_SETTINGS, &[0])])).snap_to_grid, Some(false), "0 is an explicit off");
    assert_eq!(one(Vec::new()).snap_to_grid, None, "no sprm: nothing set (resolves to on)");
}

#[test]
fn any_non_zero_byte_turns_them_on() {
    let p = one(grpprl(&[(P_F_ADJUST_RIGHT, &[0x81]), (P_F_USE_PGSU_SETTINGS, &[0x81])]));
    assert_eq!((p.adjust_right_indent, p.snap_to_grid), (Some(true), Some(true)));
}

#[test]
fn a_style_turns_both_off_and_a_paragraph_turns_one_back_on() {
    let style = grpprl(&[(P_F_ADJUST_RIGHT, &[0]), (P_F_USE_PGSU_SETTINGS, &[0])]);
    let d = doc("One\rTwo\r", style, vec![(0, 1, Vec::new()), (4, 1, grpprl(&[(P_F_ADJUST_RIGHT, &[1])]))]);
    let s = d.styles.styles.iter().find(|s| s.name == "Quiet").expect("style").para.clone();
    assert_eq!((s.adjust_right_indent, s.snap_to_grid), (Some(false), Some(false)), "the style keeps both");
    let r0 = d.styles.resolve_para(&para(&d, 0));
    assert!(!r0.adjust_right_indent && !r0.snap_to_grid, "first paragraph inherits both offs");
    let r1 = d.styles.resolve_para(&para(&d, 1));
    assert!(r1.adjust_right_indent && !r1.snap_to_grid, "second paragraph: adjust back on, snap still off");
}

#[test]
fn a_normal_paragraph_without_either_sprm_resolves_both_on() {
    let d = doc("One\r", Vec::new(), vec![(0, 0, Vec::new())]);
    let r = d.styles.resolve_para(&para(&d, 0));
    assert!(r.adjust_right_indent && r.snap_to_grid);
}

#[test]
fn bidi_reads_on_off_and_unset() {
    assert_eq!(one(grpprl(&[(P_F_BIDI, &[1])])).bidi, Some(true));
    assert_eq!(one(grpprl(&[(P_F_BIDI, &[0])])).bidi, Some(false));
    assert_eq!(one(Vec::new()).bidi, None);
}

/// Word, an RTL paragraph aligned to its start (Alignment = left in Word's object model; the .docx
/// has `<w:bidi/>` and no `w:jc`): only sprmPJc80 = 2 (physically right) is written.
#[test]
fn rtl_paragraph_aligned_to_its_start_is_not_read_as_right() {
    let p = one(grpprl(&[(P_JC80, &[2]), (P_F_BIDI, &[1])]));
    assert_eq!(p.bidi, Some(true));
    assert_eq!(p.align.unwrap_or(Align::Left), Align::Left, "{:?}", p.align);
}

/// Word, RTL paragraphs aligned to their end and centred (`<w:jc w:val="right"/>` and `"center"`
/// in the .docx): sprmPJc80 is the mirror, sprmPJc the logical value, and the logical one wins.
#[test]
fn rtl_paragraph_logical_alignment_wins() {
    let end = one(grpprl(&[(P_JC80, &[0]), (P_F_BIDI, &[1]), (P_JC, &[2])]));
    assert_eq!(end.align, Some(Align::Right));
    let centre = one(grpprl(&[(P_JC80, &[1]), (P_F_BIDI, &[1]), (P_JC, &[1])]));
    assert_eq!(centre.align, Some(Align::Center));
    // Order must not matter: the logical sprm wins wherever it stands.
    let end_first = one(grpprl(&[(P_JC, &[2]), (P_F_BIDI, &[1]), (P_JC80, &[0])]));
    assert_eq!(end_first.align, Some(Align::Right));
}

/// A left-to-right paragraph is unchanged: sprmPJc80 alone is read as it is.
#[test]
fn ltr_paragraph_alignment_is_unchanged() {
    assert_eq!(one(grpprl(&[(P_JC80, &[2]), (P_JC, &[2])])).align, Some(Align::Right));
    assert_eq!(one(grpprl(&[(P_JC80, &[2])])).align, Some(Align::Right));
    assert_eq!(one(grpprl(&[(P_JC80, &[1])])).align, Some(Align::Center));
}

/// Word, an RTL paragraph with LeftIndent 36 pt and RightIndent 72 pt (the .docx: `<w:ind
/// w:left="720" w:right="1440"/>`): the physical 80 sprms hold the mirror (right80 = 720,
/// left80 = 1440), the logical ones the model's values.
#[test]
fn rtl_paragraph_indents_are_the_logical_ones() {
    let tw = |v: i16| v.to_le_bytes();
    let word = one(grpprl(&[
        (P_JC80, &[2]),
        (P_DXA_RIGHT80, &tw(720)),
        (P_DXA_LEFT80, &tw(1440)),
        (P_F_BIDI, &[1]),
        (P_DXA_RIGHT, &tw(1440)),
        (P_DXA_LEFT, &tw(720)),
    ]));
    assert_eq!((word.indent_left, word.indent_right), (Some(36.0), Some(72.0)));
    // The logical sprms win even when they come first.
    let reordered = one(grpprl(&[
        (P_DXA_RIGHT, &tw(1440)),
        (P_DXA_LEFT, &tw(720)),
        (P_F_BIDI, &[1]),
        (P_DXA_RIGHT80, &tw(720)),
        (P_DXA_LEFT80, &tw(1440)),
    ]));
    assert_eq!((reordered.indent_left, reordered.indent_right), (Some(36.0), Some(72.0)));
}
