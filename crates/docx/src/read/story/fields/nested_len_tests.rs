//! A nested field's stored text in the outer code: where it starts and how long it is, so output
//! can replace exactly that text.

use std::io::Write;

use wordcraft_doc::{Document, InlineObject};

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

fn text(s: &str) -> String {
    format!(r#"<w:r><w:t xml:space="preserve">{s}</w:t></w:r>"#)
}

fn code(s: &str) -> String {
    format!(r#"<w:r><w:instrText xml:space="preserve">{s}</w:instrText></w:r>"#)
}

fn mark(kind: &str) -> String {
    format!(r#"<w:r><w:fldChar w:fldCharType="{kind}"/></w:r>"#)
}

fn read(content: &[String]) -> Document {
    crate::read(&package(&format!("<w:p>{}</w:p>", content.concat()))).unwrap()
}

/// The outer field's code and each nested field's stored text in it.
fn nested_texts(doc: &Document) -> (String, Vec<String>) {
    let p = doc.body.iter().find_map(|b| b.as_para()).unwrap();
    let InlineObject::Field { instr, code_paras, .. } = &p.objects[0] else { panic!("expected a field") };
    let code = code_paras.as_ref().unwrap();
    assert_eq!(code.nested_len.len(), code.nested.len());
    let texts = code.nested.iter().zip(&code.nested_len).map(|(n, len)| instr[n.at..n.at + len].to_string()).collect();
    (instr.clone(), texts)
}

fn nested_ref(stored: &str) -> Vec<String> {
    vec![mark("begin"), code(" REF x "), mark("separate"), text(stored), mark("end")]
}

#[test]
fn the_length_covers_the_nested_fields_stored_text() {
    let mut body = vec![mark("begin"), code(" = ")];
    body.extend(nested_ref("OLD"));
    body.extend([code(" +1 "), mark("separate"), text("OLD"), mark("end")]);
    let (instr, texts) = nested_texts(&read(&body));
    assert_eq!(instr, "= OLD +1");
    assert_eq!(texts, ["OLD"]);
}

#[test]
fn the_length_follows_the_trim_at_both_ends_of_the_code() {
    // The nested fields stand first and last, with spaces in their stored text that the trim cuts.
    let mut body = vec![mark("begin")];
    body.extend(nested_ref(" 12"));
    body.push(code("+"));
    body.extend(nested_ref("3 4 "));
    body.extend([mark("separate"), text("OLD"), mark("end")]);
    let (instr, texts) = nested_texts(&read(&body));
    assert_eq!(instr, "12+3 4");
    assert_eq!(texts, ["12", "3 4"]);
}
