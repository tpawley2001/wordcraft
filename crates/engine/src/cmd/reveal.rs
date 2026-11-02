//! Reveal Formatting: the font, paragraph and section formatting at the caret, resolved through
//! the styles and grouped Font, Paragraph and Section as Word's Reveal Formatting pane shows it.

use serde_json::{Value, json};
use wordcraft_doc::props::{Align, LineSpacing, TextColor, Underline, VertAlign};

use super::inspector::{char_props, para_props, style_name};
use crate::{CommandSpec, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec::new("format.reveal", "Reveal Formatting", "Home › Styles › Reveal Formatting", |s, _| Ok(reveal(s))).pure()]
}

/// A number as Word prints it: at most two decimals, no trailing zeros (`14`, `10.5`).
fn num(v: f32) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let t = format!("{:.2}", v);
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".into() } else { t.to_string() }
}

/// Points as inches (`72` -> `1"`).
fn inches(pt: f32) -> String {
    format!("{}\"", num(pt / 72.0))
}

fn section(heading: &str, items: Vec<String>) -> Value {
    json!({"heading": heading, "items": items})
}

/// What Reveal Formatting shows for the caret, as JSON.
pub fn reveal(s: &Session) -> Value {
    let styles = &s.doc.styles;
    let pp = para_props(s);
    let pstyle = pp.style.clone().unwrap_or_else(|| "Normal".into());
    let rp = styles.resolve_para(&pp);
    let rc = styles.resolve_char(Some(&pstyle), &char_props(s));

    let mut font = vec![rc.font.clone(), format!("{} pt", num(rc.size))];
    for (on, name) in [
        (rc.bold, "Bold"),
        (rc.italic, "Italic"),
        (rc.underline != Underline::None, "Underline"),
        (rc.strike, "Strikethrough"),
        (rc.double_strike, "Double strikethrough"),
        (rc.vert_align == VertAlign::Superscript, "Superscript"),
        (rc.vert_align == VertAlign::Subscript, "Subscript"),
        (rc.small_caps, "Small caps"),
        (rc.caps, "All caps"),
        (rc.hidden, "Hidden"),
    ] {
        if on {
            font.push(name.into());
        }
    }
    if let TextColor::Rgb(c) = rc.color {
        font.push(format!("Font color: {}", c.hex().to_uppercase()));
    }
    let mut font_sections = vec![section("FONT", font)];
    if let Some(lang) = rc.lang.as_deref().filter(|l| !l.is_empty()) {
        font_sections.push(section("LANGUAGE", vec![lang.to_string()]));
    }

    let align = match rp.align {
        Align::Left => "Left",
        Align::Center => "Centered",
        Align::Right => "Right",
        Align::Justify => "Justified",
        Align::Distribute => "Distributed",
    };
    let mut indent = vec![align.to_string()];
    if rp.indent_left != 0.0 {
        indent.push(format!("Left: {}", inches(rp.indent_left)));
    }
    if rp.indent_right != 0.0 {
        indent.push(format!("Right: {}", inches(rp.indent_right)));
    }
    if rp.indent_first > 0.0 {
        indent.push(format!("First line: {}", inches(rp.indent_first)));
    } else if rp.indent_first < 0.0 {
        indent.push(format!("Hanging: {}", inches(-rp.indent_first)));
    }
    let line = match rp.line_spacing {
        LineSpacing::Multiple(m) if (m - 1.0).abs() < 0.001 => "single".to_string(),
        LineSpacing::Multiple(m) if (m - 1.5).abs() < 0.001 => "1.5 lines".to_string(),
        LineSpacing::Multiple(m) if (m - 2.0).abs() < 0.001 => "double".to_string(),
        LineSpacing::Multiple(m) => format!("Multiple {} li", num(m)),
        LineSpacing::AtLeast(pt) => format!("At least {} pt", num(pt)),
        LineSpacing::Exactly(pt) => format!("Exactly {} pt", num(pt)),
    };
    let spacing = vec![format!("Before: {} pt", num(rp.space_before)), format!("After: {} pt", num(rp.space_after)), format!("Line spacing: {line}")];

    let sect = super::page::sect(s);
    let margins = vec![
        format!("Left: {}", inches(sect.margin_left)),
        format!("Right: {}", inches(sect.margin_right)),
        format!("Top: {}", inches(sect.margin_top)),
        format!("Bottom: {}", inches(sect.margin_bottom)),
    ];
    let paper = vec![format!("Width: {}", inches(sect.page_w)), format!("Height: {}", inches(sect.page_h))];

    json!({"groups": [
        {"name": "Font", "sections": font_sections},
        {"name": "Paragraph", "sections": [
            section("PARAGRAPH STYLE", vec![style_name(s, &pstyle)]),
            section("ALIGNMENT AND INDENTATION", indent),
            section("SPACING", spacing),
        ]},
        {"name": "Section", "sections": [section("MARGINS", margins), section("PAPER", paper)]},
    ]})
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use wordcraft_doc::Document;

    use crate::Session;

    fn font_items(s: &mut Session) -> Vec<String> {
        let r = s.run("format.reveal", &json!({})).unwrap();
        r["groups"][0]["sections"][0]["items"].as_array().unwrap().iter().map(|i| i.as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn font_lists_name_size_effects_and_colour() {
        let mut s = Session::new(Document::from_text("Body text"));
        let run = |s: &mut Session, id: &str, v: Value| s.run(id, &v).unwrap();
        run(&mut s, "select.all", json!({}));
        run(&mut s, "format.font", json!({"name": "Arial"}));
        run(&mut s, "format.size", json!({"size": 14}));
        run(&mut s, "format.bold", json!({"value": true}));
        assert_eq!(font_items(&mut s), ["Arial", "14 pt", "Bold"]);
        run(&mut s, "format.size", json!({"size": 10.5}));
        run(&mut s, "format.italic", json!({"value": true}));
        run(&mut s, "format.color", json!({"color": "1b1611"}));
        assert_eq!(font_items(&mut s), ["Arial", "10.5 pt", "Bold", "Italic", "Font color: 1B1611"]);
    }

    #[test]
    fn numbers_and_inches() {
        assert_eq!(super::num(14.0), "14");
        assert_eq!(super::num(10.5), "10.5");
        assert_eq!(super::num(f32::NAN), "0");
        assert_eq!(super::inches(90.0), "1.25\"");
        assert_eq!(super::inches(612.0), "8.5\"");
    }
}
