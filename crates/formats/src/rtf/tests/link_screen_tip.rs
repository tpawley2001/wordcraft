//! A hyperlink's ScreenTip: the `\o "text"` switch of its HYPERLINK field (ECMA-376 Part 1
//! §17.16.5.25), read into the linked runs and written back.

use super::*;
use wordcraft_doc::Document;

/// A document whose only paragraph holds `before`, a HYPERLINK field with the instruction
/// `instr` (RTF-escaped by the caller) showing "link", and " after".
fn open(instr: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}before {{\field{{\*\fldinst{{{instr}}}}}{{\fldrslt{{link}}}}}} after\par}}").as_bytes())
        .unwrap()
}

/// Each run's (text, link, ScreenTip) in the first paragraph.
fn runs(d: &Document) -> Vec<(String, Option<String>, Option<String>)> {
    let p = d.body.iter().find_map(|b| b.as_para()).expect("a paragraph");
    p.run_ranges().map(|(r, props)| (p.text[r].to_string(), props.link.clone().map(|l| l.to_string()), props.link_tip.clone().map(|t| t.to_string()))).collect()
}

/// The (link, ScreenTip) of the run showing "link".
fn link_and_tip(d: &Document) -> (Option<String>, Option<String>) {
    let all = runs(d);
    let (_, link, tip) = all.iter().find(|(t, ..)| t == "link").unwrap_or_else(|| panic!("no run \"link\" in {all:?}"));
    (link.clone(), tip.clone())
}

#[test]
fn the_o_switch_is_the_links_screen_tip() {
    let d = open(r#"HYPERLINK "http://example.com" \\o "Help text""#);
    assert_eq!(link_and_tip(&d), (Some("http://example.com".into()), Some("Help text".into())));
}

#[test]
fn the_switch_may_come_before_the_address_or_with_a_bookmark() {
    let d = open(r#"HYPERLINK \\o "tip" "http://example.com""#);
    assert_eq!(link_and_tip(&d), (Some("http://example.com".into()), Some("tip".into())));
    let d = open(r#"HYPERLINK \\l "heading" \\o "Go""#);
    assert_eq!(link_and_tip(&d), (Some("#heading".into()), Some("Go".into())));
}

#[test]
fn no_switch_or_an_empty_one_is_no_screen_tip() {
    for instr in [r#"HYPERLINK "http://example.com""#, r#"HYPERLINK "http://example.com" \\o """#] {
        assert_eq!(link_and_tip(&open(instr)), (Some("http://example.com".into()), None), "{instr}");
    }
}

#[test]
fn the_screen_tip_stays_inside_its_field() {
    let d = open(r#"HYPERLINK "http://example.com" \\o "Help text""#);
    for (text, link, tip) in runs(&d) {
        if text != "link" {
            assert_eq!((link, tip), (None, None), "{text:?}");
        }
    }
    // A second link without a ScreenTip does not inherit the first one's.
    let rtf = br#"{\rtf1\ansi{\field{\*\fldinst{HYPERLINK "http://a.example" \\o "first"}}{\fldrslt{one}}} {\field{\*\fldinst{HYPERLINK "http://b.example"}}{\fldrslt{two}}}\par}"#;
    let all = runs(&import(rtf).unwrap());
    let two = all.iter().find(|(t, ..)| t == "two").unwrap_or_else(|| panic!("{all:?}"));
    assert_eq!((two.1.as_deref(), two.2.as_deref()), (Some("http://b.example"), None));
}

#[test]
fn the_screen_tip_is_written_back_as_the_o_switch() {
    let d = open(r#"HYPERLINK "http://example.com" \\o "Help text""#);
    let rtf = export(&d);
    assert!(rtf.contains(r#"HYPERLINK "http://example.com" \\o "Help text""#), "{rtf}");
    assert_eq!(link_and_tip(&import(rtf.as_bytes()).unwrap()), (Some("http://example.com".into()), Some("Help text".into())));
}

#[test]
fn a_link_without_a_screen_tip_writes_no_o_switch() {
    let rtf = export(&open(r#"HYPERLINK "http://example.com""#));
    assert!(rtf.contains(r#"HYPERLINK "http://example.com""#), "{rtf}");
    assert!(!rtf.contains(r"\\o "), "{rtf}");
}

#[test]
fn quotes_backslashes_braces_and_other_scripts_round_trip_in_a_screen_tip() {
    // In the field's quotes a backslash escapes the next character; in RTF text `\\`, `\{`, `\}`
    // and `\uN?` are the escapes. So this instruction's tip is: Say "hi" a{b}\c é 日本
    let d = open(r#"HYPERLINK "http://example.com" \\o "Say \\"hi\\" a\{b\}\\\\c \u233? \u26085?\u26412?""#);
    let want = "Say \"hi\" a{b}\\c \u{e9} \u{65e5}\u{672c}";
    assert_eq!(link_and_tip(&d), (Some("http://example.com".into()), Some(want.into())));
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(link_and_tip(&back), (Some("http://example.com".into()), Some(want.into())));
    assert_eq!(runs(&back).iter().map(|(t, ..)| t.as_str()).collect::<String>(), "before link after");
}

#[test]
fn a_hostile_screen_tip_stays_text() {
    let tip = r"x\par }{\b y".repeat(400);
    let mut d = open(r#"HYPERLINK "http://example.com" \\o "Help""#);
    for b in &mut d.body {
        if let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(b) {
            for r in p.runs.iter_mut().filter(|r| r.props.link.is_some()) {
                r.props.link_tip = Some(tip.clone().into());
            }
        }
    }
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(runs(&back).iter().map(|(t, ..)| t.as_str()).collect::<String>(), "before link after");
    let (link, got) = link_and_tip(&back);
    assert_eq!(link.as_deref(), Some("http://example.com"));
    let got = got.expect("the ScreenTip survives");
    assert!(tip.starts_with(&got), "a kept or cut tip, never other text: {got:?}");
}
