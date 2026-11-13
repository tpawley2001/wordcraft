//! A `.doc` table whose first row has table borders and whose second row has none reads as table
//! borders plus a row exception of "no border" on every side (`crates/docbin` `table.rs`). Saved
//! as DOCX (`w:tblPrEx`) and read back, the table and the row keep exactly those borders.

use wordcraft_doc::props::{Border, BorderStyle, Borders, TableProps};
use wordcraft_doc::{Block, Document, Table};

const S: Border = Border { style: BorderStyle::Single, width: 0.5, color: None, space: 0.0 };

#[test]
fn row_exception_without_borders_survives_a_docx_round_trip() {
    // The model the .doc reader builds for two 2-cell rows: row 1 with sprmTTableBorders80
    // (six 0.5 pt single lines), row 2 without.
    let mut t = Table::new(2, 2, 288.0);
    t.props.style = None;
    t.props.borders = Some(Borders::all(S));
    t.rows[1].props.exceptions = Some(Box::new(TableProps { borders: Some(Borders::all(Border::default())), ..Default::default() }));
    let mut doc = Document::new();
    doc.body = vec![std::sync::Arc::new(Block::Table(t))];

    let bytes = crate::io::save_bytes("rows.docx", &doc).unwrap();
    let back = crate::io::open_bytes("rows.docx", &bytes).unwrap();
    let t = back.body.iter().find_map(|b| b.as_table()).expect("table");
    let side = |b: Option<Border>| b.map(|b| (b.style, b.width));
    let sides = |b: Borders| [b.top, b.left, b.bottom, b.right, b.between, b.inside_v].map(side);
    assert_eq!(t.props.borders.map(sides), Some([Some((BorderStyle::Single, 0.5)); 6]));
    assert!(t.rows[0].props.exceptions.is_none());
    let ex = t.rows[1].props.exceptions.as_ref().and_then(|e| e.borders).expect("row 2 keeps its exception");
    assert_eq!(sides(ex), [Some((BorderStyle::None, 0.0)); 6]);
}
