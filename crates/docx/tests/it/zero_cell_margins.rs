//! A table whose default cell margins are all zero (as a `.doc` row with no stored cell gap gives
//! them) keeps them through a save: `w:tblCellMar` (ECMA-376 Part 1 §17.4.42) is written with
//! left and right of 0, so a reader does not fall back to the 5.4 pt default.

use std::io::Read;
use std::sync::Arc;

use wordcraft_doc::table::Table;
use wordcraft_doc::{Block, Document, Paragraph, para_block};

#[test]
fn zero_table_cell_margins_are_saved_and_read_back() {
    let mut t = Table::new(1, 2, 144.0);
    t.props.cell_margins = Some([0.0; 4]);
    let mut d = Document::new();
    d.body = vec![Arc::new(Block::Table(t)), para_block(Paragraph::new())];
    let bytes = wordcraft_docx::write(&d).unwrap();

    let mut z = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    let mar = xml.split("<w:tblCellMar>").nth(1).unwrap().split("</w:tblCellMar>").next().unwrap();
    let side_zero = |a: &str, b: &str| mar.contains(&format!("<w:{a} w:w=\"0\"")) || mar.contains(&format!("<w:{b} w:w=\"0\""));
    assert!(side_zero("left", "start"), "{mar}");
    assert!(side_zero("right", "end"), "{mar}");

    let r = wordcraft_docx::read(&bytes).unwrap();
    let table = r.body.iter().find_map(|b| b.as_table()).unwrap();
    assert_eq!(table.props.cell_margins, Some([0.0; 4]));
}
