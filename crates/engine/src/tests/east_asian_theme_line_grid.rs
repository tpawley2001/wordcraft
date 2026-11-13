//! Chinese text whose East Asian font comes from the theme (`w:eastAsiaTheme="minorEastAsia"`,
//! the theme's minor `Hans` font 宋体, `w:themeFontLang w:eastAsia="zh-CN"`) takes that font's line,
//! as in Word: 12 pt single-spaced lines 15.56 pt apart, one 15.6 pt line each on a line grid
//! (`w:docGrid w:type="lines" w:linePitch="312"`), not two. Synthetic fixture.

use std::io::Write;

use wordcraft_doc::StoryRef;

use crate::Session;

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn docx(grid: &str) -> Vec<u8> {
    let para = r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:t>中文</w:t></w:r></w:p>"#;
    let doc = format!(
        r#"<w:document {W_NS}><w:body>{}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>{grid}</w:sectPr></w:body></w:document>"#,
        para.repeat(6)
    );
    let styles = format!(
        r#"<w:styles {W_NS}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Times New Roman" w:hAnsi="Times New Roman" w:eastAsiaTheme="minorEastAsia"/><w:sz w:val="24"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style></w:styles>"#
    );
    let settings = format!(r#"<w:settings {W_NS}><w:themeFontLang w:val="en-US" w:eastAsia="zh-CN"/></w:settings>"#);
    let theme = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="T"><a:themeElements><a:fontScheme name="F"><a:majorFont><a:latin typeface="Times New Roman"/><a:ea typeface=""/><a:cs typeface=""/><a:font script="Hans" typeface="宋体"/></a:majorFont><a:minorFont><a:latin typeface="Times New Roman"/><a:ea typeface=""/><a:cs typeface=""/><a:font script="Jpan" typeface="ＭＳ 明朝"/><a:font script="Hans" typeface="宋体"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#;
    let files = [
        (
            "_rels/.rels",
            format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="word/document.xml"/></Relationships>"#),
        ),
        (
            "word/_rels/document.xml.rels",
            format!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/styles" Target="styles.xml"/><Relationship Id="rId2" Type="{REL}/settings" Target="settings.xml"/><Relationship Id="rId3" Type="{REL}/theme" Target="theme/theme1.xml"/></Relationships>"#
            ),
        ),
        ("word/document.xml", doc),
        ("word/styles.xml", styles),
        ("word/settings.xml", settings),
        ("word/theme/theme1.xml", theme.to_string()),
    ];
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// The distances between consecutive body line tops on page 1, or `None` when SimSun is installed
/// (the layout then keeps the installed font's own line).
fn pitches(grid: &str) -> Option<Vec<f32>> {
    if !wordcraft_fonts::word::resolve("SimSun", false, false).substituted {
        return None;
    }
    let d = wordcraft_docx::read(&docx(grid)).expect("read");
    assert_eq!(d.styles.default_chr.font_ea.as_deref(), Some("宋体"));
    let mut s = Session::new(d);
    let ys: Vec<f32> = s.layout().pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            wordcraft_layout::Placed::Lines { story: StoryRef::Body, y, .. } => Some(*y),
            _ => None,
        })
        .collect();
    assert_eq!(ys.len(), 6, "{ys:?}");
    Some(ys.windows(2).map(|w| w[1] - w[0]).collect())
}

#[test]
fn theme_east_asian_text_takes_one_grid_line() {
    let Some(p) = pitches(r#"<w:docGrid w:type="lines" w:linePitch="312"/>"#) else { return };
    assert!(p.iter().all(|d| (d - 15.6).abs() <= 0.05), "{p:?}");
}

#[test]
fn theme_east_asian_text_takes_simsuns_line() {
    let Some(p) = pitches("") else { return };
    assert!(p.iter().all(|d| (d - 15.56).abs() <= 0.05), "{p:?}");
}
