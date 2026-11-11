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
