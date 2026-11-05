//! Word 6.0/95 paragraph numbering (`{\*\pn …}`, RTF 1.9.1 "Bullets and Numbering: Word 6.0 and Word 95 RTF"),
//! as WordPad and Word 95 write it: it imports as list numbering, and the cached `{\pntext …}` is not body text.

use super::*;
use wordcraft_doc::numbering::Counters;

const HEAD: &str = "{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fswiss Arial;}{\\f3\\fnil\\fcharset2 Symbol;}}\n";

/// Each body paragraph's text and the list label Word would show for it, in order.
fn labels(rtf: &str) -> Vec<(String, Option<String>)> {
    let doc = import(rtf.as_bytes()).unwrap();
    let mut counters = Counters::default();
    doc_paras(&doc)
        .into_iter()
        .map(|p| {
            let label =
                p.props.numbering.filter(|n| n.num != 0).and_then(|n| counters.next_label(&doc.numbering, n.num, n.level)).map(|(text, _)| text);
            (p.plain_text(), label)
        })
        .collect()
}

/// The bullet's level definition for the first list paragraph.
fn first_level(rtf: &str) -> (String, Option<String>) {
    let doc = import(rtf.as_bytes()).unwrap();
    let n = doc_paras(&doc).into_iter().find_map(|p| p.props.numbering.filter(|n| n.num != 0)).expect("a list paragraph");
    let (text, level) = Counters::default().next_label(&doc.numbering, n.num, n.level).expect("a label");
    (text, level.chr.font.clone())
}

fn item(pn: &str, text: &str) -> String {
    format!("\\pard{{\\pntext\\f3\\'B7\\tab}}{{\\*\\pn{pn}}}\\f0 {text}\\par\n")
}

#[test]
fn bullet_paragraphs_get_the_pntxtb_bullet_in_the_pnf_font() {
    let rtf = format!(
        "{HEAD}\\pard\\f0 Intro\\par\n{}{}}}",
        item("\\pnlvlblt\\pnf3\\pnindent0{\\pntxtb\\'B7}", "One"),
        item("\\pnlvlblt\\pnf3\\pnindent0{\\pntxtb\\'B7}", "Two")
    );
    let got = labels(&rtf);
    assert_eq!(got.len(), 3, "{got:?}");
    assert_eq!(got[0], ("Intro".to_string(), None));
    for (text, label) in &got[1..] {
        assert!(!text.contains('\u{b7}') && !text.contains('\u{f0b7}'), "pntext leaked into the text: {text:?}");
        assert!(label.is_some(), "{text:?} has no bullet");
    }
    assert_eq!(got[1].0, "One");
    assert_eq!(got[2].0, "Two");
    let (bullet, font) = first_level(&rtf);
    match bullet.as_str() {
        // Symbol's 0xB7 kept as the symbol-font code point or the byte, in the Symbol font.
        "\u{f0b7}" | "\u{b7}" => assert_eq!(font.as_deref(), Some("Symbol")),
        "\u{2022}" => {}
        other => panic!("bullet {other:?}"),
    }
}

#[test]
fn numbered_paragraphs_count_with_text_after() {
    let pn = "\\pnlvlbody\\pndec\\pnstart1\\pnindent360{\\pntxta .}";
    let rtf = format!("{HEAD}{}{}{}}}", item(pn, "a"), item(pn, "b"), item(pn, "c"));
    let got: Vec<_> = labels(&rtf).into_iter().map(|(_, l)| l).collect();
    assert_eq!(got, [Some("1.".to_string()), Some("2.".to_string()), Some("3.".to_string())]);
}

#[test]
fn number_formats_and_start_value() {
    for (fmt, want) in [("\\pnucrm", ["IV.", "V."]), ("\\pnlcrm", ["iv.", "v."]), ("\\pnucltr", ["D.", "E."]), ("\\pnlcltr", ["d.", "e."])] {
        let pn = format!("\\pnlvlbody{fmt}\\pnstart4{{\\pntxta .}}");
        let rtf = format!("{HEAD}{}{}}}", item(&pn, "x"), item(&pn, "y"));
        let got: Vec<_> = labels(&rtf).into_iter().map(|(_, l)| l).collect();
        assert_eq!(got, want.map(|s| Some(s.to_string())), "{fmt}");
    }
}

#[test]
fn continuation_paragraphs_show_no_label_and_do_not_advance_the_count() {
    let body = "\\pnlvlbody\\pndec\\pnstart1{\\pntxta .}";
    let cont = "\\pnlvlcont\\pndec{\\pntxta .}";
    let rtf = format!("{HEAD}{}{}{}}}", item(body, "first"), item(cont, "more"), item(body, "second"));
    let got: Vec<_> = labels(&rtf).into_iter().map(|(_, l)| l).collect();
    assert_eq!(got, [Some("1.".to_string()), None, Some("2.".to_string())]);
}

#[test]
fn an_ls_list_wins_over_pn() {
    // Word 97+ writes both; the \ls list (decimal) is the one shown, not the \pn bullet.
    let rtf = format!(
        "{HEAD}{{\\*\\listtable{{\\list\\listtemplateid1{{\\listlevel\\levelnfc0\\levelstartat1{{\\leveltext \\'02\\'00.;}}{{\\levelnumbers \\'01;}}}}\\listid7}}}}\
{{\\*\\listoverridetable{{\\listoverride\\listid7\\listoverridecount0\\ls1}}}}\n\
\\pard{{\\listtext 1.\\tab}}{{\\*\\pn\\pnlvlblt\\pnf3{{\\pntxtb\\'B7}}}}\\ls1\\f0 Item\\par\n}}"
    );
    let got = labels(&rtf);
    assert_eq!(got, [("Item".to_string(), Some("1.".to_string()))]);
}
