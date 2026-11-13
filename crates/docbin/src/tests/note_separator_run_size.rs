//! A note separator story that holds only its line but whose line run has a font size of its own
//! (sprmCHps) is the document's own separator, not the default: Word sizes the room above the notes
//! from that run (Microsoft 365 Word: a 10-pt separator run in an 11-pt Normal document leaves one
//! 10-pt line, 11.5 pt for Times New Roman; RTF probes with 11 and 20 pt runs give 12.65 and 23.0).
//! Its paragraph lays out like the default separator: no space before or after, single spacing.

use super::{StructSpec, grpprl, plc, read, struct_doc, structured_spec};
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Blocks, Document};

/// sprmCHps: font size in half-points.
const HPS: u16 = 0x4A43;

const BODY: &str = "Body\r";

/// A document whose six separator stories are `seps`, with `runs` (CP relative to the start of
/// the first separator story, sprmCHps half-points; `None` = no formatting) as character runs
/// over the stories.
fn open_with(seps: [&'static str; 6], runs: &[(u32, Option<u16>)]) -> Document {
    let ccp = BODY.chars().count() as u32;
    let mut cps = vec![0u32];
    let mut at = 0u32;
    for s in seps {
        at += s.chars().count() as u32;
        cps.push(at);
    }
    cps.extend([at; 6]);
    let hdd = plc(&cps, &[]);
    let stories: Vec<&'static str> = seps.into_iter().filter(|s| !s.is_empty()).collect();
    let mut chpx = vec![(0u32, Vec::new())];
    for &(cp, hps) in runs {
        chpx.push((ccp + cp, hps.map_or_else(Vec::new, |h| grpprl(&[(HPS, &h.to_le_bytes())]))));
    }
    let spec = StructSpec { text: BODY, header_stories: stories, extra_pairs: vec![(11, hdd)], chpx_runs: chpx, ..structured_spec(BODY, ccp) };
    read(&struct_doc(&spec)).expect("opens")
}

/// The one paragraph of a kept separator story: its line run's size, space before, space after
/// and line spacing.
fn one_para(blocks: &Option<Blocks>) -> (Option<f32>, Option<f32>, Option<f32>, Option<LineSpacing>) {
    let blocks = blocks.as_ref().expect("the story is kept");
    let paras: Vec<_> = blocks.iter().filter_map(|b| b.as_para()).collect();
    assert_eq!(paras.len(), 1, "one paragraph");
    let p = paras[0];
    let at = p.text.find('\u{FFFC}').expect("the separator line");
    let size = p.props_of_char(at).size;
    (size, p.props.space_before, p.props.space_after, p.props.line_spacing)
}

#[test]
fn c1_a_line_with_no_size_of_its_own_stays_the_default() {
    let doc = open_with(["\u{3}\r", "", "", "", "", ""], &[]);
    assert_eq!(doc.footnote_separators.separator, None);
}

#[test]
fn c2_a_10pt_line_is_kept_with_its_size_and_default_spacing() {
    // The 0x03 run and its paragraph mark at 10 pt, as in a Word 97-2003 FCC notice.
    let doc = open_with(["\u{3}\r", "", "", "", "", ""], &[(0, Some(20))]);
    let (size, before, after, line) = one_para(&doc.footnote_separators.separator);
    assert_eq!(size, Some(10.0));
    assert_eq!(before, Some(0.0));
    assert_eq!(after, Some(0.0));
    assert_eq!(line, Some(LineSpacing::Multiple(1.0)));
}

#[test]
fn c3_a_20pt_line_is_kept_with_its_size() {
    let doc = open_with(["\u{3}\r", "", "", "", "", ""], &[(0, Some(40)), (1, None)]);
    let (size, ..) = one_para(&doc.footnote_separators.separator);
    assert_eq!(size, Some(20.0));
}

#[test]
fn c4_a_10pt_continuation_separator_is_kept() {
    // Separator story: 0x03 + CR (CPs 0-1, no size); continuation separator: 0x04 + CR (CPs 2-3, 10 pt).
    let doc = open_with(["\u{3}\r", "\u{4}\r", "", "", "", ""], &[(2, Some(20))]);
    assert_eq!(doc.footnote_separators.separator, None);
    let (size, before, after, _) = one_para(&doc.footnote_separators.continuation_separator);
    assert_eq!(size, Some(10.0));
    assert_eq!((before, after), (Some(0.0), Some(0.0)));
}

#[test]
fn c5_endnote_separator_with_a_size_is_kept() {
    let doc = open_with(["", "", "", "\u{3}\r", "", ""], &[(0, Some(16))]);
    let (size, ..) = one_para(&doc.endnote_separators.separator);
    assert_eq!(size, Some(8.0));
    assert_eq!(doc.footnote_separators.separator, None);
}
