//! The view a document was saved in (`w:view`, ECMA-376 §17.15.1.92) is read and written back.

use wordcraft_doc::{Document, SavedView};

fn settings_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/settings.xml").unwrap(), &mut xml).unwrap();
    xml
}

/// `zip` with part `name` replaced by `data`.
fn replace_part(zip_bytes: &[u8], name: &str, data: &[u8]) -> Vec<u8> {
    let mut src = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let mut f = src.by_index(i).unwrap();
        let n = f.name().to_string();
        out.start_file(n.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
        if n == name {
            std::io::Write::write_all(&mut out, data).unwrap();
        } else {
            std::io::copy(&mut f, &mut out).unwrap();
        }
    }
    out.finish().unwrap().into_inner()
}

/// A file whose settings part has `extra` right after its start tag.
fn file_with(extra: &str) -> Vec<u8> {
    let plain = wordcraft_docx::write(&Document::new()).unwrap();
    let xml = settings_xml(&plain);
    let at = xml.find("<w:settings").unwrap();
    let end = at + xml[at..].find('>').unwrap() + 1;
    let xml = format!("{}{extra}{}", &xml[..end], &xml[end..]);
    replace_part(&plain, "word/settings.xml", xml.as_bytes())
}

#[test]
fn the_saved_view_is_read() {
    for (view, want) in [
        (r#"<w:view w:val="web"/>"#, Some(SavedView::Web)),
        (r#"<w:view w:val="outline"/>"#, Some(SavedView::Outline)),
        (r#"<w:view w:val="masterPages"/>"#, Some(SavedView::Outline)),
        (r#"<w:view w:val="normal"/>"#, Some(SavedView::Draft)),
        (r#"<w:view w:val="print"/>"#, None),
        (r#"<w:view w:val="none"/>"#, None),
        (r#"<w:view w:val="reading"/>"#, None),
        ("", None),
    ] {
        let d = wordcraft_docx::read(&file_with(view)).unwrap();
        assert_eq!(d.settings.view, want, "{view}");
    }
}

#[test]
fn the_view_is_written_before_the_zoom() {
    for (view, written) in [(SavedView::Web, "web"), (SavedView::Outline, "outline"), (SavedView::Draft, "normal")] {
        let mut d = Document::new();
        d.settings.view = Some(view);
        let xml = settings_xml(&wordcraft_docx::write(&d).unwrap());
        let tag = format!(r#"<w:view w:val="{written}"/>"#);
        let at = xml.find(&tag).unwrap_or_else(|| panic!("{view:?}: no {tag}: {xml}"));
        let zoom = xml.find("<w:zoom").expect("w:zoom");
        assert!(at < zoom, "{view:?}: w:view must come before w:zoom: {xml}");
    }
    let xml = settings_xml(&wordcraft_docx::write(&Document::new()).unwrap());
    assert!(!xml.contains("<w:view"), "Print Layout writes no w:view: {xml}");
}

/// A file with settings the model doesn't hold is saved by merging into its original settings
/// part: the view still comes back.
#[test]
fn the_view_survives_a_merged_settings_part() {
    let file = file_with(r#"<w:view w:val="web"/><w:doNotAutoCompressPictures/>"#);
    let back = settings_xml(&wordcraft_docx::write(&wordcraft_docx::read(&file).unwrap()).unwrap());
    assert!(back.contains(r#"<w:view w:val="web"/>"#), "{back}");
    assert!(back.contains("<w:doNotAutoCompressPictures/>"), "the unowned setting is kept: {back}");
    assert!(back.find("<w:view").unwrap() < back.find("<w:zoom").unwrap(), "{back}");

    // Turned off in the model, the original's view goes.
    let mut d = wordcraft_docx::read(&file).unwrap();
    d.settings.view = None;
    let back = settings_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(!back.contains("<w:view"), "{back}");
}
