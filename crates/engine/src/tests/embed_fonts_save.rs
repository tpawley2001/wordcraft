//! File › Options › Save › Embed fonts in the file (#6329): `document.fontEmbedding` and a .docx
//! save that writes the installed fonts the document uses into the package, obfuscated as
//! ECMA-376 Part 1 §17.8.1 describes. The bundled JetBrains Mono (TrueType, installable) is the
//! installed font these tests embed.

use std::io::Read;

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id} {v}: {e:?}"))
}

/// The bundled font file the database draws JetBrains Mono with.
fn jetbrains() -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts/JetBrainsMono-Regular.ttf")).unwrap()
}

/// A document whose text is set in `family`.
fn text_in(family: &str) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text("Embedded text"));
    run(&mut s, "select.all", json!({}));
    run(&mut s, "format.font", json!({"name": family}));
    s
}

fn save(s: &Session) -> Vec<u8> {
    crate::io::save_bytes("e.docx", &s.doc).unwrap()
}

/// The part `name` of the package `bytes`, if there is one.
fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut f = zip.by_name(name).ok()?;
    let mut out = Vec::new();
    f.read_to_end(&mut out).unwrap();
    Some(out)
}

fn names(bytes: &[u8]) -> Vec<String> {
    let zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    zip.file_names().map(str::to_string).collect()
}

fn settings_xml(bytes: &[u8]) -> String {
    String::from_utf8(part(bytes, "word/settings.xml").expect("settings.xml")).unwrap()
}

#[test]
fn font_embedding_is_off_by_default_and_one_undo_step() {
    let mut s = text_in("JetBrains Mono");
    assert_eq!(run(&mut s, "document.fontEmbedding", json!({})), json!({"embed": false, "subset": false, "skipSystemFonts": true}));
    let on = run(&mut s, "document.fontEmbedding", json!({"embed": true, "subset": true}));
    assert_eq!(on, json!({"embed": true, "subset": true, "skipSystemFonts": true}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(run(&mut s, "document.fontEmbedding", json!({})), json!({"embed": false, "subset": false, "skipSystemFonts": true}));
    for bad in [json!({"embed": 1}), json!({"subset": "yes"}), json!({"skipSystemFonts": null})] {
        assert!(s.run("document.fontEmbedding", &bad).is_err(), "{bad} is refused");
    }
}

#[test]
fn off_writes_no_font_parts_and_no_setting() {
    let s = text_in("JetBrains Mono");
    let bytes = save(&s);
    assert!(!names(&bytes).iter().any(|n| n.starts_with("word/fonts/")), "{:?}", names(&bytes));
    let xml = settings_xml(&bytes);
    for el in ["w:embedTrueTypeFonts", "w:saveSubsetFonts", "w:embedSystemFonts"] {
        assert!(!xml.contains(el), "{el} written while off: {xml}");
    }
}

#[test]
fn settings_xml_carries_the_three_settings_and_they_read_back() {
    let cases = [
        (json!({"embed": true}), [true, false, false]),
        (json!({"embed": true, "subset": true}), [true, true, false]),
        (json!({"embed": true, "skipSystemFonts": false}), [true, false, true]),
    ];
    for (set, [embed, subset, system]) in cases {
        let mut s = text_in("JetBrains Mono");
        run(&mut s, "document.fontEmbedding", set.clone());
        let bytes = save(&s);
        let xml = settings_xml(&bytes);
        assert_eq!(xml.contains("<w:embedTrueTypeFonts"), embed, "{set}: {xml}");
        assert_eq!(xml.contains("<w:saveSubsetFonts"), subset, "{set}: {xml}");
        assert_eq!(xml.contains("<w:embedSystemFonts"), system, "{set}: {xml}");
        // Schema order (§17.15.1): embedTrueTypeFonts, embedSystemFonts, saveSubsetFonts.
        let at = |el: &str| xml.find(el);
        if let (Some(a), Some(b)) = (at("<w:embedTrueTypeFonts"), at("<w:saveSubsetFonts")) {
            assert!(a < b, "{xml}");
        }
        if let (Some(a), Some(b)) = (at("<w:embedTrueTypeFonts"), at("<w:embedSystemFonts")) {
            assert!(a < b, "{xml}");
        }
        let mut back = Session::new(crate::io::open_bytes("e.docx", &bytes).unwrap());
        assert_eq!(
            run(&mut back, "document.fontEmbedding", json!({})),
            json!({"embed": embed, "subset": subset, "skipSystemFonts": !system}),
            "{set} reads back"
        );
    }
}

#[test]
fn an_installed_font_is_embedded_obfuscated_and_reads_back_as_the_same_file() {
    let mut s = text_in("JetBrains Mono");
    run(&mut s, "document.fontEmbedding", json!({"embed": true}));
    let bytes = save(&s);
    let parts: Vec<String> = names(&bytes).into_iter().filter(|n| n.starts_with("word/fonts/")).collect();
    assert_eq!(parts.len(), 1, "one face (JetBrains Mono has only its Regular file bundled): {parts:?}");
    assert!(parts[0].ends_with(".odttf"), "{parts:?}");
    let raw = part(&bytes, &parts[0]).unwrap();
    let font = jetbrains();
    assert_eq!(raw.len(), font.len());
    assert_ne!(raw[..32], font[..32], "the first 32 bytes are obfuscated");
    assert_eq!(raw[32..], font[32..], "the rest is the font file as it is");
    let types = String::from_utf8(part(&bytes, "[Content_Types].xml").unwrap()).unwrap();
    assert!(types.contains("application/vnd.openxmlformats-officedocument.obfuscatedFont"), "{types}");
    let table = String::from_utf8(part(&bytes, "word/fontTable.xml").expect("a font table")).unwrap();
    assert!(table.contains(r#"w:name="JetBrains Mono""#) && table.contains("w:embedRegular") && table.contains("w:fontKey=\"{"), "{table}");
    let reopened = crate::io::open_bytes("e.docx", &bytes).unwrap();
    let embedded = wordcraft_docx::embedded_fonts(&reopened);
    assert_eq!(embedded.len(), 1, "{:?}", embedded.iter().map(|f| &f.name).collect::<Vec<_>>());
    assert_eq!(embedded[0].name, "JetBrains Mono");
    assert!(embedded[0].data == font, "de-obfuscated with its key, it is the installed file");
}

#[test]
fn a_missing_font_is_not_embedded_nor_its_substitute() {
    let mut s = text_in("Wordcraft Nowhere Face");
    run(&mut s, "document.fontEmbedding", json!({"embed": true}));
    let bytes = save(&s);
    assert!(!names(&bytes).iter().any(|n| n.starts_with("word/fonts/")), "{:?}", names(&bytes));
    assert!(settings_xml(&bytes).contains("<w:embedTrueTypeFonts"), "the setting is still saved");
}

#[test]
fn saving_again_keeps_one_copy_of_each_face() {
    let mut s = text_in("JetBrains Mono");
    run(&mut s, "document.fontEmbedding", json!({"embed": true}));
    let first = save(&s);
    let again = save(&Session::new(crate::io::open_bytes("e.docx", &first).unwrap()));
    let parts = names(&again).into_iter().filter(|n| n.starts_with("word/fonts/")).count();
    assert_eq!(parts, 1, "{:?}", names(&again));
    let reopened = crate::io::open_bytes("e.docx", &again).unwrap();
    assert_eq!(wordcraft_docx::embedded_fonts(&reopened).len(), 1);
}
