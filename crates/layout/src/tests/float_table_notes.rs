//! A footnote referenced from a cell of a floating table (`w:tblpPr`) is placed at the foot of the
//! page the table is drawn on, like one in an inline table or a body paragraph.
use crate::*;
use wordcraft_doc::para::{Anchor, InlineObject, NoteKind};
use wordcraft_doc::props::TableFloat;
use wordcraft_doc::{PartKind, Pos, Table, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn footnote(d: &mut Document, pos: &Pos, text: &str, custom: &str) -> u32 {
    let id = d.add_part(PartKind::Footnote, vec![para_block(Paragraph::with_text(text, Default::default()))]);
    d.insert_object(pos, InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: custom.into() }, &Default::default()).unwrap();
    id
}

/// A document of `body` text with a 1x1 table (floating when `float` is set) before it whose cell
/// reads "Table reference" followed by a footnote reference; returns the note's part id.
fn table_with_note(float: Option<TableFloat>, custom: &str) -> (Document, u32) {
    let mut d = Document::from_text("After the table.");
    let mut t = Table::new(1, 1, 300.0);
    t.props.float = float;
    d.insert_block(StoryRef::Body, &Path::top(0), Block::Table(t)).unwrap();
    let cell = Pos { story: StoryRef::Body, path: Path(vec![0, 0, 0, 0]), off: 0 };
    d.para_mut(StoryRef::Body, &cell.path).unwrap().insert_text(0, "Table reference", &Default::default()).unwrap();
    let id = footnote(&mut d, &Pos { off: "Table reference".len(), ..cell }, "Floating table note payload.", custom);
    (d, id)
}

fn assert_at_page_foot(d: &Document, id: u32) {
    let l = lay(d);
    assert_eq!(l.pages.len(), 1);
    let c = l.caret(&d.start_of(StoryRef::Part(id))).expect("the floating table's footnote is placed");
    assert_eq!(c.page, 0);
    let body = l.caret(&Pos::body(1, 0)).unwrap();
    assert!(c.top > body.top + 100.0, "at the page bottom: {c:?} {body:?}");
}

/// Guard: an inline table's footnote is placed (this passes before the change).
#[test]
fn an_inline_tables_footnote_is_placed() {
    let (d, id) = table_with_note(None, "");
    assert_at_page_foot(&d, id);
}

#[test]
fn a_paragraph_anchored_floating_tables_footnote_is_placed() {
    let f = TableFloat { h_rel: Anchor::Column, v_rel: Anchor::Paragraph, overlap: false, ..Default::default() };
    let (d, id) = table_with_note(Some(f), "");
    assert_at_page_foot(&d, id);
}

/// stress011.docx's title-page table: page-anchored at tblpX 1135 / tblpY 285 twips (56.75pt,
/// 14.25pt), its note marked with a custom "*" (`w:customMarkFollows`).
#[test]
fn a_page_anchored_floating_tables_custom_mark_footnote_is_placed() {
    let f = TableFloat { h_rel: Anchor::Page, v_rel: Anchor::Page, x: 56.75, y: 14.25, overlap: false, ..Default::default() };
    let (d, id) = table_with_note(Some(f), "*");
    assert_at_page_foot(&d, id);
}

/// The floating table's note comes first in document order, so it is stacked above a note
/// referenced from the body paragraph after the table.
#[test]
fn a_floating_tables_footnote_stacks_above_a_later_body_note() {
    let f = TableFloat { h_rel: Anchor::Column, v_rel: Anchor::Paragraph, overlap: false, ..Default::default() };
    let (mut d, tid) = table_with_note(Some(f), "");
    let bid = footnote(&mut d, &Pos::body(1, "After the table.".len()), "Body note payload.", "");
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    let t = l.caret(&d.start_of(StoryRef::Part(tid))).expect("the floating table's footnote is placed");
    let b = l.caret(&d.start_of(StoryRef::Part(bid))).expect("the body footnote is placed");
    assert_eq!((t.page, b.page), (0, 0));
    assert!(t.top < b.top, "table note at {}, body note at {}", t.top, b.top);
}
