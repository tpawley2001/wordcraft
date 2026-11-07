//! Caption labels kept in the document and AutoCaption (References › Insert Caption › New Label,
//! AutoCaption…), saved as `w:captions` (ECMA-376 Part 1 §17.15.1.18).

use serde_json::json;
use wordcraft_doc::{AutoCaption, BUILTIN_CAPTION_LABELS, CAPTION_NUM_FORMATS, CaptionLabel, CaptionSep, MAX_CAPTION_ENTRIES, Path};

use super::fields::caption;
use super::{CmdError, CmdResult, Session, Value, p};

/// The object type AutoCaption names a table by.
pub const AUTO_TABLE: &str = "Microsoft Word Table";
/// The object type AutoCaption names an equation by.
pub const AUTO_EQUATION: &str = "Microsoft Equation";
/// The kinds of object WordCraft inserts that AutoCaption can caption.
pub const AUTO_OBJECTS: [&str; 2] = [AUTO_TABLE, AUTO_EQUATION];

/// Longest label or object name accepted.
const MAX_NAME: usize = 40;

/// A label name as captions can carry it: no quotes, backslashes or control characters (it goes
/// into a SEQ field), trimmed, at most [`MAX_NAME`] characters.
fn clean_name(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '"' | '\\') && !c.is_control()).take(MAX_NAME).collect::<String>().trim().to_string()
}

fn builtin(name: &str) -> Option<&'static str> {
    BUILTIN_CAPTION_LABELS.iter().find(|b| b.eq_ignore_ascii_case(name)).copied()
}

/// Every label: Figure, Table and Equation, then the document's own in document order.
pub fn all_labels(s: &Session) -> Vec<String> {
    let mut out: Vec<String> = BUILTIN_CAPTION_LABELS.iter().map(|l| l.to_string()).collect();
    for l in &s.doc.settings.captions.labels {
        if builtin(&l.name).is_none() {
            out.push(l.name.clone());
        }
    }
    out
}

/// The label's name as the document spells it, when it exists.
fn existing(s: &Session, name: &str) -> Option<String> {
    builtin(name).map(str::to_string).or_else(|| s.doc.settings.captions.label(name).map(|l| l.name.clone()))
}

/// The label's options: its `w:caption` entry, else the defaults.
pub fn label_options(s: &Session, name: &str) -> CaptionLabel {
    s.doc.settings.captions.label(name).cloned().unwrap_or_else(|| CaptionLabel::new(name))
}

/// `references.captionLabel`: list, add or delete the document's caption labels.
pub(super) fn caption_label(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(name) = p::str(v, "add") {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(CmdError::Params("a label needs a name".into()));
        }
        if existing(s, &name).is_none() {
            let labels = &mut s.doc.settings.captions.labels;
            if labels.len() >= MAX_CAPTION_ENTRIES {
                return Err(CmdError::Params(format!("a document keeps at most {MAX_CAPTION_ENTRIES} labels")));
            }
            labels.push(CaptionLabel::new(&name));
        }
    } else if let Some(name) = p::str(v, "delete") {
        let name = name.trim();
        if builtin(name).is_some() {
            return Err(CmdError::Params(format!("{name} is built in and can't be deleted")));
        }
        let c = &mut s.doc.settings.captions;
        let Some(i) = c.labels.iter().position(|l| l.name.eq_ignore_ascii_case(name)) else {
            return Err(CmdError::Params(format!("no label {name:?}")));
        };
        let gone = c.labels.remove(i).name;
        c.auto.retain(|a| !a.label.eq_ignore_ascii_case(&gone));
    }
    Ok(json!({"labels": all_labels(s)}))
}

/// `references.autoCaption`: list the objects AutoCaption knows and their labels, or turn it on
/// or off for one.
pub(super) fn auto_caption(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(set) = v.get("set") {
        let object = p::req_str(set, "object")?.trim();
        if object.is_empty() || object.chars().count() > 255 || object.chars().any(char::is_control) {
            return Err(CmdError::Params("`object` must be an object type name".into()));
        }
        let object = object.to_string();
        let above = match p::str(set, "position") {
            None => None,
            Some("above") => Some(true),
            Some("below") => Some(false),
            Some(x) => return Err(CmdError::Params(format!("`position` is above or below, not {x:?}"))),
        };
        match set.get("label") {
            Some(Value::Null) => s.doc.settings.captions.auto.retain(|a| a.object != object),
            Some(Value::String(label)) => {
                let label = existing(s, label.trim()).ok_or_else(|| CmdError::Params(format!("no caption label {label:?}; add it first")))?;
                let auto = &mut s.doc.settings.captions.auto;
                if let Some(a) = auto.iter_mut().find(|a| a.object == object) {
                    a.label = label.clone();
                } else if auto.len() >= MAX_CAPTION_ENTRIES {
                    return Err(CmdError::Params(format!("a document keeps at most {MAX_CAPTION_ENTRIES} AutoCaption entries")));
                } else {
                    auto.push(AutoCaption { object, label: label.clone() });
                }
                let num_fmt = match p::str(set, "format") {
                    None => None,
                    Some(f) => Some(num_fmt(f).ok_or_else(|| CmdError::Params(format!("unknown `format` {f:?}")))?),
                };
                let chapter = match set.get("chapter") {
                    None | Some(Value::Null) => None,
                    Some(c) => Some(
                        c.as_u64().filter(|c| *c <= 9).ok_or_else(|| CmdError::Params("`chapter` is a heading level 1-9, or 0 for none".into()))?
                            as u8,
                    ),
                };
                let separator = match p::str(set, "separator") {
                    None => None,
                    Some(x) => Some(
                        CaptionSep::ALL
                            .into_iter()
                            .find(|c| c.text() == x || c.ooxml() == x)
                            .ok_or_else(|| CmdError::Params(format!("unknown `separator` {x:?}")))?,
                    ),
                };
                set_options(s, &label, LabelChange { above, num_fmt, chapter, separator })?;
            }
            _ => return Err(CmdError::Params("`set.label` is a label name or null".into())),
        }
    }
    let c = &s.doc.settings.captions;
    let mut names: Vec<String> = AUTO_OBJECTS.iter().map(|o| o.to_string()).collect();
    for a in &c.auto {
        if !names.contains(&a.object) {
            names.push(a.object.clone());
        }
    }
    let objects: Vec<Value> = names
        .iter()
        .map(|n| {
            let label = c.auto_label(n);
            let o = label_options(s, label.unwrap_or(""));
            let mut e = json!({"name": n, "label": label, "position": if label.is_some() && o.above { "above" } else { "below" }});
            if label.is_some()
                && let Some(m) = e.as_object_mut()
            {
                m.insert("format".into(), json!(o.num_fmt));
                m.insert("chapter".into(), json!(if o.chapter { o.heading.clamp(1, 9) } else { 0 }));
                m.insert("separator".into(), json!(o.separator.text()));
            }
            e
        })
        .collect();
    Ok(json!({"objects": objects, "labels": all_labels(s)}))
}

/// What `references.autoCaption` changes on a label.
struct LabelChange {
    above: Option<bool>,
    num_fmt: Option<&'static str>,
    /// Heading level chapters start at; 0 turns the chapter number off.
    chapter: Option<u8>,
    separator: Option<CaptionSep>,
}

/// A caption number format by its OOXML name or the `\*` switch's (`ALPHABETIC`).
fn num_fmt(f: &str) -> Option<&'static str> {
    let f = match f {
        "ARABIC" => "decimal",
        "alphabetic" => "lowerLetter",
        "ALPHABETIC" => "upperLetter",
        "roman" => "lowerRoman",
        "ROMAN" => "upperRoman",
        f => f,
    };
    CAPTION_NUM_FORMATS.into_iter().find(|x| *x == f)
}

/// Store options on the label's `w:caption` entry; a built-in label back at its defaults needs
/// none.
fn set_options(s: &mut Session, label: &str, c: LabelChange) -> Result<(), CmdError> {
    let labels = &mut s.doc.settings.captions.labels;
    let i = match labels.iter().position(|l| l.name.eq_ignore_ascii_case(label)) {
        Some(i) => i,
        None if labels.len() >= MAX_CAPTION_ENTRIES => {
            return Err(CmdError::Params(format!("a document keeps at most {MAX_CAPTION_ENTRIES} labels")));
        }
        None => {
            labels.push(CaptionLabel::new(label));
            labels.len().saturating_sub(1)
        }
    };
    if let Some(l) = labels.get_mut(i) {
        if let Some(above) = c.above {
            l.above = above;
        }
        if let Some(f) = c.num_fmt {
            l.num_fmt = f.to_string();
        }
        if let Some(h) = c.chapter {
            l.chapter = h > 0;
            l.heading = h;
        }
        if let Some(sep) = c.separator {
            l.separator = sep;
        }
    }
    labels.retain(|l| builtin(&l.name).is_none() || !l.is_default());
    Ok(())
}

/// AutoCaption: when objects of type `object` get a caption, add it next to the top-level block
/// at the caret, with the label's numbering, chapter number, separator and position, as
/// `references.caption` would. Returns `Some(above)` when a caption was added; the caret is then
/// on the caption, for the caller to put back.
pub(crate) fn add_auto_caption(s: &mut Session, object: &str) -> Result<Option<bool>, CmdError> {
    let Some(label) = s.doc.settings.captions.auto_label(object).map(str::to_string) else { return Ok(None) };
    let o = label_options(s, &label);
    let mut params = json!({
        "label": label,
        "text": "",
        "position": if o.above { "above" } else { "below" },
        "format": o.num_fmt,
        "excludeLabel": o.no_label,
        "separator": o.separator.text(),
    });
    if o.chapter
        && let Some(m) = params.as_object_mut()
    {
        m.insert("chapter".into(), json!(o.heading.clamp(1, 9)));
    }
    caption(s, &params)?;
    Ok(Some(o.above))
}

/// `path` after a caption was inserted above the top-level block it is in.
pub(crate) fn below_caption(path: &Path) -> Path {
    let mut p = path.clone();
    if let Some(first) = p.0.first_mut() {
        *first = first.saturating_add(1);
    }
    p
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_doc::Document;

    use crate::Session;

    /// A display equation gets its AutoCaption above it and stays the one being edited; an
    /// inline equation (in text) gets none.
    #[test]
    fn display_equation_gets_its_caption() {
        let mut s = Session::new(Document::from_text(""));
        s.run("references.autoCaption", &json!({"set": {"object": super::AUTO_EQUATION, "label": "Equation", "position": "above"}})).unwrap();
        s.run("insert.equation", &json!({"linear": "a+b"})).unwrap();
        let texts: Vec<String> = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect();
        assert_eq!(texts.first().map(String::as_str), Some("Equation 1"), "{texts:?}");
        let at = s.math.as_ref().map(|m| m.at.path.clone()).unwrap();
        assert_eq!(at.0, vec![1], "editing the equation below its caption");
        s.run("equation.exit", &json!({})).unwrap();

        let mut s = Session::new(Document::from_text("Some text "));
        s.run("references.autoCaption", &json!({"set": {"object": super::AUTO_EQUATION, "label": "Equation"}})).unwrap();
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("insert.equation", &json!({"linear": "x"})).unwrap();
        assert_eq!(s.doc.body.len(), 1, "inline: no caption");
    }
}
