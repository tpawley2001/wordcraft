//! A file named `.doc` or `.dot` whose bytes are plain text (no compound-file header) opens as
//! text, as Word opens it; binary bytes and broken compound files are still refused, and a
//! `.doc` holding RTF still opens as RTF.

use wordcraft_engine::io;

/// The document's text through the plain-text save.
fn text_of(name: &str, bytes: &[u8]) -> String {
    let doc = io::open_bytes(name, bytes).unwrap_or_else(|e| panic!("{name} should open: {e}"));
    String::from_utf8(io::save_bytes("out.txt", &doc).expect("text save")).expect("utf-8 text")
}

const LINES: &[u8] = b"Licensed under the terms below.\r\nYou may not use this file except in compliance.\r\n\r\nLast line.\r\n";

#[test]
fn a_doc_holding_plain_text_opens_like_a_txt() {
    assert_eq!(text_of("notes.doc", LINES), text_of("notes.txt", LINES));
}

#[test]
fn a_dot_holding_plain_text_opens_like_a_txt() {
    let bytes = b"Dear reader,\nThe template text.\n";
    assert_eq!(text_of("letter.dot", bytes), text_of("letter.txt", bytes));
}

#[test]
fn a_doc_holding_utf8_text_with_a_bom_keeps_its_characters() {
    let bytes = "\u{feff}Grüße aus Köln\r\n".as_bytes();
    let text = text_of("bom.doc", bytes);
    assert!(text.contains("Grüße aus Köln"), "{text:?}");
    assert_eq!(text, text_of("bom.txt", bytes));
}

#[test]
fn a_doc_holding_json_opens_as_text_not_as_a_wordcraft_document() {
    let bytes = b"[\n{\n\"Content-Type\": \"text/plain\",\n\"_comment\": \"simplified\"\n}\n]\n";
    let text = text_of("list.doc", bytes);
    assert!(text.contains("\"Content-Type\": \"text/plain\","), "{text:?}");
}

#[test]
fn binary_bytes_named_doc_are_still_refused() {
    let bytes = [0xCBu8, 0x0A, 0x01, 0xF6, 0x01, 0xCB, 0xC0, 0x0A, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0xFF, 0x00];
    assert!(io::open_bytes("junk.doc", &bytes).is_err());
}

#[test]
fn a_broken_compound_file_named_doc_is_still_refused() {
    let mut bytes = vec![0xD0u8, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    bytes.resize(512, 0);
    assert!(io::open_bytes("broken.doc", &bytes).is_err());
}

#[test]
fn a_doc_holding_rtf_still_opens_as_rtf() {
    let text = text_of("r.doc", b"{\\rtf1\\ansi Hello RTF\\par}");
    assert_eq!(text.trim_end(), "Hello RTF");
}
