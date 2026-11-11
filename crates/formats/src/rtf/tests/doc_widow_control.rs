//! Document-level widow/orphan control (`\widowctrl`, RTF 1.9.1 document formatting): without it,
//! Word turns widow control off for paragraphs that don't set `\widctlpar` (measured in Word,
//! 2026-11-10); the writer always writes it, as Word does.

use super::*;
use wordcraft_doc::Document;

fn open(header: &str, body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{header}{body}}}").as_bytes()).unwrap()
}

fn widow(d: &Document) -> Vec<Option<bool>> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| p.props.widow_control).collect()
}

const BODY: &str = r"\pard\widctlpar on\par\pard\nowidctlpar off\par\pard unset\par";

#[test]
fn without_widowctrl_unset_paragraphs_are_off() {
    let d = open("", BODY);
    assert_eq!(widow(&d), [Some(true), Some(false), Some(false)]);
}

#[test]
fn with_widowctrl_unset_paragraphs_stay_default_on() {
    let d = open(r"\widowctrl", BODY);
    assert_eq!(widow(&d), [Some(true), Some(false), None]);
}

#[test]
fn export_writes_widowctrl_before_the_first_paragraph() {
    let out = export(&open(r"\widowctrl", BODY));
    let w = out.find(r"\widowctrl").expect("\\widowctrl written");
    let p = out.find(r"\pard").expect("a paragraph");
    assert!(w < p, "\\widowctrl after the first \\pard: {out}");
}

#[test]
fn round_trip_without_widowctrl_keeps_paragraphs_off() {
    let d = open("", BODY);
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(widow(&back), [Some(true), Some(false), Some(false)]);
}

#[test]
fn round_trip_with_widowctrl_keeps_unset() {
    let d = open(r"\widowctrl", BODY);
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(widow(&back), [Some(true), Some(false), None]);
}
