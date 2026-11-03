//! Word's "horizontal line" pictures in `.doc` ([MS-ODRAW] §2.3.4 Group Shape properties): an
//! inline `MM_SHAPE` picture with no BLIP whose shape sets `fHorizRule` (0x03BF bit 11, valid with
//! `fUsefHorizRule` bit 27). `pctHR` (0x0393, tenths of a percent of the line, default 1000),
//! `alignHR` (0x0394: 0 left, 1 centre, 2 right), `dxHeightHR` (0x0395, twips), `dxWidthHR`
//! (0x0396, twips) and `fStandardHR` (bit 13, with `fUsefStandardHR` bit 29) say how it sits on
//! its line. It reads as a shape with a [`HorizontalRule`], as a VML `o:hr` rule does in DOCX.

use wordcraft_doc::connector::HorizontalRule;
use wordcraft_doc::para::{InlineObject, ShapeKind};
use wordcraft_doc::props::Align;

use super::drawing::{fopt, fsp, rec, sp};
use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_SPEC: u16 = 0x0855;
const MM_SHAPE: u16 = 0x0064;

/// A PICF with `mm`, goal size (twips) and scale (1000 = 100 %), then `rest`.
fn picf(mm: u16, goal: (i16, i16), scale: (u16, u16), rest: &[u8]) -> Vec<u8> {
    let mut picf = vec![0u8; 0x44];
    picf[0..4].copy_from_slice(&((0x44 + rest.len()) as u32).to_le_bytes());
    picf[4..6].copy_from_slice(&0x44u16.to_le_bytes());
    picf[6..8].copy_from_slice(&mm.to_le_bytes());
    picf[28..30].copy_from_slice(&goal.0.to_le_bytes());
    picf[30..32].copy_from_slice(&goal.1.to_le_bytes());
    picf[32..34].copy_from_slice(&scale.0.to_le_bytes());
    picf[34..36].copy_from_slice(&scale.1.to_le_bytes());
    [picf.as_slice(), rest].concat()
}

fn client_anchor() -> Vec<u8> {
    rec(0, 0, 0xF010, &[0, 0, 0, 0x80])
}

/// Word's horizontal line as Word 2000-2003 writes it (as in POI's Bug51890.doc): a rectangle
/// (spt 1) with `fillColor` 0x0099A8AC (COLORREF, so RGB ACA899), a tertiary FOPT with `tertiary`
/// properties, a 1000 x 500 twip goal at 907.2 % x 6 % (453.6 x 1.5 pt).
fn rule(tertiary: &[(u16, u32)]) -> Vec<u8> {
    let mut b = Vec::new();
    for (pid, op) in tertiary {
        b.extend_from_slice(&pid.to_le_bytes());
        b.extend_from_slice(&op.to_le_bytes());
    }
    let shape = sp(&[fsp(1, 1026, 0xA00), fopt(&[(0x0181, 0x0099_A8AC)], &[]), rec(3, tertiary.len() as u16, 0xF122, &b), client_anchor()]);
    picf(MM_SHAPE, (1000, 500), (9072, 60), &shape)
}

/// fHorizRule + fStandardHR with both fUse bits.
const STANDARD: u32 = (1 << 11) | (1 << 13) | (1 << 27) | (1 << 29);
/// fHorizRule only (fUsefStandardHR set, fStandardHR clear), as in Bug51890.doc.
const PLAIN: u32 = (1 << 11) | (1 << 27) | (1 << 29);

/// "a", the picture, "b": the picture is at CP 1 and every character carries its location.
fn doc_with(data: Vec<u8>) -> wordcraft_doc::Document {
    let text = "a\u{1}b\r";
    let ccp = text.chars().count() as u32;
    let spec = StructSpec {
        text,
        data_stream: data,
        chpx_runs: vec![(0, grpprl(&[(PIC_LOCATION, &0u32.to_le_bytes()), (F_SPEC, &[1])]))],
        ..structured_spec(text, ccp)
    };
    read(&struct_doc(&spec)).expect("opens")
}

/// The first paragraph's horizontal rules: (kind, w, h, fill hex, has stroke, rule).
fn rules(doc: &wordcraft_doc::Document) -> Vec<(ShapeKind, f32, f32, Option<String>, bool, HorizontalRule)> {
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    p0.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Shape { kind, w, h, fill, stroke, extra, .. } => {
                Some((*kind, *w, *h, fill.as_ref().map(|f| f.main_color().hex().to_ascii_uppercase()), stroke.is_some(), extra.hr?))
            }
            _ => None,
        })
        .collect()
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn a_standard_centred_rule_reads_as_a_horizontal_rule_shape() {
    // No pctHR: the default 1000 tenths, the whole line. alignHR 1 = centre; dxHeightHR 30 twips.
    let doc = doc_with(rule(&[(0x0394, 1), (0x0395, 30), (0x03BF, STANDARD)]));
    let r = rules(&doc);
    assert_eq!(r.len(), 1, "one horizontal rule: {:?}", doc.body.first().and_then(|b| b.as_para()).map(|p| &p.objects));
    let (kind, _w, h, fill, stroke, hr) = r[0].clone();
    assert_eq!(kind, ShapeKind::Rectangle);
    assert!(approx(h, 1.5), "height {h} pt (dxHeightHR 30 twips)");
    assert_eq!(fill.as_deref(), Some("ACA899"), "fillColor 0x0099A8AC is COLORREF (0x00BBGGRR)");
    assert!(!stroke, "Word draws no outline on a horizontal line");
    assert!(approx(hr.pct, 100.0), "pct {}", hr.pct);
    assert_eq!(hr.align, Align::Center);
    assert!(hr.standard);
}

#[test]
fn pct_and_right_alignment_are_read() {
    // pctHR 500 tenths = 50 %; alignHR 2 = right; dxHeightHR 60 twips = 3 pt.
    let doc = doc_with(rule(&[(0x0393, 500), (0x0394, 2), (0x0395, 60), (0x03BF, STANDARD)]));
    let r = rules(&doc);
    assert_eq!(r.len(), 1);
    let (_, _, h, _, _, hr) = r[0].clone();
    assert!(approx(h, 3.0), "height {h}");
    assert!(approx(hr.pct, 50.0), "pct {}", hr.pct);
    assert_eq!(hr.align, Align::Right);
}

#[test]
fn a_left_rule_without_fstandardhr_is_not_standard() {
    // As in Bug51890.doc's six rules apart from the alignment: fUsefStandardHR set, fStandardHR clear.
    let doc = doc_with(rule(&[(0x0394, 0), (0x0395, 30), (0x03BF, PLAIN)]));
    let r = rules(&doc);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].5.align, Align::Left);
    assert!(!r[0].5.standard);
}

#[test]
fn a_rule_at_its_own_width_takes_dxwidthhr() {
    // pctHR 0: the rule is its own width, dxWidthHR 2880 twips = 144 pt.
    let doc = doc_with(rule(&[(0x0393, 0), (0x0394, 1), (0x0395, 30), (0x0396, 2880), (0x03BF, STANDARD)]));
    let r = rules(&doc);
    assert_eq!(r.len(), 1);
    let (_, w, _, _, _, hr) = r[0].clone();
    assert!(approx(hr.pct, 0.0), "pct {}", hr.pct);
    assert!(approx(w, 144.0), "own width {w} pt");
    assert!(approx(hr.width(468.0, w), 144.0));
}

#[test]
fn a_shape_without_fhorizrule_is_not_a_rule() {
    // fUsefHorizRule set but fHorizRule clear: an ordinary empty shape, no HorizontalRule.
    let doc = doc_with(rule(&[(0x0394, 1), (0x0395, 30), (0x03BF, 1 << 27)]));
    assert!(rules(&doc).is_empty());
}
