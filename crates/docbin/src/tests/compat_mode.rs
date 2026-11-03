//! Word opens every Word 97-2003 binary file in Compatibility Mode: the reader lays it out in
//! compatibility mode 11 and takes the compatibility options from the file's DOP ([MS-DOC] §2.7).

use super::{FibSpec, Piece, body_texts, cfb_file, fib_bytes, read, text_doc};

/// FibRgFcLcb97 index of `fcDop`/`lcbDop` ([MS-DOC] §2.5.6).
const DOP_PAIR: usize = 31;

#[test]
fn doc_is_laid_out_in_word_2003_mode() {
    let f = text_doc(&[Piece { compressed: true, text: "Hello\r".into() }], &[], 0);
    let doc = read(&f).expect("opens");
    assert_eq!(doc.settings.compat_mode, wordcraft_doc::WORD2003_COMPAT_MODE);
    assert_eq!(doc.settings.compat_mode, 11);
}

#[test]
fn dop_outside_the_table_stream_keeps_defaults() {
    // Same layout as `text_doc`, with a DOP pair pointing far past the end of the Table stream.
    let text = "Hello\r";
    let word_len = super::TEXT_FC as usize;
    let mut word = vec![0u8; word_len];
    word.extend(text.bytes());
    let piece = Piece { compressed: true, text: text.into() };
    let plc = super::plcpcd(&[piece], super::TEXT_FC);
    let mut clx = vec![0x02u8];
    clx.extend_from_slice(&(plc.len() as u32).to_le_bytes());
    clx.extend_from_slice(&plc);
    let mut table = vec![0u8; super::CLX_AT as usize];
    table.extend_from_slice(&clx);
    let spec = FibSpec {
        ccp_text: text.chars().count() as u32,
        pairs: vec![(33, super::CLX_AT, clx.len() as u32), (DOP_PAIR, 0x00FF_FF00, 0x0200)],
        ..Default::default()
    };
    let fib = fib_bytes(&spec);
    word[..fib.len()].copy_from_slice(&fib);
    let doc = read(&cfb_file(&[("WordDocument", &word), ("0Table", &table)])).expect("opens");
    assert_eq!(body_texts(&doc), ["Hello"]);
    assert_eq!(doc.settings.compat_mode, 11);
    let d = wordcraft_doc::Settings::default();
    assert_eq!(doc.settings.sum_paragraph_spacing, d.sum_paragraph_spacing);
    assert_eq!(doc.settings.do_not_expand_shift_return, d.do_not_expand_shift_return);
    assert_eq!(doc.settings.allow_space_of_same_style_in_table, d.allow_space_of_same_style_in_table);
}
