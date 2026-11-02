//! Linked pictures whose file is missing ([MS-DOC] §2.9.190 PICF, `mfpf.mm` = MM_SHAPEFILE;
//! [MS-ODRAW] §2.3.23.3 `pibFlags`). A picture kept as a link to a file (an
//! `INCLUDEPICTURE \d` result) has a PICF, the file name and a picture shape whose FOPT names the
//! file (`pibName`, `pibFlags` = LinkToFile) but no BLIP and no FBSE. Word lays it out at its goal
//! size and draws a box in its place; it must not vanish. A horizontal-rule shape with no BLIP
//! (Word's "horizontal line") is a different thing and does not become a box.

use wordcraft_doc::para::InlineObject;

use super::drawing::{fbse_png, fopt, fsp, rec, sp, tiny_png, wz};
use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_SPEC: u16 = 0x0855;
const MM_SHAPE: u16 = 0x0064;
const MM_SHAPEFILE: u16 = 0x0066;

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

/// The client anchor Word writes after an inline picture's FOPT.
fn client_anchor() -> Vec<u8> {
    rec(0, 0, 0xF010, &[0, 0, 0, 0x80])
}

/// A linked picture as Word 2000 writes it: MM_SHAPEFILE, the path as a Pascal string, then a
/// picture frame whose FOPT has `pibName` and `pibFlags` = File | DoNotSave | LinkToFile (13),
/// no `pib`, and no FBSE after it.
fn linked_picture(path: &str, goal: (i16, i16), scale: (u16, u16)) -> Vec<u8> {
    let mut rest = vec![path.len() as u8];
    rest.extend_from_slice(path.as_bytes());
    rest.extend(sp(&[fsp(75, 1025, 0xA00), fopt(&[(0x0106, 13), (0x01FF, 0x0008_0000)], &[(0x0105, wz(path))]), client_anchor()]));
    picf(MM_SHAPEFILE, goal, scale, &rest)
}

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

fn images(doc: &wordcraft_doc::Document) -> Vec<(f32, f32)> {
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    p0.objects.iter().filter_map(|o| if let InlineObject::Image { w, h, .. } = o { Some((*w, *h)) } else { None }).collect()
}

#[test]
fn a_linked_picture_without_its_file_keeps_its_goal_size() {
    // 4500 x 375 twips at 100 %: Word's box is 225 x 18.75 pt.
    let doc = doc_with(linked_picture("c:\\web\\title.gif", (4500, 375), (1000, 1000)));
    assert_eq!(images(&doc), vec![(225.0, 18.75)]);
}

#[test]
fn a_linked_picture_without_its_file_takes_its_scale() {
    // 2000 x 1500 twips (100 x 75 pt) at 250 % x 200 %: 250 x 150 pt.
    let doc = doc_with(linked_picture("c:\\web\\photo.jpg", (2000, 1500), (2500, 2000)));
    assert_eq!(images(&doc), vec![(250.0, 150.0)]);
}

#[test]
fn a_horizontal_rule_without_a_blip_is_not_a_picture_box() {
    // Word's "horizontal line": MM_SHAPE, 50 x 25 pt goal at 9072 x 60, a filled rectangle whose
    // tertiary FOPT sets fHorizRule (0x03BF bit 11) and fStandardHR (bit 13), no BLIP.
    let tertiary = {
        let mut b = Vec::new();
        for (pid, op) in [(0x0394u16, 1u32), (0x0395, 30), (0x03BF, 0x2800_2800)] {
            b.extend_from_slice(&pid.to_le_bytes());
            b.extend_from_slice(&op.to_le_bytes());
        }
        rec(3, 3, 0xF122, &b)
    };
    let shape = sp(&[fsp(1, 1026, 0xA00), fopt(&[(0x0181, 0x0099_A8AC)], &[]), tertiary, client_anchor()]);
    let doc = doc_with(picf(MM_SHAPE, (1000, 500), (9072, 60), &shape));
    assert_eq!(images(&doc), Vec::<(f32, f32)>::new(), "a horizontal rule is not drawn as a 453.6 x 1.5 pt picture box");
}

#[test]
fn a_picture_with_its_blip_is_unchanged() {
    let png = tiny_png();
    let shape = sp(&[fsp(75, 1025, 0xA00), fopt(&[(0x0104, 1)], &[]), client_anchor()]);
    let doc = doc_with(picf(MM_SHAPE, (1440, 720), (1000, 1000), &[shape, fbse_png(&png)].concat()));
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let Some(InlineObject::Image { media, w, h, .. }) = p0.objects.first() else { panic!("no image: {:?}", p0.objects) };
    assert_eq!((*w, *h), (72.0, 36.0));
    assert_eq!(doc.media.get(media).map(|b| b.as_slice()), Some(png.as_slice()));
}
