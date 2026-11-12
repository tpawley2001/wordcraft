//! Automatic space between Asian and Latin text (`\aspalpha`) and between Asian text and numbers
//! (`\aspnum`), RTF 1.9.1: paragraph toggles that are off unless the paragraph carries them (Word
//! adds no space in an RTF paragraph without them), the RTF form of DOCX `w:autoSpaceDE` and
//! `w:autoSpaceDN` (whose default is on).

use super::*;
use wordcraft_doc::Document;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

/// (Asian/Latin, Asian/number) automatic spacing of each body paragraph: `true` is on.
fn spacing(d: &Document) -> Vec<(bool, bool)> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| (p.props.auto_space_de.unwrap_or(true), p.props.auto_space_dn.unwrap_or(true))).collect()
}

#[test]
fn a_paragraph_without_the_words_has_both_off() {
    assert_eq!(spacing(&open(r"\pard a\par")), [(false, false)]);
    assert_eq!(spacing(&open(r"a\par")), [(false, false)]);
}

#[test]
fn each_word_turns_its_own_setting_on() {
    assert_eq!(spacing(&open(r"\pard\aspalpha a\par")), [(true, false)]);
    assert_eq!(spacing(&open(r"\pard\aspnum a\par")), [(false, true)]);
    assert_eq!(spacing(&open(r"\pard\aspalpha1\aspnum1 a\par")), [(true, true)]);
}

#[test]
fn the_words_carry_on_until_pard() {
    assert_eq!(spacing(&open(r"\pard\aspalpha\aspnum a\par b\par\pard c\par")), [(true, true), (true, true), (false, false)]);
}

#[test]
fn a_zero_parameter_turns_the_setting_off() {
    assert_eq!(spacing(&open(r"\pard\aspalpha\aspnum\aspalpha0 a\par")), [(false, true)]);
    assert_eq!(spacing(&open(r"\pard\aspalpha\aspnum\aspnum0 a\par")), [(true, false)]);
}

#[test]
fn on_is_stored_as_unset_and_off_as_false() {
    let d = open(r"\pard\aspalpha a\par");
    let p = d.body.iter().find_map(|b| b.as_para()).expect("a paragraph");
    assert_eq!((p.props.auto_space_de, p.props.auto_space_dn), (None, Some(false)));
}

#[test]
fn words_in_a_skipped_destination_are_ignored() {
    assert_eq!(spacing(&open(r"{\*\wcunknowndest \aspalpha\aspnum}\pard a\par")), [(false, false)]);
}

#[test]
fn table_cell_paragraphs_follow_the_same_rule() {
    let d = open(r"\trowd\cellx2000\cellx4000\pard\intbl\aspalpha on\cell\pard\intbl off\cell\row\pard after\par");
    let cells: Vec<(bool, bool)> = d
        .body
        .iter()
        .filter_map(|b| b.as_table())
        .flat_map(|t| t.rows.iter())
        .flat_map(|r| r.cells.iter())
        .flat_map(|c| c.blocks.iter())
        .filter_map(|b| b.as_para())
        .map(|p| (p.props.auto_space_de.unwrap_or(true), p.props.auto_space_dn.unwrap_or(true)))
        .collect();
    assert_eq!(cells, [(true, false), (false, false)]);
}

#[test]
fn export_writes_the_words_only_for_paragraphs_that_have_them_on() {
    let on = export(&open(r"\pard\aspalpha\aspnum a\par"));
    assert!(on.contains("\\aspalpha") && on.contains("\\aspnum"), "{on}");
    let off = export(&open(r"\pard a\par"));
    assert!(!off.contains("\\aspalpha") && !off.contains("\\aspnum"), "{off}");
    let alpha = export(&open(r"\pard\aspalpha a\par"));
    assert!(alpha.contains("\\aspalpha") && !alpha.contains("\\aspnum"), "{alpha}");
}

#[test]
fn export_of_an_unset_paragraph_writes_both() {
    // A paragraph from a DOCX file with neither element: on by default.
    let mut d = open(r"\pard a\par");
    for b in &mut d.body {
        if let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(b) {
            p.props.auto_space_de = None;
            p.props.auto_space_dn = None;
        }
    }
    let out = export(&d);
    assert!(out.contains("\\aspalpha") && out.contains("\\aspnum"), "{out}");
}

#[test]
fn settings_survive_a_round_trip() {
    let d = open(r"\pard\aspalpha\aspnum both\par\pard\aspalpha alpha\par\pard\aspnum num\par\pard none\par");
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(spacing(&back), [(true, true), (true, false), (false, true), (false, false)]);
}
