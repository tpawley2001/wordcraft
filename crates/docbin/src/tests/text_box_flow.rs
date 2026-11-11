//! Text box text direction and "resize shape to fit text" ([MS-ODRAW] 2.3.21, Text property set:
//! txflTextFlow 0x0088 and fFitShapeToText in the Text Boolean Properties 0x00BF). OfficeArt
//! records are hand-built from the specification; no file produced by Word is used.

use wordcraft_doc::props::{TextVert, VAlign};
use wordcraft_doc::{PartKind, TextBody};

use super::drawing::{art_content, fopt, fsp, rec, sp, spa, spa_flags};
use super::{StructSpec, plc, read, struct_doc, structured_spec};

/// `fFitShapeToText` and the bit that says it is given.
const FIT: u32 = 1 << 1;
const USE_FIT: u32 = 1 << 17;
/// `fAutoTextMargin` and its use bit: a neighbour that must not be mistaken for `FIT`.
const AUTO_MARGIN: u32 = (1 << 3) | (1 << 19);

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
fn a_box_without_the_properties_is_horizontal_and_fixed() {
    let body = text_body(&[]);
    assert_eq!(body.vert, TextVert::Horz);
    assert!(!body.auto_fit);
    assert!(body.is_default());
}

#[test]
fn text_direction_is_read() {
    for (txfl, want) in
        [(0u32, TextVert::Horz), (1, TextVert::Vert), (2, TextVert::Vert270), (3, TextVert::Vert), (4, TextVert::Horz), (77, TextVert::Horz)]
    {
        let body = text_body(&[(0x0088, txfl)]);
        assert_eq!(body.vert, want, "txflTextFlow {txfl}");
        assert!(!body.auto_fit, "txflTextFlow {txfl} leaves resize-to-fit off");
        assert_eq!(body.anchor, VAlign::Top, "txflTextFlow {txfl} leaves the alignment alone");
        assert_eq!(body.inset, None, "txflTextFlow {txfl} leaves the margins alone");
    }
}

#[test]
fn horizontal_text_written_out_stays_the_default_text_body() {
    assert!(text_body(&[(0x0088, 0)]).is_default());
    assert!(text_body(&[(0x00BF, USE_FIT)]).is_default());
    assert!(text_body(&[(0x00BF, AUTO_MARGIN)]).is_default());
}

#[test]
fn resize_shape_to_fit_text_is_read() {
    let body = text_body(&[(0x00BF, USE_FIT | FIT)]);
    assert!(body.auto_fit);
    assert_eq!(body.vert, TextVert::Horz);
    assert_eq!(body.inset, None);
}

#[test]
fn resize_to_fit_needs_its_use_bit() {
    for (flags, want) in [
        (USE_FIT | FIT, true),
        (USE_FIT, false),
        (FIT, false),
        (AUTO_MARGIN, false),
        (AUTO_MARGIN | FIT, false),
        (AUTO_MARGIN | USE_FIT | FIT, true),
        (0, false),
        (0xFFFF_FFFF, true),
        (!FIT, false),
    ] {
        assert_eq!(text_body(&[(0x00BF, flags)]).auto_fit, want, "Text Boolean Properties {flags:#010x}");
    }
}

#[test]
fn direction_fit_alignment_and_margins_together() {
    let body = text_body(&[(0x0081, 0), (0x0082, 0), (0x0083, 0), (0x0084, 0), (0x0087, 1), (0x0088, 2), (0x00BF, USE_FIT | FIT)]);
    assert_eq!((body.vert, body.auto_fit), (TextVert::Vert270, true));
    assert_eq!((body.inset, body.anchor), (Some([0, 0, 0, 0]), VAlign::Center));
}
