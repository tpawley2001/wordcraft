//! A floating table placed relative to the text (`w:tblpPr w:vertAnchor="text"`) with one row that
//! runs past the bottom margin stays where its offset puts it, on its anchor's page, as in Word; one
//! that would run past the page's bottom edge is moved up to end at it. Measured in Word (Letter,
//! 1-inch margins): a 108 pt row 576 pt below its anchor ends at 781 pt (page 1 of 1); a 160 pt row
//! there is drawn from 632 to 792 pt (page 1 of 1).
use crate::hit::Caret;
use crate::*;
use wordcraft_doc::para::Anchor;
use wordcraft_doc::props::{HeightRule, TableFloat};
use wordcraft_doc::{Pos, Table};

fn lay(doc: &Document) -> DocLayout {
    crate::test_fonts();
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// "BEFORE", a one-column floating table `y` pt below its anchor with one exact `row_h` pt row that
/// can't split (its cell reads "ROWONE"), then "ANCHOR" (the table's anchor) and "AFTER".
fn doc(y: f32, row_h: f32) -> Document {
    let mut d = Document::from_text("BEFORE\nANCHOR\nAFTER");
    assert_eq!(d.last_section.page_h, 792.0);
    assert_eq!(d.last_section.margin_bottom, 72.0);
    let mut t = Table::new(1, 1, 200.0);
    t.props.float = Some(TableFloat { h_rel: Anchor::Margin, v_rel: Anchor::Paragraph, y, dist: [9.0, 0.0, 9.0, 0.0], ..Default::default() });
    for r in &mut t.rows {
        r.props.height = Some(row_h);
        r.props.height_rule = HeightRule::Exact;
        r.props.cant_split = true;
    }
    d.insert_block(StoryRef::Body, &Path::top(1), Block::Table(t)).unwrap();
    d.para_mut(StoryRef::Body, &Path(vec![1, 0, 0, 0])).unwrap().insert_text(0, "ROWONE", &Default::default()).unwrap();
    d
}

/// (pages, anchor paragraph's caret, table cell text's caret).
fn place(d: &Document) -> (usize, Caret, Caret) {
    let l = lay(d);
    let anchor = l.caret(&Pos::body(2, 0)).expect("the anchor paragraph is placed");
    let cell = l.caret(&Pos { story: StoryRef::Body, path: Path(vec![1, 0, 0, 0]), off: 0 }).expect("the table is placed");
    (l.pages.len(), anchor, cell)
}

#[test]
fn a_one_row_paragraph_anchored_table_past_the_bottom_margin_stays_on_its_page() {
    let (pages, anchor, cell) = place(&doc(576.0, 108.0));
    assert_eq!(pages, 1, "Word: 1 page; anchor {anchor:?}, table {cell:?}");
    assert_eq!(anchor.page, 0, "the anchor paragraph stays on page 1: {anchor:?}");
    assert_eq!(cell.page, 0, "the table stays on page 1: {cell:?}");
    let top = anchor.top + 576.0;
    assert!(top + 108.0 > 720.0, "the table does run past the bottom margin: {top}");
    assert!((cell.top - top).abs() < 1.5, "at its offset below the anchor ({top}), hanging past the margin: {cell:?}");
}

#[test]
fn a_one_row_table_past_the_page_edge_is_moved_up_to_end_at_it() {
    let (pages, anchor, cell) = place(&doc(576.0, 160.0));
    assert_eq!(pages, 1, "Word: 1 page; anchor {anchor:?}, table {cell:?}");
    assert_eq!(anchor.page, 0, "the anchor paragraph stays on page 1: {anchor:?}");
    assert_eq!(cell.page, 0, "the table stays on page 1: {cell:?}");
    // Word: drawn from 632 to 792, its bottom on the page's bottom edge.
    let top = 792.0 - 160.0;
    assert!((cell.top - top).abs() < 1.5, "moved up to end at the page edge (top {top}): {cell:?}");
}

/// Guard (passes before the change): a table that fits sits at its offset below its anchor.
#[test]
fn a_paragraph_anchored_table_that_fits_is_placed_at_its_offset() {
    let (pages, anchor, cell) = place(&doc(72.0, 108.0));
    assert_eq!(pages, 1);
    assert_eq!((anchor.page, cell.page), (0, 0));
    assert!((cell.top - (anchor.top + 72.0)).abs() < 1.5, "anchor {anchor:?}, table {cell:?}");
}
