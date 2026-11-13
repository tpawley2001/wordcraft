//! A `.doc` table row whose row-end mark anchored a floating shape reads as a row of its stored
//! two cells with the shape at the end of the second cell's paragraph (`crates/docbin` `table.rs`).
//! Saved as DOCX, the table has two grid columns, the row two `w:tc`, and the anchored drawing is
//! inside the second cell; read back, the row still has two cells and the shape. The model is the
//! one the `.doc` reader builds for such a row, written here in code.

use std::io::Read;

use wordcraft_doc::para::{Anchor, Float, InlineObject, ShapeKind, Wrap};
use wordcraft_doc::{Block, Document, Paragraph, Table};

fn shape() -> InlineObject {
    InlineObject::Shape {
        kind: ShapeKind::Ellipse,
        w: 72.0,
        h: 36.0,
        fill: None,
        stroke: None,
        stroke_width: 1.0,
        float: Float { wrap: Wrap::Square, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, ..Default::default() },
        story: None,
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    }
}

fn document_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    xml
}

#[test]
fn a_row_end_anchor_saves_inside_the_second_of_two_cells() {
    let mut t = Table::new(1, 2, 535.65);
    t.props.style = None;
    t.grid = vec![55.6, 480.05];
    for (cell, (w, text)) in t.rows[0].cells.iter_mut().zip([(55.6, "A"), (480.05, "B")]) {
        cell.props.width = Some(w);
        cell.blocks = vec![wordcraft_doc::para_block(Paragraph::with_text(text, Default::default()))];
    }
    let mut p = Paragraph::with_text("B", Default::default());
    p.insert_object(1, shape(), &Default::default()).unwrap();
    t.rows[0].cells[1].blocks = vec![wordcraft_doc::para_block(p)];
    let mut doc = Document::new();
    doc.body = vec![std::sync::Arc::new(Block::Table(t)), wordcraft_doc::para_block(Paragraph::new())];

    let bytes = crate::io::save_bytes("row.docx", &doc).unwrap();
    let xml = document_xml(&bytes);
    assert_eq!(xml.matches("<w:gridCol ").count(), 2, "{xml}");
    let row = xml.split("<w:tr").nth(1).and_then(|r| r.split("</w:tr>").next()).expect("a row");
    let cells: Vec<&str> = row.split("<w:tc>").skip(1).collect();
    assert_eq!(cells.len(), 2, "{row}");
    assert!(!cells[0].contains("<w:drawing>") && cells[1].contains("<wp:anchor"), "{row}");

    let back = crate::io::open_bytes("row.docx", &bytes).unwrap();
    let t = back.body.iter().find_map(|b| b.as_table()).expect("table");
    assert_eq!(t.rows[0].cells.len(), 2);
    let p = t.rows[0].cells[1].blocks.last().and_then(|b| b.as_para()).expect("paragraph");
    assert_eq!(p.objects.len(), 1);
    assert!(p.objects[0].is_floating());
}
