//! Text boxes and shapes with text (`{\shp{\*\shpinst … {\shptxt …}}}`, RTF 1.9.1 "Shapes"): their
//! text opens as the box's story, floating where `\shpleft`/`\shptop` and `\shpbx*`/`\shpby*` say,
//! in the body and in headers, as Word shows them.

use super::*;
use wordcraft_doc::para::{Anchor, InlineObject, OBJ, Wrap};
use wordcraft_doc::{Block, Document, PartKind};

const FONTS: &str = r"{\fonttbl{\f0 Courier New;}}";

fn shape(pos: &str, props: &str, text: &str) -> String {
    format!(r"{{\shp{{\*\shpinst{pos}{{\sp{{\sn shapeType}}{{\sv 202}}}}{props}{{\shptxt \pard\plain \f0 {text}\par}}}}}}")
}

fn body_doc(shp: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{FONTS}\pard\plain\f0 Before {shp} after.\par}}").as_bytes()).unwrap()
}

/// Each shape with a story in `blocks`: (size, wrap, x, y, horizontal anchor, its text).
fn boxes(d: &Document, blocks: &[std::sync::Arc<Block>]) -> Vec<((f32, f32), Wrap, (f32, f32), Anchor, String)> {
    let mut out = Vec::new();
    for b in blocks {
        let Some(p) = b.as_para() else { continue };
        for o in &p.objects {
            if let InlineObject::Shape { w, h, float, story: Some(id), .. } = o {
                let part = d.parts.get(id).expect("the box's story");
                assert_eq!(part.kind, PartKind::TextBox);
                let text = part.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.replace(OBJ, "")).collect::<Vec<_>>().join("|");
                out.push(((*w, *h), float.wrap, (float.x, float.y), float.h_rel, text.trim().to_string()));
            }
        }
    }
    out
}

const BODY_POS: &str = r"\shpleft0\shptop0\shpright4000\shpbottom600\shpbxcolumn\shpbypara\shpwr3\shpz0\shplid1025";

#[test]
fn a_body_text_box_keeps_its_text() {
    let d = body_doc(&shape(BODY_POS, "", "BOXTEXT"));
    // 4000 × 600 twips = 200 × 30 pt; `\shpwr3` = no wrap, in front of the text.
    assert_eq!(boxes(&d, &d.body), [((200.0, 30.0), Wrap::InFrontOfText, (0.0, 0.0), Anchor::Column, "BOXTEXT".to_string())]);
    // Its text isn't the paragraph's.
    let body = d.body[0].as_para().unwrap().text.replace(OBJ, "");
    assert!(!body.contains("BOXTEXT") && body.starts_with("Before") && body.ends_with("after."), "{body:?}");
}

#[test]
fn wrap_and_behind_text() {
    let square = BODY_POS.replace(r"\shpwr3", r"\shpwr2");
    let d = body_doc(&shape(&square, "", "Square"));
    assert_eq!(boxes(&d, &d.body)[0].1, Wrap::Square);
    let behind = format!(r"{BODY_POS}\shpfblwtxt1");
    let d = body_doc(&shape(&behind, "", "Behind"));
    assert_eq!(boxes(&d, &d.body)[0].1, Wrap::BehindText);
}

#[test]
fn a_rectangle_with_text_keeps_it_too() {
    let rect = shape(BODY_POS, "", "In a rectangle").replace(r"{\sv 202}", r"{\sv 1}");
    let d = body_doc(&rect);
    let got = boxes(&d, &d.body);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].4, "In a rectangle");
}

#[test]
fn a_header_text_box_holds_the_page_number() {
    // An RTF transcript's header: a text box 3024 twips from the page's left edge holding a tab and
    // a PAGE field. Word prints the page number on every page.
    let pos = r"\shpleft3024\shptop0\shpright10512\shpbottom240\shpfhdr1\shpbxpage\shpbypara\shpwr3\shpwrk0\shpfblwtxt0\shpz3\shplid1028";
    let props = r"{\sp{\sn fFilled}{\sv 0}}{\sp{\sn fLine}{\sv 0}}";
    let shp = shape(pos, props, r"BOXTEXT {\field{\*\fldinst page}{\fldrslt 1}}");
    let rtf = format!(r"{{\rtf1\ansi\deff0{FONTS}\sectd\headery360{{\header \pard\plain \f0 {shp}\par}}\pard\plain\f0 Body text.\par}}");
    let d = import(rtf.as_bytes()).unwrap();
    let headers: Vec<_> = d.parts.values().filter(|p| p.kind == PartKind::Header).collect();
    let got: Vec<_> = headers.iter().flat_map(|h| boxes(&d, &h.blocks)).collect();
    let [((w, h), wrap, (x, _), anchor, text)] = got.as_slice() else { panic!("{got:?}") };
    assert_eq!((*w, *h), (374.4, 12.0));
    assert_eq!((*wrap, *x, *anchor), (Wrap::InFrontOfText, 151.2, Anchor::Page));
    assert!(text.starts_with("BOXTEXT"), "{text:?}");
    // The PAGE field is kept as a field, not frozen to its cached "1".
    let id = headers.iter().flat_map(|h| h.blocks.iter()).filter_map(|b| b.as_para()).flat_map(|p| p.objects.iter()).find_map(|o| match o {
        InlineObject::Shape { story: Some(id), .. } => Some(*id),
        _ => None,
    });
    let part = d.parts.get(&id.unwrap()).unwrap();
    let rtf_back = export(&d);
    assert!(rtf_back.to_lowercase().contains("page"), "{rtf_back}");
    assert!(part.blocks.iter().filter_map(|b| b.as_para()).any(|p| p.text.contains(OBJ) || p.text.contains('1')));
    // The body is untouched.
    assert_eq!(d.body[0].as_para().unwrap().text, "Body text.");
}

#[test]
fn text_boxes_round_trip() {
    let d = body_doc(&shape(BODY_POS, "", "BOXTEXT"));
    assert_eq!(boxes(&d, &d.body).len(), 1);
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(boxes(&back, &back.body), boxes(&d, &d.body));
}

#[test]
fn shapes_without_text_and_old_drawing_objects_do_not_panic() {
    let empty = format!(r"{{\shp{{\*\shpinst{BODY_POS}{{\sp{{\sn shapeType}}{{\sv 1}}}}}}}}");
    let d = body_doc(&empty);
    assert!(boxes(&d, &d.body).is_empty());
    let deep = r"{\shp{\*\shpinst{\shptxt ".repeat(60);
    let _ = import(format!(r"{{\rtf1\ansi{deep}x\par}}").as_bytes());
}
