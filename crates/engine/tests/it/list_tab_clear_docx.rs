//! A numbered paragraph whose tab list clears its list level's number tab, read from DOCX and
//! saved again (synthetic probes; see `crates/layout/src/tests/list_tab_clear.rs`): a
//! `w:tab w:val="clear"` at the level's `num` tab position in the paragraph style or the
//! paragraph sends the text to the next stop, as in Word, and a saved file keeps the clear.

use std::io::Write;

use wordcraft_doc::Document;
use wordcraft_layout::{LayoutCache, LayoutOptions, Placed, layout, para::ClKind};

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

fn zip(entries: &[(&str, String)]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// One probe: compatibility mode `compat`, the ParaNum style's `w:tabs` content, the level's
/// `w:ind`, and the paragraph's own `w:tabs` content.
fn probe(compat: u32, style_tabs: &str, level_ind: &str, direct_tabs: &str) -> Vec<u8> {
    let tabs = |t: &str| if t.is_empty() { String::new() } else { format!("<w:tabs>{t}</w:tabs>") };
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings {W}><w:defaultTabStop w:val="720"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="{compat}"/></w:compat></w:settings>"#
    );
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles {W}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman"/><w:sz w:val="22"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="ParaNum"><w:name w:val="ParaNum"/><w:basedOn w:val="Normal"/><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr>{}</w:pPr></w:style></w:styles>"#,
        tabs(style_tabs)
    );
    let numbering = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering {W}><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:pStyle w:val="ParaNum"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/><w:pPr><w:tabs><w:tab w:val="num" w:pos="1080"/></w:tabs>{level_ind}</w:pPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    );
    let ppr = if direct_tabs.is_empty() {
        r#"<w:pStyle w:val="ParaNum"/>"#.to_string()
    } else {
        format!(r#"<w:pStyle w:val="ParaNum"/>{}"#, tabs(direct_tabs))
    };
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W}><w:body><w:p><w:pPr>{ppr}</w:pPr><w:r><w:t>Section</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    zip(&[
        ("[Content_Types].xml", content_types.to_string()),
        ("_rels/.rels", rels.to_string()),
        ("word/_rels/document.xml.rels", doc_rels.to_string()),
        ("word/settings.xml", settings),
        ("word/styles.xml", styles),
        ("word/numbering.xml", numbering),
        ("word/document.xml", document),
    ])
}

/// Where the numbered paragraph's text starts on the page.
fn text_x(doc: &Document) -> f32 {
    let l = layout(doc, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    let (pl, x) = page
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { para, x, .. } if para.label.is_some() => Some((para, *x)),
            _ => None,
        })
        .expect("the numbered paragraph");
    let line = pl.lines.first().expect("a line");
    (line.c0..line.c1)
        .filter(|&k| pl.clusters.get(k).is_some_and(|c| matches!(c.kind, ClKind::Text)))
        .filter_map(|k| line.cl_left(k))
        .fold(f32::MAX, f32::min)
        + x
}

const FIRST_LINE: &str = r#"<w:ind w:left="0" w:firstLine="720"/>"#;
const HANGING: &str = r#"<w:ind w:left="1080" w:hanging="360"/>"#;
const STYLE_A: &str = r#"<w:tab w:val="left" w:pos="720"/><w:tab w:val="clear" w:pos="1080"/><w:tab w:val="left" w:pos="1440"/>"#;
const CLEAR: &str = r#"<w:tab w:val="clear" w:pos="1080"/>"#;

#[test]
fn word_probes_place_the_text_as_word_does() {
    let probes: [(&str, Vec<u8>, f32); 6] = [
        ("a", probe(11, STYLE_A, FIRST_LINE, ""), 144.0),
        ("b", probe(11, r#"<w:tab w:val="left" w:pos="720"/><w:tab w:val="left" w:pos="1440"/>"#, FIRST_LINE, ""), 126.0),
        ("c", probe(11, "", FIRST_LINE, CLEAR), 144.0),
        ("d", probe(15, STYLE_A, FIRST_LINE, ""), 144.0),
        ("e", probe(11, CLEAR, FIRST_LINE, ""), 144.0),
        ("f", probe(11, CLEAR, HANGING, ""), 126.0),
    ];
    for (name, bytes, word) in probes {
        let doc = wordcraft_docx::read(&bytes).unwrap();
        let x = text_x(&doc);
        assert!((x - word).abs() <= 0.1, "probe {name}: text at {x}, Word {word}");
    }
}

#[test]
fn a_saved_file_keeps_the_clear() {
    for (name, bytes) in [("a", probe(11, STYLE_A, FIRST_LINE, "")), ("c", probe(11, "", FIRST_LINE, CLEAR))] {
        let doc = wordcraft_docx::read(&bytes).unwrap();
        let again = wordcraft_docx::read(&wordcraft_docx::write(&doc).unwrap()).unwrap();
        let x = text_x(&again);
        assert!((x - 144.0).abs() <= 0.1, "probe {name} after save and reopen: text at {x}, Word 144");
    }
}
