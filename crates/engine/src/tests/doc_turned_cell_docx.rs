//! A `.doc` header cell whose TC80 text flow turns it (`crates/docbin` `table.rs`) reads as a cell
//! with a text direction. Saved as DOCX (`w:textDirection`) and read back, it keeps the turn.

use wordcraft_doc::props::TextDirection;
use wordcraft_doc::{Block, Document, Table};

#[test]
fn a_turned_doc_cell_is_saved_with_its_direction() {
    // The model the .doc reader builds for a header row: a text cell, then two cells whose TCGRF
    // text flow is 3 (bottom to top) and 1 (top to bottom).
    let mut t = Table::new(2, 3, 186.0);
    t.rows[0].cells[1].props.text_direction = TextDirection::Up;
    t.rows[0].cells[2].props.text_direction = TextDirection::Down;
    let mut doc = Document::new();
    doc.body = vec![std::sync::Arc::new(Block::Table(t))];

    let bytes = crate::io::save_bytes("turned.docx", &doc).unwrap();
    let back = crate::io::open_bytes("turned.docx", &bytes).unwrap();
    let t = back.body.iter().find_map(|b| b.as_table()).expect("table");
    let dirs: Vec<_> = t.rows.iter().map(|r| r.cells.iter().map(|c| c.props.text_direction).collect::<Vec<_>>()).collect();
    assert_eq!(dirs[0], vec![TextDirection::Horizontal, TextDirection::Up, TextDirection::Down]);
    assert_eq!(dirs[1], vec![TextDirection::Horizontal; 3]);
}
