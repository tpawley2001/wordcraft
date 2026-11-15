//! `\viewkindN` (RTF 1.9.1 document formatting: the view the document was saved in) is the RTF form
//! of DOCX `w:view` (`Settings::view`): 1 Page Layout, 2 Outline, 3 Master Document, 4 Normal (Draft),
//! 5 Online Layout (Web).

use super::*;
use wordcraft_doc::{Document, SavedView};

fn open(header: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{header}\sectd\pard a\par}}").as_bytes()).unwrap()
}

fn viewkinds(rtf: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = rtf;
    while let Some(i) = rest.find(r"\viewkind") {
        let tail = &rest[i + r"\viewkind".len()..];
        let n: String = tail.chars().take_while(char::is_ascii_digit).collect();
        out.push(n);
        rest = tail;
    }
    out
}

#[test]
fn viewkind_is_read() {
    let cases: [(&str, Option<SavedView>); 9] = [
        ("", None),
        (r"\viewkind0", None),
        (r"\viewkind1", None),
        (r"\viewkind2", Some(SavedView::Outline)),
        (r"\viewkind3", Some(SavedView::Outline)),
        (r"\viewkind4", Some(SavedView::Draft)),
        (r"\viewkind5", Some(SavedView::Web)),
        (r"\viewkind9", None),
        (r"\viewkind5\viewkind2", Some(SavedView::Outline)),
    ];
    for (header, want) in cases {
        assert_eq!(open(header).settings.view, want, "header {header:?}");
    }
}

#[test]
fn export_writes_the_documents_view() {
    let cases: [(Option<SavedView>, &str); 4] =
        [(None, "1"), (Some(SavedView::Web), "5"), (Some(SavedView::Outline), "2"), (Some(SavedView::Draft), "4")];
    for (view, want) in cases {
        let mut d = open("");
        d.settings.view = view;
        let rtf = export(&d);
        assert_eq!(viewkinds(&rtf), vec![want.to_string()], "view {view:?}: exactly one \\viewkind{want}");
        assert_eq!(import(rtf.as_bytes()).unwrap().settings.view, view, "round trip keeps {view:?}");
    }
}

#[test]
fn word_print_layout_header_round_trips_as_print() {
    let d = open(r"\viewkind1\viewscale100");
    assert_eq!(d.settings.view, None);
    assert_eq!(viewkinds(&export(&d)), vec!["1".to_string()]);
}
