//! An automatic footnote or endnote reference (0x02) keeps the character formatting stored for it
//! ([MS-DOC] §2.6.1: its CHPX, e.g. a character style through sprmCIstd and a size through
//! sprmCHps), as Word shows it: in a 12 pt paragraph style with 10 pt text, a mark set to 10 pt
//! stays 10 pt and its line is no taller than the others. The note's own number does the same.

use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::props::{CharProps, TextColor};

use super::{StructSpec, grpprl, plc, read, std_bytes, struct_doc_styled, structured_spec};

const ISTD: u16 = 0x4A30; // sprmCIstd
const HPS: u16 = 0x4A43; // sprmCHps, half-points
const ISS: u16 = 0x2A48; // sprmCIss: 1 = superscript
const BOLD: u16 = 0x0835; // sprmCFBold
const ICO: u16 = 0x2A42; // sprmCIco
const FTC0: u16 = 0x4A4F; // sprmCRgFtc0
const INS: u16 = 0x0801; // sprmCFRMarkIns
const DEL: u16 = 0x0800; // sprmCFRMarkDel

/// "Normal" 12 pt (istd 0), "footnote reference" superscript (istd 1, character),
/// "footnote text" 9 pt (istd 2).
fn styles() -> Vec<Vec<u8>> {
    vec![
        std_bytes(1, 0x0FFF, "Normal", &[], &grpprl(&[(HPS, &24u16.to_le_bytes())])),
        std_bytes(2, 0x0FFF, "footnote reference", &[], &grpprl(&[(ISS, &[1])])),
        std_bytes(1, 0, "footnote text", &[], &grpprl(&[(HPS, &18u16.to_le_bytes())])),
    ]
}

fn hps(half_points: u16) -> (u16, Vec<u8>) {
    (HPS, half_points.to_le_bytes().to_vec())
}

fn chpx(prls: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let refs: Vec<(u16, &[u8])> = prls.iter().map(|(op, v)| (*op, v.as_slice())).collect();
    grpprl(&refs)
}

/// The mark's own run: the character style and a direct 10 pt.
fn styled_10pt() -> Vec<u8> {
    chpx(&[(ISTD, 1u16.to_le_bytes().to_vec()), hps(20)])
}

/// A one-paragraph document `text` (paragraph style Normal) with automatic note references at
/// `refs`, one note story per reference (each paragraph "footnote text"), and the given runs.
fn doc_with(text: &'static str, stories: Vec<&'static str>, refs: &[u32], runs: Vec<(u32, Vec<u8>)>, endnotes: bool) -> wordcraft_doc::Document {
    let ccp = text.chars().count() as u32;
    let mut cps = refs.to_vec();
    cps.push(ccp);
    let data: Vec<u8> = refs.iter().flat_map(|_| 1u16.to_le_bytes()).collect();
    let mut story_cps = vec![0u32];
    for s in &stories {
        let last = story_cps.last().copied().unwrap_or(0);
        story_cps.push(last + s.chars().count() as u32);
    }
    // The note stories follow the main text (endnotes after one dummy footnote story).
    let first_story = if endnotes { ccp + 4 } else { ccp };
    // PAPX entries are keyed by the paragraph's first CP.
    let mut papx = vec![(0, 0u16, Vec::new())];
    papx.extend(story_cps.iter().take(stories.len()).map(|&start| (first_story + start, 2u16, Vec::new())));
    let (ref_pair, txt_pair) = if endnotes { (46, 47) } else { (2, 3) };
    let spec = StructSpec {
        text,
        note_stories: if endnotes { vec!["Ftn\r"] } else { stories.clone() },
        endnote_stories: if endnotes { stories } else { vec![] },
        chpx_runs: runs,
        papx_entries: papx,
        extra_pairs: vec![(ref_pair, plc(&cps, &data)), (txt_pair, plc(&story_cps, &[]))],
        ..structured_spec(text, ccp)
    };
    read(&struct_doc_styled(&spec, &styles())).expect("opens")
}

fn first_para(doc: &wordcraft_doc::Document) -> wordcraft_doc::Paragraph {
    doc.body.first().and_then(|b| b.as_para()).cloned().expect("para")
}

/// The properties of the run holding the byte at `at`.
fn props_at(p: &wordcraft_doc::Paragraph, at: usize) -> CharProps {
    let mut start = 0;
    for r in &p.runs {
        if at >= start && at < start + r.len {
            return r.props.clone();
        }
        start += r.len;
    }
    panic!("no run at {at}: {:?}", p.runs)
}

/// The note references of `p`: (kind, id, the placeholder's properties).
fn marks(p: &wordcraft_doc::Paragraph) -> Vec<(NoteKind, u32, CharProps)> {
    let anchors: Vec<usize> = p.text.char_indices().filter(|(_, c)| *c == '\u{FFFC}').map(|(i, _)| i).collect();
    assert_eq!(anchors.len(), p.objects.len(), "one placeholder per object: {:?}", p.text);
    anchors
        .iter()
        .zip(&p.objects)
        .filter_map(|(&at, o)| match o {
            InlineObject::NoteRef { kind, id, .. } => Some((*kind, *id, props_at(p, at))),
            _ => None,
        })
        .collect()
}

fn style_id(doc: &wordcraft_doc::Document, name: &str) -> String {
    doc.styles.find(name).map(|s| s.id.clone()).unwrap_or_else(|| panic!("no style {name}"))
}

/// The note part's first paragraph.
fn note_para(doc: &wordcraft_doc::Document, id: u32) -> wordcraft_doc::Paragraph {
    doc.parts.get(&id).and_then(|p| p.blocks.iter().find_map(|b| b.as_para())).cloned().expect("note paragraph")
}

#[test]
fn a_reference_keeps_its_style_and_size() {
    // "Text" 10 pt, the mark styled and 10 pt, " more" 10 pt again.
    let runs = vec![(0, chpx(&[hps(20)])), (4, styled_10pt()), (5, chpx(&[hps(20)]))];
    let doc = doc_with("Text\u{2} more\r", vec!["\u{2} Note\r"], &[4], runs, false);
    let p = first_para(&doc);
    assert_eq!(p.text, "Text\u{FFFC} more");
    let m = marks(&p);
    assert_eq!(m.len(), 1, "{:?}", p.objects);
    let (kind, id, props) = m.first().cloned().expect("mark");
    assert_eq!(kind, NoteKind::Footnote);
    assert_eq!(doc.parts.get(&id).map(|p| p.kind), Some(wordcraft_doc::PartKind::Footnote));
    assert_eq!(props.size, Some(10.0));
    assert_eq!(props.style, Some(style_id(&doc, "footnote reference")));
}

#[test]
fn a_plain_reference_gets_what_plain_text_gets() {
    let doc = doc_with("Text\u{2} more\r", vec!["\u{2} Note\r"], &[4], vec![(0, Vec::new())], false);
    let p = first_para(&doc);
    let (_, _, props) = marks(&p).first().cloned().expect("mark");
    assert_eq!(props, props_at(&p, 0));
    assert_eq!(props, CharProps::default());
}

#[test]
fn bold_colour_and_font_are_kept() {
    let mark = chpx(&[(BOLD, vec![1]), (ICO, vec![6]), (FTC0, 1u16.to_le_bytes().to_vec())]);
    let runs = vec![(0, Vec::new()), (4, mark), (5, Vec::new())];
    let doc = doc_with("Text\u{2} more\r", vec!["\u{2} Note\r"], &[4], runs, false);
    let p = first_para(&doc);
    let (_, _, props) = marks(&p).first().cloned().expect("mark");
    assert_eq!(props.bold, Some(true));
    assert!(matches!(props.color, Some(TextColor::Rgb(_))), "{:?}", props.color);
    assert_eq!(props.font.as_deref(), Some("Wingdings"));
}

#[test]
fn the_notes_own_number_keeps_its_size() {
    let text = "Text\u{2} more\r";
    let ccp = text.chars().count() as u32;
    // The note's own number (its story's first character) set to 10 pt.
    let runs = vec![(0, Vec::new()), (ccp, chpx(&[hps(20)])), (ccp + 1, Vec::new())];
    let doc = doc_with(text, vec!["\u{2} Note\r"], &[4], runs, false);
    let (_, id, _) = marks(&first_para(&doc)).first().cloned().expect("mark");
    let np = note_para(&doc, id);
    assert_eq!(np.props.style, Some(style_id(&doc, "footnote text")));
    let own = marks(&np);
    assert_eq!(own.first().map(|m| m.2.size), Some(Some(10.0)), "{:?}", np.runs);
    // Without a size of its own it takes the paragraph style's 9 pt.
    let doc = doc_with(text, vec!["\u{2} Note\r"], &[4], vec![(0, Vec::new())], false);
    let (_, id, _) = marks(&first_para(&doc)).first().cloned().expect("mark");
    let np = note_para(&doc, id);
    assert_eq!(marks(&np).first().map(|m| m.2.size), Some(None));
    assert_eq!(doc.styles.find("footnote text").and_then(|s| s.chr.size), Some(9.0));
}

#[test]
fn tracked_references_keep_their_marks() {
    for (rev, inserted) in [(INS, true), (DEL, false)] {
        let mark = chpx(&[(rev, vec![1]), hps(20)]);
        let runs = vec![(0, Vec::new()), (4, mark), (5, Vec::new())];
        let doc = doc_with("Text\u{2} more\r", vec!["\u{2} Note\r"], &[4], runs, false);
        let (_, _, props) = marks(&first_para(&doc)).first().cloned().expect("mark");
        assert_eq!(props.size, Some(10.0));
        assert_eq!(props.ins.is_some(), inserted, "{props:?}");
        assert_eq!(props.del.is_some(), !inserted, "{props:?}");
    }
}

#[test]
fn an_endnote_reference_keeps_its_size() {
    let runs = vec![(0, Vec::new()), (3, chpx(&[hps(16)])), (4, Vec::new())];
    let doc = doc_with("Ref\u{2}\r", vec!["\u{2} Endnote\r"], &[3], runs, true);
    let m = marks(&first_para(&doc));
    let (kind, id, props) = m.first().cloned().expect("mark");
    assert_eq!(kind, NoteKind::Endnote);
    assert_eq!(doc.parts.get(&id).map(|p| p.kind), Some(wordcraft_doc::PartKind::Endnote));
    assert_eq!(props.size, Some(8.0));
}

#[test]
fn two_references_keep_their_own_formatting() {
    let runs = vec![(0, Vec::new()), (1, styled_10pt()), (2, Vec::new()), (4, chpx(&[(BOLD, vec![1]), hps(16)])), (5, Vec::new())];
    let doc = doc_with("A\u{2} b\u{2} c\r", vec!["\u{2} One\r", "\u{2} Two\r"], &[1, 4], runs, false);
    let p = first_para(&doc);
    let m = marks(&p);
    assert_eq!(m.len(), 2, "{:?}", p.objects);
    let (first, second) = (m.first().cloned().expect("first"), m.get(1).cloned().expect("second"));
    assert_ne!(first.1, second.1);
    let texts: Vec<String> = [first.1, second.1].iter().map(|id| note_para(&doc, *id).text).collect();
    assert_eq!(texts, vec!["\u{FFFC} One", "\u{FFFC} Two"], "object order unchanged");
    assert_eq!((first.2.size, first.2.style.clone(), first.2.bold), (Some(10.0), Some(style_id(&doc, "footnote reference")), None));
    assert_eq!((second.2.size, second.2.style, second.2.bold), (Some(8.0), None, Some(true)));
}
