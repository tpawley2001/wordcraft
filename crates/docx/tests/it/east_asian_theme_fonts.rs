//! The East Asian font a `w:eastAsiaTheme` slot names (ECMA-376 Part 1 §17.3.2.26, §17.18.96):
//! the major / minor theme font for the script of `w:themeFontLang/@w:eastAsia` (§17.15.1.88;
//! Japanese when the settings name none), else the theme's `a:ea`, else none. Measured in Word:
//! the theme slot supersedes a `w:eastAsia` font beside it.

use std::io::Write;

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// The usual theme fonts: minor `Hans` 宋体, `Hant` 新細明體, `Jpan` ＭＳ 明朝, `Hang` 맑은 고딕;
/// major `Jpan` ＭＳ ゴシック, `Hans` `major_hans`.
fn theme(minor_ea: &str, major_hans: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="T"><a:themeElements><a:fontScheme name="F"><a:majorFont><a:latin typeface="Verdana"/><a:ea typeface=""/><a:cs typeface=""/><a:font script="Jpan" typeface="ＭＳ ゴシック"/><a:font script="Hans" typeface="{major_hans}"/></a:majorFont><a:minorFont><a:latin typeface="Georgia"/><a:ea typeface="{minor_ea}"/><a:cs typeface=""/><a:font script="Jpan" typeface="ＭＳ 明朝"/><a:font script="Hang" typeface="맑은 고딕"/><a:font script="Hans" typeface="宋体"/><a:font script="Hant" typeface="新細明體"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#
    )
}

/// A package whose `docDefaults` fonts are `rfonts` (attributes of `w:rFonts`), with the settings'
/// `w:themeFontLang` attributes `font_lang` (no element when `None`) and the theme part `theme`.
fn package(rfonts: &str, font_lang: Option<&str>, theme: Option<&str>) -> Vec<u8> {
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles {W_NS}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts {rfonts}/><w:sz w:val="24"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style></w:styles>"#
    );
    let font_lang = font_lang.map(|l| format!("<w:themeFontLang {l}/>")).unwrap_or_default();
    let settings = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings {W_NS}>{font_lang}</w:settings>"#);
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {W_NS}><w:body><w:p><w:r><w:t>L001 中文</w:t></w:r></w:p></w:body></w:document>"#
    );
    let mut rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/styles" Target="styles.xml"/><Relationship Id="rId2" Type="{REL}/settings" Target="settings.xml"/>"#
    );
    if theme.is_some() {
        rels.push_str(&format!(r#"<Relationship Id="rId3" Type="{REL}/theme" Target="theme/theme1.xml"/>"#));
    }
    rels.push_str("</Relationships>");
    let root = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let mut parts = vec![
        ("_rels/.rels", root),
        ("word/document.xml", doc),
        ("word/_rels/document.xml.rels", rels),
        ("word/styles.xml", styles),
        ("word/settings.xml", settings),
    ];
    if let Some(t) = theme {
        parts.push(("word/theme/theme1.xml", t.to_string()));
    }
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, xml) in &parts {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(xml.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// The East Asian font of the first run, after style resolution.
fn ea_font(bytes: &[u8]) -> Option<String> {
    let d = wordcraft_docx::read(bytes).unwrap();
    let p = d.body[0].as_para().unwrap();
    d.styles.resolve_char(p.props.style.as_deref(), &p.runs[0].props).font_ea
}

const TNR: &str = r#"w:ascii="Times New Roman" w:hAnsi="Times New Roman""#;

fn themed(slot: &str) -> String {
    format!(r#"{TNR} w:eastAsiaTheme="{slot}""#)
}

#[test]
fn the_theme_font_follows_the_east_asian_theme_language() {
    let plain = theme("", "宋体");
    let cases: &[(String, Option<&str>, String, &str)] = &[
        (themed("minorEastAsia"), Some(r#"w:val="en-US" w:eastAsia="zh-CN""#), plain.clone(), "宋体"),
        (themed("minorEastAsia"), Some(r#"w:val="en-US" w:eastAsia="ja-JP""#), plain.clone(), "ＭＳ 明朝"),
        // No East Asian language: Japanese.
        (themed("minorEastAsia"), Some(r#"w:val="en-US""#), plain.clone(), "ＭＳ 明朝"),
        (themed("minorEastAsia"), None, plain.clone(), "ＭＳ 明朝"),
        // The per-script font wins over a non-empty `a:ea`.
        (themed("minorEastAsia"), Some(r#"w:eastAsia="zh-CN""#), theme("SimHei", "宋体"), "宋体"),
        // The major slot takes the major font's entry.
        (themed("majorEastAsia"), Some(r#"w:eastAsia="zh-CN""#), theme("", "SimHei"), "SimHei"),
        // The theme slot wins over the font named beside it.
        (format!(r#"{} w:eastAsia="SimHei""#, themed("minorEastAsia")), Some(r#"w:eastAsia="zh-CN""#), plain.clone(), "宋体"),
        (themed("minorEastAsia"), Some(r#"w:eastAsia="zh-TW""#), plain.clone(), "新細明體"),
        (themed("minorEastAsia"), Some(r#"w:eastAsia="ko-KR""#), plain.clone(), "맑은 고딕"),
    ];
    for (i, (rfonts, lang, theme, want)) in cases.iter().enumerate() {
        assert_eq!(ea_font(&package(rfonts, *lang, Some(theme))).as_deref(), Some(*want), "case {i}");
    }
}

#[test]
fn the_theme_ea_font_stands_in_for_a_missing_script() {
    let theme = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="T"><a:themeElements><a:fontScheme name="F"><a:majorFont><a:latin typeface="Verdana"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Georgia"/><a:ea typeface="SimHei"/><a:cs typeface=""/><a:font script="Jpan" typeface="ＭＳ 明朝"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#;
    let b = package(&themed("minorEastAsia"), Some(r#"w:eastAsia="zh-CN""#), Some(theme));
    assert_eq!(ea_font(&b).as_deref(), Some("SimHei"));
}

#[test]
fn without_a_theme_font_the_east_asian_font_is_unset_or_named() {
    let zh = Some(r#"w:eastAsia="zh-CN""#);
    // No theme part.
    assert_eq!(ea_font(&package(&themed("minorEastAsia"), zh, None)), None);
    // A theme without per-script fonts and an empty `a:ea`.
    let bare = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="T"><a:themeElements><a:fontScheme name="F"><a:majorFont><a:latin typeface="Verdana"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Georgia"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#;
    assert_eq!(ea_font(&package(&themed("minorEastAsia"), zh, Some(bare))), None);
    // A named font without a theme slot.
    let named = format!(r#"{TNR} w:eastAsia="SimSun""#);
    assert_eq!(ea_font(&package(&named, zh, Some(&theme("", "宋体")))).as_deref(), Some("SimSun"));
    // A non-East Asian slot names that slot's Latin font.
    assert_eq!(ea_font(&package(&themed("minorHAnsi"), zh, Some(&theme("", "宋体")))).as_deref(), Some("Georgia"));
}

#[test]
fn the_theme_east_asian_font_survives_a_save() {
    let b = package(&themed("minorEastAsia"), Some(r#"w:eastAsia="zh-CN""#), Some(&theme("", "宋体")));
    let d = wordcraft_docx::read(&b).unwrap();
    let out = wordcraft_docx::write(&d).unwrap();
    assert_eq!(ea_font(&out).as_deref(), Some("宋体"));
}
