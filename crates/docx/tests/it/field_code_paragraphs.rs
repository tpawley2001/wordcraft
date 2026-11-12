//! A complex field whose CODE runs over several paragraphs (`begin` and some `instrText` in one
//! `w:p`, more `instrText` in the next, `separate`, the result and `end` in a later one). Word
//! treats the paragraph marks inside the code as part of the hidden code: it shows ONE paragraph,
//! laid out with the first paragraph's properties and the last paragraph's space after, and
//! counts one paragraph. The file is saved with the `w:p` sequence it was read from. The expected
//! values were measured in Word; the packages are original synthetic fixtures.

use std::io::{Read, Write};

use wordcraft_doc::props::Align;
use wordcraft_doc::{Document, InlineObject, Paragraph};

fn package(body: &str) -> Vec<u8> {
    let xml = format!(r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, value) in [
        ("word/document.xml", xml.as_str()),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="main" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(value.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// `word/document.xml` of a written package.
fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

/// The `w:p` elements of the body of a written `word/document.xml`, in order (the fixtures have
/// no nested paragraphs).
fn paragraphs(xml: &str) -> Vec<String> {
    let body = xml.split_once("<w:body>").map_or(xml, |(_, b)| b);
    body.split("</w:p>").filter(|p| p.contains("<w:p>") || p.contains("<w:p ")).map(str::to_string).collect()
}

fn text(s: &str) -> String {
    format!(r#"<w:r><w:t xml:space="preserve">{s}</w:t></w:r>"#)
}

fn code(s: &str) -> String {
    format!(r#"<w:r><w:instrText xml:space="preserve">{s}</w:instrText></w:r>"#)
}

fn mark(kind: &str) -> String {
    format!(r#"<w:r><w:fldChar w:fldCharType="{kind}"/></w:r>"#)
}

fn para(props: &str, content: &[String]) -> String {
    let ppr = if props.is_empty() { String::new() } else { format!("<w:pPr>{props}</w:pPr>") };
    format!("<w:p>{ppr}{}</w:p>", content.concat())
}

/// `BEFORE` / `AAA ` + begin + ` QUOTE "x` / `y` / `z" ` + separate + `RES` + end + ` tail` / `NEXT`,
/// with the three field paragraphs' `w:pPr` contents.
fn three(p1: &str, p2: &str, p3: &str) -> Vec<u8> {
    package(
        &[
            para("", &[text("BEFORE")]),
            para(p1, &[text("AAA "), mark("begin"), code(r#" QUOTE "x"#)]),
            para(p2, &[code("y")]),
            para(p3, &[code(r#"z" "#), mark("separate"), text("RES"), mark("end"), text(" tail")]),
            para("", &[text("NEXT")]),
        ]
        .concat(),
    )
}

/// `BEFORE` / `AAA ` + begin + ` QUOTE "x` / `z" ` + separate + `RES` + end + ` tail` / `NEXT`.
fn two(p1: &str, p2: &str) -> Vec<u8> {
    package(
        &[
            para("", &[text("BEFORE")]),
            para(p1, &[text("AAA "), mark("begin"), code(r#" QUOTE "x"#)]),
            para(p2, &[code(r#"z" "#), mark("separate"), text("RES"), mark("end"), text(" tail")]),
            para("", &[text("NEXT")]),
        ]
        .concat(),
    )
}

fn body_paras(doc: &Document) -> Vec<&Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

fn texts(doc: &Document) -> Vec<String> {
    body_paras(doc).iter().map(|p| p.plain_text()).collect()
}

/// The fields of a paragraph as (code, result).
fn fields(p: &Paragraph) -> Vec<(String, String)> {
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Field { instr, result, .. } => Some((instr.clone(), result.clone())),
            _ => None,
        })
        .collect()
}

const CENTER: &str = r#"<w:jc w:val="center"/>"#;
const RIGHT: &str = r#"<w:jc w:val="right"/>"#;
const LEFT: &str = r#"<w:jc w:val="left"/>"#;
const SPACED: &str = r#"<w:spacing w:before="600" w:after="600"/><w:ind w:left="2880"/>"#;

#[test]
fn a_code_over_three_paragraphs_is_one_paragraph_with_the_first_paragraphs_alignment() {
    // Word: `BEFORE¶AAA RES tail¶NEXT¶`, 3 paragraphs, 1 field, the line centred.
    let doc = wordcraft_docx::read(&three(CENTER, RIGHT, LEFT)).unwrap();
    assert_eq!(texts(&doc), ["BEFORE", "AAA RES tail", "NEXT"]);
    let joined = body_paras(&doc)[1];
    assert_eq!(joined.props.align, Some(Align::Center));
    assert_eq!(fields(joined), [(r#"QUOTE "x y z""#.to_string(), "RES".to_string())]);

    // Word: the same file with the first paragraph left and the last centred is left aligned.
    let doc = wordcraft_docx::read(&three(LEFT, RIGHT, CENTER)).unwrap();
    assert_eq!(texts(&doc), ["BEFORE", "AAA RES tail", "NEXT"]);
    assert_eq!(body_paras(&doc)[1].props.align, Some(Align::Left));
}

#[test]
fn indent_and_space_before_come_from_the_first_paragraph_and_space_after_from_the_last() {
    // Word: indented 2 in, 30 pt above the line, nothing below it.
    let doc = wordcraft_docx::read(&two(SPACED, "")).unwrap();
    assert_eq!(texts(&doc), ["BEFORE", "AAA RES tail", "NEXT"]);
    let p = &body_paras(&doc)[1].props;
    assert_eq!(p.indent_left, Some(144.0));
    assert_eq!(p.space_before, Some(30.0));
    assert_eq!(p.space_after.unwrap_or(0.0), 0.0);

    // Word: at the left margin, nothing above the line, 30 pt below it.
    let doc = wordcraft_docx::read(&two("", SPACED)).unwrap();
    assert_eq!(texts(&doc), ["BEFORE", "AAA RES tail", "NEXT"]);
    let p = &body_paras(&doc)[1].props;
    assert_eq!(p.indent_left.unwrap_or(0.0), 0.0);
    assert_eq!(p.space_before.unwrap_or(0.0), 0.0);
    assert_eq!(p.space_after, Some(30.0));
}

#[test]
fn a_paragraph_wholly_inside_the_code_adds_no_page_break_and_no_spacing() {
    // Word: `{It is stale tail` on one line of page 1; the stored result is shown.
    let middle = r#"<w:pageBreakBefore/><w:spacing w:before="600" w:after="600"/>"#;
    let doc = wordcraft_docx::read(&package(
        &[
            para("", &[text("BEFORE")]),
            para("", &[text("{It is "), mark("begin"), code(r#"IF 1 = 1 "yes" "#)]),
            para(middle, &[code(r#""no" "#)]),
            para("", &[code(r"\* MERGEFORMAT "), mark("separate"), text("stale"), mark("end"), text(" tail")]),
            para("", &[text("NEXT")]),
        ]
        .concat(),
    ))
    .unwrap();
    assert_eq!(texts(&doc), ["BEFORE", "{It is stale tail", "NEXT"]);
    let joined = body_paras(&doc)[1];
    assert_ne!(joined.props.page_break_before, Some(true));
    assert_eq!(joined.props.space_before.unwrap_or(0.0), 0.0);
    assert_eq!(joined.props.space_after.unwrap_or(0.0), 0.0);
    assert_eq!(fields(joined), [(r#"IF 1 = 1 "yes" "no" \* MERGEFORMAT"#.to_string(), "stale".to_string())]);
}

#[test]
fn a_last_paragraph_that_starts_with_the_separator_joins_too() {
    // Word: one line, right aligned (the first paragraph's alignment).
    let doc = wordcraft_docx::read(&package(
        &[
            para("", &[text("BEFORE")]),
            para(RIGHT, &[text("AAA "), mark("begin"), code(r#" QUOTE "x" "#)]),
            para(CENTER, &[mark("separate"), text("RES"), mark("end"), text(" tail")]),
            para("", &[text("NEXT")]),
        ]
        .concat(),
    ))
    .unwrap();
    assert_eq!(texts(&doc), ["BEFORE", "AAA RES tail", "NEXT"]);
    let joined = body_paras(&doc)[1];
    assert_eq!(joined.props.align, Some(Align::Right));
    assert_eq!(fields(joined), [(r#"QUOTE "x""#.to_string(), "RES".to_string())]);
}

#[test]
fn the_file_is_saved_with_the_paragraphs_it_was_read_from() {
    let doc = wordcraft_docx::read(&three(CENTER, RIGHT, LEFT)).unwrap();
    let saved = wordcraft_docx::write(&doc).unwrap();
    let xml = document_xml(&saved);
    let ps = paragraphs(&xml);
    assert_eq!(ps.len(), 5, "{xml}");
    assert!(!xml.contains("w:vanish"), "{xml}");
    assert_eq!(xml.matches(r#"w:fldCharType="begin""#).count(), 1, "{xml}");
    assert_eq!(xml.matches(r#"w:fldCharType="separate""#).count(), 1, "{xml}");
    assert_eq!(xml.matches(r#"w:fldCharType="end""#).count(), 1, "{xml}");

    // The first paragraph: its own alignment, the text, `begin` and the first part of the code.
    assert!(ps[1].contains(r#"<w:jc w:val="center"/>"#), "{xml}");
    assert!(ps[1].contains("AAA "), "{xml}");
    assert!(ps[1].contains(r#"w:fldCharType="begin""#), "{xml}");
    assert!(ps[1].contains("QUOTE &quot;x") || ps[1].contains(r#"QUOTE "x"#), "{xml}");
    assert!(!ps[1].contains("RES"), "{xml}");

    // The paragraph inside the code: its own alignment and nothing but code.
    assert!(ps[2].contains(r#"<w:jc w:val="right"/>"#), "{xml}");
    assert!(ps[2].contains("<w:instrText"), "{xml}");
    assert!(!ps[2].contains("<w:t>") && !ps[2].contains("<w:t "), "{xml}");
    assert!(!ps[2].contains("w:fldCharType"), "{xml}");

    // The last paragraph: the rest of the code, `separate`, the result, `end` and the tail.
    assert!(!ps[3].contains(r#"<w:jc w:val="center"/>"#) && !ps[3].contains(r#"<w:jc w:val="right"/>"#), "{xml}");
    assert!(ps[3].contains("<w:instrText"), "{xml}");
    let sep = ps[3].find(r#"w:fldCharType="separate""#).expect("separate in the last paragraph");
    let res = ps[3].find("RES").expect("the result in the last paragraph");
    let end = ps[3].find(r#"w:fldCharType="end""#).expect("end in the last paragraph");
    let tail = ps[3].find(" tail").expect("the tail in the last paragraph");
    assert!(sep < res && res < end && end < tail, "{xml}");

    // Reading the saved file gives the same paragraphs, properties and field again.
    let again = wordcraft_docx::read(&saved).unwrap();
    assert_eq!(texts(&again), ["BEFORE", "AAA RES tail", "NEXT"]);
    let joined = body_paras(&again)[1];
    assert_eq!(joined.props.align, Some(Align::Center));
    assert_eq!(fields(joined), [(r#"QUOTE "x y z""#.to_string(), "RES".to_string())]);
}

#[test]
fn the_last_paragraphs_space_after_is_saved_on_the_last_paragraph() {
    let doc = wordcraft_docx::read(&two("", SPACED)).unwrap();
    let xml = document_xml(&wordcraft_docx::write(&doc).unwrap());
    let ps = paragraphs(&xml);
    assert_eq!(ps.len(), 4, "{xml}");
    assert!(!ps[1].contains("w:ind") && !ps[1].contains(r#"w:after="600""#), "{xml}");
    assert!(ps[2].contains(r#"w:left="2880""#), "{xml}");
    assert!(ps[2].contains(r#"w:before="600""#) && ps[2].contains(r#"w:after="600""#), "{xml}");
}

#[test]
fn a_field_that_is_never_finished_leaves_the_following_paragraphs_alone() {
    let doc = wordcraft_docx::read(&package(
        &[para("", &[text("AAA "), mark("begin"), code(r#" QUOTE "x"#)]), para(CENTER, &[text("Second.")]), para("", &[text("Third.")])].concat(),
    ))
    .unwrap();
    assert_eq!(texts(&doc).len(), 3);
    assert_eq!(body_paras(&doc)[1].plain_text(), "Second.");
    assert_eq!(body_paras(&doc)[1].props.align, Some(Align::Center));
    assert_eq!(body_paras(&doc)[2].plain_text(), "Third.");
}

#[test]
fn a_table_between_the_parts_of_the_code_is_not_joined() {
    let table = format!("<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl>", para("", &[text("Cell.")]));
    let doc = wordcraft_docx::read(&package(
        &[
            para("", &[text("AAA "), mark("begin"), code(r#" QUOTE "x"#)]),
            table,
            para("", &[code(r#"z" "#), mark("separate"), text("RES"), mark("end"), text(" tail")]),
        ]
        .concat(),
    ))
    .unwrap();
    assert_eq!(doc.body.len(), 3);
    assert!(doc.body[1].as_para().is_none());
    assert!(body_paras(&doc)[1].plain_text().ends_with(" tail"));
}

#[test]
fn a_field_in_one_paragraph_is_untouched() {
    let doc = wordcraft_docx::read(&package(
        &[
            para("", &[text("BEFORE")]),
            para(CENTER, &[text("AAA "), mark("begin"), code(r#" QUOTE "xyz" "#), mark("separate"), text("RES"), mark("end"), text(" tail")]),
            para("", &[text("NEXT")]),
        ]
        .concat(),
    ))
    .unwrap();
    assert_eq!(texts(&doc), ["BEFORE", "AAA RES tail", "NEXT"]);
    assert_eq!(fields(body_paras(&doc)[1]), [(r#"QUOTE "xyz""#.to_string(), "RES".to_string())]);
    let xml = document_xml(&wordcraft_docx::write(&doc).unwrap());
    assert_eq!(paragraphs(&xml).len(), 3, "{xml}");
}
