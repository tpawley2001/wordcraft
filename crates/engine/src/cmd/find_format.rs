//! Find and Replace by formatting (#4893): Find what's Format conditions, Replace with's Format,
//! and No Formatting. Conditions test the effective formatting (styles and direct formatting).

use serde_json::Value;
use wordcraft_doc::props::{CharProps, Highlight, TextColor, Underline};
use wordcraft_doc::{Paragraph, Path, Pos, StoryRef, StyleKind};

use crate::session::FindFormat;
use crate::{CmdError, Session};

/// `key` of the params: absent (`None`), `null` = No Formatting (`Some(None)`), or the
/// formatting, checked against the document (a known style, a valid colour).
pub(crate) fn read(s: &Session, v: &Value, key: &str) -> Result<Option<Option<FindFormat>>, CmdError> {
    let Some(x) = v.get(key) else { return Ok(None) };
    if x.is_null() {
        return Ok(Some(None));
    }
    if !x.is_object() {
        return Err(CmdError::Params(format!("`{key}` is an object of formatting or null (No Formatting)")));
    }
    let f: FindFormat = serde_json::from_value(x.clone()).map_err(|e| CmdError::Params(format!("`{key}`: {e}")))?;
    if let Some(sz) = f.size
        && !(1.0..=1638.0).contains(&sz)
    {
        return Err(CmdError::Params(format!("`{key}.size` must be between 1 and 1638")));
    }
    if let Some(n) = &f.font
        && (n.trim().is_empty() || n.len() > 128)
    {
        return Err(CmdError::Params(format!("bad font name in `{key}`")));
    }
    compile(s, &f)?;
    Ok(Some((!f.is_empty()).then_some(f)))
}

/// A [`FindFormat`] resolved against the document.
pub(crate) struct Cond {
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    font: Option<String>,
    size: Option<f32>,
    color: Option<TextColor>,
    /// `Some(None)`: any highlight colour (Word's Format › Highlight).
    highlight: Option<Option<Highlight>>,
    /// The style's id and whether it is a character style.
    style: Option<(String, bool)>,
}

pub(crate) fn compile(s: &Session, f: &FindFormat) -> Result<Cond, CmdError> {
    let color = f.color.as_deref().map(|c| TextColor::parse(c).ok_or_else(|| CmdError::Params(format!("bad colour `{c}`")))).transpose()?;
    let highlight = match f.highlight.as_deref() {
        Some(h) if h.eq_ignore_ascii_case("any") => Some(None),
        Some(h) => Some(Some(super::format::parse_highlight(h).ok_or_else(|| CmdError::Params(format!("unknown highlight colour `{h}`")))?)),
        None => None,
    };
    let style = match f.style.as_deref() {
        Some(n) => {
            let st = s.doc.styles.find(n).ok_or_else(|| CmdError::Params(format!("no style `{n}`")))?;
            if !matches!(st.kind, StyleKind::Paragraph | StyleKind::Character) {
                return Err(CmdError::Params(format!("`{n}` is not a paragraph or character style")));
            }
            Some((st.id.clone(), st.kind == StyleKind::Character))
        }
        None => None,
    };
    Ok(Cond {
        bold: f.bold,
        italic: f.italic,
        underline: f.underline,
        font: f.font.as_ref().map(|n| n.trim().to_string()),
        size: f.size.map(|sz| (sz * 2.0).round() / 2.0),
        color,
        highlight,
        style,
    })
}

impl Cond {
    /// Whether a run's characters (`c`, in paragraph `p`) have the formatting.
    fn run_matches(&self, s: &Session, p: &Paragraph, c: &CharProps) -> bool {
        let para_style = p.props.style.as_deref();
        if let Some((id, character)) = &self.style {
            let ok = if *character { c.style.as_deref() == Some(id.as_str()) } else { para_style.unwrap_or("Normal") == id };
            if !ok {
                return false;
            }
        }
        let r = s.doc.styles.resolve_char(para_style, c);
        self.bold.is_none_or(|b| r.bold == b)
            && self.italic.is_none_or(|b| r.italic == b)
            && self.underline.is_none_or(|b| (r.underline != Underline::None) == b)
            && self.font.as_deref().is_none_or(|f| r.font.eq_ignore_ascii_case(f))
            && self.size.is_none_or(|sz| (r.size - sz).abs() < 0.01)
            && self.color.is_none_or(|col| match (col, r.color) {
                (TextColor::Auto, TextColor::Auto) => true,
                (TextColor::Rgb(a), TextColor::Rgb(b)) => a == b,
                _ => false,
            })
            && self.highlight.is_none_or(|h| match h {
                Some(h) => r.highlight == h.rgb(),
                None => r.highlight.is_some(),
            })
    }

    /// Whether every live (not tracked-deleted) character of `a..b` has the formatting.
    pub(crate) fn range_matches(&self, s: &Session, a: &Pos, b: &Pos) -> bool {
        for path in s.doc.paths_between(a, b) {
            let Some(p) = s.doc.para(a.story, &path) else { continue };
            let from = if path == a.path { a.off } else { 0 };
            let to = if path == b.path { b.off } else { p.len() };
            for (r, c) in p.run_ranges() {
                if c.del.is_none() && r.end > from && r.start < to && r.start < r.end && !self.run_matches(s, p, c) {
                    return false;
                }
            }
        }
        true
    }

    /// Each longest stretch of live characters in one paragraph that all have the formatting:
    /// what Find with no text and a format finds.
    pub(crate) fn stretches(&self, s: &Session, story: StoryRef, cap: usize) -> Vec<(Pos, Pos)> {
        let mut out: Vec<(Pos, Pos)> = Vec::new();
        for path in s.doc.para_paths(story) {
            let Some(p) = s.doc.para(story, &path) else { continue };
            let mut open: Option<(usize, usize)> = None;
            let at = |off: usize| Pos { story, path: path.clone(), off };
            for (r, c) in p.run_ranges() {
                if r.start >= r.end {
                    continue;
                }
                // Deleted text isn't there: it neither matches nor breaks a stretch.
                if c.del.is_some() {
                    continue;
                }
                if self.run_matches(s, p, c) {
                    open = Some(match open {
                        Some((a, _)) => (a, r.end),
                        None => (r.start, r.end),
                    });
                } else if let Some((a, b)) = open.take() {
                    out.push((at(a), at(b)));
                }
            }
            if let Some((a, b)) = open {
                out.push((at(a), at(b)));
            }
            if out.len() >= cap {
                out.truncate(cap);
                break;
            }
        }
        out
    }
}

/// Give `a..b` Replace with's formatting: character formatting directly, a paragraph style to
/// the range's paragraphs.
pub(crate) fn apply(s: &mut Session, a: &Pos, b: &Pos, f: &FindFormat) -> Result<(), CmdError> {
    let c = compile(s, f)?;
    let para_style = match &c.style {
        Some((id, false)) => Some(id.clone()),
        _ => None,
    };
    let char_style = match &c.style {
        Some((id, true)) => Some(id.clone()),
        _ => None,
    };
    if a != b {
        s.doc.format_range(a, b, &|x: &mut CharProps| {
            if let Some(v) = c.bold {
                x.bold = Some(v);
                x.bold_cs = Some(v);
            }
            if let Some(v) = c.italic {
                x.italic = Some(v);
                x.italic_cs = Some(v);
            }
            if let Some(v) = c.underline {
                x.underline = Some(if v { Underline::Single } else { Underline::None });
            }
            if let Some(n) = &c.font {
                x.font = Some(n.clone());
                x.font_cs = Some(n.clone());
            }
            if let Some(sz) = c.size {
                x.size = Some(sz);
                x.size_cs = Some(sz);
            }
            if let Some(col) = c.color {
                x.color = Some(col);
            }
            if let Some(h) = c.highlight {
                // Any colour, as a format to give: the first one, yellow.
                x.highlight = Some(h.unwrap_or(Highlight::Yellow));
            }
            if let Some(id) = &char_style {
                x.style = Some(id.clone());
            }
        })?;
    }
    if let Some(id) = para_style {
        let paths: Vec<Path> = s.doc.para_paths_in(a, b);
        for path in paths {
            let p = s.doc.para_mut(a.story, &path)?;
            p.props.style = Some(id.clone());
            p.touch();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use wordcraft_doc::{Document, Pos, StoryRef};

    use crate::{Selection, Session};

    fn run(s: &mut Session, id: &str, v: Value) -> Value {
        s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
    }

    fn offs(v: &Value) -> Vec<(u64, u64)> {
        v["matches"].as_array().unwrap().iter().map(|m| (m["start"]["off"].as_u64().unwrap(), m["end"]["off"].as_u64().unwrap())).collect()
    }

    /// "one two three" with "two" red and highlighted yellow.
    fn doc() -> Session {
        let mut s = Session::new(Document::new());
        run(&mut s, "text.insert", json!({"text": "one two three"}));
        s.sel = Selection { anchor: Pos::body(0, 4), focus: Pos::body(0, 7) };
        run(&mut s, "format.color", json!({"color": "FF0000"}));
        run(&mut s, "format.highlight", json!({"color": "yellow"}));
        s.sel = Selection::caret(Pos::body(0, 0));
        s
    }

    #[test]
    fn colour_and_highlight_conditions() {
        let mut s = doc();
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"color": "ff0000"}}));
        assert_eq!(offs(&r), vec![(4, 7)]);
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"highlight": "yellow"}}));
        assert_eq!(offs(&r), vec![(4, 7)]);
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"highlight": "any"}}));
        assert_eq!(offs(&r), vec![(4, 7)]);
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"highlight": "none"}}));
        assert_eq!(offs(&r), vec![(0, 4), (7, 13)]);
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"color": "auto"}}));
        assert_eq!(offs(&r), vec![(0, 4), (7, 13)]);
    }

    #[test]
    fn style_condition_matches_only_that_styles_paragraphs() {
        let mut s = Session::new(Document::new());
        run(&mut s, "text.insert", json!({"text": "beta body"}));
        run(&mut s, "text.newParagraph", json!({}));
        run(&mut s, "text.insert", json!({"text": "beta heading"}));
        run(&mut s, "styles.apply", json!({"style": "Heading 1"}));
        let r = run(&mut s, "edit.advancedFind", json!({"text": "beta", "format": {"style": "Heading 1"}}));
        assert_eq!(r["count"], 1);
        assert_eq!(r["matches"][0]["start"]["path"], json!([1]));
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"style": "Normal"}}));
        assert_eq!(offs(&r), vec![(0, 9)]);
    }

    #[test]
    fn bold_from_a_character_style_counts_as_bold() {
        let mut s = Session::new(Document::new());
        run(&mut s, "text.insert", json!({"text": "plain strong"}));
        s.sel = Selection { anchor: Pos::body(0, 6), focus: Pos::body(0, 12) };
        run(&mut s, "styles.apply", json!({"style": "Strong"}));
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"bold": true}}));
        assert_eq!(offs(&r), vec![(6, 12)]);
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"style": "Strong"}}));
        assert_eq!(offs(&r), vec![(6, 12)]);
    }

    #[test]
    fn replace_format_applies_colour_highlight_and_paragraph_style() {
        let mut s = doc();
        let r = run(
            &mut s,
            "edit.replaceAll",
            json!({"text": "three", "with": "", "format": null, "replaceFormat": {"highlight": "brightGreen", "style": "Heading 2"}}),
        );
        assert_eq!(r["replaced"], 1);
        assert_eq!(s.doc.plain_text(StoryRef::Body), "one two three");
        let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"highlight": "brightGreen"}, "replaceFormat": null}));
        assert_eq!(offs(&r), vec![(8, 13)]);
        assert_eq!(s.doc.para_at(&Pos::body(0, 0)).and_then(|p| p.props.style.clone()).as_deref(), Some("Heading2"));
    }

    #[test]
    fn bad_formatting_is_an_error_and_changes_nothing() {
        let mut s = doc();
        run(&mut s, "edit.advancedFind", json!({"text": "two", "format": {"bold": false}}));
        for bad in [
            json!("bold"),
            json!({"size": -3}),
            json!({"size": 1e30}),
            json!({"color": "zz"}),
            json!({"highlight": "plaid"}),
            json!({"style": "No Such"}),
            json!({"bold": 3}),
        ] {
            assert!(s.run("edit.advancedFind", &json!({"text": "x", "format": bad})).is_err(), "{bad}");
        }
        assert_eq!(s.find.query, "two");
        assert_eq!(s.find.format.as_ref().and_then(|f| f.bold), Some(false));
    }

    #[test]
    fn find_with_only_a_format_and_no_match_is_empty() {
        let mut s = doc();
        let r = run(&mut s, "edit.advancedFind", json!({"format": {"bold": true}}));
        assert_eq!(r["count"], 0);
        assert!(s.run("edit.findNext", &json!({})).is_err());
    }

    #[test]
    fn tracked_replace_all_with_a_format_keeps_the_new_text_formatted() {
        let mut s = doc();
        run(&mut s, "review.trackChanges", json!({"value": true}));
        let r = run(&mut s, "edit.replaceAll", json!({"text": "one", "with": "ONE", "replaceFormat": {"italic": true}}));
        assert_eq!(r["replaced"], 1);
        let text = s.doc.para_at(&Pos::body(0, 0)).map(|p| p.text.clone()).unwrap_or_default();
        let at = text.find("ONE").unwrap();
        s.sel = Selection { anchor: Pos::body(0, at), focus: Pos::body(0, at + 3) };
        assert_eq!(run(&mut s, "format.state", json!({}))["italic"], json!(true));
        run(&mut s, "edit.undo", json!({}));
        assert_eq!(s.doc.plain_text(StoryRef::Body), "one two three");
    }
}
