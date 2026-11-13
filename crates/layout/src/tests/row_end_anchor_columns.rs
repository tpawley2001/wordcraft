//! A Word 97-2003 table whose last row anchors a floating shape at its row end: the reader moves
//! the shape into the row's last cell and makes no extra cell, so the table keeps its two stored
//! columns (55.6 pt and 480.05 pt) and the second column's text starts where Word draws it
//! (x 97 pt on a page with 36 pt margins, cell margins 5.75 pt, table indent 5.4 pt). The model is
//! the one the `.doc` reader builds for such a row; it is written here in code.
use super::*;
use wordcraft_doc::Paragraph;
use wordcraft_doc::para::{Anchor, Float, Wrap};

fn row_end_anchor_table() -> Document {
    let mut d = Document::from_text("tail");
    d.settings.compat_mode = wordcraft_doc::WORD2003_COMPAT_MODE;
    d.last_section.page_w = 612.0;
    d.last_section.page_h = 792.0;
    d.last_section.margin_left = 36.0;
    d.last_section.margin_right = 36.0;
    d.last_section.margin_top = 36.0;
    d.last_section.margin_bottom = 36.0;
    let mut t = Table::new(3, 2, 535.65);
    t.props.style = None;
    t.props.indent = Some(5.4);
    t.props.cell_margins = Some([0.0, 5.75, 0.0, 5.75]);
    t.grid = vec![55.6, 480.05];
    for row in &mut t.rows {
        for (cell, (w, text)) in row.cells.iter_mut().zip([(55.6, "A"), (480.05, "B")]) {
            cell.props.width = Some(w);
            cell.blocks = vec![wordcraft_doc::para_block(Paragraph::with_text(text, Default::default()))];
        }
    }
    // The last row's floating shape (in front of text, so it moves no line), now at the end of
    // its second cell's paragraph.
    let shape = rect_shape(72.0, 36.0, Float { wrap: Wrap::InFrontOfText, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, ..Default::default() });
    let mut p = Paragraph::with_text("B", Default::default());
    p.insert_object(1, shape, &Default::default()).unwrap();
    if let Some(cell) = t.rows.get_mut(2).and_then(|r| r.cells.get_mut(1)) {
        cell.blocks = vec![wordcraft_doc::para_block(p)];
    }
    d.insert_block(StoryRef::Body, &Path::top(0), Block::Table(t)).unwrap();
    d
}

#[test]
fn a_row_end_anchor_keeps_the_second_column_where_word_draws_it() {
    let d = row_end_anchor_table();
    let layout = lay(&d);
    let items = &layout.pages[0].items;
    let xs: Vec<f32> = items
        .iter()
        .filter_map(|p| {
            if let Placed::Lines { x, para, .. } = p { Some(x + para.lines.first().and_then(|l| l.xs.first()).copied().unwrap_or(0.0)) } else { None }
        })
        .collect();
    let second: Vec<f32> = xs.iter().copied().filter(|x| *x > 60.0 && *x < 400.0).collect();
    assert_eq!(second.len(), 3, "one second-column paragraph per row: {xs:?}");
    for x in second {
        assert!((x - 97.0).abs() <= 0.6, "second column text at {x}: {xs:?}");
    }
}
