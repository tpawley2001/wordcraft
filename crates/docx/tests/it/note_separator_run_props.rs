//! A note separator whose run has properties of its own (`w:rPr` on the run holding
//! `w:separator` / `w:continuationSeparator`, ECMA-376 Part 1 §17.11.1, §17.11.2, §17.3.2) keeps
//! them when read, saved and read again: layout sizes the separator line's room from that run's
//! font and size, so dropping them changes how much text a page holds.
use std::io::{Cursor, Read, Write};

use wordcraft_doc::{Blocks, Document, Paragraph};

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(text.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// A package with one footnote and one endnote, whose notes parts start with the given
/// separator and continuation separator runs.
fn package(foot_sep: &str, foot_cont: &str, end_sep: &str) -> Vec<u8> {
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W_NS}><w:body><w:p><w:r><w:t>Text</w:t></w:r><w:r><w:footnoteReference w:id="2"/></w:r><w:r><w:endnoteReference w:id="2"/></w:r></w:p></w:body></w:document>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;
    // Word uses a separator story only when the settings list its id (§17.11.12).
    let settings = format!(
        r#"<w:settings {W_NS}><w:footnotePr><w:footnote w:id="-1"/><w:footnote w:id="0"/></w:footnotePr><w:endnotePr><w:endnote w:id="-1"/><w:endnote w:id="0"/></w:endnotePr></w:settings>"#
    );
    let notes = |root: &str, tag: &str, sep: &str, cont: &str| {
        format!(
            r#"<w:{root} {W_NS}><w:{tag} w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0"/></w:pPr>{sep}</w:p></w:{tag}><w:{tag} w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0"/></w:pPr>{cont}</w:p></w:{tag}><w:{tag} w:id="2"><w:p><w:r><w:t xml:space="preserve"> The note.</w:t></w:r></w:p></w:{tag}></w:{root}>"#
        )
    };
    let foot = notes("footnotes", "footnote", foot_sep, foot_cont);
    let end = notes("endnotes", "endnote", end_sep, "<w:r><w:continuationSeparator/></w:r>");
    zip(&[
        ("_rels/.rels", ROOT_RELS),
        ("word/document.xml", &doc),
        ("word/_rels/document.xml.rels", rels),
        ("word/footnotes.xml", &foot),
        ("word/endnotes.xml", &end),
        ("word/settings.xml", &settings),
    ])
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

/// The first paragraph of a separator story.
fn first(b: &Option<Blocks>) -> Paragraph {
    b.as_ref().expect("the document's own separator").first().and_then(|b| b.as_para()).unwrap().clone()
}

/// The `w:r` element (start to end) holding `<{el}/>` inside the `w:{tag}` of type `kind`.
fn sep_run(xml: &str, tag: &str, kind: &str, el: &str) -> String {
    let start = xml.find(&format!(r#"<w:{tag} w:type="{kind}""#)).unwrap_or_else(|| panic!("{kind}: {xml}"));
    let end = start + xml[start..].find(&format!("</w:{tag}>")).unwrap();
    let story = &xml[start..end];
    let at = story.find(&format!("<{el}/>")).unwrap_or_else(|| panic!("{el} in {story}"));
    let r = story[..at].rfind("<w:r>").or_else(|| story[..at].rfind("<w:r ")).expect("its run");
    let e = at + story[at..].find("</w:r>").expect("run end") + "</w:r>".len();
    story[r..e].to_string()
}

fn sized() -> Vec<u8> {
    package(
        r#"<w:r><w:rPr><w:sz w:val="20"/></w:rPr><w:separator/></w:r>"#,
        r#"<w:r><w:rPr><w:rFonts w:ascii="Courier New" w:hAnsi="Courier New"/><w:sz w:val="16"/></w:rPr><w:continuationSeparator/></w:r>"#,
        r#"<w:r><w:rPr><w:sz w:val="18"/></w:rPr><w:separator/></w:r>"#,
    )
}

fn check_sized(d: &Document) {
    let sep = first(&d.footnote_separators.separator);
    assert_eq!(sep.props_of_char(0).size, Some(10.0), "footnote separator run size");
    let cont = first(&d.footnote_separators.continuation_separator);
    assert_eq!(cont.props_of_char(0).size, Some(8.0), "continuation separator run size");
    assert_eq!(cont.props_of_char(0).font.as_deref(), Some("Courier New"), "continuation separator run font");
    let esep = first(&d.endnote_separators.separator);
    assert_eq!(esep.props_of_char(0).size, Some(9.0), "endnote separator run size");
}

#[test]
fn separator_run_properties_are_read() {
    check_sized(&wordcraft_docx::read(&sized()).unwrap());
}

#[test]
fn separator_run_properties_are_saved_on_the_separator_run() {
    let d = wordcraft_docx::read(&sized()).unwrap();
    let out = wordcraft_docx::write(&d).unwrap();
    let foot = part(&out, "word/footnotes.xml");
    let run = sep_run(&foot, "footnote", "separator", "w:separator");
    assert!(run.contains(r#"<w:sz w:val="20"/>"#), "{run}");
    assert!(run.find("<w:rPr>").unwrap() < run.find("<w:separator/>").unwrap(), "rPr comes first: {run}");
    let run = sep_run(&foot, "footnote", "continuationSeparator", "w:continuationSeparator");
    assert!(run.contains(r#"<w:sz w:val="16"/>"#), "{run}");
    assert!(run.contains("Courier New"), "{run}");
    let end = part(&out, "word/endnotes.xml");
    let run = sep_run(&end, "endnote", "separator", "w:separator");
    assert!(run.contains(r#"<w:sz w:val="18"/>"#), "{run}");
}

#[test]
fn separator_run_properties_survive_save_and_reopen() {
    let d = wordcraft_docx::read(&sized()).unwrap();
    let again = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    check_sized(&again);
}

#[test]
fn a_separator_run_without_properties_stays_bare() {
    let bare = package("<w:r><w:separator/></w:r>", "<w:r><w:continuationSeparator/></w:r>", "<w:r><w:separator/></w:r>");
    let d = wordcraft_docx::read(&bare).unwrap();
    assert_eq!(first(&d.footnote_separators.separator).props_of_char(0).size, None);
    let out = wordcraft_docx::write(&d).unwrap();
    let foot = part(&out, "word/footnotes.xml");
    assert_eq!(sep_run(&foot, "footnote", "separator", "w:separator"), "<w:r><w:separator/></w:r>");
    assert_eq!(sep_run(&foot, "footnote", "continuationSeparator", "w:continuationSeparator"), "<w:r><w:continuationSeparator/></w:r>");
}
