//! File › Print › Document Info: `print::document_info` builds the report Word prints for Document
//! Properties, List of Markup, Styles and AutoText Entries, and a print job's `what` prints it.

use serde_json::{Value, json};

use crate::Session;
use crate::print::document_info;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn open(body: &str) -> Session {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    Session::new(wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads"))
}

fn info(s: &Session, what: &str) -> Vec<String> {
    document_info(s, what).unwrap_or_else(|e| panic!("{what}: {e}"))
}

/// The value after `label` + TAB on its line, or panic.
fn value<'a>(lines: &'a [String], label: &str) -> &'a str {
    let prefix = format!("{label}\t");
    let line = lines.iter().find(|l| l.starts_with(&prefix)).unwrap_or_else(|| panic!("no {label:?} line in {lines:#?}"));
    &line[prefix.len()..]
}

#[test]
fn document_properties_lists_word_labels_in_order_with_their_values() {
    let mut s = open("<w:p><w:r><w:t>one two three</w:t></w:r></w:p>");
    run(&mut s, "file.properties", json!({"title": "Quarterly Report", "author": "Ann Lee", "keywords": "q3", "comments": "draft"}));
    let lines = info(&s, "properties");
    let labels = [
        "Filename:",
        "Directory:",
        "Template:",
        "Title:",
        "Subject:",
        "Author:",
        "Keywords:",
        "Comments:",
        "Creation Date:",
        "Change Number:",
        "Last Saved On:",
        "Last Saved By:",
        "Total Editing Time:",
        "Last Printed On:",
        "As of Last Complete Printing",
        "Number of Pages:",
        "Number of Words:",
        "Number of Characters:",
    ];
    let at: Vec<usize> = labels
        .iter()
        .map(|l| lines.iter().position(|x| x == l || x.starts_with(&format!("{l}\t"))).unwrap_or_else(|| panic!("no {l:?} in {lines:#?}")))
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "labels out of Word's order: {lines:#?}");
    assert_eq!(value(&lines, "Title:"), "Quarterly Report");
    assert_eq!(value(&lines, "Author:"), "Ann Lee");
    assert_eq!(value(&lines, "Keywords:"), "q3");
    assert_eq!(value(&lines, "Comments:"), "draft");
    assert_eq!(value(&lines, "Subject:"), "");
    assert_eq!(value(&lines, "Number of Pages:"), "1");
    assert_eq!(value(&lines, "Number of Words:"), "3");
    // "one two three" without spaces.
    assert_eq!(value(&lines, "Number of Characters:"), "11");
}

#[test]
fn list_of_markup_has_one_entry_per_change_in_document_order() {
    let s = open(concat!(
        r#"<w:p><w:r><w:t xml:space="preserve">Keep </w:t></w:r>"#,
        r#"<w:ins w:id="1" w:author="Ann" w:date="2026-11-01T10:00:00Z"><w:r><w:t>added</w:t></w:r></w:ins>"#,
        r#"<w:del w:id="2" w:author="Bo" w:date="2026-11-02T10:00:00Z"><w:r><w:delText>gone</w:delText></w:r></w:del></w:p>"#,
    ));
    let lines = info(&s, "markup");
    let heads: Vec<(usize, &String)> = lines.iter().enumerate().filter(|(_, l)| l.starts_with("Page ")).collect();
    assert_eq!(heads.len(), 2, "{lines:#?}");
    let (i, ins) = heads[0];
    assert!(ins.starts_with("Page 1: [1] Inserted\tAnn\t"), "{ins:?}");
    assert_eq!(lines[i + 1], "added");
    let (j, del) = heads[1];
    assert!(del.starts_with("Page 1: [2] Deleted\tBo\t"), "{del:?}");
    assert_eq!(lines[j + 1], "gone");
}

#[test]
fn list_of_markup_is_empty_without_changes_and_printing_it_is_an_error() {
    let s = open("<w:p><w:r><w:t>Plain</w:t></w:r></w:p>");
    assert!(info(&s, "markup").is_empty());
    assert!(s.print_pdf(&json!({"what": "markup"})).is_err());
}

#[test]
fn styles_lists_the_styles_in_use_and_custom_ones_sorted_each_with_a_description() {
    let mut s = open("<w:p><w:r><w:t>Title here</w:t></w:r></w:p><w:p><w:r><w:t>Body</w:t></w:r></w:p>");
    run(&mut s, "para.style", json!({"style": "Heading 1"}));
    run(&mut s, "styles.create", json!({"name": "Zeta Note"}));
    let lines = info(&s, "styles");
    let names = ["Heading 1", "Normal", "Zeta Note"];
    let at: Vec<usize> = names.iter().map(|n| lines.iter().position(|l| l == n).unwrap_or_else(|| panic!("no {n:?} in {lines:#?}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "not sorted by name: {lines:#?}");
    for i in at {
        let desc = lines.get(i + 1).map(String::as_str).unwrap_or("");
        assert!(!desc.is_empty() && !names.contains(&desc), "{} has no description: {lines:#?}", lines[i]);
    }
    // A built-in style nobody uses is not listed.
    assert!(!lines.iter().any(|l| l == "Heading 9"), "{lines:#?}");
}

#[test]
fn autotext_lists_the_documents_entries_by_name_with_their_text() {
    let mut s = open("<w:p><w:r><w:t>Kind regards</w:t></w:r></w:p>");
    run(&mut s, "select.all", json!({}));
    run(&mut s, "insert.autoText", json!({"save": "sig", "in": "document"}));
    let lines = info(&s, "autotext");
    let i = lines.iter().position(|l| l == "sig").unwrap_or_else(|| panic!("{lines:#?}"));
    assert_eq!(lines[i + 1].trim_end(), "Kind regards");
}

#[test]
fn unknown_what_is_a_params_error_and_a_report_prints() {
    let s = open("<w:p><w:r><w:t>Text</w:t></w:r></w:p>");
    assert!(document_info(&s, "keys2").is_err());
    assert!(s.print_pdf(&json!({"what": "bogus"})).is_err());
    let (pdf, plan) = s.print_pdf(&json!({"what": "properties"})).expect("properties print");
    assert!(pdf.starts_with(b"%PDF"));
    assert_eq!(plan.chosen, 1);
    // The default still prints the document.
    assert!(s.print_pdf(&json!({"what": "document"})).is_ok());
}
