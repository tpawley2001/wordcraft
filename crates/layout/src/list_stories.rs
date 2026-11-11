//! List numbers count separately in each story, as in Word (ECMA-376 §17.9): the body, all
//! headers and footers together, all text boxes together, all footnotes together and all
//! endnotes together. A list used in several stories starts from its start value in each.
//!
//! The body counts as the layout walks it. Every other story is laid out apart (a header again
//! for each page, a note or text box again when the page is laid out again), so each of its
//! parts starts from counters worked out here once, in its story's order.

use std::collections::{HashMap, HashSet};

use wordcraft_doc::numbering::Counters;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::{Document, PartKind, StoryRef};

/// The list counters each header, footer, text box, footnote and endnote part starts from.
pub(crate) fn part_starts(doc: &Document) -> HashMap<u32, Counters> {
    let mut out = HashMap::new();
    if doc.parts.is_empty() {
        return out;
    }
    // Headers and footers: per section, Word's header-story order.
    let mut hf = Vec::new();
    for (_, s) in doc.sections() {
        for id in [s.headers.even, s.headers.default, s.footers.even, s.footers.default, s.headers.first, s.footers.first].into_iter().flatten() {
            hf.push(id);
        }
    }
    // Text boxes and notes: in the order the body shows them, text boxes included.
    let (mut boxes, mut feet, mut ends) = (Vec::new(), Vec::new(), Vec::new());
    doc.objects_in_reading_order_at(&mut |p, k| {
        if let Some(o) = p.objects.get(k) {
            if let InlineObject::NoteRef { kind, id, .. } = o {
                match kind {
                    NoteKind::Footnote => feet.push(*id),
                    NoteKind::Endnote => ends.push(*id),
                }
            }
            boxes.extend(o.text_boxes());
        }
        true
    });
    for (kinds, order) in [
        (&[PartKind::Header, PartKind::Footer][..], hf),
        (&[PartKind::TextBox][..], boxes),
        (&[PartKind::Footnote][..], feet),
        (&[PartKind::Endnote][..], ends),
    ] {
        // Parts nothing in the body points at go last, in id order, so they still count apart.
        let rest: Vec<u32> = doc.parts.iter().filter(|(_, p)| kinds.contains(&p.kind)).map(|(id, _)| *id).collect();
        let mut counters = Counters::default();
        let mut seen = HashSet::new();
        for id in order.into_iter().chain(rest) {
            if !doc.parts.get(&id).is_some_and(|p| kinds.contains(&p.kind)) || !seen.insert(id) {
                continue;
            }
            out.insert(id, counters.clone());
            count(doc, StoryRef::Part(id), &mut counters);
        }
    }
    out
}

/// Advance `counters` over the list paragraphs of `story`, in document order.
fn count(doc: &Document, story: StoryRef, counters: &mut Counters) {
    for path in doc.para_paths(story) {
        let Some(p) = doc.para(story, &path) else { continue };
        if let Some(n) = p.props.numbering.or_else(|| doc.styles.resolve_para(&p.props).numbering).filter(|n| n.num != 0) {
            let _ = counters.next_label(&doc.numbering, n.num, n.level);
        }
    }
}

#[cfg(test)]
mod tests {
    //! The text box, footnote and endnote rows of Word's table for one list used in every story
    //! (the body, header and footer rows are `crate::tests::list_numbers_per_story`).
    use crate::*;
    use std::sync::Arc;
    use wordcraft_doc::props::NumRef;
    use wordcraft_doc::{PartKind, para_block};

    fn numbered(text: &str, num: u32) -> Paragraph {
        let mut p = Paragraph::with_text(text, CharProps::default());
        p.props.numbering = Some(NumRef { num, level: 0 });
        p
    }

    /// The list labels of `items`, top to bottom.
    fn labels(items: &[Placed]) -> Vec<String> {
        let mut v: Vec<(f32, String)> = items
            .iter()
            .filter_map(|it| match it {
                Placed::Lines { para, y, l0: 0, .. } => para.label.as_ref().map(|l| (*y, l.text.trim().to_owned())),
                _ => None,
            })
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.into_iter().map(|(_, t)| t).collect()
    }

    fn lay(d: &Document) -> DocLayout {
        let mut c = LayoutCache::new();
        layout(d, &mut c, &LayoutOptions::default())
    }

    /// Body B1-B3, then a paragraph with a text box (T1-T3), two footnotes (F1-F3, G1-G3) and an
    /// endnote (E1-E3), then body B4-B6: every numbered paragraph in one decimal list, as in the
    /// real-file table. Returns the document and the text box, footnote and endnote part ids.
    fn doc_with_notes_and_box() -> (Document, [u32; 4]) {
        use wordcraft_doc::para::{InlineObject, NoteKind, ShapeKind};
        use wordcraft_doc::{Path, Pos};
        crate::test_fonts();
        let mut d = Document::new();
        let num = Arc::make_mut(&mut d.numbering).add_list(wordcraft_doc::ListKind::Numbered);
        let three = |a: &str| (1..=3).map(|i| para_block(numbered(&format!("{a}{i}"), num))).collect::<Vec<_>>();
        let tb = d.add_part(PartKind::TextBox, three("T"));
        let f1 = d.add_part(PartKind::Footnote, three("F"));
        let f2 = d.add_part(PartKind::Footnote, three("G"));
        let e1 = d.add_part(PartKind::Endnote, three("E"));
        let mut body = three("B");
        body.push(para_block(Paragraph::with_text("anchor", CharProps::default())));
        body.extend((4..=6).map(|i| para_block(numbered(&format!("B{i}"), num))));
        d.body = body;
        let shape = InlineObject::Shape {
            kind: ShapeKind::TextBox,
            w: 200.0,
            h: 80.0,
            fill: None,
            stroke: None,
            stroke_width: 1.0,
            float: Default::default(),
            story: Some(tb),
            effects: Default::default(),
            freeform: None,
            extra: Default::default(),
        };
        let at = |d: &Document| d.para(wordcraft_doc::StoryRef::Body, &Path::top(3)).map_or(0, |p| p.len());
        d.insert_object(&Pos::body(3, at(&d)), shape, &CharProps::default()).expect("box");
        for (kind, id) in [(NoteKind::Footnote, f1), (NoteKind::Footnote, f2), (NoteKind::Endnote, e1)] {
            let off = at(&d);
            d.insert_object(&Pos::body(3, off), InlineObject::NoteRef { kind, id, custom: String::new() }, &CharProps::default()).expect("note");
        }
        (d, [tb, f1, f2, e1])
    }

    /// The list labels of story `story` on every page, top to bottom within a page.
    fn story_labels(l: &DocLayout, story: wordcraft_doc::StoryRef) -> Vec<String> {
        l.pages
            .iter()
            .flat_map(|pg| {
                let items: Vec<Placed> = pg.items.iter().filter(|it| matches!(it, Placed::Lines { story: s, .. } if *s == story)).cloned().collect();
                labels(&items)
            })
            .collect()
    }

    #[test]
    fn the_body_runs_on_past_a_text_box_and_notes() {
        let (d, _) = doc_with_notes_and_box();
        let l = lay(&d);
        assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Body), ["1.", "2.", "3.", "4.", "5.", "6."]);
    }

    #[test]
    fn a_text_box_counts_from_one() {
        let (d, [tb, ..]) = doc_with_notes_and_box();
        let l = lay(&d);
        assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Part(tb)), ["1.", "2.", "3."]);
    }

    #[test]
    fn footnotes_share_one_count() {
        let (d, [_, f1, f2, _]) = doc_with_notes_and_box();
        let l = lay(&d);
        assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Part(f1)), ["1.", "2.", "3."]);
        assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Part(f2)), ["4.", "5.", "6."]);
    }

    #[test]
    fn an_endnote_counts_from_one() {
        let (d, [.., e1]) = doc_with_notes_and_box();
        let l = lay(&d);
        assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Part(e1)), ["1.", "2.", "3."]);
    }
}
