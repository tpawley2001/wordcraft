//! A field nested in another field's code whose stored result is written as `w:instrText` (after
//! the nested field's `separate`) reads as that field's stored text, as Word reads it: Microsoft
//! 365 Word prints `big` for `IF {MERGEFIELD Amount} >= 100 "big" "small"` with the nested result
//! `<w:instrText>150</w:instrText>`.

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

/// instrText without xml:space, as the generator writes the nested result.
fn bare_code(s: &str) -> String {
    format!(r#"<w:r><w:instrText>{s}</w:instrText></w:r>"#)
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

/// `begin <head> {begin <nested> separate <result runs> end} <tail> separate <shown> end`.
fn outer(head: &str, nested: &str, result: Vec<String>, tail: &str, shown: &str) -> Vec<String> {
    let mut body = vec![mark("begin"), code(head), mark("begin"), code(nested), mark("separate")];
    body.extend(result);
    body.extend([mark("end"), code(tail), mark("separate"), text(shown), mark("end")]);
    body
}

#[test]
fn an_if_reads_the_nested_merge_fields_instr_text_result() {
    let doc = read(&outer(" IF ", " MERGEFIELD Amount ", vec![bare_code("150")], r#" >= 100 "big" "small" "#, "big"));
    let (instr, texts) = nested_texts(&doc);
    assert_eq!(instr, r#"IF 150 >= 100 "big" "small""#);
    assert_eq!(texts, ["150"]);
}

#[test]
fn a_compare_reads_the_nested_merge_fields_instr_text_result() {
    let doc = read(&outer(" COMPARE ", " MERGEFIELD City ", vec![bare_code("Sydney")], r#" = "S*" "#, "1"));
    let (instr, texts) = nested_texts(&doc);
    assert_eq!(instr, r#"COMPARE Sydney = "S*""#);
    assert_eq!(texts, ["Sydney"]);
}

#[test]
fn a_result_split_over_two_instr_text_runs_joins() {
    let doc = read(&outer(" IF ", " MERGEFIELD Amount ", vec![bare_code("15"), bare_code("0")], r#" >= 100 "big" "small" "#, "big"));
    let (instr, texts) = nested_texts(&doc);
    assert_eq!(instr, r#"IF 150 >= 100 "big" "small""#);
    assert_eq!(texts, ["150"]);
}

#[test]
fn control_a_nested_result_as_text_reads_the_same() {
    let doc = read(&outer(" IF ", " MERGEFIELD Amount ", vec![text("150")], r#" >= 100 "big" "small" "#, "big"));
    let (instr, texts) = nested_texts(&doc);
    assert_eq!(instr, r#"IF 150 >= 100 "big" "small""#);
    assert_eq!(texts, ["150"]);
}

#[test]
fn control_instr_text_in_the_code_stays_code() {
    let body = vec![mark("begin"), code(" MERGEFIELD "), bare_code("Amount"), mark("separate"), text("«Amount»"), mark("end")];
    let doc = read(&body);
    let p = doc.body.iter().find_map(|b| b.as_para()).unwrap();
    let InlineObject::Field { instr, result, .. } = &p.objects[0] else { panic!("expected a field") };
    assert_eq!(instr, "MERGEFIELD Amount");
    assert_eq!(result, "«Amount»");
}
