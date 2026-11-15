//! SAVEDATE shows when the document was last saved: the package's last-modified property
//! (`docProps/core.xml` `dcterms:modified`), in local time under the field's picture. Word does this
//! when it opens the file and on Update Field; it never shows "now" for SAVEDATE (except at output,
//! where the export itself counts as a save). Measured in Microsoft 365 Word, 2026-11-15:
//! modified 2003-04-05T06:07:00Z, stored result `0000`, on a machine at UTC-8, field result after
//! open = `2003-04-04 22:07`. Packages are hand-written: the writer never stores a stale result.

use std::io::Write;

use wordcraft_doc::{InlineObject, StoryRef};

fn zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// A field run: its code, its cached result, and whether it carries `w:fldLock`.
fn field(instr: &str, cached: &str, locked: bool) -> String {
    let lock = if locked { r#" w:fldLock="1""# } else { "" };
    format!(
        concat!(
            r#"<w:r><w:fldChar w:fldCharType="begin"{}/></w:r>"#,
            r#"<w:r><w:instrText xml:space="preserve"> {} </w:instrText></w:r>"#,
            r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r>"#,
            r#"<w:r><w:t xml:space="preserve">{}</w:t></w:r>"#,
            r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#,
        ),
        lock,
        instr.replace('"', "&quot;"),
        cached
    )
}

/// A package whose core properties carry `modified` (no core part at all when `None`).
fn open_with_modified(body: &str, modified: Option<&str>) -> wordcraft_doc::Document {
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}</w:body></w:document>"#
    );
    let mut parts = vec![("_rels/.rels", rels.to_string()), ("word/document.xml", doc)];
    if let Some(m) = modified {
        parts.push((
            "docProps/core.xml",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dcterms:modified xsi:type="dcterms:W3CDTF">{m}</dcterms:modified></cp:coreProperties>"#
            ),
        ));
    }
    let entries: Vec<(&str, &str)> = parts.iter().map(|(n, b)| (*n, b.as_str())).collect();
    wordcraft_engine::io::open_bytes("d.docx", &zip(&entries)).expect("opens")
}

/// Every body field's result, in order.
fn results(d: &wordcraft_doc::Document) -> Vec<String> {
    let mut out = Vec::new();
    for path in d.para_paths(StoryRef::Body) {
        let Some(p) = d.para(StoryRef::Body, &path) else { continue };
        for o in &p.objects {
            if let InlineObject::Field { result, .. } = o {
                out.push(result.clone());
            }
        }
    }
    out
}

/// `value` (an RFC 3339 instant) in the local time zone under a Word date picture.
fn local(value: &str, picture: &str) -> String {
    let unix = u64::try_from(chrono::DateTime::parse_from_rfc3339(value).unwrap().timestamp()).unwrap();
    wordcraft_engine::cmd::insert::format_date_at(picture, unix, wordcraft_engine::cmd::local_offset_secs(unix))
}

const SAVED: &str = "2003-04-05T06:07:00Z";

#[test]
fn savedate_shows_the_last_saved_time_on_open() {
    let d = open_with_modified(
        &format!(
            "<w:p>{}{}{}</w:p>",
            field(r#"SAVEDATE \@ "yyyy-MM-dd HH:mm""#, "0000", false),
            field(r#"SAVEDATE \@ "yyyy-MM-dd""#, "CACHED", false),
            field("SAVEDATE", "stale", false),
        ),
        Some(SAVED),
    );
    assert_eq!(d.core.modified, SAVED, "opening keeps the property as stored");
    assert_eq!(results(&d), [local(SAVED, "yyyy-MM-dd HH:mm"), local(SAVED, "yyyy-MM-dd"), local(SAVED, "M/d/yyyy")]);
}

#[test]
fn savedate_on_open_reads_fractional_seconds_and_offsets() {
    for stamp in ["2003-04-05T06:07:00.123Z", "2003-04-05T08:07:00+02:00"] {
        let d = open_with_modified(&format!("<w:p>{}</w:p>", field(r#"SAVEDATE \@ "yyyy-MM-dd HH:mm""#, "stale", false)), Some(stamp));
        assert_eq!(results(&d), [local(SAVED, "yyyy-MM-dd HH:mm")], "{stamp}");
    }
}

#[test]
fn a_locked_savedate_keeps_its_stored_result() {
    let d = open_with_modified(&format!("<w:p>{}</w:p>", field(r#"SAVEDATE \@ "yyyy""#, "1999", true)), Some(SAVED));
    assert_eq!(results(&d), ["1999"]);
    let mut s = wordcraft_engine::Session::new(d);
    s.run("references.updateFields", &serde_json::json!({})).unwrap();
    assert_eq!(results(&s.doc), ["1999"]);
}

#[test]
fn savedate_keeps_its_stored_result_when_the_property_is_missing_or_unreadable() {
    // No core part, an empty property, and values that name no instant.
    for stamp in [None, Some(""), Some("missing"), Some("2001-02-29T13:04:05Z"), Some("2003-04-05")] {
        let d = open_with_modified(&format!("<w:p>{}</w:p>", field(r#"SAVEDATE \@ "yyyy-MM-dd""#, "cached", false)), stamp);
        assert_eq!(results(&d), ["cached"], "open, {stamp:?}");
        let mut s = wordcraft_engine::Session::new(d);
        s.run("references.updateFields", &serde_json::json!({})).unwrap();
        assert_eq!(results(&s.doc), ["cached"], "update, {stamp:?}: never today's date");
    }
}

#[test]
fn update_field_gives_savedate_the_last_saved_time_not_now() {
    let d = open_with_modified(&format!("<w:p>{}</w:p>", field(r#"SAVEDATE \@ "yyyy-MM-dd HH:mm""#, "0000", false)), Some(SAVED));
    let mut s = wordcraft_engine::Session::new(d);
    // Change the property after open: Update Field reads the current value.
    s.doc.core.modified = "2010-06-07T08:09:00Z".into();
    s.run("references.updateFields", &serde_json::json!({})).unwrap();
    assert_eq!(results(&s.doc), [local("2010-06-07T08:09:00Z", "yyyy-MM-dd HH:mm")]);
    assert_eq!(s.doc.core.modified, "2010-06-07T08:09:00Z", "updating fields doesn't touch the property");
}

#[test]
fn savedate_applies_its_format_switches() {
    // 2000-02-29 12:00 UTC is a Tuesday in every zone from UTC-11 to UTC+11.
    let stamp = "2000-02-29T12:00:00Z";
    for (switches, expected) in [("Upper", "TUESDAY"), ("Lower", "tuesday"), ("Caps", "Tuesday")] {
        let d = open_with_modified(&format!("<w:p>{}</w:p>", field(&format!(r#"SAVEDATE \@ "dddd" \* {switches}"#), "x", false)), Some(stamp));
        assert_eq!(results(&d), [expected], "open, {switches}");
        let mut s = wordcraft_engine::Session::new(d);
        s.run("references.updateFields", &serde_json::json!({})).unwrap();
        assert_eq!(results(&s.doc), [expected], "update, {switches}");
    }
}
