//! Tab stops, character-unit indents and character spacing in stylesheet entries (RTF 1.9.1,
//! "Style sheet": an entry takes the same paragraph and character words as the text). Word writes
//! them on its own styles (a TOC style's right tab with dot leader, `\tqr\tldot\tx9350`; East Asian
//! styles' `\cufi200`; a condensed style's `\expndtw-3`). Read into the style, written back.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::{TabAlign, TabLeader};

const MSFTEDIT: &str = r"{\*\generator Msftedit 5.41.15.1507;}";

fn open(generator: &str, stylesheet: &str, body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{generator}{{\stylesheet{{\s0 Normal;}}{stylesheet}}}{body}}}").as_bytes())
        .unwrap()
}

/// Style `id`'s own tab stops: (pos pt, align, leader), sorted by position.
fn style_tabs(d: &Document, id: &str) -> Vec<(f32, TabAlign, TabLeader)> {
    let s = d.styles.get(id).unwrap_or_else(|| panic!("style {id}"));
    let mut v: Vec<_> = s.para.tabs.iter().flatten().map(|t| (t.pos, t.align, t.leader)).collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

/// Style `id`'s indents in characters: (first, left, right).
fn style_chars(d: &Document, id: &str) -> (Option<f32>, Option<f32>, Option<f32>) {
    let p = &d.styles.get(id).unwrap_or_else(|| panic!("style {id}")).para;
    (p.indent_first_chars, p.indent_left_chars, p.indent_right_chars)
}

const AGENDA: &str = r"{\s15\sbasedon0\snext15\tqr\tldot\tx9350\tb5040\tx720 Agenda Item;}";
const CHAR_INDENT: &str = r"{\s16\sbasedon0\snext16\cufi200\culi350\curi100 Char Indent;}";
const TIGHT: &str = r"{\*\cs17\additive\expndtw-3 Tight Char;}";
const WIDE: &str = r"{\s18\sbasedon0\snext18\expnd4 Wide Para;}";

#[test]
fn a_style_entry_keeps_its_tab_stops() {
    let d = open("", AGENDA, r"\pard\s15 Call to Order\tab 3\par");
    assert_eq!(
        style_tabs(&d, "AgendaItem"),
        [(36.0, TabAlign::Left, TabLeader::None), (252.0, TabAlign::Bar, TabLeader::None), (467.5, TabAlign::Right, TabLeader::Dot)]
    );
}

#[test]
fn tab_alignment_and_leader_do_not_leak_between_entries_or_into_the_body() {
    // A `\tqr\tldot` with no `\tx` after it ends with its entry.
    let d = open("", r"{\s15\sbasedon0\tqr\tldot Dangling;}{\s16\sbasedon0\tx720 Plain Tab;}", r"\pard\tx1440 a\par");
    assert!(style_tabs(&d, "Dangling").is_empty());
    assert_eq!(style_tabs(&d, "PlainTab"), [(36.0, TabAlign::Left, TabLeader::None)]);
    let p = d.body[0].as_para().unwrap();
    let body: Vec<_> = p.props.tabs.iter().flatten().map(|t| (t.pos, t.align, t.leader)).collect();
    assert_eq!(body, [(72.0, TabAlign::Left, TabLeader::None)]);
}

#[test]
fn a_style_entry_keeps_its_character_unit_indents() {
    let d = open("", CHAR_INDENT, r"\pard\s16 a\par");
    assert_eq!(style_chars(&d, "CharIndent"), (Some(2.0), Some(3.5), Some(1.0)));
}

#[test]
fn a_style_entry_keeps_its_character_spacing() {
    // `\expndtw` is twips, `\expnd` quarter points, as on text.
    let d = open("", &format!("{TIGHT}{WIDE}"), r"\pard\s18 plain {\cs17\expndtw-3 tight}\par");
    assert_eq!(d.styles.get("TightChar").expect("TightChar").chr.spacing, Some(-0.15));
    assert_eq!(d.styles.get("WidePara").expect("WidePara").chr.spacing, Some(1.0));
}

#[test]
fn a_heading_entry_from_another_program_keeps_its_tab_stop() {
    let d = open(MSFTEDIT, r"{\s1\sbasedon0\snext0\tqr\tx9000\b\fs28 heading 1;}", r"\pard\s1 Title\tab 1\par");
    let id = d.body[0].as_para().unwrap().props.style.clone().expect("paragraph style");
    assert_eq!(style_tabs(&d, &id), [(450.0, TabAlign::Right, TabLeader::None)]);
}

#[test]
fn style_entry_extras_round_trip() {
    let d = open("", &format!("{AGENDA}{CHAR_INDENT}{TIGHT}{WIDE}"), r"\pard\s15 a\par\pard\s16 b\par\pard\s18 c {\cs17 d}\par");
    let rtf = export(&d);
    let sheet = &rtf[rtf.find(r"{\stylesheet").expect("stylesheet")..];
    let agenda = &sheet[..sheet.find("Agenda Item;").expect("Agenda Item entry")];
    let agenda = &agenda[agenda.rfind('{').unwrap()..];
    assert!(agenda.contains(r"\tqr\tldot\tx9350") && agenda.contains(r"\tb5040") && agenda.contains(r"\tx720"), "{agenda}");
    assert!(sheet.contains(r"\cufi200") && sheet.contains(r"\culi350") && sheet.contains(r"\curi100"), "{sheet}");
    assert!(sheet.contains(r"\expndtw-3") && sheet.contains(r"\expndtw20"), "{sheet}");
    let back = import(rtf.as_bytes()).unwrap();
    for id in ["AgendaItem", "CharIndent"] {
        assert_eq!(style_tabs(&back, id), style_tabs(&d, id), "{id}");
        assert_eq!(style_chars(&back, id), style_chars(&d, id), "{id}");
    }
    assert_eq!(back.styles.get("TightChar").expect("TightChar").chr.spacing, Some(-0.15));
    assert_eq!(back.styles.get("WidePara").expect("WidePara").chr.spacing, Some(1.0));
}

#[test]
fn a_style_with_no_tabs_or_spacing_writes_none() {
    let d = open("", r"{\s15\sbasedon0\snext15\li720 Plain Style;}", r"\pard\s15 a\par");
    let rtf = export(&d);
    let sheet = &rtf[rtf.find(r"{\stylesheet").unwrap()..];
    let entry = &sheet[..sheet.find("Plain Style;").expect("Plain Style entry")];
    let entry = &entry[entry.rfind('{').unwrap()..];
    for w in [r"\tx", r"\tb", r"\tq", r"\cufi", r"\culi", r"\curi", r"\expnd"] {
        assert!(!entry.contains(w), "{w} in {entry}");
    }
}
