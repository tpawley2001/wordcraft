//! HTML div margins (`w:divId`, ISO/IEC 29500-1 §17.3.1.10 and §17.4.9; `w:divs` in
//! webSettings, §17.15.2.8 `w:div`, §17.15.2.22 `w:marLeft`, §17.15.2.23 `w:marRight`):
//! Word indents a paragraph or table row that names a div by the div's left and right
//! margins, on top of the paragraph's own indents. Documents saved from HTML (Word's "Web
//! Page" round trip) carry them on every paragraph. Fixtures are hand-written from the spec;
//! the 37.5pt shift for `marLeft="750"` was measured in Word's own PDF of such a document.

use std::io::{Read, Write};

use wordcraft_doc::StoryRef;
use wordcraft_layout::{LayoutCache, LayoutOptions, Placed, layout};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// One div with the given left/right margins (twips), as Word writes it in webSettings.xml.
fn div(id: &str, left: u32, right: u32) -> String {
    format!(
        r#"<w:div w:id="{id}"><w:bodyDiv w:val="1"/><w:marLeft w:val="{left}"/><w:marRight w:val="{right}"/><w:marTop w:val="0"/><w:marBottom w:val="0"/><w:divBdr><w:top w:val="none" w:sz="0" w:space="0" w:color="auto"/><w:left w:val="none" w:sz="0" w:space="0" w:color="auto"/><w:bottom w:val="none" w:sz="0" w:space="0" w:color="auto"/><w:right w:val="none" w:sz="0" w:space="0" w:color="auto"/></w:divBdr></w:div>"#
    )
}

/// A package with `body` (Letter page, 1in left/right margins) and, if given, webSettings `divs`.
fn docx(body: &str, divs: Option<&str>) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/webSettings" Target="webSettings.xml"/></Relationships>"#;
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/webSettings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.webSettings+xml"/></Types>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let mut parts = vec![("[Content_Types].xml", types.to_string()), ("_rels/.rels", root.to_string()), ("word/document.xml", doc)];
    if let Some(d) = divs {
        parts.push(("word/_rels/document.xml.rels", doc_rels.to_string()));
        parts.push((
            "word/webSettings.xml",
            format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:webSettings xmlns:w="{W}"><w:divs>{d}</w:divs><w:optimizeForBrowser/></w:webSettings>"#),
        ));
    }
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, xml) in &parts {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(xml.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> Option<String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name(name).ok()?.read_to_string(&mut s).unwrap();
    Some(s)
}

fn para(div_id: Option<&str>, ppr: &str, text: &str) -> String {
    let d = div_id.map(|id| format!(r#"<w:divId w:val="{id}"/>"#)).unwrap_or_default();
    format!(r#"<w:p><w:pPr>{ppr}{d}</w:pPr><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/><w:sz w:val="20"/></w:rPr><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#)
}

/// Page-space (left, right) x of the first line of the body block at `index` (for a table,
/// its first cell paragraph), and how many lines that block's first paragraph has.
fn first_line(bytes: &[u8], index: u32) -> (f32, f32, usize) {
    let d = wordcraft_docx::read(bytes).unwrap();
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    for page in &l.pages {
        for it in &page.items {
            if let Placed::Lines { story: StoryRef::Body, path, para, l0, x, .. } = it {
                if path.0.first() == Some(&index) && *l0 == 0 {
                    let xs = &para.lines[0].xs;
                    return (x + xs[0], x + xs[xs.len() - 1], para.lines.len());
                }
            }
        }
    }
    panic!("block {index} not laid out");
}

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// marLeft 750 twips = 37.5pt, marRight 150 twips = 7.5pt; div 42 has zero margins.
fn divs() -> String {
    div("1748839239", 750, 150) + &div("42", 0, 0)
}

#[test]
fn a_paragraph_in_a_div_starts_at_the_divs_left_margin() {
    let body = para(None, "", "Plain") + &para(Some("1748839239"), "", "In the div");
    let bytes = docx(&body, Some(&divs()));
    let (plain, _, _) = first_line(&bytes, 0);
    let (in_div, _, _) = first_line(&bytes, 1);
    assert!(near(plain, 72.0, 0.5), "a paragraph outside any div starts at the margin: {plain}");
    assert!(near(in_div - plain, 37.5, 0.1), "marLeft 750 twips moves the text 37.5pt right: {plain} -> {in_div}");
}

#[test]
fn the_div_margin_adds_to_the_paragraphs_own_indents() {
    // Measured in Word: Title style with w:ind w:left="-605" inside the 750-twip div starts at
    // 72 + 37.5 - 30.25 = 79.25pt.
    let ind = r#"<w:ind w:left="-605"/>"#;
    let body = para(None, ind, "Outdented") + &para(Some("1748839239"), ind, "Outdented in the div");
    let bytes = docx(&body, Some(&divs()));
    let (out, _, _) = first_line(&bytes, 0);
    let (in_div, _, _) = first_line(&bytes, 1);
    assert!(near(out, 72.0 - 30.25, 0.5), "{out}");
    assert!(near(in_div, 72.0 + 37.5 - 30.25, 0.5), "{in_div}");
}

#[test]
fn the_divs_right_margin_pulls_in_the_right_edge() {
    let jc = r#"<w:jc w:val="right"/>"#;
    let body = para(None, jc, "Right") + &para(Some("1748839239"), jc, "Right");
    let bytes = docx(&body, Some(&divs()));
    let (_, plain_end, _) = first_line(&bytes, 0);
    let (_, div_end, _) = first_line(&bytes, 1);
    assert!(near(plain_end, 612.0 - 72.0, 0.5), "{plain_end}");
    assert!(near(plain_end - div_end, 7.5, 0.1), "marRight 150 twips: {plain_end} -> {div_end}");
}

#[test]
fn a_narrower_div_wraps_more_lines() {
    // ~45pt narrower text column: a paragraph that just fills five lines needs more in the div.
    let text = "word ".repeat(400);
    let body = para(None, "", &text) + &para(Some("1748839239"), "", &text);
    let bytes = docx(&body, Some(&divs()));
    let (_, _, plain) = first_line(&bytes, 0);
    let (_, _, in_div) = first_line(&bytes, 1);
    assert!(in_div > plain, "{plain} lines outside, {in_div} inside");
}

#[test]
fn a_table_row_in_a_div_moves_with_it() {
    let row = |d: &str| {
        format!(r#"<w:tr><w:trPr>{d}</w:trPr><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/></w:tcPr>{}</w:tc></w:tr>"#, para(None, "", "Cell"))
    };
    let tbl = |d: &str| format!(r#"<w:tbl><w:tblPr><w:tblW w:w="3000" w:type="dxa"/><w:tblInd w:w="0" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid>{}</w:tbl>"#, row(d));
    let body = tbl("") + "<w:p/>" + &tbl(r#"<w:divId w:val="1748839239"/>"#) + "<w:p/>";
    let bytes = docx(&body, Some(&divs()));
    let (plain, _, _) = first_line(&bytes, 0);
    let (in_div, _, _) = first_line(&bytes, 2);
    assert!(near(in_div - plain, 37.5, 0.1), "{plain} -> {in_div}");
}

#[test]
fn zero_margins_unknown_ids_and_missing_web_settings_change_nothing() {
    let body = para(None, "", "Plain") + &para(Some("42"), "", "Zero div") + &para(Some("999"), "", "No such div");
    for bytes in [docx(&body, Some(&divs())), docx(&body, None)] {
        let (plain, _, _) = first_line(&bytes, 0);
        assert!(near(first_line(&bytes, 1).0, plain, 0.01));
        assert!(near(first_line(&bytes, 2).0, plain, 0.01));
    }
}

#[test]
fn div_ids_and_web_settings_divs_survive_a_save() {
    let body = para(Some("1748839239"), "", "In the div");
    let d = wordcraft_docx::read(&docx(&body, Some(&divs()))).unwrap();
    let out = wordcraft_docx::write(&d).unwrap();
    let xml = part(&out, "word/document.xml").unwrap();
    assert!(xml.contains(r#"<w:divId w:val="1748839239"/>"#), "{xml}");
    let web = part(&out, "word/webSettings.xml").expect("webSettings.xml is written when the document has divs");
    assert!(web.contains(r#"w:id="1748839239""#) && web.contains(r#"<w:marLeft w:val="750"/>"#), "{web}");
    // And it lays out the same after reading it back.
    let (a, _, _) = first_line(&out, 0);
    assert!(near(a, 72.0 + 37.5, 0.5), "{a}");
}
