//! An object inserted, then deleted (ODF 1.3 §5.5.4 deleted content holding a §5.5.7-8 change
//! range around a `draw:frame`) keeps both marks, through a save; RTF is unchanged by them.

use crate::odt::tests::zip_of;
use crate::odt::{export, import};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Document, RevisionKind};

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0""#;

fn region(id: &str, kind: &str, author: &str, content: &str) -> String {
    format!(
        r#"<text:changed-region text:id="{id}"><text:{kind}><office:change-info><dc:creator>{author}</dc:creator><dc:date>2024-03-01T10:00:00</dc:date></office:change-info>{content}</text:{kind}></text:changed-region>"#
    )
}

fn doc() -> Document {
    let frame = r#"<draw:frame draw:name="P" text:anchor-type="as-char" svg:width="1in" svg:height="1in"><draw:image xlink:href="Pictures/a.png"/></draw:frame>"#;
    let deleted = format!(r#"<text:p>x<text:change-start text:change-id="i1"/>{frame}<text:change-end text:change-id="i1"/></text:p>"#);
    let regions = region("i1", "insertion", "Alice", "") + &region("d1", "deletion", "Bob", &deleted);
    let content = format!(
        r#"<office:document-content {NS}><office:body><office:text><text:tracked-changes>{regions}</text:tracked-changes><text:p>Keep <text:change text:change-id="d1"/>end.</text:p></office:text></office:body></office:document-content>"#
    );
    let png = [b"\x89PNG\r\n\x1a\n".as_slice(), &[7u8; 300]].concat();
    import(&zip_of(&[("content.xml", content.as_bytes()), ("Pictures/a.png", &png)])).unwrap()
}

/// The object run's (insertion, deletion) authors.
fn picture_marks(doc: &Document) -> (Option<String>, Option<String>) {
    let author = |i: Option<u32>, k: RevisionKind| i.and_then(|i| doc.revisions.get(i as usize)).filter(|r| r.kind == k).map(|r| r.author.clone());
    let p = doc.body.first().and_then(|b| b.as_para()).unwrap();
    let (_, cp) = p.run_ranges().find(|(r, _)| p.text[r.clone()].contains(OBJ)).expect("the object's run");
    (author(cp.ins, RevisionKind::Insert), author(cp.del, RevisionKind::Delete))
}

#[test]
fn an_inserted_then_deleted_picture_keeps_both_marks() {
    let doc = doc();
    let want = (Some("Alice".to_string()), Some("Bob".to_string()));
    assert_eq!(picture_marks(&doc), want);
    let back = import(&export(&doc).unwrap()).unwrap();
    assert_eq!(picture_marks(&back), want, "both marks survive a save");
    assert_eq!(back.revisions.len(), doc.revisions.len());
}

#[test]
fn rtf_writes_no_deleted_picture() {
    // RTF's flow asks for no object marks: a deleted picture stays out, as before.
    let rtf = crate::rtf::export(&doc());
    assert!(!rtf.contains("\\pict"), "{rtf}");
}

#[test]
fn a_deleted_shape_is_kept_as_deleted_content() {
    let rect = r#"<draw:rect draw:name="R" text:anchor-type="as-char" svg:width="1in" svg:height="1in"/>"#;
    let regions = region("d1", "deletion", "Bob", &format!("<text:p>old {rect}</text:p>"));
    let content = format!(
        r#"<office:document-content {NS}><office:body><office:text><text:tracked-changes>{regions}</text:tracked-changes><text:p>Keep <text:change text:change-id="d1"/>end.</text:p></office:text></office:body></office:document-content>"#
    );
    let doc = import(&zip_of(&[("content.xml", content.as_bytes())])).unwrap();
    assert_eq!(picture_marks(&doc), (None, Some("Bob".to_string())), "the shape is deleted content");
    let back = import(&export(&doc).unwrap()).unwrap();
    assert_eq!(picture_marks(&back), (None, Some("Bob".to_string())), "and stays so through a save");
    let objects = |d: &Document| d.body.iter().filter_map(|b| b.as_para()).map(|p| p.objects.len()).sum::<usize>();
    assert_eq!(objects(&back), 1);
}
