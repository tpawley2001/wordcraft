//! Footnotes and endnotes with a custom reference mark (Insert › Footnote › Custom mark). The
//! reference PLC's data element is 0 for such a note ([MS-DOC] §2.8, PlcffndRef / PlcfendRef) and
//! there is no 0x02 at its CP: the characters there are the mark. They become one `NoteRef` whose
//! `custom` is the mark, so the note is laid out and saved with its reference.

use wordcraft_doc::para::{InlineObject, NoteKind};

use super::{StructSpec, grpprl, plc, read, struct_doc, structured_spec};

const ISS: u16 = 0x2A48; // sprmCIss: 1 = superscript

/// A one-paragraph document `text` with footnote stories `stories`, references at `refs`
/// (CP, data element) and the given character runs.
fn doc_with(text: &'static str, stories: Vec<&'static str>, refs: &[(u32, u16)], runs: Vec<(u32, Vec<u8>)>, endnotes: bool) -> wordcraft_doc::Document {
    let ccp = text.chars().count() as u32;
    let mut cps: Vec<u32> = refs.iter().map(|r| r.0).collect();
    cps.push(ccp);
    let data: Vec<u8> = refs.iter().flat_map(|r| r.1.to_le_bytes()).collect();
    let mut story_cps = vec![0u32];
    for s in &stories {
        let last = story_cps.last().copied().unwrap_or(0);
        story_cps.push(last + s.chars().count() as u32);
    }
    let (ref_pair, txt_pair) = if endnotes { (46, 47) } else { (2, 3) };
    let spec = StructSpec {
        text,
        note_stories: if endnotes { vec!["Ftn\r"] } else { stories.clone() },
        endnote_stories: if endnotes { stories } else { vec![] },
        chpx_runs: runs,
        extra_pairs: vec![(ref_pair, plc(&cps, &data)), (txt_pair, plc(&story_cps, &[]))],
        ..structured_spec(text, ccp)
    };
    read(&struct_doc(&spec)).expect("opens")
}

fn first_para(doc: &wordcraft_doc::Document) -> wordcraft_doc::Paragraph {
    doc.body.first().and_then(|b| b.as_para()).cloned().expect("para")
}

/// The note references of `p`: (byte offset of the anchor, kind, id, custom).
fn note_refs(p: &wordcraft_doc::Paragraph) -> Vec<(usize, NoteKind, u32, String)> {
    let anchors = p.text.char_indices().filter(|(_, c)| *c == '\u{FFFC}').map(|(i, _)| i);
    anchors
        .zip(&p.objects)
        .filter_map(|(at, o)| match o {
            InlineObject::NoteRef { kind, id, custom } => Some((at, *kind, *id, custom.clone())),
            _ => None,
        })
        .collect()
}

fn part_text(doc: &wordcraft_doc::Document, id: u32) -> String {
    let part = doc.parts.get(&id).expect("note part");
    part.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_star_footnote_is_a_note_reference() {
    let doc = doc_with("See* here\r", vec!["* Note body\r"], &[(3, 0)], vec![(0, Vec::new())], false);
    let p = first_para(&doc);
    assert_eq!(p.text, "See\u{FFFC} here");
    let refs = note_refs(&p);
    assert_eq!(refs.len(), 1, "{:?}", p.objects);
    let (at, kind, id, custom) = refs.first().cloned().expect("ref");
    assert_eq!((at, kind, custom.as_str()), (3, NoteKind::Footnote, "*"));
    assert_eq!(doc.parts.get(&id).map(|p| p.kind), Some(wordcraft_doc::PartKind::Footnote));
    // The note's own text keeps its mark as text, with no automatic number added.
    assert_eq!(part_text(&doc, id), "* Note body");
    assert!(doc.parts.get(&id).is_some_and(|p| p.blocks.iter().filter_map(|b| b.as_para()).all(|p| p.objects.is_empty())));
}

#[test]
fn longer_marks_are_read_whole() {
    for (text, mark) in [("See** here\r", "**"), ("See a) here\r", "a)")] {
        let at = text.find(mark).expect("mark") as u32;
        let doc = doc_with(text, vec!["Note body\r"], &[(at, 0)], vec![(0, Vec::new())], false);
        let p = first_para(&doc);
        let refs = note_refs(&p);
        assert_eq!(refs.iter().map(|r| r.3.as_str()).collect::<Vec<_>>(), vec![mark]);
        assert!(!p.text.contains('*') && !p.text.contains("a)"), "nothing of the mark left: {:?}", p.text);
    }
}

#[test]
fn the_mark_ends_at_a_formatting_change() {
    // `*` superscript, then plain `here` straight after it (no space): another character run.
    let text = "See*here\r";
    let runs = vec![(0, Vec::new()), (3, grpprl(&[(ISS, &[1])])), (4, Vec::new())];
    let doc = doc_with(text, vec!["Note body\r"], &[(3, 0)], runs, false);
    let p = first_para(&doc);
    assert_eq!(p.text, "See\u{FFFC}here");
    let refs = note_refs(&p);
    assert_eq!(refs.first().map(|r| r.3.as_str()), Some("*"));
    // The object takes the run's formatting.
    let run = p.runs.iter().scan(0, |at, r| {
        let start = *at;
        *at += r.len;
        Some((start, r))
    });
    let obj_run = run.filter(|(start, r)| *start <= 3 && 3 < start + r.len).map(|(_, r)| r.props.vert_align).next();
    assert_eq!(obj_run, Some(Some(wordcraft_doc::props::VertAlign::Superscript)));
}

#[test]
fn the_mark_is_capped_at_ten_characters() {
    let text = "See ABCDEFGHIJKLMN\r";
    let doc = doc_with(text, vec!["Note body\r"], &[(4, 0)], vec![(0, Vec::new())], false);
    let p = first_para(&doc);
    assert_eq!(note_refs(&p).first().map(|r| r.3.clone()), Some("ABCDEFGHIJ".to_string()));
    assert_eq!(p.text, "See \u{FFFC}KLMN");
}

#[test]
fn a_custom_mark_endnote_is_a_note_reference() {
    let doc = doc_with("Ref*\r", vec!["* Endnote body\r"], &[(3, 0)], vec![(0, Vec::new())], true);
    let p = first_para(&doc);
    assert_eq!(p.text, "Ref\u{FFFC}");
    let refs = note_refs(&p);
    let (_, kind, id, custom) = refs.first().cloned().expect("ref");
    assert_eq!((kind, custom.as_str()), (NoteKind::Endnote, "*"));
    assert_eq!(doc.parts.get(&id).map(|p| p.kind), Some(wordcraft_doc::PartKind::Endnote));
    assert_eq!(part_text(&doc, id), "* Endnote body");
}

#[test]
fn auto_and_custom_footnotes_in_one_paragraph() {
    let doc = doc_with("A\u{2} b* c\r", vec!["One\r", "* Two\r"], &[(1, 1), (4, 0)], vec![(0, Vec::new())], false);
    let p = first_para(&doc);
    assert_eq!(p.text, "A\u{FFFC} b\u{FFFC} c");
    let refs = note_refs(&p);
    assert_eq!(refs.iter().map(|r| r.3.as_str()).collect::<Vec<_>>(), vec!["", "*"]);
    let ids: Vec<u32> = refs.iter().map(|r| r.2).collect();
    assert_ne!(ids.first(), ids.get(1));
    assert_eq!(ids.iter().map(|&id| part_text(&doc, id)).collect::<Vec<_>>(), vec!["One", "* Two"]);
}

#[test]
fn a_custom_reference_on_the_paragraph_mark_still_links() {
    let doc = doc_with("Text\rMore\r", vec!["Note body\r"], &[(4, 0)], vec![(0, Vec::new())], false);
    let p = first_para(&doc);
    assert_eq!(p.text, "Text\u{FFFC}");
    let refs = note_refs(&p);
    assert_eq!(refs.first().map(|r| r.3.as_str()), Some(""));
    let second = doc.body.get(1).and_then(|b| b.as_para()).map(|p| p.text.clone());
    assert_eq!(second.as_deref(), Some("More"), "the paragraph still ends at its mark");
}

#[test]
fn hostile_custom_references_never_panic() {
    // A reference past the end of the text is ignored.
    let doc = doc_with("Text\r", vec!["Note body\r"], &[(900, 0)], vec![(0, Vec::new())], false);
    assert_eq!(first_para(&doc).text, "Text");
    // More references than the cap: the table is not read, as for automatic notes.
    let refs: Vec<(u32, u16)> = (0..10_001).map(|_| (1, 0)).collect();
    let doc = doc_with("Text\r", vec!["Note body\r"], &refs, vec![(0, Vec::new())], false);
    assert_eq!(first_para(&doc).text, "Text");
}
