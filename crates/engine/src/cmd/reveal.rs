//! Reveal Formatting: the font, paragraph and section formatting at the caret, resolved through
//! the styles and grouped Font, Paragraph and Section as Word's Reveal Formatting pane shows it.

use serde_json::{Value, json};
use wordcraft_doc::props::{Align, LineSpacing, TextColor, Underline, VertAlign};

use super::inspector::{char_props, para_props, style_name};
use crate::{CommandSpec, Selection, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("format.reveal", "Reveal Formatting", "Home › Styles › Reveal Formatting", |s, _| Ok(reveal(s))).pure(),
        CommandSpec::new("view.revealFormattingPane", "Reveal Formatting", "Home › Styles", |s, v| {
            s.view.reveal_formatting_pane = p::bool(v, "value").unwrap_or(!s.view.reveal_formatting_pane);
            let mut out = reveal(s);
            if let Value::Object(m) = &mut out {
                m.insert("pane".into(), json!(s.view.reveal_formatting_pane));
            }
            Ok(out)
        })
        .params(r#"{"value"?: bool (show/hide the pane)}"#)
        .key("Shift+F1")
        .pure(),
        CommandSpec::new("view.revealCompare", "Compare to another selection", "Home › Styles › Reveal Formatting", |s, v| {
            let on = p::bool(v, "value").unwrap_or(s.view.reveal_compare.is_none());
            s.view.reveal_compare = if on { Some(s.sel.clone()) } else { None };
            Ok(reveal(s))
        })
        .params(r#"{"value"?: bool (compare with the selection where it was turned on; absent toggles)}"#)
        .pure(),
    ]
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

/// English names of the common language tags; other tags are shown as they are.
const LANGUAGES: [(&str, &str); 16] = [
    ("en-US", "English (United States)"),
    ("en-GB", "English (United Kingdom)"),
    ("en-AU", "English (Australia)"),
    ("en-CA", "English (Canada)"),
    ("fr-FR", "French (France)"),
    ("de-DE", "German (Germany)"),
    ("es-ES", "Spanish (Spain)"),
    ("es-MX", "Spanish (Mexico)"),
    ("it-IT", "Italian (Italy)"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("pl-PL", "Polish"),
    ("nl-NL", "Dutch (Netherlands)"),
    ("ru-RU", "Russian"),
    ("ja-JP", "Japanese"),
    ("zh-CN", "Chinese (PRC)"),
    ("ko-KR", "Korean"),
];

fn language_name(tag: &str) -> String {
    LANGUAGES.iter().find(|(t, _)| t.eq_ignore_ascii_case(tag)).map(|(_, n)| (*n).to_string()).unwrap_or_else(|| tag.to_string())
}

/// The section holding the start of `sel` (the caret's section for a body selection).
fn caret_section(s: &Session, sel: &Selection) -> wordcraft_doc::section::SectionProps {
    let (a, _) = sel.ordered();
    if a.story != wordcraft_doc::StoryRef::Body {
        return super::page::sect(s);
    }
    let i = s.doc.section_index_of(a.path.0.first().copied().unwrap_or(0) as usize);
    s.doc.sections().get(i).map(|(_, x)| (*x).clone()).unwrap_or_default()
}

/// A section's lines: `items` as English text, and `entries` the same lines as data (a label and a
/// kind-tagged value: `pt`, `inches`, `text`, `font`, `lang`, `style`, `hex`, `multiple`) for a
/// front end to word in its own language.
#[derive(Default)]
struct Lines {
    items: Vec<String>,
    entries: Vec<Value>,
}

impl Lines {
    fn add(&mut self, item: String, entry: Value) {
        self.items.push(item);
        self.entries.push(entry);
    }

    /// `Label: value"` from points.
    fn inches(&mut self, label: &str, pt: f32) {
        self.add(format!("{label}: {}", inches(pt)), json!({"label": label, "inches": num(pt / 72.0).parse::<f64>().unwrap_or(0.0)}));
    }

    /// An English word on its own (`Bold`, `Centered`).
    fn text(&mut self, text: &str) {
        self.add(text.to_string(), json!({"label": null, "text": text}));
    }
}

fn section(heading: &str, lines: Lines) -> Value {
    json!({"heading": heading, "items": lines.items, "entries": lines.entries})
}

/// Points rounded as shown, as a JSON number.
fn pt_value(v: f32) -> f64 {
    num(v).parse::<f64>().unwrap_or(0.0)
}

/// What Reveal Formatting shows for the caret, as JSON.
pub fn reveal(s: &Session) -> Value {
    let mut out = reveal_at(s, &s.sel);
    let first = s.view.reveal_compare.as_ref().map(|sel| reveal_at(s, &clamped(s, sel)));
    let diffs = first.as_ref().map(|first| differences(first, &out));
    if let Value::Object(m) = &mut out {
        m.insert("compare".into(), json!(first.is_some()));
        if let Some(diffs) = diffs {
            m.insert("differences".into(), diffs);
        }
    }
    out
}

/// `sel` made valid for the document as it is now (its text may have been deleted since).
fn clamped(s: &Session, sel: &Selection) -> Selection {
    Selection { anchor: s.doc.clamp(&sel.anchor), focus: s.doc.clamp(&sel.focus) }
}

/// What Reveal Formatting shows for `sel`, as JSON.
pub fn reveal_at(s: &Session, sel: &Selection) -> Value {
    let styles = &s.doc.styles;
    let pp = para_props(s, sel);
    let pstyle = pp.style.clone().unwrap_or_else(|| "Normal".into());
    let rp = styles.resolve_para(&pp);
    let rc = styles.resolve_char(Some(&pstyle), &char_props(s, sel));

    let mut font = Lines::default();
    font.add(rc.font.clone(), json!({"label": null, "font": rc.font}));
    font.add(format!("{} pt", num(rc.size)), json!({"label": null, "pt": pt_value(rc.size)}));
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
            font.text(name);
        }
    }
    if let TextColor::Rgb(c) = rc.color {
        let hex = c.hex().to_uppercase();
        font.add(format!("Font color: {hex}"), json!({"label": "Font color", "hex": hex}));
    }
    let mut font_sections = vec![section("FONT", font)];
    if let Some(lang) = rc.lang.as_deref().filter(|l| !l.is_empty()) {
        let mut language = Lines::default();
        language.add(language_name(lang), json!({"label": null, "lang": lang}));
        font_sections.push(section("LANGUAGE", language));
    }

    let align = match rp.align {
        Align::Left => "Left",
        Align::Center => "Centered",
        Align::Right => "Right",
        Align::Justify => "Justified",
        Align::Distribute => "Distributed",
    };
    let mut indent = Lines::default();
    indent.text(align);
    if rp.indent_left != 0.0 {
        indent.inches("Left", rp.indent_left);
    }
    if rp.indent_right != 0.0 {
        indent.inches("Right", rp.indent_right);
    }
    if rp.indent_first > 0.0 {
        indent.inches("First line", rp.indent_first);
    } else if rp.indent_first < 0.0 {
        indent.inches("Hanging", -rp.indent_first);
    }
    let (line, line_value) = match rp.line_spacing {
        LineSpacing::Multiple(m) if (m - 1.0).abs() < 0.001 => ("single".to_string(), json!({"text": "single"})),
        LineSpacing::Multiple(m) if (m - 1.5).abs() < 0.001 => ("1.5 lines".to_string(), json!({"text": "1.5 lines"})),
        LineSpacing::Multiple(m) if (m - 2.0).abs() < 0.001 => ("double".to_string(), json!({"text": "double"})),
        LineSpacing::Multiple(m) => (format!("Multiple {} li", num(m)), json!({"multiple": pt_value(m)})),
        LineSpacing::AtLeast(pt) => (format!("At least {} pt", num(pt)), json!({"atLeast": pt_value(pt)})),
        LineSpacing::Exactly(pt) => (format!("Exactly {} pt", num(pt)), json!({"exactly": pt_value(pt)})),
    };
    let mut spacing = Lines::default();
    spacing.add(format!("Before: {} pt", num(rp.space_before)), json!({"label": "Before", "pt": pt_value(rp.space_before)}));
    spacing.add(format!("After: {} pt", num(rp.space_after)), json!({"label": "After", "pt": pt_value(rp.space_after)}));
    let mut line_entry = line_value;
    if let Value::Object(m) = &mut line_entry {
        m.insert("label".into(), json!("Line spacing"));
    }
    spacing.add(format!("Line spacing: {line}"), line_entry);

    let sect = caret_section(s, sel);
    let mut margins = Lines::default();
    margins.inches("Left", sect.margin_left);
    margins.inches("Right", sect.margin_right);
    margins.inches("Top", sect.margin_top);
    margins.inches("Bottom", sect.margin_bottom);
    let mut paper = Lines::default();
    paper.inches("Width", sect.page_w);
    paper.inches("Height", sect.page_h);
    let style = style_name(s, &pstyle);
    let mut style_lines = Lines::default();
    style_lines.add(style.clone(), json!({"label": null, "style": style}));

    json!({"groups": [
        {"name": "Font", "sections": font_sections},
        {"name": "Paragraph", "sections": [
            section("PARAGRAPH STYLE", style_lines),
            section("ALIGNMENT AND INDENTATION", indent),
            section("SPACING", spacing),
        ]},
        {"name": "Section", "sections": [section("MARGINS", margins), section("PAPER", paper)]},
    ]})
}

/// The key two entries in a section are matched by: the label when it has one, else the name of
/// its value kind. A label-less `text` entry in the FONT section is an effect (`Bold`), keyed by
/// its text so an effect on one side only is a difference.
fn entry_key(entry: &Value, font_section: bool) -> String {
    if let Some(label) = entry.get("label").and_then(Value::as_str) {
        return label.to_string();
    }
    let Some(map) = entry.as_object() else { return String::new() };
    for kind in ["font", "pt", "style", "lang", "hex", "multiple", "atLeast", "exactly", "text", "inches"] {
        if let Some(v) = map.get(kind) {
            if kind == "text" && font_section {
                return v.as_str().unwrap_or_default().to_string();
            }
            return kind.to_string();
        }
    }
    String::new()
}

/// The entries of a section by key, in order.
fn keyed(section: &Value, font_section: bool) -> Vec<(String, Value)> {
    section
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(|e| (entry_key(e, font_section), e.clone())).collect())
        .unwrap_or_default()
}

/// The Formatting differences between two listings: each section's entries that differ, the first
/// selection's entries in order then keys only the second side has.
fn differences(first: &Value, second: &Value) -> Value {
    let mut out = Vec::new();
    let groups = |v: &Value| v.get("groups").and_then(Value::as_array).cloned().unwrap_or_default();
    for group in &groups(first) {
        let name = group.get("name").and_then(Value::as_str).unwrap_or_default();
        let other = groups(second).into_iter().find(|g| g.get("name").and_then(Value::as_str) == Some(name));
        let sections = |g: Option<&Value>| g.and_then(|g| g.get("sections")).and_then(Value::as_array).cloned().unwrap_or_default();
        for section in sections(Some(group)) {
            let heading = section.get("heading").and_then(Value::as_str).unwrap_or_default();
            let font_section = name == "Font" && heading == "FONT";
            let other_section = sections(other.as_ref()).into_iter().find(|x| x.get("heading").and_then(Value::as_str) == Some(heading));
            let a = keyed(&section, font_section);
            let b = keyed(other_section.as_ref().unwrap_or(&Value::Null), font_section);
            for (key, entry) in &a {
                let same = b.iter().find(|(k, _)| k == key).map(|(_, e)| e);
                if same != Some(entry) {
                    out.push(json!({"group": name, "heading": heading, "first": entry, "second": same}));
                }
            }
            for (key, entry) in &b {
                if !a.iter().any(|(k, _)| k == key) {
                    out.push(json!({"group": name, "heading": heading, "first": null, "second": entry}));
                }
            }
        }
    }
    Value::Array(out)
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
    fn entries_parallel_items() {
        let mut s = Session::new(Document::from_text("Body text"));
        s.run("format.color", &json!({"color": "1b1611"})).unwrap();
        let r = s.run("format.reveal", &json!({})).unwrap();
        for group in r["groups"].as_array().unwrap() {
            for section in group["sections"].as_array().unwrap() {
                assert_eq!(section["items"].as_array().unwrap().len(), section["entries"].as_array().unwrap().len());
            }
        }
        let spacing = &r["groups"][1]["sections"][2]["entries"];
        assert_eq!(spacing[2]["label"], "Line spacing");
        let margins = &r["groups"][2]["sections"][0]["entries"];
        assert_eq!(margins[0], json!({"label": "Left", "inches": 1.0}));
    }

    #[test]
    fn entries_match_by_label_then_value_kind() {
        let key = |e: Value, font: bool| super::entry_key(&e, font);
        assert_eq!(key(json!({"label": "After", "pt": 12.0}), false), "After");
        assert_eq!(key(json!({"label": null, "font": "Arial"}), false), "font");
        assert_eq!(key(json!({"label": null, "pt": 14.0}), false), "pt");
        assert_eq!(key(json!({"label": null, "text": "Left"}), false), "text");
        assert_eq!(key(json!({"label": null, "text": "Bold"}), true), "Bold");
        assert_eq!(key(json!({"label": null, "text": "Bold"}), false), "text");
        assert_eq!(key(json!({"label": null, "inches": 1.0}), false), "inches");
        assert_eq!(key(json!({"label": null, "atLeast": 12.0}), false), "atLeast");
        assert_eq!(key(json!({"label": null, "exactly": 12.0}), false), "exactly");
        assert_eq!(key(json!({"label": null, "multiple": 1.5}), false), "multiple");
        assert_eq!(key(json!({"label": null, "style": "Normal"}), false), "style");
        assert_eq!(key(json!({"label": null, "lang": "en-US"}), false), "lang");
        assert_eq!(key(json!({"label": "Font color", "hex": "FF0000"}), false), "Font color");
    }

    #[test]
    fn language_by_name() {
        assert_eq!(super::language_name("en-us"), "English (United States)");
        assert_eq!(super::language_name("xx-YY"), "xx-YY");
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
