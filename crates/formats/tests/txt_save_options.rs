//! Plain-text save with Word's File Conversion choices: encoding, line ends, character substitution.
//! Expected bytes computed with Python's codecs (cp1252, cp437, utf-8, utf-16-le/be).

use wordcraft_doc::Document;
use wordcraft_formats::txt::{LineEnd, TxtEncoding, TxtSaveOptions, export_text, export_with, unencodable};

const LINE: &str = "Caf\u{e9} \u{201c}quoted\u{201d} \u{2265} 5";

fn opts(encoding: TxtEncoding, line_end: LineEnd, allow_substitution: bool) -> TxtSaveOptions {
    TxtSaveOptions { encoding, line_end, allow_substitution, insert_line_breaks: false }
}

fn doc(text: &str) -> Document {
    Document::from_text(text)
}

#[test]
fn default_is_windows_crlf_without_substitution() {
    assert_eq!(TxtSaveOptions::default(), opts(TxtEncoding::Windows, LineEnd::CrLf, false));
}

#[test]
fn windows_code_page_writes_a_question_mark_for_what_it_cant_hold() {
    let out = export_with(&doc(LINE), &opts(TxtEncoding::Windows, LineEnd::CrLf, false));
    assert_eq!(out, vec![67, 97, 102, 233, 32, 147, 113, 117, 111, 116, 101, 100, 148, 32, 63, 32, 53]);
    assert!(!String::from_utf8_lossy(&out).contains("&#"), "no HTML numeric references");
}

#[test]
fn ms_dos_is_ibm437() {
    let out = export_with(&doc(LINE), &opts(TxtEncoding::MsDos, LineEnd::CrLf, false));
    assert_eq!(out, vec![67, 97, 102, 130, 32, 63, 113, 117, 111, 116, 101, 100, 63, 32, 242, 32, 53]);
}

#[test]
fn substitution_straightens_quotes_the_code_page_lacks() {
    let out = export_with(&doc(LINE), &opts(TxtEncoding::MsDos, LineEnd::CrLf, true));
    assert_eq!(out, vec![67, 97, 102, 130, 32, 34, 113, 117, 111, 116, 101, 100, 34, 32, 242, 32, 53]);
    // windows-1252 holds the curly quotes and the ellipsis: substitution leaves them alone.
    let out = export_with(&doc("\u{201c}\u{2026}\u{2013}"), &opts(TxtEncoding::Windows, LineEnd::CrLf, true));
    assert_eq!(out, vec![147, 133, 150]);
    // IBM437 has no ellipsis or en dash: three dots and a hyphen.
    let out = export_with(&doc("\u{2026}\u{2013}"), &opts(TxtEncoding::MsDos, LineEnd::CrLf, true));
    assert_eq!(out, b"...-".to_vec());
}

#[test]
fn unicode_encodings_carry_their_byte_order_mark() {
    let d = doc("\u{e9}\u{2265}");
    assert_eq!(export_with(&d, &opts(TxtEncoding::Utf8, LineEnd::CrLf, false)), vec![0xEF, 0xBB, 0xBF, 195, 169, 226, 137, 165]);
    assert_eq!(export_with(&d, &opts(TxtEncoding::Utf16Le, LineEnd::CrLf, false)), vec![0xFF, 0xFE, 233, 0, 101, 34]);
    assert_eq!(export_with(&d, &opts(TxtEncoding::Utf16Be, LineEnd::CrLf, false)), vec![0xFE, 0xFF, 0, 233, 34, 101]);
}

#[test]
fn a_labelled_code_page() {
    let out = export_with(&doc("\u{416}"), &opts(TxtEncoding::Label("windows-1251".into()), LineEnd::CrLf, false));
    assert_eq!(out, vec![198]);
}

#[test]
fn line_ends() {
    let d = doc("a\nb");
    let cases =
        [(LineEnd::CrLf, b"a\r\nb".to_vec()), (LineEnd::Cr, b"a\rb".to_vec()), (LineEnd::Lf, b"a\nb".to_vec()), (LineEnd::LfCr, b"a\n\rb".to_vec())];
    for (end, want) in cases {
        assert_eq!(export_with(&d, &opts(TxtEncoding::Windows, end, false)), want, "{end:?}");
    }
}

#[test]
fn preview_text_and_red_characters() {
    let d = doc(LINE);
    let text = export_text(&d);
    assert_eq!(text, LINE);
    // Char indices: 5 = left quote, 12 = right quote, 14 = the greater-than-or-equal sign.
    assert_eq!(unencodable(&text, &opts(TxtEncoding::MsDos, LineEnd::CrLf, false)), vec![5, 12]);
    assert_eq!(unencodable(&text, &opts(TxtEncoding::MsDos, LineEnd::CrLf, true)), Vec::<usize>::new());
    assert_eq!(unencodable(&text, &opts(TxtEncoding::Windows, LineEnd::CrLf, true)), vec![14]);
    assert!(unencodable(&text, &opts(TxtEncoding::Utf8, LineEnd::CrLf, false)).is_empty());
}
