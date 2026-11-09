//! Run content written inside a text element is read the way Word reads it: a `w:br` or a
//! nested `w:t` inside `w:t` acts as if it stood in the run, in document order, and a
//! `w:instrText` outside any field shows as ordinary text (ECMA-376 §17.3.3.31 gives `t` text
//! content only and §17.3.3.1 puts `br` in a run; Word opens such files and re-saves the
//! markup as sibling `w:t`/`w:br` elements). All markup here is synthetic.

use wordcraft_doc::para::LINE_BREAK;
use wordcraft_doc::{Block, Document};

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

fn docx(body: &str) -> Vec<u8> {
    let doc = format!(r#"<w:document {NS}><w:body>{body}<w:sectPr/></w:body></w:document>"#);
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    crate::fixtures::zip(&[("[Content_Types].xml", types.as_bytes()), ("_rels/.rels", rels.as_bytes()), ("word/document.xml", doc.as_bytes())])
}

/// The body's paragraph texts, in order (tables skipped).
fn texts(d: &Document) -> Vec<String> {
    d.body
        .iter()
        .filter_map(|b| match &**b {
            Block::Para(p) => Some(p.text.clone()),
            Block::Table(_) => None,
        })
        .collect()
}

fn read_texts(body: &str) -> Vec<String> {
    texts(&wordcraft_docx::read(&docx(body)).unwrap())
}

const BR_IN_T: &str = r#"<w:p><w:r><w:t xml:space="preserve">BR3 before<w:br/>BR3 after</w:t></w:r></w:p>"#;

#[test]
fn br_inside_t_is_a_line_break() {
    assert_eq!(read_texts(BR_IN_T), vec![format!("BR3 before{LINE_BREAK}BR3 after")]);
}

#[test]
fn nested_t_elements_keep_their_text() {
    let body = r#"<w:p><w:r><w:t><w:t>BR4 before</w:t><w:br/><w:t>BR4 after</w:t></w:t></w:r></w:p>"#;
    assert_eq!(read_texts(body), vec![format!("BR4 before{LINE_BREAK}BR4 after")]);
}

#[test]
fn instr_text_outside_a_field_is_text() {
    let body = r#"<w:p><w:r><w:instrText xml:space="preserve"> <w:br/> I2 line one <w:br/> I2 line two </w:instrText></w:r></w:p>"#;
    assert_eq!(read_texts(body), vec![format!(" {LINE_BREAK} I2 line one {LINE_BREAK} I2 line two ")]);
}

#[test]
fn instr_text_inside_a_field_stays_hidden() {
    let body = concat!(
        r#"<w:p><w:r><w:t xml:space="preserve">start </w:t></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r>"#,
        r#"<w:r><w:instrText xml:space="preserve"> QUOTE "code before<w:br/>code after" </w:instrText></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>result</w:t></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t xml:space="preserve"> end</w:t></w:r></w:p>"#
    );
    let t = read_texts(body);
    assert_eq!(t.len(), 1);
    assert!(t[0].contains("result"), "{:?}", t[0]);
    assert!(!t[0].contains("code"), "{:?}", t[0]);
    assert!(!t[0].contains(LINE_BREAK), "{:?}", t[0]);
}

#[test]
fn misplaced_break_survives_a_save() {
    let d = wordcraft_docx::read(&docx(BR_IN_T)).unwrap();
    let again = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    assert_eq!(texts(&again), vec![format!("BR3 before{LINE_BREAK}BR3 after")]);
}
