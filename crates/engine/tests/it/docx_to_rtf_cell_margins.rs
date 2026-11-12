//! A DOCX table's cell margins saved as RTF and reopened: the table's default cell margins (`w:tblCellMar`,
//! ECMA-376 Part 1 §17.4.43), one cell's own left margin (`w:tcMar`, §17.4.42; the other sides stay the table's)
//! and Wrap text off (`w:noWrap`, §17.4.30) come back as they were. In RTF the default is the row's `\trpadd*`
//! words, the cell's own left is `\clpadt` (Word swaps the cell's left and top words) and Wrap text off is
//! `\clNoWrap`. The fixture is hand-written from the spec.

use std::io::Write;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn docx(body: &str) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, xml) in [("[Content_Types].xml", types.to_string()), ("_rels/.rels", root.to_string()), ("word/document.xml", doc)] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(xml.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn cell(tc_pr: &str, text: &str) -> String {
    format!(r#"<w:tc><w:tcPr><w:tcW w:w="2880" w:type="dxa"/>{tc_pr}</w:tcPr><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>"#)
}

/// Default margins top 1 pt, left 2 pt, bottom 3 pt, right 4 pt; cell A's own left 12 pt; cell B Wrap text off.
fn fixture() -> Vec<u8> {
    let a = cell(r#"<w:tcMar><w:left w:w="240" w:type="dxa"/></w:tcMar>"#, "A");
    let b = cell("<w:noWrap/>", "B");
    docx(&format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblCellMar><w:top w:w="20" w:type="dxa"/><w:left w:w="40" w:type="dxa"/><w:bottom w:w="60" w:type="dxa"/><w:right w:w="80" w:type="dxa"/></w:tblCellMar></w:tblPr><w:tblGrid><w:gridCol w:w="2880"/><w:gridCol w:w="2880"/></w:tblGrid><w:tr>{a}{b}</w:tr></w:tbl><w:p><w:r><w:t>after</w:t></w:r></w:p>"#
    ))
}

fn table(d: &wordcraft_doc::Document) -> &wordcraft_doc::Table {
    d.body.iter().find_map(|b| b.as_table()).expect("a table")
}

/// What the document says about the table's cell margins: the default, each cell's own sides and Wrap text off.
type Margins = ([f32; 4], Vec<[Option<f32>; 4]>, Vec<bool>);

fn margins(d: &wordcraft_doc::Document) -> Margins {
    let t = table(d);
    let round = |v: f32| (v * 100.0).round() / 100.0;
    let cells = &t.rows[0].cells;
    let own = cells.iter().map(|c| c.props.margins.map(|m| m.map(Some)).or(c.props.partial_margins).unwrap_or([None; 4]).map(|s| s.map(round)));
    (t.props.cell_margins.expect("the table's default cell margins").map(round), own.collect(), cells.iter().map(|c| c.props.no_wrap).collect())
}

fn expected() -> Margins {
    ([1.0, 2.0, 3.0, 4.0], vec![[None, Some(12.0), None, None], [None; 4]], vec![false, true])
}

#[test]
fn the_docx_fixture_reads_as_written() {
    assert_eq!(margins(&wordcraft_docx::read(&fixture()).unwrap()), expected());
}

#[test]
fn cell_margins_and_wrap_text_off_survive_a_save_as_rtf() {
    let d = wordcraft_docx::read(&fixture()).unwrap();
    let rtf = wordcraft_formats::rtf::export(&d);
    assert!(rtf.contains(r"\trpaddl40\trpaddt20\trpaddb60\trpaddr80\trpaddfl3\trpaddft3\trpaddfb3\trpaddfr3"), "the default margins:\n{rtf}");
    assert_eq!(rtf.matches(r"\clpadt240\clpadft3").count(), 1, "cell A's own left margin, in Word's swapped word:\n{rtf}");
    assert!(!rtf.contains(r"\clpadl") && !rtf.contains(r"\clpadb") && !rtf.contains(r"\clpadr"), "only the left is the cell's own:\n{rtf}");
    assert_eq!(rtf.matches(r"\clNoWrap").count(), 1, "{rtf}");
    let back = wordcraft_formats::rtf::import(rtf.as_bytes()).unwrap();
    assert_eq!(margins(&back), expected());
    // And once more through RTF: nothing drifts.
    let again = wordcraft_formats::rtf::import(wordcraft_formats::rtf::export(&back).as_bytes()).unwrap();
    assert_eq!(margins(&again), expected());
}
