//! A footnote or endnote separator story (`w:type="separator"` / `"continuationSeparator"` in the
//! notes part, ECMA-376 Part 1 §17.11) is the document's own only when the settings list its id
//! in `w:footnotePr` / `w:endnotePr` (§17.11.11, §17.11.12). Microsoft 365 Word ignores a story
//! that isn't listed and draws its built-in separator (measured from Word's PDFs: a 24 pt
//! separator story drew a 1.2 pt rule when listed and the Normal style's 0.48 pt one when not).
use std::io::{Cursor, Write};

use wordcraft_doc::Document;

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

/// A package with one footnote and one endnote, each notes part holding a 24 pt separator and a
/// continuation separator story. `settings`: the `w:settings` children (`None`: no settings part).
fn package(settings: Option<&str>) -> Vec<u8> {
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W_NS}><w:body><w:p><w:r><w:t>Text</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:endnoteReference w:id="1"/></w:r></w:p></w:body></w:document>"#
    );
    let mut rels = String::from(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/>"#,
    );
    if settings.is_some() {
        rels.push_str(
            r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>"#,
        );
    }
    rels.push_str("</Relationships>");
    let notes = |root: &str, tag: &str| {
        format!(
            r#"<w:{root} {W_NS}><w:{tag} w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:sz w:val="48"/></w:rPr><w:separator/></w:r></w:p></w:{tag}><w:{tag} w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:{tag}><w:{tag} w:id="1"><w:p><w:r><w:t xml:space="preserve"> The note.</w:t></w:r></w:p></w:{tag}></w:{root}>"#
        )
    };
    let foot = notes("footnotes", "footnote");
    let end = notes("endnotes", "endnote");
    let settings = settings.map(|s| format!(r#"<w:settings {W_NS}>{s}</w:settings>"#));
    let mut entries = vec![
        ("_rels/.rels", ROOT_RELS),
        ("word/document.xml", doc.as_str()),
        ("word/_rels/document.xml.rels", rels.as_str()),
        ("word/footnotes.xml", foot.as_str()),
        ("word/endnotes.xml", end.as_str()),
    ];
    if let Some(s) = settings.as_deref() {
        entries.push(("word/settings.xml", s));
    }
    zip(&entries)
}

const FOOT_LINKS: &str = r#"<w:footnotePr><w:footnote w:id="-1"/><w:footnote w:id="0"/></w:footnotePr>"#;
const END_LINKS: &str = r#"<w:endnotePr><w:endnote w:id="-1"/><w:endnote w:id="0"/></w:endnotePr>"#;

fn read(settings: Option<&str>) -> Document {
    wordcraft_docx::read(&package(settings)).unwrap()
}

/// (footnote separator, footnote continuation separator, endnote separator, endnote continuation
/// separator): whether the document has its own.
fn own(d: &Document) -> [bool; 4] {
    [
        d.footnote_separators.separator.is_some(),
        d.footnote_separators.continuation_separator.is_some(),
        d.endnote_separators.separator.is_some(),
        d.endnote_separators.continuation_separator.is_some(),
    ]
}

#[test]
fn listed_separator_stories_are_the_documents_own() {
    assert_eq!(own(&read(Some(&format!("{FOOT_LINKS}{END_LINKS}")))), [true; 4]);
}

#[test]
fn separator_stories_without_a_settings_part_are_not_used() {
    assert_eq!(own(&read(None)), [false; 4]);
}

#[test]
fn separator_stories_the_settings_dont_list_are_not_used() {
    assert_eq!(own(&read(Some(r#"<w:footnotePr><w:numFmt w:val="decimal"/></w:footnotePr>"#))), [false; 4]);
}

#[test]
fn each_list_covers_only_its_own_notes_and_ids() {
    // Footnotes list only the separator (-1); endnotes list both.
    let d = read(Some(&format!(r#"<w:footnotePr><w:footnote w:id="-1"/></w:footnotePr>{END_LINKS}"#)));
    assert_eq!(own(&d), [true, false, true, true]);
    // Only the footnotes are listed.
    assert_eq!(own(&read(Some(FOOT_LINKS))), [true, true, false, false]);
}

#[test]
fn a_listed_story_still_saves_and_reopens_listed() {
    let d = read(Some(&format!("{FOOT_LINKS}{END_LINKS}")));
    let again = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
    assert_eq!(own(&again), [true; 4]);
}
