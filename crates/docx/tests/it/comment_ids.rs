//! Comments whose ids repeat, and comment ranges whose id names no comment, save as a package whose comment
//! markers are consistent: each comment has a unique id, one reference mark and at most one range
//! (ECMA-376 Part 1 §17.13.4). Synthetic fixture.

use std::collections::BTreeMap;
use std::io::Read;

use crate::fixtures::zip;

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const ROOT_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const DOC_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#;
const TEXTS: [&str; 4] = ["First zero", "Second zero", "One", "Three"];

fn text_run(t: &str) -> String {
    format!(r#"<w:r><w:t xml:space="preserve">{t}</w:t></w:r>"#)
}

fn reference(id: u32) -> String {
    format!(r#"<w:r><w:commentReference w:id="{id}"/></w:r>"#)
}

fn start(id: u32) -> String {
    format!(r#"<w:commentRangeStart w:id="{id}"/>"#)
}

fn end(id: u32) -> String {
    format!(r#"<w:commentRangeEnd w:id="{id}"/>"#)
}

fn package() -> Vec<u8> {
    let body = [
        text_run("A "),
        reference(0),
        start(2),
        text_run("B "),
        end(2),
        reference(0),
        start(1),
        text_run("C "),
        end(1),
        reference(1),
        start(3),
        text_run("D"),
        end(3),
        reference(3),
    ]
    .concat();
    let doc = format!(r#"<w:document {W_NS}><w:body><w:p>{body}</w:p></w:body></w:document>"#);
    let comment =
        |id: u32, t: &str| format!(r#"<w:comment w:id="{id}" w:author="Ann" w:initials="A"><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:comment>"#);
    let comments = format!(
        r#"<w:comments {W_NS}>{}{}{}{}</w:comments>"#,
        comment(0, TEXTS[0]),
        comment(0, TEXTS[1]),
        comment(1, TEXTS[2]),
        comment(3, TEXTS[3])
    );
    zip(&[
        ("_rels/.rels", ROOT_RELS.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/_rels/document.xml.rels", DOC_RELS.as_bytes()),
        ("word/comments.xml", comments.as_bytes()),
    ])
}

fn unzip(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        out.insert(f.name().to_string(), s);
    }
    out
}

fn save(bytes: &[u8]) -> Vec<u8> {
    wordcraft_docx::write(&wordcraft_docx::read(bytes).expect("read")).expect("write")
}

/// The `w:id` values of every `<w:{tag} ` element in `xml`, in order.
fn ids(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<w:{tag} ");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(at) = rest.find(&open) {
        rest = &rest[at + open.len()..];
        let el = &rest[..rest.find('>').unwrap()];
        let key = "w:id=\"";
        let a = el.find(key).unwrap_or_else(|| panic!("<w:{tag} without w:id: {el}")) + key.len();
        out.push(el[a..a + el[a..].find('"').unwrap()].to_string());
    }
    out
}

fn check(files: &BTreeMap<String, String>) {
    let comments = &files["word/comments.xml"];
    let written = ids(comments, "comment");
    assert_eq!(written.len(), 4, "every comment is kept: {comments}");
    for t in TEXTS {
        assert!(comments.contains(t), "comment text {t:?} lost: {comments}");
    }
    let mut unique = written.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), written.len(), "comment ids repeat: {written:?}");
    let doc = &files["word/document.xml"];
    let (starts, ends, refs) = (ids(doc, "commentRangeStart"), ids(doc, "commentRangeEnd"), ids(doc, "commentReference"));
    for id in starts.iter().chain(&ends).chain(&refs) {
        assert!(written.contains(id), "marker for id {id} names no comment ({written:?}): {doc}");
    }
    for id in &written {
        let n = |v: &Vec<String>| v.iter().filter(|x| *x == id).count();
        assert_eq!(n(&refs), 1, "comment {id}: reference marks in {doc}");
        assert!(n(&starts) <= 1 && n(&starts) == n(&ends), "comment {id}: range start/end counts in {doc}");
        if n(&starts) == 1 {
            let s = doc.find(&format!(r#"<w:commentRangeStart w:id="{id}""#)).unwrap();
            let e = doc.find(&format!(r#"<w:commentRangeEnd w:id="{id}""#)).unwrap();
            assert!(s < e, "comment {id}: range ends before it starts in {doc}");
        }
    }
    let plain: String = doc.split("<w:t").skip(1).filter_map(|s| s.split_once('>').map(|(_, r)| r.split('<').next().unwrap())).collect();
    assert_eq!(plain, "A B C D", "body text");
}

#[test]
fn repeated_comment_ids_and_orphan_ranges_save_consistent_markers() {
    check(&unzip(&save(&package())));
}

#[test]
fn the_markers_stay_consistent_on_a_second_save() {
    check(&unzip(&save(&save(&package()))));
}
