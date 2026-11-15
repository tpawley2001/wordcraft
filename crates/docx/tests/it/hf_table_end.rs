//! A header or footer whose last block is a table is saved without an extra empty paragraph after
//! the table (Word opens such headers as they are, no taller), while body text, table cells and
//! headers that need a paragraph (a pending watermark, a table that writes nothing) keep one.

use std::sync::Arc;

use wordcraft_doc::props::CharProps;
use wordcraft_doc::table::{Cell, Row, Table};
use wordcraft_doc::{Block, Blocks, Document, Paragraph, PartKind, Watermark, para_block};

fn part_xml(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut z.by_name(name).unwrap(), &mut xml).unwrap();
    xml
}

fn para(s: &str) -> Arc<Block> {
    para_block(Paragraph::with_text(s, CharProps::default()))
}

fn table_1x1(s: &str) -> Arc<Block> {
    let mut t = Table::new(1, 1, 144.0);
    t.rows[0].cells[0] = Cell::with_text(s);
    t.rows[0].cells[0].props.width = Some(144.0);
    Arc::new(Block::Table(t))
}

/// A document with body text and the given blocks as its default header (or footer).
fn doc_with_hf(blocks: Blocks, footer: bool) -> Document {
    let mut d = Document::new();
    d.body = vec![para("Body")];
    if footer {
        let id = d.add_part(PartKind::Footer, blocks);
        d.last_section.footers.default = Some(id);
    } else {
        let id = d.add_part(PartKind::Header, blocks);
        d.last_section.headers.default = Some(id);
    }
    d
}

fn squeeze(xml: &str) -> String {
    xml.split_whitespace().collect::<Vec<_>>().join("")
}

fn hf_blocks(doc: &Document, kind: PartKind) -> Blocks {
    doc.parts.values().find(|p| p.kind == kind).map(|p| p.blocks.clone()).expect("part")
}

/// Case A: header [Para, Table] ends with the table and reads back as two blocks.
#[test]
fn header_ending_in_table_gets_no_trailing_paragraph() {
    let d = doc_with_hf(vec![para("x"), table_1x1("c")], false);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let hdr = squeeze(&part_xml(&bytes, "word/header1.xml"));
    assert!(hdr.ends_with("</w:tbl></w:hdr>"), "header1.xml ends: {}", &hdr[hdr.len().saturating_sub(80)..]);
    let r = wordcraft_docx::read(&bytes).unwrap();
    let blocks = hf_blocks(&r, PartKind::Header);
    assert_eq!(blocks.len(), 2, "no empty paragraph added after the table");
    assert!(matches!(*blocks[1], Block::Table(_)));
}

/// Case B: the same for a footer.
#[test]
fn footer_ending_in_table_gets_no_trailing_paragraph() {
    let d = doc_with_hf(vec![para("x"), table_1x1("c")], true);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let ftr = squeeze(&part_xml(&bytes, "word/footer1.xml"));
    assert!(ftr.ends_with("</w:tbl></w:ftr>"), "footer1.xml ends: {}", &ftr[ftr.len().saturating_sub(80)..]);
    let r = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(hf_blocks(&r, PartKind::Footer).len(), 2);
}

/// Case C: a header holding only a table still gets a paragraph for the pending watermark, after the table.
#[test]
fn watermark_header_ending_in_table_keeps_its_shape_paragraph() {
    let mut d = doc_with_hf(vec![table_1x1("c")], false);
    d.settings.watermark = Some(Watermark::default());
    let bytes = wordcraft_docx::write(&d).unwrap();
    let hdr = part_xml(&bytes, "word/header1.xml");
    assert_eq!(hdr.matches("<v:shape ").count(), 1, "one watermark shape");
    let tbl_end = hdr.rfind("</w:tbl>").expect("table written");
    let shape = hdr.find("<v:shape ").unwrap();
    assert!(shape > tbl_end, "the watermark paragraph follows the table");
    assert!(hdr[tbl_end..].contains("<w:p"), "the shape sits in a paragraph");
    assert_eq!(wordcraft_docx::read(&bytes).unwrap().settings.watermark, d.settings.watermark);
}

/// Case D: a table the writer skips (no row has cells) leaves the story with its closing paragraph.
#[test]
fn header_whose_table_writes_nothing_keeps_a_paragraph() {
    let mut t = Table::new(1, 1, 144.0);
    t.rows = vec![Row { cells: Vec::new(), ..t.rows[0].clone() }];
    let d = doc_with_hf(vec![Arc::new(Block::Table(t))], false);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let hdr = squeeze(&part_xml(&bytes, "word/header1.xml"));
    assert!(!hdr.contains("<w:tbl>"), "the empty table is not written");
    assert!(hdr.contains("<w:p/>") || hdr.contains("<w:p>"), "the header still holds a paragraph: {hdr}");
}

/// Case E: the body still ends with a paragraph after a final table.
#[test]
fn body_ending_in_table_keeps_its_trailing_paragraph() {
    let mut d = Document::new();
    d.body = vec![para("x"), table_1x1("c")];
    let bytes = wordcraft_docx::write(&d).unwrap();
    let doc = squeeze(&part_xml(&bytes, "word/document.xml"));
    let tbl_end = doc.rfind("</w:tbl>").expect("table");
    let sect = doc.rfind("<w:sectPr").expect("sectPr");
    assert!(doc[tbl_end..sect].contains("<w:p"), "a paragraph between the last table and the body's sectPr");
}

/// Case F: a header cell that ends in a nested table still ends with a paragraph (a cell must).
#[test]
fn header_cell_ending_in_nested_table_keeps_its_paragraph() {
    let mut outer = Table::new(1, 1, 288.0);
    outer.rows[0].cells[0].blocks = vec![para("a"), table_1x1("inner")];
    let d = doc_with_hf(vec![Arc::new(Block::Table(outer)), para("after")], false);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let hdr = squeeze(&part_xml(&bytes, "word/header1.xml"));
    assert!(hdr.contains("</w:tbl><w:p/></w:tc>"), "the outer cell ends with a paragraph after its nested table: {hdr}");
}

/// Write -> read -> write gives the same header and footer XML.
#[test]
fn header_xml_is_stable_over_two_round_trips() {
    for footer in [false, true] {
        let d = doc_with_hf(vec![para("x"), table_1x1("c")], footer);
        let name = if footer { "word/footer1.xml" } else { "word/header1.xml" };
        let b1 = wordcraft_docx::write(&d).unwrap();
        let b2 = wordcraft_docx::write(&wordcraft_docx::read(&b1).unwrap()).unwrap();
        assert_eq!(squeeze(&part_xml(&b1, name)), squeeze(&part_xml(&b2, name)), "{name}");
    }
}
