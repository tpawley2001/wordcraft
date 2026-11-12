//! `w:wpJustification` (a legacy `w:compat` option, ECMA-376 Part 1 §17.15.1) is read into the
//! document settings (serialized as `wpJustification`) and written back, in CT_Compat's order:
//! before `w:noLeading`.

use wordcraft_doc::Document;

fn settings_xml(d: &Document) -> String {
    let bytes = wordcraft_docx::write(d).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/settings.xml").unwrap(), &mut xml).unwrap();
    xml
}

fn rt(d: &Document) -> Document {
    wordcraft_docx::read(&wordcraft_docx::write(d).unwrap()).unwrap()
}

/// Whether the document's settings carry the flag.
fn flag(d: &Document) -> bool {
    serde_json::to_value(&d.settings).unwrap()["wpJustification"].as_bool().unwrap_or(false)
}

/// A new document with the flag set through the settings' serialized form.
fn flagged() -> Document {
    let mut d = Document::new();
    let mut v = serde_json::to_value(&d.settings).unwrap();
    v["wpJustification"] = serde_json::Value::Bool(true);
    d.settings = serde_json::from_value(v).unwrap();
    d
}

#[test]
fn wp_justification_round_trips() {
    let d = Document::new();
    assert!(!flag(&rt(&d)));
    assert!(!settings_xml(&d).contains("wpJustification"));
    let d = flagged();
    assert!(flag(&d), "the settings keep the flag");
    assert!(flag(&rt(&d)));
    assert_eq!(settings_xml(&d).matches("<w:wpJustification").count(), 1);
    // Saved twice, still once.
    assert_eq!(settings_xml(&rt(&d)).matches("<w:wpJustification").count(), 1);
}

#[test]
fn wp_justification_keeps_the_compatibility_mode() {
    let mut d = flagged();
    d.settings.compat_mode = 11;
    let back = rt(&d);
    assert!(flag(&back));
    assert_eq!(back.settings.compat_mode, 11);
}

#[test]
fn wp_justification_is_written_before_no_leading() {
    let mut d = flagged();
    d.settings.no_leading = true;
    d.settings.do_not_expand_shift_return = true;
    let xml = settings_xml(&d);
    let at = |e: &str| xml.find(e).unwrap_or_else(|| panic!("{e} missing in {xml}"));
    assert!(at("<w:compat>") < at("<w:wpJustification"));
    assert!(at("<w:wpJustification") < at("<w:noLeading"));
    assert!(at("<w:wpJustification") < at("<w:doNotExpandShiftReturn"));
    assert!(at("<w:wpJustification") < at("<w:compatSetting"));
}
