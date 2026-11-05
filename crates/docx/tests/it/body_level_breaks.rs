//! A `w:br` written straight into a story container (between paragraphs, not in a run) is read
//! the way Word reads it: as a break at the start of the next paragraph (ECMA-376 §17.3.3.1 puts
//! `br` in a run; Word re-saves such a file with the break moved into the next paragraph's first
//! run). All markup here is synthetic.

use wordcraft_doc::para::{COLUMN_BREAK, LINE_BREAK, PAGE_BREAK};
use wordcraft_doc::{Block, Document};

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

fn p(text: &str) -> String {
    format!(r#"<w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#)
}

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

#[test]
fn page_break_in_body_starts_the_next_paragraph() {
    let body = format!(r#"{}<w:br w:type="page"/>{}"#, p("before"), p("after"));
    assert_eq!(read_texts(&body), vec!["before".to_string(), format!("{PAGE_BREAK}after")]);
}

#[test]
fn text_wrapping_break_in_body_is_a_line_break() {
    let body = format!(r#"{}<w:br/>{}"#, p("before"), p("after"));
    assert_eq!(read_texts(&body), vec!["before".to_string(), format!("{LINE_BREAK}after")]);
    let body = format!(r#"{}<w:br w:type="textWrapping" w:clear="all"/>{}"#, p("before"), p("after"));
    assert_eq!(read_texts(&body), vec!["before".to_string(), format!("{LINE_BREAK}after")]);
}

#[test]
fn column_break_and_several_breaks_keep_their_order() {
    let body = format!(r#"{}<w:br w:type="column"/><w:br w:type="page"/>{}"#, p("before"), p("after"));
    assert_eq!(read_texts(&body), vec!["before".to_string(), format!("{COLUMN_BREAK}{PAGE_BREAK}after")]);
}

#[test]
fn break_with_no_following_paragraph_is_dropped() {
    let body = format!(r#"{}<w:br w:type="page"/>"#, p("only"));
    assert_eq!(read_texts(&body), vec!["only".to_string()]);
}

#[test]
fn body_break_survives_a_save_as_a_run_break() {
    let body = format!(r#"{}<w:br w:type="page"/>{}"#, p("before"), p("after"));
    let d = wordcraft_docx::read(&docx(&body)).unwrap();
    let again = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    assert_eq!(texts(&again), vec!["before".to_string(), format!("{PAGE_BREAK}after")]);
}
