//! A legacy VML float that wraps text (`w10:wrap` square, tight or through) and states no
//! `mso-wrap-distance-left` / `-right` keeps text 9 pt away at its sides, as in Word (measured:
//! text beside a Square box whose right edge is at 144 pt starts at 153 pt in Microsoft 365
//! Word's PDF) and as ECMA-376 Part 4 §14.1.2.19 gives for the VML style defaults; top and bottom
//! default to 0. A stated distance wins; a shape that does not wrap keeps 0.

use std::io::Write;

use wordcraft_doc::para::Float;
use wordcraft_doc::{Document, InlineObject};

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn read_body(body: &str) -> Document {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word"><w:body>{body}</w:body></w:document>"#
    );
    wordcraft_docx::read(&zip(&[("_rels/.rels", root.as_bytes()), ("word/document.xml", doc.as_bytes())])).unwrap()
}

/// The float of the first shape in the body's first paragraph.
fn first_float(d: &Document) -> Float {
    let p = d.body.iter().find_map(|b| b.as_para()).expect("a paragraph");
    p.objects
        .iter()
        .find_map(|o| match o {
            InlineObject::Shape { float, .. } | InlineObject::Image { float, .. } | InlineObject::Group { float, .. } => Some(float.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no shape in {:?}", p.objects))
}

const POS: &str = "position:absolute;margin-left:0;margin-top:0;width:72pt;height:72pt;z-index:1;mso-position-horizontal-relative:margin;mso-position-vertical-relative:paragraph";

/// A text box rectangle with `extra` appended to its style and `wrap` as its `w10:wrap` element.
fn rect(extra: &str, wrap: &str) -> Float {
    let body =
        format!(r#"<w:p><w:r><w:pict><v:rect style="{POS}{extra}">{wrap}</v:rect></w:pict></w:r><w:r><w:t>Text beside the box.</w:t></w:r></w:p>"#);
    first_float(&read_body(&body))
}

const SQUARE: &str = r#"<w10:wrap type="square"/>"#;

#[test]
fn square_wrap_with_no_distance_keeps_9pt_at_the_sides() {
    let f = rect("", SQUARE);
    assert_eq!((f.dist, f.dist_top, f.dist_bottom), (9.0, 0.0, 0.0));
}

#[test]
fn tight_wrap_with_no_distance_keeps_9pt_at_the_sides() {
    let f = rect("", r#"<w10:wrap type="tight"/>"#);
    assert_eq!((f.dist, f.dist_top, f.dist_bottom), (9.0, 0.0, 0.0));
}

#[test]
fn a_stated_zero_distance_stays_zero() {
    let f = rect(";mso-wrap-distance-left:0;mso-wrap-distance-right:0", SQUARE);
    assert_eq!((f.dist, f.dist_top, f.dist_bottom), (0.0, 0.0, 0.0));
}

#[test]
fn an_unstated_right_side_still_defaults_to_9pt() {
    let f = rect(";mso-wrap-distance-left:4pt", SQUARE);
    assert_eq!(f.dist, 9.0);
}

#[test]
fn stated_side_distances_win() {
    let f = rect(";mso-wrap-distance-left:12pt;mso-wrap-distance-right:3pt", SQUARE);
    assert_eq!(f.dist, 12.0);
}

#[test]
fn top_distance_is_read_and_sides_default() {
    let f = rect(";mso-wrap-distance-top:6pt", SQUARE);
    assert_eq!((f.dist, f.dist_top, f.dist_bottom), (9.0, 6.0, 0.0));
}

#[test]
fn top_and_bottom_wrap_keeps_zero_side_distance() {
    let f = rect("", r#"<w10:wrap type="topAndBottom"/>"#);
    assert_eq!(f.dist, 0.0);
}

#[test]
fn a_shape_in_front_of_text_keeps_zero_side_distance() {
    let f = rect("", "");
    assert_eq!(f.dist, 0.0);
}

#[test]
fn a_square_wrapped_group_with_no_distance_keeps_9pt() {
    let body = format!(
        r#"<w:p><w:r><w:pict><v:group style="{POS}" coordsize="100,100" coordorigin="0,0"><v:rect style="position:absolute;left:0;top:0;width:100;height:100"/><w10:wrap type="square"/></v:group></w:pict></w:r><w:r><w:t>Text.</w:t></w:r></w:p>"#
    );
    assert_eq!(first_float(&read_body(&body)).dist, 9.0);
}
