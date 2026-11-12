//! A field code over several paragraphs: the bounds, nested fields, marks inside the code and
//! edits before saving.

use std::io::{Read, Write};
use std::sync::Arc;

use wordcraft_doc::props::Align;
use wordcraft_doc::{Block, Document, InlineObject, Paragraph};

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

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

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

fn read(body: &[String]) -> Document {
    crate::read(&package(&body.concat())).unwrap()
}

fn paras(doc: &Document) -> Vec<&Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

fn texts(doc: &Document) -> Vec<String> {
    paras(doc).iter().map(|p| p.plain_text()).collect()
}

fn fields(p: &Paragraph) -> Vec<(String, String)> {
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Field { instr, result, .. } => Some((instr.clone(), result.clone())),
            _ => None,
        })
        .collect()
}

/// `AAA ` + begin + `QUOTE "x`, then `inner` paragraphs of code `y`, then `z" ` + separate +
/// `RES` + end + ` tail`, then `NEXT`.
fn with_inner(inner: usize) -> Vec<String> {
    let mut body = vec![para("", &[text("AAA "), mark("begin"), code(r#"QUOTE "x"#)])];
    body.extend((0..inner).map(|_| para("", &[code("y")])));
    body.push(para("", &[code(r#"z" "#), mark("separate"), text("RES"), mark("end"), text(" tail")]));
    body.push(para("", &[text("NEXT")]));
    body
}

#[test]
fn a_code_completed_within_64_paragraphs_joins_and_one_further_does_not() {
    // 63 paragraphs inside the code and the last one: 64 following paragraphs.
    let doc = read(&with_inner(63));
    assert_eq!(texts(&doc), ["AAA RES tail", "NEXT"]);
    let want = format!(r#"QUOTE "x {}z""#, "y ".repeat(63));
    assert_eq!(fields(paras(&doc)[0]), [(want.clone(), "RES".to_string())]);
    let saved = crate::write(&doc).unwrap();
    assert_eq!(paragraphs(&document_xml(&saved)).len(), 66);
    let again = crate::read(&saved).unwrap();
    assert_eq!(fields(paras(&again)[0]), [(want, "RES".to_string())]);

    // 65 following paragraphs: the field ends at the first paragraph end, as before.
    let doc = read(&with_inner(64));
    assert_eq!(paras(&doc).len(), 67);
    assert!(paras(&doc)[0].code_paras.is_none());
    assert!(paras(&doc)[65].plain_text().ends_with(" tail"));
}

#[test]
fn a_field_nested_in_the_code_joins_the_code() {
    let doc = read(&[
        para("", &[mark("begin"), code("IF "), mark("begin"), code("PAGE"), mark("separate"), text("3"), mark("end"), code(r#" = 3 "a""#)]),
        para("", &[code(r#""b" "#), mark("separate"), text("a"), mark("end")]),
    ]);
    assert_eq!(texts(&doc), ["a"]);
    assert_eq!(fields(paras(&doc)[0]), [(r#"IF 3 = 3 "a" "b""#.to_string(), "a".to_string())]);
    let again = crate::read(&crate::write(&doc).unwrap()).unwrap();
    assert_eq!(fields(paras(&again)[0]), [(r#"IF 3 = 3 "a" "b""#.to_string(), "a".to_string())]);
}

#[test]
fn a_paragraph_mark_inside_a_nested_fields_code_is_not_joined() {
    let doc = read(&[
        para("", &[mark("begin"), code("IF "), mark("begin"), code("PAGE")]),
        para("", &[mark("separate"), text("3"), mark("end"), code(r#" = 3 "a" "#), mark("separate"), text("a"), mark("end")]),
    ]);
    assert_eq!(paras(&doc).len(), 2);
    assert!(paras(&doc).iter().all(|p| p.code_paras.is_none()));
}

#[test]
fn bookmarks_and_proofing_marks_inside_a_code_paragraph_join() {
    let doc = read(&[
        para("", &[text("AAA "), mark("begin"), code(r#"QUOTE "x"#)]),
        para(
            "",
            &[
                r#"<w:bookmarkStart w:id="1" w:name="Mid"/><w:proofErr w:type="spellStart"/>"#.to_string(),
                code("y"),
                r#"<w:proofErr w:type="spellEnd"/><w:bookmarkEnd w:id="1"/><w:r/>"#.to_string(),
            ],
        ),
        para("", &[code(r#"z" "#), mark("separate"), text("RES"), mark("end")]),
    ]);
    assert_eq!(texts(&doc), ["AAA RES"]);
    let p = paras(&doc)[0];
    assert_eq!(fields(p), [(r#"QUOTE "x y z""#.to_string(), "RES".to_string())]);
    assert!(p.objects.iter().any(|o| matches!(o, InlineObject::BookmarkStart { name } if name == "Mid")));
    let xml = document_xml(&crate::write(&doc).unwrap());
    assert_eq!(paragraphs(&xml).len(), 3, "{xml}");
    assert!(xml.contains("Mid"), "{xml}");
}

#[test]
fn text_in_a_paragraph_inside_the_code_is_not_joined() {
    let doc = read(&[
        para("", &[text("AAA "), mark("begin"), code(r#"QUOTE "x"#)]),
        para("", &[text("Shown.")]),
        para("", &[code(r#"z" "#), mark("separate"), text("RES"), mark("end")]),
    ]);
    assert_eq!(paras(&doc).len(), 3);
    assert_eq!(paras(&doc)[1].plain_text(), "Shown.");
}

#[test]
fn a_section_break_in_the_code_is_not_joined() {
    let doc = read(&[
        para("", &[text("AAA "), mark("begin"), code(r#"QUOTE "x"#)]),
        para("<w:sectPr/>", &[code("y")]),
        para("", &[code(r#"z" "#), mark("separate"), text("RES"), mark("end")]),
    ]);
    assert_eq!(paras(&doc).len(), 3);
}

fn edited(doc: &mut Document, f: impl FnOnce(&mut Paragraph)) {
    let Some(b) = doc.body.first_mut() else { panic!("no block") };
    let Block::Para(p) = Arc::make_mut(b) else { panic!("not a paragraph") };
    f(p);
}

#[test]
fn an_edited_alignment_goes_on_the_first_paragraph() {
    let right = r#"<w:jc w:val="right"/>"#;
    let mut doc = read(&[
        para("", &[text("AAA "), mark("begin"), code(r#"QUOTE "x"#)]),
        para(right, &[code("y")]),
        para(r#"<w:spacing w:after="240"/>"#, &[code(r#"z" "#), mark("separate"), text("RES"), mark("end")]),
    ]);
    assert_eq!(paras(&doc)[0].props.space_after, Some(12.0));
    edited(&mut doc, |p| {
        p.props.align = Some(Align::Center);
        p.props.space_after = Some(6.0);
    });
    let saved = crate::write(&doc).unwrap();
    let xml = document_xml(&saved);
    let ps = paragraphs(&xml);
    assert_eq!(ps.len(), 3, "{xml}");
    assert!(ps[0].contains(r#"<w:jc w:val="center"/>"#) && !ps[0].contains("w:after"), "{xml}");
    assert!(ps[1].contains(r#"<w:jc w:val="right"/>"#), "{xml}");
    assert!(ps[2].contains(r#"w:after="120""#) && !ps[2].contains("w:jc"), "{xml}");
    let again = crate::read(&saved).unwrap();
    assert_eq!(paras(&again)[0].props.align, Some(Align::Center));
    assert_eq!(paras(&again)[0].props.space_after, Some(6.0));
}

#[test]
fn an_edited_code_is_saved_in_one_paragraph() {
    let mut doc = read(&with_inner(1));
    edited(&mut doc, |p| {
        for o in &mut p.objects {
            if let InlineObject::Field { instr, .. } = o {
                *instr = "PAGE".into();
            }
        }
    });
    let xml = document_xml(&crate::write(&doc).unwrap());
    assert_eq!(paragraphs(&xml).len(), 2, "{xml}");
    assert!(!xml.contains("w:vanish"), "{xml}");

    // A deleted field: the paragraph is written as one.
    let mut doc = read(&with_inner(1));
    edited(&mut doc, |p| {
        let _ = p.delete(0, p.len());
    });
    assert_eq!(paragraphs(&document_xml(&crate::write(&doc).unwrap())).len(), 2);
}
