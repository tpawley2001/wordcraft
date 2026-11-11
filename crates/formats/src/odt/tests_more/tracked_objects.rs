//! Pictures and other objects inside tracked changes (ODF 1.3 §5.5.4 `text:deletion` holds the
//! deleted content, pictures included; §5.5.7-8 `text:change-start`/`-end` mark an insertion):
//! a deleted picture stays in the document as deleted content, an inserted one as inserted
//! content, and saving writes each inside its change rather than as live body content.

use crate::odt::tests::zip_of;
use crate::odt::{export, import};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Document, Pos, Revision, RevisionKind};

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0""#;

const FRAME: &str = r#"<draw:frame draw:name="Pic1" text:anchor-type="as-char" svg:width="1in" svg:height="1in"><draw:image xlink:href="Pictures/a.png"/></draw:frame>"#;

fn png() -> Vec<u8> {
    [b"\x89PNG\r\n\x1a\n".as_slice(), &[7u8; 300]].concat()
}

fn odt(regions: &str, body: &str) -> Vec<u8> {
    let tc = if regions.is_empty() {
        String::new()
    } else {
        format!(r#"<text:tracked-changes text:track-changes="true">{regions}</text:tracked-changes>"#)
    };
    let content =
        format!(r#"<office:document-content {NS}><office:body><office:text>{tc}{body}</office:text></office:body></office:document-content>"#);
    zip_of(&[("content.xml", content.as_bytes()), ("Pictures/a.png", &png())])
}

fn region(id: &str, kind: &str, author: &str, date: &str, content: &str) -> String {
    format!(
        r#"<text:changed-region text:id="{id}"><text:{kind}><office:change-info><dc:creator>{author}</dc:creator><dc:date>{date}</dc:date></office:change-info>{content}</text:{kind}></text:changed-region>"#
    )
}

/// Each body paragraph's runs as (text, revision kind and author), deletion first.
fn runs(doc: &Document) -> Vec<Vec<(String, Option<(RevisionKind, String)>)>> {
    let rev = |i: Option<u32>| i.and_then(|i| doc.revisions.get(i as usize)).map(|r| (r.kind, r.author.clone()));
    doc.body
        .iter()
        .filter_map(|b| b.as_para())
        .map(|p| p.run_ranges().map(|(r, cp)| (p.text[r].to_string(), rev(cp.del).or(rev(cp.ins)))).collect())
        .collect()
}

fn objects(doc: &Document) -> usize {
    doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.objects.len()).sum()
}

fn content_xml(bytes: &[u8]) -> String {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("content.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

/// The body of content.xml with its `text:tracked-changes` block cut out.
fn body_outside_regions(xml: &str) -> String {
    match (xml.find("<text:tracked-changes"), xml.find("</text:tracked-changes>")) {
        (Some(a), Some(b)) => format!("{}{}", &xml[..a], &xml[b..]),
        _ => xml.to_string(),
    }
}

fn s(v: &str) -> String {
    v.to_string()
}

fn deleted_picture() -> Vec<u8> {
    let regions = region("d1", "deletion", "Bob", "2024-03-02T11:30:00", &format!("<text:p>old {FRAME}</text:p>"));
    odt(&regions, r#"<text:p>Keep <text:change text:change-id="d1"/>end.</text:p>"#)
}

#[test]
fn a_deleted_picture_is_kept_as_deleted_content() {
    let doc = import(&deleted_picture()).unwrap();
    let del = Some((RevisionKind::Delete, s("Bob")));
    assert_eq!(runs(&doc), vec![vec![(s("Keep "), None), (format!("old {OBJ}"), del), (s("end."), None)]]);
    assert_eq!(objects(&doc), 1, "the deleted picture is in the paragraph");
    assert_eq!(doc.media.len(), 1, "its picture data is kept");
}

#[test]
fn an_inserted_picture_is_inserted_content() {
    let regions = region("i1", "insertion", "Alice", "2024-03-01T10:00:00", "");
    let body = format!(r#"<text:p>A <text:change-start text:change-id="i1"/>{FRAME}<text:change-end text:change-id="i1"/> B</text:p>"#);
    let doc = import(&odt(&regions, &body)).unwrap();
    let ins = Some((RevisionKind::Insert, s("Alice")));
    assert_eq!(runs(&doc), vec![vec![(s("A "), None), (OBJ.to_string(), ins), (s(" B"), None)]]);
}

#[test]
fn a_deleted_picture_is_saved_inside_its_deletion() {
    let doc = import(&deleted_picture()).unwrap();
    let saved = export(&doc).unwrap();
    let xml = content_xml(&saved);
    let region = xml.split("<text:deletion").nth(1).expect("a deletion region").split("</text:deletion>").next().unwrap_or("");
    assert!(region.contains("<draw:frame"), "the picture is in the deleted content: {region}");
    assert!(!body_outside_regions(&xml).contains("<draw:frame"), "no live copy in the body: {xml}");
    let back = import(&saved).unwrap();
    assert_eq!(runs(&back), runs(&doc));
    assert_eq!(back.revisions.len(), doc.revisions.len(), "one deletion stays one region");
    assert_eq!(back.media.len(), 1);
}

#[test]
fn an_inserted_picture_is_saved_inside_its_change_range() {
    let regions = region("i1", "insertion", "Alice", "2024-03-01T10:00:00", "");
    let body = format!(r#"<text:p>A <text:change-start text:change-id="i1"/>{FRAME}<text:change-end text:change-id="i1"/> B</text:p>"#);
    let doc = import(&odt(&regions, &body)).unwrap();
    let xml = content_xml(&export(&doc).unwrap());
    let body = body_outside_regions(&xml);
    let (start, frame, end) = (body.find("<text:change-start"), body.find("<draw:frame"), body.find("<text:change-end"));
    assert!(matches!((start, frame, end), (Some(a), Some(f), Some(e)) if a < f && f < e), "frame between change-start and change-end: {body}");
    let back = import(&export(&doc).unwrap()).unwrap();
    let ins = Some((RevisionKind::Insert, s("Alice")));
    assert_eq!(runs(&back), vec![vec![(s("A "), None), (OBJ.to_string(), ins), (s(" B"), None)]]);
}

#[test]
fn a_picture_deleted_in_another_format_is_not_saved_as_live_content() {
    // As a DOCX `w:del` around a `w:drawing` reads: the picture's U+FFFC carries the deletion.
    let mut doc = import(&odt("", &format!("<text:p>Before {FRAME} after</text:p>"))).unwrap();
    doc.revisions.push(Revision::new(RevisionKind::Delete, "Carol", "2024-05-06T07:08:00Z"));
    let rev = (doc.revisions.len() - 1) as u32;
    let text = doc.body.first().and_then(|b| b.as_para()).map(|p| p.text.clone()).unwrap();
    let at = text.find(OBJ).expect("the picture");
    doc.format_range(&Pos::body(0, at), &Pos::body(0, at + OBJ.len_utf8()), &|c| c.del = Some(rev)).unwrap();
    let saved = export(&doc).unwrap();
    let xml = content_xml(&saved);
    assert!(!body_outside_regions(&xml).contains("<draw:frame"), "the deleted picture isn't live body content: {xml}");
    let back = import(&saved).unwrap();
    let del = Some((RevisionKind::Delete, s("Carol")));
    assert_eq!(runs(&back), vec![vec![(s("Before "), None), (OBJ.to_string(), del), (s(" after"), None)]]);
    assert_eq!(objects(&back), 1);
}

#[test]
fn a_picture_with_no_tracked_change_is_written_as_before() {
    let doc = import(&odt("", &format!("<text:p>Before {FRAME} after</text:p>"))).unwrap();
    let xml = content_xml(&export(&doc).unwrap());
    assert!(xml.contains("<draw:frame"));
    assert!(!xml.contains("text:changed-region") && !xml.contains("text:change"), "no change marks: {xml}");
    assert_eq!(runs(&import(&export(&doc).unwrap()).unwrap()), vec![vec![(format!("Before {OBJ} after"), None)]]);
}
