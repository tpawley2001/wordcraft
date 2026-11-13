//! A field code written by a non-English Word carries a localized keyword (`TM` for TOC,
//! `RENVOIPAGE` for PAGEREF, `NBPAGES` for NUMPAGES). The field table of each story ([MS-DOC]
//! 2.8.25 Plcfld, 2.9.x Fld) gives every field begin a type byte (the flt enumeration) that does
//! not depend on the language: a code whose keyword is not an English field name takes the English
//! name its type byte gives. A known English keyword always wins over the byte.

use super::{StructSpec, plc, read, struct_doc, structured_spec};
use wordcraft_doc::para::InlineObject;

/// FIB pair of the main text's field table (PlcfFldMom) and of the headers' (PlcfFldHdr).
const MOM: usize = 16;
const HDR: usize = 17;

/// A Plcfld for `story`: one entry per field character (story-relative CPs), each begin with the
/// next type byte of `types` (0 once they run out), then the story's end as the limit CP.
fn plcfld(story: &str, types: &[u8]) -> Vec<u8> {
    plcfld_with(story, types, |_, e| e)
}

/// [`plcfld`] with each entry `(cp, fldch, grffld)` passed through `edit` with its index.
fn plcfld_with(story: &str, types: &[u8], edit: impl Fn(usize, (u32, u8, u8)) -> (u32, u8, u8)) -> Vec<u8> {
    let mut types = types.iter();
    let mut entries = Vec::new();
    for (cp, c) in story.chars().enumerate() {
        let e = match c {
            '\u{13}' => (cp as u32, 0x13, types.next().copied().unwrap_or(0)),
            '\u{14}' => (cp as u32, 0x14, 0),
            '\u{15}' => (cp as u32, 0x15, 0x40),
            _ => continue,
        };
        entries.push(edit(entries.len(), e));
    }
    let mut cps: Vec<u32> = entries.iter().map(|e| e.0).collect();
    cps.push(story.chars().count() as u32);
    let data: Vec<u8> = entries.iter().flat_map(|e| [e.1, e.2]).collect();
    plc(&cps, &data)
}

fn leak(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}

/// The document for main text `text` with field table `pairs` (none when empty).
fn open_with(text: &str, pairs: Vec<(usize, Vec<u8>)>) -> wordcraft_doc::Document {
    let text = leak(text);
    let spec = StructSpec { extra_pairs: pairs, ..structured_spec(text, text.chars().count() as u32) };
    read(&struct_doc(&spec)).expect("opens")
}

/// The document for main text `text`, its field begins typed by `types`.
fn open(text: &str, types: &[u8]) -> wordcraft_doc::Document {
    open_with(text, vec![(MOM, plcfld(text, types))])
}

fn paras(doc: &wordcraft_doc::Document) -> Vec<&wordcraft_doc::Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

/// The codes of the body's inline field objects and range-field starts, in order.
fn codes(doc: &wordcraft_doc::Document) -> Vec<String> {
    paras(doc)
        .iter()
        .flat_map(|p| &p.objects)
        .filter_map(|o| match o {
            InlineObject::Field { instr, .. } | InlineObject::FieldStart { instr, .. } => Some(instr.clone()),
            _ => None,
        })
        .collect()
}

/// The body's paragraphs as the model holds them (text, runs, objects), for comparing a file with
/// its English twin.
fn model(doc: &wordcraft_doc::Document) -> Vec<String> {
    paras(doc).iter().map(|p| format!("{:?} {:?} {:?}", p.text, p.runs, p.objects)).collect()
}

/// The links of the body's runs, in order.
fn links(doc: &wordcraft_doc::Document) -> Vec<String> {
    paras(doc).iter().flat_map(|p| &p.runs).filter_map(|r| r.props.link.clone()).collect()
}

const TOC_FR: &str = "\u{13} TM \\o \"1-3\" \u{14}Intro\t1\rMethods\t2\rResults\t3\u{15}\rBody\r";
const TOC_EN: &str = "\u{13} TOC \\o \"1-3\" \u{14}Intro\t1\rMethods\t2\rResults\t3\u{15}\rBody\r";

/// Paragraphs from the one holding the `FieldStart` to the one holding the `FieldEnd`.
fn range_paragraphs(doc: &wordcraft_doc::Document) -> usize {
    let ps = paras(doc);
    let has = |p: &&wordcraft_doc::Paragraph, start: bool| {
        p.objects.iter().any(|o| if start { matches!(o, InlineObject::FieldStart { .. }) } else { matches!(o, InlineObject::FieldEnd) })
    };
    match (ps.iter().position(|p| has(p, true)), ps.iter().position(|p| has(p, false))) {
        (Some(a), Some(b)) if b >= a => b - a + 1,
        _ => 0,
    }
}

#[test]
fn row1_a_localized_contents_field_is_a_range_field() {
    let doc = open(TOC_FR, &[0x0D]);
    assert_eq!(codes(&doc), [r#"TOC \o "1-3""#]);
    assert_eq!(range_paragraphs(&doc), 3);
    assert_eq!(model(&doc), model(&open(TOC_EN, &[0x0D])));
}

#[test]
fn row2_a_localized_hyperlink_is_a_link() {
    let doc = open("See \u{13} LIENHYPERTEXTE \\l \"_Toc1\" \u{14}Intro\u{15}\r", &[0x58]);
    let english = open("See \u{13} HYPERLINK \\l \"_Toc1\" \u{14}Intro\u{15}\r", &[0x58]);
    assert_eq!(links(&english), ["#_Toc1"]);
    assert_eq!(links(&doc), links(&english));
    assert_eq!(model(&doc), model(&english));
}

#[test]
fn row3_a_localized_page_reference_is_pageref() {
    let doc = open("Page \u{13} RENVOIPAGE _Toc1 \\h \u{14}4\u{15}\r", &[0x25]);
    assert_eq!(codes(&doc), [r#"PAGEREF _Toc1 \h"#]);
    let result = paras(&doc).iter().flat_map(|p| &p.objects).find_map(|o| match o {
        InlineObject::Field { result, .. } => Some(result.clone()),
        _ => None,
    });
    assert_eq!(result.as_deref(), Some("4"));
}

/// The codes of the fields in the document's header and footer parts.
fn part_codes(doc: &wordcraft_doc::Document) -> Vec<String> {
    doc.parts
        .values()
        .flat_map(|p| p.blocks.iter())
        .filter_map(|b| b.as_para())
        .flat_map(|p| &p.objects)
        .filter_map(|o| match o {
            InlineObject::Field { instr, .. } => Some(instr.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn row4_a_localized_page_count_in_a_header_story_is_numpages() {
    let story = "2/\u{13} NBPAGES \u{14}1\u{15}\r\r";
    // The header table's CPs are relative to the header subdocument; the main text has a field
    // at the same CPs that must not take the header's type.
    let text = "Bo\u{13} NBPAGES \u{14}1\u{15}\r";
    let spec = StructSpec {
        header_stories: vec![story],
        extra_pairs: vec![(HDR, plcfld(story, &[0x1A]))],
        ..structured_spec(text, text.chars().count() as u32)
    };
    let doc = read(&struct_doc(&spec)).expect("opens");
    assert_eq!(part_codes(&doc), ["NUMPAGES"]);
    assert_eq!(codes(&doc), ["NBPAGES"]);
}

#[test]
fn row5_an_english_code_with_its_own_type_is_unchanged() {
    assert_eq!(codes(&open("\u{13} PAGE \u{14}3\u{15}\r", &[0x21])), ["PAGE"]);
}

#[test]
fn row6_a_known_keyword_wins_over_a_stale_type() {
    assert_eq!(codes(&open("\u{13} PAGE \u{14}3\u{15}\r", &[0x0C])), ["PAGE"]);
}

#[test]
fn row7_a_contact_field_typed_0x5e_is_unchanged() {
    assert_eq!(codes(&open("\u{13} CONTACT _Con-1 \\c \u{14}x\u{15}\r", &[0x5E])), [r#"CONTACT _Con-1 \c"#]);
}

#[test]
fn row8_a_fillin_starting_with_its_prompt_is_unchanged() {
    assert_eq!(codes(&open("\u{13} \"Your name\" \u{14}x\u{15}\r", &[0x27])), [r#""Your name""#]);
}

#[test]
fn row9_unmapped_types_leave_a_bookmark_name() {
    for t in [0x02, 0x01, 0x00, 0x60, 0xFF] {
        assert_eq!(codes(&open("\u{13} Bookmark1 \u{14}x\u{15}\r", &[t])), ["Bookmark1"], "{t:#04x}");
    }
}

#[test]
fn row10_without_a_field_table_nothing_changes() {
    assert_eq!(codes(&open_with(TOC_FR, vec![])), [r#"TM \o "1-3""#]);
    // A table for another story only.
    assert_eq!(codes(&open_with(TOC_FR, vec![(HDR, plcfld(TOC_FR, &[0x0D]))])), [r#"TM \o "1-3""#]);
}

#[test]
fn row11_an_entry_that_is_not_this_begin_gives_no_type() {
    for fldch in [0x14, 0x15] {
        let pairs = vec![(MOM, plcfld_with(TOC_FR, &[0x0D], |i, (cp, c, t)| if i == 0 { (cp, fldch, t) } else { (cp, c, t) }))];
        assert_eq!(codes(&open_with(TOC_FR, pairs)), [r#"TM \o "1-3""#], "{fldch:#04x}");
    }
    let pairs = vec![(MOM, plcfld_with(TOC_FR, &[0x0D], |i, (cp, c, t)| if i == 0 { (cp + 1, c, t) } else { (cp, c, t) }))];
    assert_eq!(codes(&open_with(TOC_FR, pairs)), [r#"TM \o "1-3""#]);
}

#[test]
fn row12_only_the_low_five_bits_name_the_character() {
    let pairs = vec![(MOM, plcfld_with(TOC_FR, &[0x0D], |i, (cp, c, t)| if i == 0 { (cp, 0x93, t) } else { (cp, c, t) }))];
    let doc = open_with(TOC_FR, pairs);
    assert_eq!(codes(&doc), [r#"TOC \o "1-3""#]);
    assert_eq!(range_paragraphs(&doc), 3);
}

#[test]
fn row13_case_and_a_switch_right_after_the_keyword() {
    for (code, want) in [("tm \\O \"1-3\"", r#"TOC \O "1-3""#), ("TM\\o \"1-3\"", r#"TOC\o "1-3""#)] {
        let text = format!("\u{13}{code}\u{14}Intro\t1\rMethods\t2\u{15}\rBody\r");
        let doc = open(&text, &[0x0D]);
        assert_eq!(codes(&doc), [want], "{code}");
        assert_eq!(range_paragraphs(&doc), 2, "{code}");
    }
}

#[test]
fn row14_entries_of_a_localized_contents_field_link_and_reference_pages() {
    let entry = |link: &str, pageref: &str| {
        format!("\u{13} {link} \\l \"_Toc1\" \u{14}Intro\t\u{13} {pageref} _Toc1 \\h \u{14}4\u{15}\u{15}\r")
    };
    let fr = format!("\u{13} TM \\o \"1-3\" \u{14}{}\u{15}\rBody\r", entry("LIENHYPERTEXTE", "RENVOIPAGE"));
    let en = format!("\u{13} TOC \\o \"1-3\" \u{14}{}\u{15}\rBody\r", entry("HYPERLINK", "PAGEREF"));
    let types = [0x0D, 0x58, 0x25];
    let (doc, english) = (open(&fr, &types), open(&en, &types));
    assert_eq!(codes(&doc), codes(&english));
    assert!(!links(&english).is_empty() && links(&english).iter().all(|l| l == "#_Toc1"), "{:?}", links(&english));
    assert_eq!(links(&doc), links(&english));
    assert_eq!(model(&doc), model(&english));
}
