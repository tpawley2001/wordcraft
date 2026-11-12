//! `DOCPROPERTY` and the summary properties a file does not carry, and what the count names
//! count, as measured in Word:
//!
//! - Title, Subject, Author, Keywords and Comments show `Error! Unknown document property name.`
//!   when `docProps/core.xml` has no element for them, and nothing when the element is there but
//!   empty; saving keeps that difference.
//! - Characters, CharactersWithSpaces and Paragraphs count the body text with every field result
//!   in it, and each count is taken when its field is updated, so it sees the results of the
//!   fields updated before it.
//!
//! Synthetic fixtures written for these tests.

use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_doc::para::InlineObject;
use wordcraft_engine::Session;

const UNKNOWN: &str = "Error! Unknown document property name.";

const CORE_NS: &str = concat!(
    r#"xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" "#,
    r#"xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" "#,
    r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#
);

/// The five summary properties, by the name `DOCPROPERTY` uses and their core.xml element.
const SUMMARY: [(&str, &str); 5] =
    [("Title", "dc:title"), ("Subject", "dc:subject"), ("Author", "dc:creator"), ("Keywords", "cp:keywords"), ("Comments", "dc:description")];

fn part(bytes: &[u8], name: &str) -> Option<String> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut f = z.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    Some(s)
}

fn replace_part(bytes: &[u8], name: &str, data: &str) -> Vec<u8> {
    let mut src = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut f = src.by_index(i).unwrap();
        let n = f.name().to_string();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        out.start_file(n.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
        out.write_all(if n == name { data.as_bytes() } else { &buf }).unwrap();
    }
    out.finish().unwrap().into_inner()
}

/// A one-paragraph DOCX whose core part holds exactly `elements` (and a revision number).
fn docx_with_core(elements: &str) -> Vec<u8> {
    let mut d = Document::new();
    d.core.last_modified_by = "Someone".into();
    let base = wordcraft_docx::write(&d).unwrap();
    assert!(part(&base, "docProps/core.xml").is_some(), "the fixture needs a core part to replace");
    let core = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties {CORE_NS}>{elements}<cp:revision>7</cp:revision></cp:coreProperties>"#
    );
    replace_part(&base, "docProps/core.xml", &core)
}

/// The file opened, with one `DOCPROPERTY` field per name inserted in the body.
fn opened_with_fields(bytes: &[u8], names: &[&str]) -> Session {
    let mut s = Session::new(wordcraft_docx::read(bytes).unwrap());
    for name in names {
        s.run("insert.field", &json!({"instr": format!("DOCPROPERTY {name}"), "result": "stale"})).unwrap();
        s.run("text.insert", &json!({"text": " "})).unwrap();
    }
    s
}

/// The displayed results of the fields in the body, in order.
fn results(s: &Session) -> Vec<String> {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| p.objects.iter())
        .filter_map(|o| match o {
            InlineObject::Field { result, .. } => Some(result.clone()),
            _ => None,
        })
        .collect()
}

fn update(s: &mut Session) -> Vec<String> {
    s.run("references.updateFields", &json!({})).unwrap();
    results(s)
}

const ALL_EMPTY: &str = "<dc:title></dc:title><dc:subject/><dc:creator></dc:creator><cp:keywords/><dc:description></dc:description>";

// ---- P05-197 ----

#[test]
fn a_summary_property_the_file_does_not_carry_shows_the_unknown_name_error() {
    // Word: no element -> the error, for all five; Category and LastSavedBy show nothing.
    let names = ["Title", "Subject", "Author", "Keywords", "Comments", "Category", "LastSavedBy"];
    let mut s = opened_with_fields(&docx_with_core(""), &names);
    assert_eq!(update(&mut s), [UNKNOWN, UNKNOWN, UNKNOWN, UNKNOWN, UNKNOWN, "", ""]);
}

#[test]
fn the_error_is_not_changed_by_a_capitalization_switch() {
    let mut s = opened_with_fields(&docx_with_core(""), &[r"Title \* Upper", r"Author \* MERGEFORMAT"]);
    assert_eq!(update(&mut s), [UNKNOWN, UNKNOWN]);
}

#[test]
fn a_summary_property_that_is_there_but_empty_shows_nothing() {
    let names = ["Title", "Subject", "Author", "Keywords", "Comments"];
    let mut s = opened_with_fields(&docx_with_core(ALL_EMPTY), &names);
    assert_eq!(update(&mut s), ["", "", "", "", ""]);
}

#[test]
fn each_property_is_judged_on_its_own() {
    // Word (measured): keywords and comments empty, title set, no subject and no creator.
    let core = "<dc:title>Report</dc:title><cp:keywords/><dc:description></dc:description>";
    let names = ["Title", "Keywords", "Comments", "Subject", "Author"];
    let mut s = opened_with_fields(&docx_with_core(core), &names);
    assert_eq!(update(&mut s), ["Report", "", "", UNKNOWN, UNKNOWN]);
}

#[test]
fn a_custom_property_with_the_name_wins_over_the_error() {
    let mut s = opened_with_fields(&docx_with_core(""), &["Title"]);
    s.run("file.properties", &json!({"custom": {"Title": "Custom title"}})).unwrap();
    assert_eq!(update(&mut s), ["Custom title"]);
}

#[test]
fn saving_keeps_an_empty_property_empty_and_an_absent_one_absent() {
    // There but empty: still an (empty) element after our save, so Word still shows nothing.
    let doc = wordcraft_docx::read(&docx_with_core(ALL_EMPTY)).unwrap();
    let saved = wordcraft_docx::write(&doc).unwrap();
    let core = part(&saved, "docProps/core.xml").expect("core.xml written");
    for (name, tag) in SUMMARY {
        assert!(core.contains(&format!("<{tag}/>")) || core.contains(&format!("<{tag}></{tag}>")), "{name}: {core}");
    }
    let names = ["Title", "Subject", "Author", "Keywords", "Comments"];
    assert_eq!(update(&mut opened_with_fields(&saved, &names)), ["", "", "", "", ""]);

    // Absent: no element after our save, so the field still shows the error.
    let doc = wordcraft_docx::read(&docx_with_core("")).unwrap();
    let saved = wordcraft_docx::write(&doc).unwrap();
    let core = part(&saved, "docProps/core.xml").unwrap_or_default();
    for (name, tag) in SUMMARY {
        assert!(!core.contains(&format!("<{tag}")), "{name}: {core}");
    }
    assert_eq!(update(&mut opened_with_fields(&saved, &names)), [UNKNOWN; 5]);
}

// ---- P05-198 / P05-199 ----

/// A document read from DOCX whose body holds these paragraphs; `{NAME}` in a paragraph is a
/// `DOCPROPERTY NAME` field with the stored result `stale`, or the one `stored` gives for the name
/// (at most one field per paragraph).
fn body_of(paragraphs: &[&str], stored: &[(&str, &str)]) -> Session {
    let run = |t: &str| format!(r#"<w:r><w:t xml:space="preserve">{t}</w:t></w:r>"#);
    let mut body = String::new();
    for para in paragraphs {
        body.push_str("<w:p>");
        match para.split_once('{') {
            Some((before, rest)) => {
                let (name, after) = rest.split_once('}').unwrap();
                let result = stored.iter().find(|(n, _)| *n == name).map_or("stale", |(_, r)| *r);
                body.push_str(&run(before));
                body.push_str(r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r>"#);
                body.push_str(&format!(r#"<w:r><w:instrText xml:space="preserve"> DOCPROPERTY {name} </w:instrText></w:r>"#));
                body.push_str(r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r>"#);
                body.push_str(&run(result));
                body.push_str(r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#);
                body.push_str(&run(after));
            }
            None if para.is_empty() => {}
            None => body.push_str(&run(para)),
        }
        body.push_str("</w:p>");
    }
    let xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{}"#,
            r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr></w:body></w:document>"#
        ),
        body
    );
    let base = wordcraft_docx::write(&Document::new()).unwrap();
    Session::new(wordcraft_docx::read(&replace_part(&base, "word/document.xml", &xml)).unwrap())
}

#[test]
fn the_counts_include_the_results_of_other_fields() {
    // Word's rule (measured: 13 words / 64 characters / 73 with spaces / 4 paragraphs for a body
    // with two field results): the text as shown, field results in. Here: `One two three.` (3
    // words, 12 characters, 14 with spaces), `[co:Acme Stored Co]` (3, 17, 19) and the count's
    // own paragraph with its stored result, `[n:stale]` (1, 9, 9).
    for (name, expected) in [("Words", "7"), ("Characters", "38"), ("CharactersWithSpaces", "42"), ("Paragraphs", "3")] {
        let count = format!("[n:{{{name}}}]");
        let mut s = body_of(&["One two three.", "[co:{Company}]", &count], &[("Company", "Acme Stored Co")]);
        s.doc.passthrough.insert(wordcraft_doc::APP_KEPT.into(), Arc::new(b"Company\tAcme Stored Co\n".to_vec()));
        assert_eq!(results(&s), ["Acme Stored Co", "stale"], "the fixture's stored results");
        assert_eq!(update(&mut s), ["Acme Stored Co", expected], "{name}");
    }
}

#[test]
fn each_count_is_taken_when_its_field_is_updated() {
    // Measured in Word: 7 / 44 / 5 / 39. Characters sees `[w:7]` but still `[c:stale]`,
    // `[p:stale]`, `[s:stale]`: 12 + 5 + 9 + 9 + 9. CharactersWithSpaces sees `[w:7]`, `[c:44]`,
    // `[p:5]` and its own `[s:stale]`: 14 + 5 + 6 + 5 + 9.
    let mut s = body_of(&["One two three.", "[w:{Words}]", "[c:{Characters}]", "[p:{Paragraphs}]", "[s:{CharactersWithSpaces}]"], &[]);
    assert_eq!(results(&s), ["stale"; 4], "the fixture's stored results");
    assert_eq!(update(&mut s), ["7", "44", "5", "39"]);
}

#[test]
fn an_empty_paragraph_is_not_counted() {
    let mut s = body_of(&["One two three.", "", "[p:{Paragraphs}]"], &[]);
    assert_eq!(update(&mut s), ["2"]);
}
