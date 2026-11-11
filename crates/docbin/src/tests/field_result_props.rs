//! A field that becomes one inline object (PAGE, SEQ, REF, ...) takes the character
//! formatting of its cached result, as Word displays it ([MS-DOC] §2.9.84 Plcfld; the
//! `.docx` reader does the same for `w:fldChar` fields): the first non-empty result run's
//! properties, or the field begin character's when the result is empty. A hidden SEQ result
//! (`sprmCFVanish`) stays hidden, a 12 pt PAGE result stays 12 pt.

use super::{RichSpec, StructSpec, grpprl, para0, plc, read, rich_doc, struct_doc, structured_spec};

const BOLD: u16 = 0x0835; // sprmCFBold
const HIDDEN: u16 = 0x083C; // sprmCFVanish
const HPS: u16 = 0x4A43; // sprmCHps, half-points

// CPs: x=0 ' '=1 \x13=2 PAGE=3..=6 \x14=7 '1'=8 '2'=9 \x15=10 ' '=11 y=12 \r=13
const TEXT: &str = "x \u{13}PAGE\u{14}12\u{15} y\r";
// CPs: x=0 ' '=1 \x13=2 PAGE=3..=6 \x14=7 \x15=8 ' '=9 y=10 \r=11
const EMPTY_RESULT: &str = "x \u{13}PAGE\u{14}\u{15} y\r";

fn doc(text: &'static str, chpx_runs: Vec<(u32, Vec<u8>)>) -> wordcraft_doc::Document {
    let spec = RichSpec {
        text,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs,
        papx_entries: vec![(0, 0, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

/// The run holding the paragraph's single field object, and the field's result.
fn field_run(p: &wordcraft_doc::Paragraph) -> (wordcraft_doc::CharProps, String) {
    let Some(wordcraft_doc::para::InlineObject::Field { result, .. }) = p.objects.first() else { panic!("no field object: {:?}", p.objects) };
    let at = p.text.find('\u{FFFC}').expect("object char");
    let mut start = 0;
    for r in &p.runs {
        if at >= start && at < start + r.len {
            return (r.props.clone(), result.clone());
        }
        start += r.len;
    }
    panic!("no run covers the object: {:?}", p.runs)
}

/// The run covering byte `at` of the paragraph text.
fn run_at(p: &wordcraft_doc::Paragraph, at: usize) -> wordcraft_doc::CharProps {
    let mut start = 0;
    for r in &p.runs {
        if at >= start && at < start + r.len {
            return r.props.clone();
        }
        start += r.len;
    }
    panic!("no run at {at}: {:?}", p.runs)
}

#[test]
fn bold_result_makes_a_bold_field() {
    let d = doc(TEXT, vec![(0, Vec::new()), (8, grpprl(&[(BOLD, &[0x01])])), (10, Vec::new())]);
    let p = para0(&d);
    assert_eq!(p.text, "x \u{FFFC} y");
    let (props, result) = field_run(&p);
    assert_eq!(result, "12");
    assert_eq!(props.bold, Some(true));
    // Text around the field keeps its own (plain) formatting.
    assert_eq!(run_at(&p, 0).bold, None);
    assert_eq!(run_at(&p, p.text.len() - 1).bold, None);
}

#[test]
fn hidden_result_makes_a_hidden_field() {
    // The SEQ-in-a-footer case: begin, code and result all carry sprmCFVanish 0x81 (the
    // opposite of the style's off), as Word 97 writes it; the field must come out hidden.
    let d = doc(TEXT, vec![(0, Vec::new()), (2, grpprl(&[(HIDDEN, &[0x81])])), (11, Vec::new())]);
    let p = para0(&d);
    let (props, _) = field_run(&p);
    assert_eq!(props.hidden, Some(true));
    assert_eq!(run_at(&p, 0).hidden, None);
}

#[test]
fn result_size_wins_over_the_code_size() {
    // Begin and code at 8 pt, result at 12 pt: the field shows at 12 pt.
    let d =
        doc(TEXT, vec![(0, Vec::new()), (2, grpprl(&[(HPS, &16u16.to_le_bytes())])), (8, grpprl(&[(HPS, &24u16.to_le_bytes())])), (10, Vec::new())]);
    let p = para0(&d);
    let (props, _) = field_run(&p);
    assert_eq!(props.size, Some(12.0));
}

#[test]
fn first_result_run_wins_when_the_result_is_mixed() {
    // '1' bold, '2' plain: the object takes the first visible run's properties.
    let d = doc(TEXT, vec![(0, Vec::new()), (8, grpprl(&[(BOLD, &[0x01])])), (9, Vec::new())]);
    let p = para0(&d);
    let (props, _) = field_run(&p);
    assert_eq!(props.bold, Some(true));
}

#[test]
fn empty_result_keeps_the_begin_characters_formatting() {
    let d = doc(EMPTY_RESULT, vec![(0, Vec::new()), (2, grpprl(&[(BOLD, &[0x01])])), (3, Vec::new())]);
    let p = para0(&d);
    let (props, result) = field_run(&p);
    assert_eq!(result, "");
    assert_eq!(props.bold, Some(true));
}

#[test]
fn plain_field_stays_plain() {
    // Guard: no formatting anywhere -> nothing set on the field.
    let d = doc(TEXT, vec![(0, Vec::new())]);
    let p = para0(&d);
    let (props, result) = field_run(&p);
    assert_eq!(result, "12");
    assert_eq!(props.bold, None);
    assert_eq!(props.hidden, None);
    assert_eq!(props.size, None);
}

// Fields in results, range fields and note numbers (carried from #4484).

/// A note's text starts with its own reference character, which Word draws as the note's
/// number. It was dropped (only body references were matched), so the note text at the foot of
/// the page had no number; now it is the note's own number, as `w:footnoteRef` is in DOCX.
#[test]
fn a_note_text_starts_with_its_own_number() {
    use wordcraft_doc::para::{InlineObject, NoteKind};
    let text = "A\u{2} b\u{2}\r";
    let ccp = text.chars().count() as u32;
    let spec = StructSpec {
        text,
        note_stories: vec!["\u{2} Note body\r"],
        endnote_stories: vec!["\u{2} End\r"],
        extra_pairs: vec![
            (2, plc(&[1, ccp], &1u16.to_le_bytes())),
            (3, plc(&[0, 12], &[])),
            (46, plc(&[4, ccp], &1u16.to_le_bytes())),
            (47, plc(&[0, 6], &[])),
        ],
        ..structured_spec(text, ccp)
    };
    let doc = read(&struct_doc(&spec)).expect("opens");
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let refs: Vec<(NoteKind, u32)> = p0
        .objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::NoteRef { kind, id, .. } => Some((*kind, *id)),
            _ => None,
        })
        .collect();
    assert_eq!(refs.iter().map(|r| r.0).collect::<Vec<_>>(), [NoteKind::Footnote, NoteKind::Endnote]);
    for (kind, id) in refs {
        let first = doc.parts.get(&id).and_then(|p| p.blocks.first()).and_then(|b| b.as_para()).expect("note text");
        assert!(first.text.starts_with('\u{FFFC}'), "{:?}", first.text);
        assert!(matches!(first.objects.first(), Some(InlineObject::NoteRef { kind: k, id: i, .. }) if *k == kind && *i == id), "{:?}", first.objects);
    }
}

/// A table of contents is a TOC field whose result holds one paragraph per entry, each a
/// HYPERLINK with a PAGEREF inside. The entries were swallowed into one field object, leaving
/// empty paragraphs and one line of `HYPERLINK \l … PAGEREF …` codes; now they stay paragraphs
/// between range-field markers, as the DOCX reader keeps them.
#[test]
fn toc_entries_stay_paragraphs_with_their_links_and_page_numbers() {
    use wordcraft_doc::para::InlineObject;
    let entry =
        |bm: &str, title: &str, page: &str| format!("\u{13}HYPERLINK \\l \"{bm}\"\u{14}{title}\t\u{13} PAGEREF {bm} \\h \u{14}{page}\u{15}\u{15}");
    let text = format!("\u{13} TOC \\o \"1-2\" \\h \\z \u{14}{}\r{}\u{15}\rBody\r", entry("_Toc1", "Intro", "4"), entry("_Toc2", "Method", "5"));
    let text: &'static str = Box::leak(text.into_boxed_str());
    let spec = structured_spec(text, text.chars().count() as u32);
    let doc = read(&struct_doc(&spec)).expect("opens");
    let paras: Vec<_> = doc.body.iter().filter_map(|b| b.as_para()).collect();
    let texts: Vec<&str> = paras.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, ["\u{FFFC}Intro\t4", "Method\t5\u{FFFC}", "Body"]);
    let first = paras.first().expect("first entry");
    assert!(matches!(first.objects.first(), Some(InlineObject::FieldStart { instr, .. }) if instr == r#"TOC \o "1-2" \h \z"#), "{:?}", first.objects);
    let second = paras.get(1).expect("second entry");
    assert!(matches!(second.objects.first(), Some(InlineObject::FieldEnd)), "{:?}", second.objects);
    // Each entry links to its heading's bookmark.
    for (p, target) in [(first, "#_Toc1"), (second, "#_Toc2")] {
        assert!(p.runs.iter().any(|r| r.props.link.as_deref() == Some(target)), "{target}: {:?}", p.runs);
    }
}

/// A TOC field with no result yet is still a whole range field.
#[test]
fn an_empty_toc_field_is_a_whole_range_field() {
    use wordcraft_doc::para::InlineObject;
    let text = "\u{13}TOC \\o\u{15}\r";
    let spec = structured_spec(text, text.chars().count() as u32);
    let doc = read(&struct_doc(&spec)).expect("opens");
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    assert!(matches!(p0.objects.as_slice(), [InlineObject::FieldStart { .. }, InlineObject::FieldEnd]), "{:?}", p0.objects);
}

/// A table of contents still open when the story ends gets its end there: a `FieldStart` without
/// one would be saved as a field begin with no end.
#[test]
fn a_toc_left_open_at_the_story_end_is_closed() {
    use wordcraft_doc::para::InlineObject;
    for (text, pairs) in [("\u{13}TOC \\o\u{14}Intro\rMethod\r", 1), ("\u{13}TOC\u{14}\u{13}TOC\u{14}x\r", 2)] {
        let spec = structured_spec(text, text.chars().count() as u32);
        let doc = read(&struct_doc(&spec)).expect("opens");
        let objs: Vec<&InlineObject> = doc.body.iter().filter_map(|b| b.as_para()).flat_map(|p| &p.objects).collect();
        let starts = objs.iter().filter(|o| matches!(o, InlineObject::FieldStart { .. })).count();
        let ends = objs.iter().filter(|o| matches!(o, InlineObject::FieldEnd)).count();
        assert_eq!((starts, ends), (pairs, pairs), "{text:?}: {objs:?}");
        assert!(matches!(objs.last(), Some(InlineObject::FieldEnd)), "{text:?}: the end comes last");
    }
}

/// A field inside another field's result shows its result there, not its code.
#[test]
fn a_field_in_a_result_adds_its_result_not_its_code() {
    let text = "\u{13}QUOTE x\u{14}a\u{13}PAGE\u{14}7\u{15}b\u{15}\r";
    let spec = structured_spec(text, text.chars().count() as u32);
    let doc = read(&struct_doc(&spec)).expect("opens");
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let Some(wordcraft_doc::para::InlineObject::Field { instr, result, .. }) = p0.objects.first() else { panic!("no field: {:?}", p0.objects) };
    assert_eq!((instr.as_str(), result.as_str()), ("QUOTE x", "a7b"));
}

/// Hostile nesting: fields opened inside results thousands deep stay bounded and the TOC
/// markers still pair up.
#[test]
fn deeply_nested_fields_in_results_are_bounded() {
    use wordcraft_doc::para::InlineObject;
    let n = 5_000;
    let text = format!("\u{13}TOC\u{14}{}y{}\u{15}\r", "\u{13}X\u{14}".repeat(n), "\u{15}".repeat(n));
    let text: &'static str = Box::leak(text.into_boxed_str());
    let spec = structured_spec(text, text.chars().count() as u32);
    let doc = read(&struct_doc(&spec)).expect("opens");
    let objs: Vec<&InlineObject> = doc.body.iter().filter_map(|b| b.as_para()).flat_map(|p| &p.objects).collect();
    let starts = objs.iter().filter(|o| matches!(o, InlineObject::FieldStart { .. })).count();
    let ends = objs.iter().filter(|o| matches!(o, InlineObject::FieldEnd)).count();
    assert_eq!((starts, ends), (1, 1));
    // Past the cap the fields are only counted: their codes ("X") never reach the result.
    let results: Vec<&str> = objs
        .iter()
        .filter_map(|o| match o {
            InlineObject::Field { result, .. } => Some(result.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(results, ["y"]);
}
