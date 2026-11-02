//! Pictures linked to a file that isn't in the document (`Document::linked_media`): saved as
//! `a:blip r:link` with an external relationship (ECMA-376 §20.1.8.13), never as an image part.
use std::io::Read;

use wordcraft_doc::{Document, InlineObject, Pos};

const PATH: &str = "c:\\web\\title.gif";

fn linked_doc() -> Document {
    let mut doc = Document::new();
    doc.linked_media.insert("link1".into(), PATH.into());
    let pic =
        InlineObject::Image { media: "link1".into(), w: 225.0, h: 18.75, alt: String::new(), float: Default::default(), crop: [0.0; 4], ole: None };
    doc.insert_object(&Pos::default(), pic, &Default::default()).unwrap();
    doc
}

fn parts(bytes: &[u8]) -> Vec<(String, String)> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut f = zip.by_index(i).unwrap();
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            (f.name().to_string(), s)
        })
        .collect()
}

fn images(doc: &Document) -> Vec<(String, f32, f32)> {
    let p = doc.body.first().and_then(|b| b.as_para()).unwrap();
    p.objects.iter().filter_map(|o| if let InlineObject::Image { media, w, h, .. } = o { Some((media.clone(), *w, *h)) } else { None }).collect()
}

#[test]
fn a_linked_picture_saves_as_an_external_link_without_an_image_part() {
    let bytes = wordcraft_docx::write(&linked_doc()).unwrap();
    let parts = parts(&bytes);
    assert!(parts.iter().all(|(n, _)| !n.starts_with("word/media/")), "no image part: {:?}", parts.iter().map(|p| &p.0).collect::<Vec<_>>());
    let (_, doc_xml) = parts.iter().find(|(n, _)| n == "word/document.xml").unwrap();
    let (_, rels) = parts.iter().find(|(n, _)| n == "word/_rels/document.xml.rels").unwrap();
    let rid = doc_xml.split("<a:blip r:link=\"").nth(1).and_then(|s| s.split('"').next()).expect("a:blip r:link");
    assert!(!doc_xml.contains("r:embed"), "no embedded picture");
    let rel = rels.split("<Relationship ").find(|r| r.contains(&format!("Id=\"{rid}\""))).expect("the link's relationship");
    assert!(rel.contains("TargetMode=\"External\""), "{rel}");
    assert!(rel.contains("Target=\"file:///c:\\web\\title.gif\""), "{rel}");
    assert!(rel.contains("/image\""), "{rel}");
}

#[test]
fn a_linked_picture_round_trips_as_a_link_at_its_size() {
    let back = wordcraft_docx::read(&wordcraft_docx::write(&linked_doc()).unwrap()).unwrap();
    let pics = images(&back);
    assert_eq!(pics.len(), 1);
    let (key, w, h) = &pics[0];
    assert_eq!((*w, *h), (225.0, 18.75));
    assert!(back.media.is_empty(), "nothing fetched or embedded");
    assert_eq!(back.linked_media.get(key).map(String::as_str), Some("file:///c:\\web\\title.gif"));
    // Saved again, it is the same link.
    let again = parts(&wordcraft_docx::write(&back).unwrap());
    assert!(again.iter().all(|(n, _)| !n.starts_with("word/media/")));
    let (_, rels) = again.iter().find(|(n, _)| n == "word/_rels/document.xml.rels").unwrap();
    assert_eq!(rels.matches("file:///c:\\web\\title.gif").count(), 1);
}
