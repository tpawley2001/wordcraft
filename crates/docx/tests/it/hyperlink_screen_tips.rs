//! A hyperlink's ScreenTip: `w:hyperlink/@w:tooltip` (ECMA-376 Part 1 §17.16.22) and the `\o`
//! switch of a `HYPERLINK` field (§17.16.5.25). Word shows it on hover instead of the address
//! (Insert Hyperlink › ScreenTip…). It is kept on open and save, when text is typed inside the
//! link and when the link's address changes. Original synthetic packages.
use std::io::{Read, Write};
use wordcraft_doc::{Document, Pos};

const WEB: &str = "https://example.com/page";

fn package(body: &str) -> Vec<u8> {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr></w:body></w:document>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdWeb" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{WEB}" TargetMode="External"/></Relationships>"#
    );
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("word/document.xml", xml.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

fn run(text: &str) -> String {
    format!(r#"<w:r><w:t xml:space="preserve">{text}</w:t></w:r>"#)
}

/// A `w:hyperlink` to the web relationship with `attrs` (e.g. ` w:tooltip="x"`) around `text`.
fn web_link(attrs: &str, text: &str) -> String {
    format!(r#"<w:hyperlink r:id="rIdWeb"{attrs} w:history="1">{}</w:hyperlink>"#, run(text))
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// The value of attribute `name` in an element's open tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!(" {name}=\""))?;
    let value = tag.get(at + name.len() + 3..)?;
    Some(unescape(value.get(..value.find('"')?)?))
}

/// Every `w:hyperlink` of the saved document, in order: its text and its ScreenTip.
fn saved_links(bytes: &[u8]) -> Vec<(String, Option<String>)> {
    let xml = part(bytes, "word/document.xml");
    let mut out = Vec::new();
    for (at, _) in xml.match_indices("<w:hyperlink ") {
        let rest = xml.get(at..).unwrap();
        let tag = rest.get(..rest.find('>').unwrap()).unwrap();
        let inner = rest.get(..rest.find("</w:hyperlink>").unwrap()).unwrap();
        let mut text = String::new();
        for (t, _) in inner.match_indices("<w:t") {
            let piece = inner.get(t..).unwrap();
            let Some(open) = piece.find('>') else { continue };
            if piece.as_bytes().get(4).is_some_and(|b| *b != b' ' && *b != b'>') {
                continue; // `w:tab`, `w:tbl`…: not a `w:t`
            }
            let Some(close) = piece.find("</w:t>") else { continue };
            text.push_str(&unescape(piece.get(open + 1..close).unwrap_or("")));
        }
        out.push((text, attr(tag, "w:tooltip")));
    }
    out
}

fn link(text: &str, tip: Option<&str>) -> (String, Option<String>) {
    (text.to_string(), tip.map(str::to_string))
}

fn open_save(body: &str) -> Vec<u8> {
    let doc = wordcraft_docx::read(&package(body)).unwrap();
    wordcraft_docx::write(&doc).unwrap()
}

/// The saved document's relationships hold `target`.
fn has_target(bytes: &[u8], target: &str) -> bool {
    part(bytes, "word/_rels/document.xml.rels").contains(&format!(r#"Target="{target}""#))
}

#[test]
fn a_screen_tip_is_kept_on_open_and_save() {
    let body = format!("<w:p>{}{}{}</w:p>", run("Before "), web_link(r#" w:tooltip="Tip one""#, "Alpha"), run(" after"));
    let bytes = open_save(&body);
    assert_eq!(saved_links(&bytes), [link("Alpha", Some("Tip one"))]);
    assert!(has_target(&bytes, WEB));
    // And through a second open and save.
    let again = wordcraft_docx::write(&wordcraft_docx::read(&bytes).unwrap()).unwrap();
    assert_eq!(saved_links(&again), [link("Alpha", Some("Tip one"))]);
}

#[test]
fn xml_special_characters_in_a_screen_tip_survive() {
    let body = format!("<w:p>{}</w:p>", web_link(r#" w:tooltip="Tom &amp; &quot;Jerry&quot; &lt;1&gt;""#, "Alpha"));
    assert_eq!(saved_links(&open_save(&body)), [link("Alpha", Some("Tom & \"Jerry\" <1>"))]);
}

#[test]
fn an_empty_screen_tip_writes_no_attribute() {
    let body = format!("<w:p>{}{}</w:p>", web_link(r#" w:tooltip="""#, "Alpha"), run(" plain"));
    let bytes = open_save(&body);
    assert_eq!(saved_links(&bytes), [link("Alpha", None)]);
    assert!(!part(&bytes, "word/document.xml").contains("w:tooltip"));
}

#[test]
fn a_link_without_a_screen_tip_is_saved_without_one() {
    let body = format!("<w:p>{}</w:p>", web_link("", "Alpha"));
    let bytes = open_save(&body);
    assert_eq!(saved_links(&bytes), [link("Alpha", None)]);
    assert!(!part(&bytes, "word/document.xml").contains("w:tooltip"));
}

#[test]
fn a_long_screen_tip_is_kept_whole() {
    let tip = "0123456789".repeat(30);
    let body = format!("<w:p>{}</w:p>", web_link(&format!(r#" w:tooltip="{tip}""#), "Alpha"));
    assert_eq!(saved_links(&open_save(&body)), [link("Alpha", Some(&tip))]);
}

#[test]
fn a_bookmark_link_keeps_its_screen_tip() {
    let body = format!(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="Spot"/>{}<w:bookmarkEnd w:id="1"/></w:p><w:p><w:hyperlink w:anchor="Spot" w:tooltip="Go" w:history="1">{}</w:hyperlink></w:p>"#,
        run("Target"),
        run("Jump")
    );
    let bytes = open_save(&body);
    assert_eq!(saved_links(&bytes), [link("Jump", Some("Go"))]);
    assert!(part(&bytes, "word/document.xml").contains(r#"w:anchor="Spot""#));
}

#[test]
fn neighbouring_links_to_one_address_keep_their_own_screen_tips() {
    let body = format!("<w:p>{}{}{}</w:p>", web_link(r#" w:tooltip="One""#, "Aa"), web_link(r#" w:tooltip="Two""#, "Bb"), web_link("", "Cc"));
    assert_eq!(saved_links(&open_save(&body)), [link("Aa", Some("One")), link("Bb", Some("Two")), link("Cc", None)]);
}

#[test]
fn a_hyperlink_fields_o_switch_is_its_screen_tip() {
    let field = |code: &str, result: &str| {
        format!(
            r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve">{code}</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>{}<w:r><w:fldChar w:fldCharType="end"/></w:r>"#,
            run(result)
        )
    };
    let body = format!(
        "<w:p>{}</w:p><w:p>{}</w:p>",
        field(r#" HYPERLINK "https://example.com/f" \o "Field tip" "#, "Field"),
        field(r#" HYPERLINK "https://example.com/g" "#, "Plain")
    );
    let bytes = open_save(&body);
    assert_eq!(saved_links(&bytes), [link("Field", Some("Field tip")), link("Plain", None)]);
    assert!(has_target(&bytes, "https://example.com/f") && has_target(&bytes, "https://example.com/g"));
}

/// The document of one paragraph `Before ` + link "Alpha" (tip "Tip one") + ` after`.
fn tipped() -> Document {
    let body = format!("<w:p>{}{}{}</w:p>", run("Before "), web_link(r#" w:tooltip="Tip one""#, "Alpha"), run(" after"));
    wordcraft_docx::read(&package(&body)).unwrap()
}

#[test]
fn text_typed_inside_a_link_keeps_the_screen_tip() {
    let mut doc = tipped();
    // "Before " is 7 bytes: offset 9 is inside "Alpha", between "Al" and "pha".
    let props = doc.body[0].as_para().unwrap().props_of_char(9).clone();
    doc.insert_text(&Pos::body(0, 9), "XY", &props).unwrap();
    let bytes = wordcraft_docx::write(&doc).unwrap();
    assert_eq!(saved_links(&bytes), [link("AlXYpha", Some("Tip one"))]);
}

#[test]
fn changing_a_links_address_keeps_the_screen_tip() {
    let mut doc = tipped();
    doc.format_range(&Pos::body(0, 7), &Pos::body(0, 12), &|c| c.link = Some("https://example.org/new".to_string())).unwrap();
    let bytes = wordcraft_docx::write(&doc).unwrap();
    assert_eq!(saved_links(&bytes), [link("Alpha", Some("Tip one"))]);
    assert!(has_target(&bytes, "https://example.org/new"));
}

#[test]
fn a_removed_link_leaves_no_screen_tip_behind() {
    let mut doc = tipped();
    doc.format_range(&Pos::body(0, 7), &Pos::body(0, 12), &|c| c.link = None).unwrap();
    let bytes = wordcraft_docx::write(&doc).unwrap();
    let xml = part(&bytes, "word/document.xml");
    assert!(saved_links(&bytes).is_empty(), "{xml}");
    assert!(!xml.contains("w:tooltip"), "{xml}");
}
