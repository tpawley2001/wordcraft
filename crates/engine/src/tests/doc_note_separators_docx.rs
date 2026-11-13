//! A `.doc` file's own footnote continuation separator and continuation notice (`crates/docbin`,
//! the six separator stories) survive a save as DOCX (`w:footnote w:type="continuationSeparator"`
//! and `w:type="continuationNotice"`) and reading it back. The model is the one the `.doc` reader
//! builds (see `crates/docbin/src/tests/note_separators.rs`).

use wordcraft_doc::para::InlineObject;
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

#[test]
fn doc_continuation_separator_and_notice_survive_a_docx_round_trip() {
    let mut sep = Paragraph::with_text("(Continued from previous page)  ", CharProps::default());
    sep.text.push('\u{FFFC}');
    sep.runs[0].len += '\u{FFFC}'.len_utf8();
    sep.objects = vec![InlineObject::note_separator(false)];
    let notice = Paragraph::with_text("(continued\u{2026}.)", CharProps::default());
    let mut doc = Document::new();
    doc.footnote_separators.continuation_separator = Some(vec![para_block(sep)]);
    doc.footnote_separators.continuation_notice = Some(vec![para_block(notice)]);

    let bytes = crate::io::save_bytes("seps.docx", &doc).unwrap();
    let back = crate::io::open_bytes("seps.docx", &bytes).unwrap();
    let seps = &back.footnote_separators;
    let cont = seps.continuation_separator.as_ref().and_then(|b| b.first()).and_then(|b| b.as_para()).expect("continuation separator");
    assert_eq!(cont.text, "(Continued from previous page)  \u{FFFC}");
    assert_eq!(cont.objects.first().and_then(InlineObject::as_note_separator), Some(false));
    let notice = seps.continuation_notice.as_ref().and_then(|b| b.first()).and_then(|b| b.as_para()).expect("continuation notice");
    assert_eq!(notice.text, "(continued\u{2026}.)");
}
