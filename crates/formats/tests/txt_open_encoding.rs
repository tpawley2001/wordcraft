//! Opening plain text in a chosen encoding (Word's File Conversion on open). Bytes from Python's codecs.

use wordcraft_doc::Document;
use wordcraft_formats::txt::{LineEnd, TxtEncoding, TxtSaveOptions, decode_with, detect, export_with, import_with};

fn texts(d: &Document) -> Vec<String> {
    d.body.iter().filter_map(|b| if let wordcraft_doc::Block::Para(p) = &**b { Some(p.plain_text()) } else { None }).collect()
}

#[test]
fn detect_reads_what_decode_reads() {
    assert_eq!(detect(&[0xEF, 0xBB, 0xBF, b'h', b'i']), TxtEncoding::Utf8);
    assert_eq!(detect(&[0xFF, 0xFE, b'h', 0]), TxtEncoding::Utf16Le);
    assert_eq!(detect(&[0xFE, 0xFF, 0, b'h']), TxtEncoding::Utf16Be);
    assert_eq!(detect(&[b'a', 0, b'b', 0, b'c', 0, b'd', 0]), TxtEncoding::Utf16Le);
    assert_eq!(detect(&[0, b'a', 0, b'b', 0, b'c', 0, b'd']), TxtEncoding::Utf16Be);
    assert_eq!(detect("h\u{e9}llo".as_bytes()), TxtEncoding::Utf8);
    assert_eq!(detect(b"plain ascii"), TxtEncoding::Utf8);
}

#[test]
fn decode_in_the_chosen_encoding() {
    let cases: [(&[u8], TxtEncoding, &str); 8] = [
        (&[0xE9], TxtEncoding::Windows, "\u{e9}"),
        (&[0x82], TxtEncoding::Windows, "\u{201a}"),
        (&[0x82], TxtEncoding::MsDos, "\u{e9}"),
        (&[0xC6], TxtEncoding::Label("windows-1251".into()), "\u{416}"),
        (&[0xEF, 0xBB, 0xBF, b'h', b'i'], TxtEncoding::Utf8, "hi"),
        (&[0xFF, 0xFE, b'h', 0], TxtEncoding::Utf16Le, "h"),
        (&[0xFE, 0xFF, 0, b'h'], TxtEncoding::Utf16Be, "h"),
        (&[0xE9], TxtEncoding::Label("nope".into()), "\u{e9}"),
    ];
    for (bytes, enc, want) in cases {
        assert_eq!(decode_with(bytes, &enc), want, "{enc:?} {bytes:?}");
    }
}

#[test]
fn a_saved_file_opens_back_in_its_encoding() {
    let page = wordcraft_doc::SectionProps::default();
    let encs = [
        (TxtEncoding::Windows, "Caf\u{e9} \u{201c}x\u{201d}\nline two"),
        (TxtEncoding::MsDos, "Caf\u{e9} \u{2265} 5\nline two"),
        (TxtEncoding::Utf8, "\u{416}\u{e9}\u{4e2d}\nline two"),
        (TxtEncoding::Utf16Le, "\u{416}\u{e9}\u{4e2d}\nline two"),
        (TxtEncoding::Utf16Be, "\u{416}\u{e9}\u{4e2d}\nline two"),
        (TxtEncoding::Label("windows-1251".into()), "\u{416}\u{436}\nline two"),
        (TxtEncoding::Label("shift_jis".into()), "\u{65e5}\u{672c}\nline two"),
    ];
    for (enc, text) in encs {
        let d = Document::from_text(text);
        let bytes = export_with(&d, &TxtSaveOptions { encoding: enc.clone(), line_end: LineEnd::CrLf, allow_substitution: false });
        assert_eq!(texts(&import_with(&bytes, &enc, &page)), texts(&d), "{enc:?}");
    }
}
