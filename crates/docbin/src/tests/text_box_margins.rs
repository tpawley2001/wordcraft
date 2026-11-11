//! Text box internal margins and vertical text alignment ([MS-ODRAW] 2.3.21, Text property set:
//! dxTextLeft 0x0081, dyTextTop 0x0082, dxTextRight 0x0083, dyTextBottom 0x0084, anchorText
//! 0x0087). OfficeArt records are hand-built from the specification; no file produced by Word
//! is used.

use wordcraft_doc::props::VAlign;
use wordcraft_doc::{MAX_TEXT_BOX_INSET, PartKind, TextBody};

use super::drawing::{art_content, fopt, fsp, rec, sp, spa, spa_flags};
use super::{StructSpec, plc, read, struct_doc, structured_spec};

/// Overwrite little-endian u32s in the WordDocument stream of a compound file.
fn patch_word(file: &[u8], at: &[(usize, u32)]) -> Vec<u8> {
    use std::io::Read;
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(file.to_vec())).unwrap();
    let mut streams = Vec::new();
    for name in ["WordDocument", "0Table", "Data"] {
        if let Ok(mut s) = comp.open_stream(format!("/{name}")) {
            let mut v = Vec::new();
            s.read_to_end(&mut v).unwrap();
            if name == "WordDocument" {
                for (o, x) in at {
                    v[*o..*o + 4].copy_from_slice(&x.to_le_bytes());
                }
            }
            streams.push((name, v));
        }
    }
    let refs: Vec<(&str, &[u8])> = streams.iter().map(|(n, v)| (*n, v.as_slice())).collect();
    super::cfb_file(&refs)
}

/// A document with an ellipse and a text box (spid 1027, story "Inside") whose property table
/// holds `text_props` (ids ascending, all below 0x01BF) before its fill flags. Returns the text
/// box story's `TextBody`.
fn text_body(text_props: &[(u16, u32)]) -> TextBody {
    let ellipse = sp(&[fsp(3, 1026, 0xA00), fopt(&[(0x0181, 0x0000_00FF)], &[])]);
    let mut props = text_props.to_vec();
    props.push((0x01BF, 1 << 20));
    let tb = sp(&[fsp(202, 1027, 0xA00), fopt(&props, &[]), rec(0, 0, 0xF00D, &0x0001_0000u32.to_le_bytes())]);
    let art = art_content(&[], &[ellipse, tb]);
    let spas =
        plc(&[1, 2, 4], &[spa(1026, [0, 0, 1440, 1440], spa_flags(2, 2, 3, true)), spa(1027, [0, 0, 2880, 720], spa_flags(0, 0, 1, false))].concat());
    // FTXBXS: cTxbx, cReusable, fReusable, reserved, lid (the shape's spid), txidUndo.
    let ftxbxs =
        [1i32.to_le_bytes().as_slice(), &0i32.to_le_bytes(), &0i16.to_le_bytes(), &0i32.to_le_bytes(), &1027u32.to_le_bytes(), &0i32.to_le_bytes()]
            .concat();
    let txbx = plc(&[0, 7], &ftxbxs);
    let text = "A\u{8}\u{8}B\r";
    let ccp = text.chars().count() as u32;
    // The text box story is written in the endnote slot and the FIB patched to call it the text
    // box subdocument (ccpEdn = 0, ccpTxbx = 7).
    let spec =
        StructSpec { text, endnote_stories: vec!["Inside\r"], extra_pairs: vec![(40, spas), (50, art), (56, txbx)], ..structured_spec(text, ccp) };
    let file = patch_word(&struct_doc(&spec), &[(0x60, 0), (0x64, 7)]);
    let doc = read(&file).expect("opens");
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let story = p0
        .objects
        .iter()
        .find_map(|o| match o {
            wordcraft_doc::para::InlineObject::Shape { story: Some(id), .. } => Some(*id),
            _ => None,
        })
        .expect("a text box with a story");
    let part = doc.parts.get(&story).expect("text box story part");
    assert_eq!(part.kind, PartKind::TextBox);
    assert_eq!(part.blocks.first().and_then(|b| b.as_para()).map(|p| p.text.as_str()), Some("Inside"));
    part.body
}

#[test]
fn zero_internal_margins_are_kept() {
    let body = text_body(&[(0x0081, 0), (0x0082, 0), (0x0083, 0), (0x0084, 0)]);
    assert_eq!(body.inset, Some([0, 0, 0, 0]));
    assert_eq!(body.insets_pt(), [0.0; 4]);
    assert_eq!(body.anchor, VAlign::Top);
}

#[test]
fn a_margin_that_is_not_given_keeps_words_default() {
    let body = text_body(&[(0x0081, 0)]);
    assert_eq!(body.inset, Some([0, 45_720, 91_440, 45_720]));
    let body = text_body(&[(0x0084, 12_700)]);
    assert_eq!(body.inset, Some([91_440, 45_720, 91_440, 12_700]));
}

#[test]
fn each_side_is_read_from_its_own_property() {
    let body = text_body(&[(0x0081, 182_880), (0x0082, 91_440), (0x0083, 0), (0x0084, 12_700)]);
    assert_eq!(body.inset, Some([182_880, 91_440, 0, 12_700]));
    assert_eq!(body.insets_pt(), [14.4, 7.2, 0.0, 1.0]);
}

#[test]
fn default_margins_stay_the_default_text_body() {
    let written_out = text_body(&[(0x0081, 91_440), (0x0082, 45_720), (0x0083, 91_440), (0x0084, 45_720)]);
    assert_eq!(written_out.inset, None);
    assert!(written_out.is_default());
    let absent = text_body(&[]);
    assert_eq!(absent.inset, None);
    assert!(absent.is_default());
}

#[test]
fn negative_and_huge_margins_are_bounded() {
    let body = text_body(&[(0x0081, 0xFFFF_FFFF), (0x0083, 0x7FFF_FFFF)]);
    assert_eq!(body.inset, Some([0, 45_720, MAX_TEXT_BOX_INSET, 45_720]));
}

#[test]
fn vertical_text_alignment_is_read() {
    for (anchor, want) in [
        (0u32, VAlign::Top),
        (1, VAlign::Center),
        (2, VAlign::Bottom),
        (3, VAlign::Top),
        (4, VAlign::Center),
        (5, VAlign::Bottom),
        (6, VAlign::Top),
        (7, VAlign::Bottom),
        (8, VAlign::Top),
        (9, VAlign::Bottom),
        (77, VAlign::Top),
    ] {
        let body = text_body(&[(0x0087, anchor)]);
        assert_eq!(body.anchor, want, "anchorText {anchor}");
        assert_eq!(body.inset, None, "anchorText {anchor} leaves the margins alone");
    }
}

#[test]
fn margins_and_alignment_together() {
    let body = text_body(&[(0x0081, 0), (0x0082, 0), (0x0083, 0), (0x0084, 0), (0x0087, 1)]);
    assert_eq!((body.inset, body.anchor), (Some([0, 0, 0, 0]), VAlign::Center));
}
