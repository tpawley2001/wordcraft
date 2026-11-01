//! HTML divs (`w:divs` in the web settings part, ECMA-376 §17.15.2.8) and the `w:divId` that
//! names them on paragraphs (§17.3.1.10) and table rows (§17.4.9). Hand-written fixtures.

use std::io::{Read, Write};

use wordcraft_doc::{Block, Document, Paragraph, WebDiv, div_margins};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn package(body: &str, web: &str) -> Vec<u8> {
    let parts = [
        (
            "_rels/.rels",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="word/document.xml"/></Relationships>"#
            ),
        ),
        (
            "word/_rels/document.xml.rels",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/webSettings" Target="webSettings.xml"/></Relationships>"#
            ),
        ),
        ("word/document.xml", format!(r#"<w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#)),
        ("word/webSettings.xml", format!(r#"<w:webSettings xmlns:w="{W}">{web}</w:webSettings>"#)),
    ];
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, xml) in &parts {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(xml.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

const NESTED: &str = r#"<w:divs><w:div w:id="1"><w:bodyDiv w:val="1"/><w:marLeft w:val="200"/><w:marRight w:val="100"/><w:marTop w:val="0"/><w:marBottom w:val="0"/><w:divBdr><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:divBdr><w:divsChild><w:div w:id="2"><w:marLeft w:val="400"/><w:marRight w:val="0"/><w:marTop w:val="20"/><w:marBottom w:val="40"/></w:div></w:divsChild></w:div></w:divs>"#;

#[test]
fn nested_divs_are_read_with_their_margins() {
    let d = wordcraft_docx::read(&package(r#"<w:p><w:pPr><w:divId w:val="2"/></w:pPr></w:p>"#, NESTED)).unwrap();
    let divs = &d.settings.web_divs;
    assert_eq!(divs.len(), 1);
    let outer = &divs[0];
    assert_eq!((outer.id.as_str(), outer.body_div, outer.mar_left, outer.mar_right), ("1", true, 10.0, 5.0));
    assert!(outer.border.and_then(|b| b.top).is_some(), "divBdr is kept");
    let inner = &outer.children[0];
    assert_eq!((inner.id.as_str(), inner.body_div, inner.mar_left, inner.mar_top, inner.mar_bottom), ("2", false, 20.0, 1.0, 2.0));
    // A nested div adds its ancestors' margins.
    assert_eq!(div_margins(divs, "2"), Some((30.0, 5.0)));
    assert_eq!(div_margins(divs, "1"), Some((10.0, 5.0)));
    assert_eq!(div_margins(divs, "3"), None);
    let p = d.body.iter().find_map(|b| b.as_para()).unwrap();
    assert_eq!(p.props.div_id.as_deref(), Some("2"));
}

#[test]
fn div_ids_are_written_in_schema_order_with_a_web_settings_part() {
    let mut d = Document::new();
    d.settings.web_divs = vec![WebDiv {
        id: "7".into(),
        mar_left: 37.5,
        children: vec![WebDiv { id: "8".into(), mar_right: 7.5, ..Default::default() }],
        ..Default::default()
    }];
    let mut p = Paragraph::with_text("x", Default::default());
    p.props.outline_level = Some(1);
    p.props.div_id = Some("8".into());
    d.body = vec![std::sync::Arc::new(Block::Para(p))];
    let out = wordcraft_docx::write(&d).unwrap();
    let xml = part(&out, "word/document.xml");
    let (lvl, div) = (xml.find("<w:outlineLvl").unwrap(), xml.find(r#"<w:divId w:val="8"/>"#).unwrap());
    assert!(lvl < div, "divId follows outlineLvl: {xml}");
    let web = part(&out, "word/webSettings.xml");
    assert!(web.contains(r#"<w:div w:id="7"><w:marLeft w:val="750"/>"#) && web.contains(r#"<w:divsChild><w:div w:id="8">"#), "{web}");
    assert!(part(&out, "word/_rels/document.xml.rels").contains("/webSettings\""));
    assert!(part(&out, "[Content_Types].xml").contains("webSettings+xml"));
    let back = wordcraft_docx::read(&out).unwrap();
    assert_eq!(back.settings.web_divs, d.settings.web_divs);
}

#[test]
fn hostile_div_nesting_and_margins_are_bounded() {
    let mut web = String::from("<w:divs>");
    for i in 0..60 {
        web.push_str(&format!(r#"<w:div w:id="{i}"><w:marLeft w:val="99999999999"/><w:divsChild>"#));
    }
    for _ in 0..60 {
        web.push_str("</w:divsChild></w:div>");
    }
    web.push_str("</w:divs>");
    let d = wordcraft_docx::read(&package(r#"<w:p><w:pPr><w:divId w:val="59"/></w:pPr></w:p>"#, &web)).unwrap();
    let (l, _) = div_margins(&d.settings.web_divs, "5").unwrap();
    assert!(l.is_finite());
    assert_eq!(div_margins(&d.settings.web_divs, "59"), None, "nesting is cut off");
}
