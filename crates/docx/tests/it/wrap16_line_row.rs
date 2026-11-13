//! Line spacing (ECMA-376 Part 1 §17.3.1.33 `w:spacing/@w:line`) and row heights (§17.4.81
//! `w:trHeight`) written as plain twips above 32767: Word keeps them in one signed 16-bit number
//! (negative = exactly, positive = at least, as [MS-DOC] `sprmPDyaLine` / `sprmTDyaRowHeight`), so
//! the low 16 bits of `-N` (exact rule) or `N` (any other rule) are kept and their sign decides
//! the rule. Measured in Word: `w:line="65535" w:lineRule="exact"` is a natural single line and
//! `w:trHeight w:val="66256" w:hRule="exact"` is a 36 pt row. Fixtures are hand-written.

use std::io::Write;

use wordcraft_doc::Document;
use wordcraft_doc::props::{HeightRule, LineSpacing, ParaProps, RowProps};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// A package whose body is `body` and whose styles part holds `styles`.
fn docx(body: &str, styles: &str) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#);
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>{styles}</w:styles>"#
    );
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, part) in
        [("_rels/.rels", root), ("word/document.xml", doc.as_str()), ("word/_rels/document.xml.rels", rels), ("word/styles.xml", styles.as_str())]
    {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(part.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn read(body: &str, styles: &str) -> Document {
    wordcraft_docx::read(&docx(body, styles)).unwrap()
}

/// The paragraph properties of a paragraph with `<w:spacing {attrs}/>`.
fn para(attrs: &str) -> ParaProps {
    let d = read(&format!(r#"<w:p><w:pPr><w:spacing {attrs}/></w:pPr><w:r><w:t>a</w:t></w:r></w:p>"#), "");
    d.body.iter().find_map(|b| b.as_para()).unwrap().props.clone()
}

/// The row properties of a one-cell table whose row has `<w:trHeight {attrs}/>`.
fn row(attrs: &str) -> RowProps {
    let d = read(
        &format!(
            r#"<w:tbl><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:trPr><w:trHeight {attrs}/></w:trPr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#
        ),
        "",
    );
    d.body.iter().find_map(|b| b.as_table()).unwrap().rows[0].props.clone()
}

fn assert_line(got: Option<LineSpacing>, want: LineSpacing) {
    let close = |a: f32, b: f32| (a - b).abs() < 0.001;
    let ok = match (got, want) {
        (Some(LineSpacing::Exactly(a)), LineSpacing::Exactly(b)) => close(a, b),
        (Some(LineSpacing::AtLeast(a)), LineSpacing::AtLeast(b)) => close(a, b),
        (Some(LineSpacing::Multiple(a)), LineSpacing::Multiple(b)) => close(a, b),
        _ => false,
    };
    assert!(ok, "got {got:?}, want {want:?}");
}

fn line(value: &str, rule: &str) -> Option<LineSpacing> {
    para(&format!(r#"w:line="{value}" w:lineRule="{rule}""#)).line_spacing
}

fn assert_row(attrs: &str, height: f32, rule: HeightRule) {
    let r = row(attrs);
    let h = r.height.unwrap_or(f32::NAN);
    assert!((h - height).abs() < 0.001 && r.height_rule == rule, "{attrs}: got {:?} {:?}, want {height} {rule:?}", r.height, r.height_rule);
}

#[test]
fn exact_lines_beyond_16_bits_wrap() {
    assert_line(line("65535", "exact"), LineSpacing::AtLeast(0.05));
    assert_line(line("65536", "exact"), LineSpacing::AtLeast(0.0));
    assert_line(line("65776", "exact"), LineSpacing::Exactly(12.0));
    assert_line(line("131552", "exact"), LineSpacing::Exactly(24.0));
    assert_line(line("32768", "exact"), LineSpacing::Exactly(1638.4));
    assert_line(line("40000", "exact"), LineSpacing::AtLeast(1276.8));
}

#[test]
fn at_least_lines_beyond_16_bits_wrap() {
    assert_line(line("65535", "atLeast"), LineSpacing::Exactly(0.05));
    assert_line(line("66016", "atLeast"), LineSpacing::AtLeast(24.0));
    assert_line(line("32768", "atLeast"), LineSpacing::Exactly(1638.4));
    assert_line(line("40000", "atLeast"), LineSpacing::Exactly(1276.8));
    // The rule is matched case-insensitively.
    assert_line(line("65535", "atleast"), LineSpacing::Exactly(0.05));
}

#[test]
fn auto_lines_beyond_16_bits_wrap() {
    assert_line(line("65776", "auto"), LineSpacing::Multiple(1.0));
    assert_line(line("65535", "auto"), LineSpacing::Exactly(0.05));
    // An absent rule is auto.
    assert_line(para(r#"w:line="65776""#).line_spacing, LineSpacing::Multiple(1.0));
}

#[test]
fn lines_in_range_or_with_a_unit_are_unchanged() {
    assert_line(line("480", "exact"), LineSpacing::Exactly(24.0));
    assert_line(line("480", "atLeast"), LineSpacing::AtLeast(24.0));
    assert_line(line("240", "auto"), LineSpacing::Multiple(1.0));
    assert_line(line("13.90pt", "exact"), LineSpacing::Exactly(13.9));
    assert_line(line("-240", "exact"), LineSpacing::Exactly(12.0));
}

#[test]
fn a_style_line_beyond_16_bits_wraps_the_same() {
    let d = read(
        r#"<w:p><w:pPr><w:pStyle w:val="Tight"/></w:pPr><w:r><w:t>a</w:t></w:r></w:p>"#,
        r#"<w:style w:type="paragraph" w:styleId="Tight"><w:name w:val="Tight"/><w:pPr><w:spacing w:line="65776" w:lineRule="exact"/></w:pPr></w:style>"#,
    );
    let st = d.styles.get("Tight").expect("the style is read");
    assert_line(st.para.line_spacing, LineSpacing::Exactly(12.0));
}

#[test]
fn row_heights_beyond_16_bits_wrap() {
    assert_row(r#"w:val="65537" w:hRule="exact""#, 0.05, HeightRule::Exact);
    assert_row(r#"w:val="66256" w:hRule="exact""#, 36.0, HeightRule::Exact);
    assert_row(r#"w:val="131792" w:hRule="exact""#, 36.0, HeightRule::Exact);
    assert_row(r#"w:val="65535" w:hRule="exact""#, 0.05, HeightRule::AtLeast);
    assert_row(r#"w:val="65537" w:hRule="atLeast""#, 0.05, HeightRule::AtLeast);
    assert_row(r#"w:val="66256" w:hRule="atLeast""#, 36.0, HeightRule::AtLeast);
    assert_row(r#"w:val="32768" w:hRule="exact""#, 1638.4, HeightRule::Exact);
    assert_row(r#"w:val="32768" w:hRule="atLeast""#, 1638.4, HeightRule::Exact);
    assert_row(r#"w:val="40000" w:hRule="exact""#, 1276.8, HeightRule::AtLeast);
    assert_row(r#"w:val="40000" w:hRule="atLeast""#, 1276.8, HeightRule::Exact);
    assert_row(r#"w:val="65536" w:hRule="exact""#, 0.0, HeightRule::AtLeast);
    // An absent rule is at least.
    assert_row(r#"w:val="65537""#, 0.05, HeightRule::AtLeast);
}

#[test]
fn row_heights_in_range_are_unchanged() {
    assert_row(r#"w:val="720" w:hRule="exact""#, 36.0, HeightRule::Exact);
    assert_row(r#"w:val="720" w:hRule="atLeast""#, 36.0, HeightRule::AtLeast);
    assert_row(r#"w:val="720""#, 36.0, HeightRule::AtLeast);
}
