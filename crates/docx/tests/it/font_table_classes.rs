//! A document with no font table kept from a .docx (opened from .doc, RTF, Word 6, or new) saves a generated
//! `word/fontTable.xml` (ECMA-376 Part 1 §17.8.3.11) holding its font classes, so `Settings::roman_fonts`
//! (`w:family w:val="roman"`) and `Settings::symbol_class_fonts` (symbol charset + pictorial PANOSE) survive a
//! save and reopen. Without it a missing roman font is substituted differently after the save (a Word 97 CV
//! with the Mac font "New York" came back one page longer).

use wordcraft_doc::Document;

fn part(bytes: &[u8], name: &str) -> Option<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    std::io::Read::read_to_string(&mut f, &mut s).unwrap();
    Some(s)
}

fn font_table_target(bytes: &[u8]) -> Option<String> {
    let rels = part(bytes, "word/_rels/document.xml.rels").unwrap();
    let n = rels.matches("relationships/fontTable\"").count();
    assert!(n <= 1, "{n} fontTable relationships in {rels}");
    let at = rels.find("relationships/fontTable\"")?;
    let start = rels[..at].rfind("<Relationship ").unwrap();
    let end = start + rels[start..].find("/>").unwrap();
    let tag = &rels[start..end];
    let t = tag.split("Target=\"").nth(1).unwrap().split('"').next().unwrap();
    Some(format!("word/{}", t.trim_start_matches('/').trim_start_matches("word/")))
}

fn doc(roman: &[&str], symbol: &[&str]) -> Document {
    let mut d = Document::new();
    d.settings.roman_fonts = roman.iter().map(|s| s.to_string()).collect();
    d.settings.symbol_class_fonts = symbol.iter().map(|s| s.to_string()).collect();
    d
}

fn rt(d: &Document) -> Document {
    wordcraft_docx::read(&wordcraft_docx::write(d).unwrap()).unwrap()
}

#[test]
fn roman_and_symbol_classes_round_trip() {
    let d = doc(&["New York", "CG Times"], &["wp phonetic"]);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let target = font_table_target(&bytes).expect("a fontTable relationship");
    let xml = part(&bytes, &target).expect("the font table part");
    assert!(xml.contains("<w:fonts"), "{xml}");
    let ct = part(&bytes, "[Content_Types].xml").unwrap();
    assert!(ct.contains("wordprocessingml.fontTable+xml"), "{ct}");
    let back = rt(&d);
    assert_eq!(back.settings.roman_fonts, vec!["New York".to_string(), "CG Times".to_string()]);
    assert_eq!(back.settings.symbol_class_fonts, vec!["wp phonetic".to_string()]);
}

#[test]
fn a_font_in_both_lists_is_listed_once() {
    let d = doc(&["Symbol", "Times New Roman"], &["symbol"]);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let xml = part(&bytes, &font_table_target(&bytes).unwrap()).unwrap();
    assert_eq!(xml.to_lowercase().matches("w:name=\"symbol\"").count(), 1, "{xml}");
    let back = rt(&d);
    assert_eq!(back.settings.roman_fonts, vec!["Symbol".to_string(), "Times New Roman".to_string()]);
    assert_eq!(back.settings.symbol_class_fonts, vec!["symbol".to_string()]);
}

#[test]
fn names_are_escaped() {
    let d = doc(&["A & B Roman"], &[]);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let xml = part(&bytes, &font_table_target(&bytes).unwrap()).unwrap();
    assert!(xml.contains("A &amp; B Roman"), "{xml}");
    assert_eq!(rt(&d).settings.roman_fonts, vec!["A & B Roman".to_string()]);
}

#[test]
fn saving_twice_keeps_one_table() {
    let d = doc(&["Palatino"], &["wingdings"]);
    let once = rt(&d);
    let bytes = wordcraft_docx::write(&once).unwrap();
    assert!(font_table_target(&bytes).is_some());
    let twice = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(twice.settings.roman_fonts, vec!["Palatino".to_string()]);
    assert_eq!(twice.settings.symbol_class_fonts, vec!["wingdings".to_string()]);
}

#[test]
fn no_classes_reopen_empty() {
    let back = rt(&Document::new());
    assert!(back.settings.roman_fonts.is_empty());
    assert!(back.settings.symbol_class_fonts.is_empty());
}
