//! Update Fields (F9, and the update before printing) leaves an `EMBED` field alone: an embedded
//! object read from a .doc without its storage is a range field whose result is its picture, and
//! the picture stays inside the field.

use serde_json::json;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

use crate::Session;

fn embed_doc() -> Document {
    let mut d = Document::new();
    let media = d.add_media(b"\x89PNG\r\n\x1a\nsynthetic".to_vec(), "png");
    let pic = InlineObject::Image { media, w: 456.0, h: 50.0, alt: String::new(), float: Default::default(), crop: [0.0; 4], fx: Default::default(), ole: None };
    let mut p = Paragraph::new();
    let props = CharProps::default();
    p.insert_object(0, InlineObject::FieldStart { instr: "EMBED Excel.Sheet.8".into(), locked: false, dirty: false }, &props).unwrap();
    p.insert_object(p.len(), pic, &props).unwrap();
    p.insert_object(p.len(), InlineObject::FieldEnd, &props).unwrap();
    d.body = vec![para_block(p)].into_iter().collect();
    d
}

fn kinds(s: &Session) -> Vec<&'static str> {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| p.objects.iter())
        .map(|o| match o {
            InlineObject::FieldStart { .. } => "start",
            InlineObject::Image { .. } => "image",
            InlineObject::FieldEnd => "end",
            _ => "other",
        })
        .collect()
}

#[test]
fn update_fields_keeps_the_picture_of_an_embed_field() {
    let mut s = Session::new(embed_doc());
    s.run("select.all", &json!({})).unwrap();
    s.run("references.updateFields", &json!({"all": true})).unwrap();
    assert_eq!(kinds(&s), ["start", "image", "end"]);
    crate::cmd::references::update_fields_in(&mut s, true, false).unwrap();
    assert_eq!(kinds(&s), ["start", "image", "end"]);
}
