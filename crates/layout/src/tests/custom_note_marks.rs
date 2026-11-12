//! A footnote with a custom reference mark (`NoteRef::custom`, as Word 97-2003 and DOCX files
//! give it) shows its mark, places its text at the foot of its reference's page, and takes no
//! automatic number: the next automatic footnote is 1.
use crate::*;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::{PartKind, Pos, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn footnote(d: &mut Document, pos: &Pos, text: &str, custom: &str) -> u32 {
    let id = d.add_part(PartKind::Footnote, vec![para_block(Paragraph::with_text(text, Default::default()))]);
    d.insert_object(pos, InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: custom.into() }, &Default::default()).unwrap();
    id
}

/// The glyphs of each body note reference mark, by note id, and the glyphs of every body cluster.
fn body_glyphs(l: &DocLayout) -> (HashMap<u32, Vec<u32>>, Vec<Vec<u32>>) {
    let mut marks = HashMap::new();
    let mut all = Vec::new();
    for it in l.pages.iter().flat_map(|p| p.items.iter()) {
        if let Placed::Lines { story: StoryRef::Body, para, .. } = it {
            let gids = |ci: usize| para.clusters.get(ci).map(|c| para.glyphs[c.g0 as usize..c.g1 as usize].iter().map(|g| g.gid).collect::<Vec<_>>());
            for (ci, id) in &para.notes {
                marks.insert(*id, gids(*ci).unwrap_or_default());
            }
            all.extend((0..para.clusters.len()).filter_map(gids));
        }
    }
    (marks, all)
}

#[test]
fn a_custom_mark_note_shows_its_mark_and_takes_no_number() {
    crate::test_fonts();
    // The second paragraph's `*` and `1` give the glyphs the marks should use.
    let mut d = Document::from_text("See here and there.\n*1");
    let star = footnote(&mut d, &Pos::body(0, 3), "* Starred note payload.", "*");
    let auto = footnote(&mut d, &Pos::body(0, "See\u{FFFC} here and".len()), "Numbered note payload.", "");

    let nums = crate::notes::note_numbers(&d, HideDeleted::default());
    assert_eq!(nums.get(&auto), Some(&1), "the automatic note after a custom one is 1: {nums:?}");

    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    let (marks, all) = body_glyphs(&l);
    let star_mark = marks.get(&star).cloned().expect("the custom mark is laid out");
    let auto_mark = marks.get(&auto).cloned().expect("the automatic mark is laid out");
    let n = all.len();
    let (star_glyph, one_glyph) = (all.get(n - 2).cloned().unwrap(), all.get(n - 1).cloned().unwrap());
    assert_eq!(star_mark, star_glyph, "the reference shows `*`");
    assert_eq!(auto_mark, one_glyph, "the automatic note is numbered 1");

    // Both notes' text sits at the foot of the page, below the body.
    let body = l.caret(&Pos::body(1, 0)).unwrap();
    for id in [star, auto] {
        let c = l.caret(&d.start_of(StoryRef::Part(id))).expect("the note is placed");
        assert_eq!(c.page, 0);
        assert!(c.top > body.top + 100.0, "at the page foot: {c:?} {body:?}");
    }
}
