//! `\ftnlytwnine` (RTF 1.9.1 document formatting: emulate Word 6.0/95/97 footnote placement) is the
//! RTF form of DOCX `w:footnoteLayoutLikeWW8` (`Settings::footnote_layout_like_ww8`).

use super::*;
use wordcraft_doc::Document;

fn open(header: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{header}\sectd\pard a\par}}").as_bytes()).unwrap()
}

#[test]
fn ftnlytwnine_is_read() {
    // Word reads the option as on unless `\ftnlytwnine` turns it off; `\ftnlytwnine0` is ignored.
    assert!(open("").settings.footnote_layout_like_ww8);
    assert!(!open(r"\ftnlytwnine").settings.footnote_layout_like_ww8);
    assert!(!open(r"\ftnlytwnine1").settings.footnote_layout_like_ww8);
    assert!(open(r"\ftnlytwnine0").settings.footnote_layout_like_ww8);
}

#[test]
fn export_writes_ftnlytwnine_only_when_off() {
    let mut d = open("");
    assert!(!export(&d).contains(r"\ftnlytwnine"));
    d.settings.footnote_layout_like_ww8 = false;
    let rtf = export(&d);
    assert!(rtf.contains(r"\ftnlytwnine"));
    assert!(!import(rtf.as_bytes()).unwrap().settings.footnote_layout_like_ww8, "round trip keeps it off");
}
