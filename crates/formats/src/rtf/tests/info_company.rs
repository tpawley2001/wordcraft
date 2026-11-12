//! Company, Manager and Hyperlink base (`{\company …}`, `{\manager …}`, `{\hlinkbase …}` in the
//! information group, RTF 1.9.1): the RTF form of DOCX `docProps/app.xml` `Company`, `Manager` and
//! `HyperlinkBase`. They live where the DOCX reader keeps them: the `wordcraft:app.kept`
//! passthrough entry, one `Name\tvalue\n` line each.

use super::*;
use std::sync::Arc;
use wordcraft_doc::StoryRef;

/// The DOCX code's key for the kept extended properties.
const KEPT: &str = "wordcraft:app.kept";

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

fn with_info(info: &str) -> String {
    format!(r"{{\rtf1\ansi\ansicpg1252{{\info{{\title T}}{info}}}\pard Body\par}}")
}

/// The kept extended properties as `(name, value)`, in stored order.
fn kept(d: &Document) -> Vec<(String, String)> {
    let Some(bytes) = d.passthrough.get(KEPT) else { return Vec::new() };
    let text = String::from_utf8(bytes.to_vec()).expect("the kept properties are UTF-8");
    text.lines()
        .map(|line| {
            let (name, value) = line.split_once('\t').expect("every line is name, tab, value");
            (name.to_string(), value.to_string())
        })
        .collect()
}

fn value(d: &Document, name: &str) -> Option<String> {
    let found: Vec<String> = kept(d).into_iter().filter(|(n, _)| n == name).map(|(_, v)| v).collect();
    assert!(found.len() <= 1, "{name} is stored once, found {found:?}");
    found.into_iter().next()
}

fn with_kept(lines: &str) -> Document {
    let mut d = Document::new();
    d.core.title = "T".into();
    d.passthrough.insert(KEPT.into(), Arc::new(lines.as_bytes().to_vec()));
    d
}

const ALL: &str = r"{\manager Ann Lee}{\company Acme Ltd}{\hlinkbase https://example.com/docs/}";

#[test]
fn company_manager_and_hyperlink_base_are_read() {
    let d = doc(&with_info(ALL));
    assert_eq!(value(&d, "Manager").as_deref(), Some("Ann Lee"));
    assert_eq!(value(&d, "Company").as_deref(), Some("Acme Ltd"));
    assert_eq!(value(&d, "HyperlinkBase").as_deref(), Some("https://example.com/docs/"));
    assert_eq!(d.core.title, "T");
}

#[test]
fn they_are_stored_as_the_docx_reader_stores_them() {
    // Given in another order in the file; stored as Manager, Company, HyperlinkBase lines.
    let d = doc(&with_info(r"{\hlinkbase https://example.com/docs/}{\company Acme Ltd}{\manager Ann Lee}"));
    let bytes = d.passthrough.get(KEPT).expect("the kept properties entry");
    assert_eq!(
        String::from_utf8(bytes.to_vec()).unwrap(),
        "Manager\tAnn Lee\nCompany\tAcme Ltd\nHyperlinkBase\thttps://example.com/docs/\n"
    );
}

#[test]
fn one_property_alone_is_kept() {
    let d = doc(&with_info(r"{\company Acme Ltd}"));
    assert_eq!(kept(&d), [("Company".to_string(), "Acme Ltd".to_string())]);
}

#[test]
fn a_file_without_them_has_no_entry() {
    let d = doc(&with_info(r"{\operator Bob}{\category Reports}"));
    assert!(!d.passthrough.contains_key(KEPT));
    assert_eq!(d.core.last_modified_by, "Bob");
}

#[test]
fn empty_groups_are_not_kept() {
    let d = doc(&with_info(r"{\company }{\manager}{\hlinkbase   }"));
    assert!(!d.passthrough.contains_key(KEPT));
    let d = doc(&with_info(r"{\company }{\manager Ann Lee}"));
    assert_eq!(kept(&d), [("Manager".to_string(), "Ann Lee".to_string())]);
}

#[test]
fn their_text_never_reaches_the_body() {
    let d = doc(&with_info(ALL));
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Body");
}

#[test]
fn outside_the_information_group_they_are_still_skipped() {
    let d = doc(r"{\rtf1\ansi{\company Loose}{\manager Loose}{\hlinkbase Loose}\pard Body\par}");
    assert!(!d.passthrough.contains_key(KEPT));
    assert_eq!(d.plain_text(StoryRef::Body).trim(), "Body");
}

#[test]
fn escaped_and_unicode_text_is_decoded() {
    let d = doc(&with_info(r"{\company Caf\'e9 \u8364?uro \{A\}}{\manager \u26085?\u26412?}"));
    assert_eq!(value(&d, "Company").as_deref(), Some("Caf\u{e9} \u{20ac}uro {A}"));
    assert_eq!(value(&d, "Manager").as_deref(), Some("\u{65e5}\u{672c}"));
}

#[test]
fn surrounding_space_is_trimmed_and_tabs_and_line_ends_become_spaces() {
    let d = doc(&with_info(r"{\company   A\u9?B\u10?C\u13?D  }"));
    assert_eq!(value(&d, "Company").as_deref(), Some("A B C D"));
    // One line per property, whatever the text held.
    assert_eq!(kept(&d).len(), 1);
}

#[test]
fn a_repeated_group_keeps_the_last_value() {
    let d = doc(&with_info(r"{\company One}{\manager Ann}{\company Two}"));
    assert_eq!(value(&d, "Company").as_deref(), Some("Two"));
    assert_eq!(value(&d, "Manager").as_deref(), Some("Ann"));
    assert_eq!(kept(&d).len(), 2);
}

#[test]
fn a_long_value_is_cut_to_2048_bytes_on_a_character_boundary() {
    // 3000 × é = 6000 bytes of UTF-8; 2048 bytes hold 1024 of them.
    let long = r"\'e9".repeat(3000);
    let d = doc(&with_info(&format!(r"{{\company {long}}}{{\manager Ann}}")));
    let v = value(&d, "Company").expect("the cut value is kept");
    assert_eq!(v.len(), 2048);
    assert_eq!(v.chars().count(), 1024);
    assert!(v.chars().all(|c| c == '\u{e9}'));
    assert_eq!(value(&d, "Manager").as_deref(), Some("Ann"));

    // x + 1024 × é would end inside a character at 2048 bytes: the cut steps back to 2047.
    let d = doc(&with_info(&format!(r"{{\company x{long}}}")));
    let v = value(&d, "Company").unwrap();
    assert_eq!(v.len(), 2047);
    assert_eq!(v.chars().count(), 1024);
}

#[test]
fn they_are_written_in_the_information_group() {
    let d = with_kept("Manager\tAnn Lee\nCompany\tAcme Ltd\nHyperlinkBase\thttps://example.com/docs/\n");
    let rtf = export(&d);
    let info = rtf.find(r"{\info").expect("an information group");
    for group in [r"{\manager Ann Lee}", r"{\company Acme Ltd}", r"{\hlinkbase https://example.com/docs/}"] {
        let at = rtf.find(group).unwrap_or_else(|| panic!("{group} is written"));
        assert!(at > info, "{group} comes inside the information group");
        assert_eq!(rtf.matches(group).count(), 1);
    }
    assert_eq!(kept(&doc(&rtf)), kept(&d));
}

#[test]
fn a_document_without_them_writes_none() {
    let mut d = Document::new();
    d.core.title = "T".into();
    let rtf = export(&d);
    for word in [r"\manager", r"\company", r"\hlinkbase"] {
        assert!(!rtf.contains(word), "{word} is not written");
    }
}

#[test]
fn values_with_braces_backslashes_and_non_ascii_round_trip() {
    let d = with_kept("Manager\tZo\u{eb} {Ops}\nCompany\tA\\B \u{2013} \u{65e5}\u{672c}\nHyperlinkBase\tC:\\docs\\{x}\\\n");
    let back = doc(&export(&d));
    assert_eq!(value(&back, "Manager").as_deref(), Some("Zo\u{eb} {Ops}"));
    assert_eq!(value(&back, "Company").as_deref(), Some("A\\B \u{2013} \u{65e5}\u{672c}"));
    assert_eq!(value(&back, "HyperlinkBase").as_deref(), Some("C:\\docs\\{x}\\"));
}

#[test]
fn an_rtf_file_round_trips() {
    let d = doc(&with_info(ALL));
    let back = doc(&export(&d));
    assert_eq!(kept(&back), kept(&d));
    assert_eq!(kept(&back).len(), 3);
    assert_eq!(back.core.title, "T");
}

#[test]
fn remove_personal_information_leaves_out_company_and_manager() {
    let mut d = with_kept("Manager\tAnn Lee\nCompany\tAcme Ltd\nHyperlinkBase\thttps://example.com/docs/\n");
    d.settings.remove_personal_info = true;
    let rtf = export(&d);
    assert!(!rtf.contains(r"\manager") && !rtf.contains(r"\company"));
    assert!(rtf.contains(r"{\hlinkbase https://example.com/docs/}"));
    assert!(!rtf.contains("Ann Lee") && !rtf.contains("Acme Ltd"));
}

#[test]
fn other_kept_properties_and_broken_lines_are_not_written() {
    let d = with_kept("Template\tNormal.dotm\nno tab here\nScaleCrop\tfalse\nCompany\tAcme Ltd\nManager\t\n\tstray\n");
    let rtf = export(&d);
    assert!(rtf.contains(r"{\company Acme Ltd}"));
    assert!(!rtf.contains("Normal.dotm") && !rtf.contains("no tab here") && !rtf.contains("stray"));
    // An empty value writes no group.
    assert!(!rtf.contains(r"\manager"));
    assert_eq!(kept(&doc(&rtf)), [("Company".to_string(), "Acme Ltd".to_string())]);
}
