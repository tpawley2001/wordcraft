//! A table row at the foot of a page moves to the next page whole when one of its cells holds a
//! single one-line paragraph whose line fits but whose space after doesn't: Word never leaves
//! that cell empty beside the other cells' first lines, in any compatibility mode or with
//! widow control off (measured in Word 16.0 with synthetic documents).

use super::{lay, para_props};
use crate::{Arc, Block, Document, Placed};
use wordcraft_doc::para::LINE_BREAK;
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{Paragraph, Table};

/// Lines of (cell 0, cell 1) on each page: a filler paragraph leaves `room` pt above the bottom
/// margin, then a one-row table whose cell 0 holds `lines` lines with `after` pt after and whose
/// cell 1 holds two lines without widow control.
fn pages(lines: usize, after: f32, widow: bool, room: f32, mode: Option<u32>) -> Vec<(usize, usize)> {
    let exact = ParaProps { line_spacing: Some(LineSpacing::Exactly(14.0)), space_before: Some(0.0), space_after: Some(0.0), ..Default::default() };
    let mut d = Document::from_text("Filler");
    if let Some(mode) = mode {
        d.settings.compat_mode = mode;
    }
    *para_props(&mut d, 0) = ParaProps { line_spacing: Some(LineSpacing::Exactly(d.last_section.text_height() - room)), ..exact.clone() };
    let mut table = Table::new(1, 2, 400.0);
    table.props.style = None;
    table.props.cell_margins = Some([0.0; 4]);
    let join = |words: Vec<String>| words.join(&LINE_BREAK.to_string());
    let a = join((1..=lines).map(|i| format!("IDID{i}")).collect());
    let b = join(vec!["BBBB1".into(), "BBBB2".into()]);
    for (cell, text) in [a, b].into_iter().enumerate() {
        let mut p = Paragraph::with_text(&text, Default::default());
        p.props = exact.clone();
        if cell == 0 {
            p.props.space_after = Some(after);
            p.props.widow_control = Some(widow);
        } else {
            p.props.widow_control = Some(false);
        }
        table.rows[0].cells[cell].blocks = vec![wordcraft_doc::para_block(p)];
    }
    d.body.push(Arc::new(Block::Table(table)));
    lay(&d)
        .pages
        .iter()
        .map(|page| {
            let count = |cell| {
                page.items
                    .iter()
                    .filter_map(|item| match item {
                        Placed::Lines { path, l0, l1, .. } if path.0 == [1, 0, cell, 0] => Some(l1 - l0),
                        _ => None,
                    })
                    .sum::<usize>()
            };
            (count(0), count(1))
        })
        .collect()
}

#[test]
fn a_one_line_cell_without_room_for_its_space_after_moves_the_row() {
    crate::test_fonts();
    for mode in [None, Some(11)] {
        // Probe a2 (a1 in mode 11): the line fits, the 8 pt after doesn't.
        assert_eq!(pages(1, 8.0, false, 18.0, mode)[..2], [(0, 0), (1, 2)], "mode {mode:?}");
        // Probe a4 (a3): no space after, the row splits.
        assert_eq!(pages(1, 0.0, false, 18.0, mode)[..2], [(1, 1), (0, 1)], "mode {mode:?}");
    }
    // A single line has no widow or orphan: widow control changes nothing.
    assert_eq!(pages(1, 8.0, true, 18.0, None)[..2], [(0, 0), (1, 2)]);
}

#[test]
fn rows_that_keep_something_in_every_cell_still_split() {
    crate::test_fonts();
    // Probe a6: two lines, the first stays.
    assert_eq!(pages(2, 8.0, false, 18.0, None)[..2], [(1, 1), (1, 1)]);
    // Probe c2: the line and its space after fit.
    assert_eq!(pages(1, 8.0, false, 22.0, None)[..2], [(1, 1), (0, 1)]);
}

/// A row starting its page whose one-line cell can never have room for its space after (taller
/// than the page) is still placed, and layout ends with every line placed once.
#[test]
fn a_row_starting_its_page_still_splits_or_is_placed() {
    crate::test_fonts();
    let mut d = Document::from_text("");
    let exact = ParaProps { line_spacing: Some(LineSpacing::Exactly(14.0)), space_before: Some(0.0), space_after: Some(0.0), ..Default::default() };
    let mut table = Table::new(1, 2, 400.0);
    table.props.style = None;
    table.props.cell_margins = Some([0.0; 4]);
    let mut a = Paragraph::with_text("IDID", Default::default());
    a.props = ParaProps { space_after: Some(d.last_section.text_height() + 100.0), widow_control: Some(false), ..exact.clone() };
    let b: Vec<String> = (1..=80).map(|i| format!("B{i}")).collect();
    let mut b = Paragraph::with_text(&b.join(&LINE_BREAK.to_string()), Default::default());
    b.props = ParaProps { widow_control: Some(false), ..exact };
    table.rows[0].cells[0].blocks = vec![wordcraft_doc::para_block(a)];
    table.rows[0].cells[1].blocks = vec![wordcraft_doc::para_block(b)];
    d.body = vec![Arc::new(Block::Table(table))];
    let l = lay(&d);
    let count = |cell: u32| {
        l.pages
            .iter()
            .flat_map(|p| &p.items)
            .filter_map(|item| match item {
                Placed::Lines { path, l0, l1, .. } if path.0 == [0, 0, cell, 0] => Some(l1 - l0),
                _ => None,
            })
            .sum::<usize>()
    };
    assert_eq!((count(0), count(1)), (1, 80));
}
