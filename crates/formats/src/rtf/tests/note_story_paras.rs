//! Paragraphs inside footnotes, endnotes and comments keep on save what the same paragraph keeps
//! in the body: tab stops, widow control, keep lines together, shading, character spacing and
//! hidden text. Word writes and reads these words inside `{\footnote …}` and `{\*\annotation …}`
//! as it does in the body (RTF 1.9.1).

use super::*;
use std::fmt::Debug;
use wordcraft_doc::props::{TabAlign, TabLeader};
use wordcraft_doc::{Document, Paragraph, StoryRef};

const HEAD: &str = r"{\fonttbl{\f0 Times New Roman;}}{\colortbl;\red255\green0\blue0;\red0\green0\blue255;}\widowctrl";

/// The three stories a paragraph can sit in besides the body, each holding `inner`.
fn stories(inner: &str) -> [(&'static str, String); 3] {
    [
        ("footnote", format!(r"\pard\plain\f0\fs24 body{{\footnote\pard\plain\f0\fs20 {inner}}} text\par")),
        ("endnote", format!(r"\pard\plain\f0\fs24 body{{\footnote\ftnalt\pard\plain\f0\fs20 {inner}}} text\par")),
        (
            "comment",
            format!(
                r"\pard\plain\f0\fs24 body {{\*\atrfstart 1}}commented{{\*\atrfend 1}}{{\*\atnid A}}{{\*\atnauthor A}}\chatn{{\*\annotation{{\*\atnref 1}}\pard\plain\f0\fs20 {inner}}} text\par"
            ),
        ),
    ]
}

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{HEAD}{body}}}").as_bytes()).unwrap()
}

/// The paragraph holding `needle` in any story but the body.
fn story_para(d: &Document, needle: &str) -> Option<Paragraph> {
    for id in d.parts.keys() {
        let s = StoryRef::Part(*id);
        for path in d.para_paths(s) {
            if let Some(p) = d.para(s, &path)
                && p.text.contains(needle)
            {
                return Some(p.clone());
            }
        }
    }
    None
}

/// Opens `inner` (it must hold the text STORY) in a footnote, an endnote and a comment, saves and
/// opens again: `get` must give something other than `unset` on the first open (the reader's
/// part) and the same value after the save (the writer's part).
fn kept<T: PartialEq + Debug>(inner: &str, get: impl Fn(&Paragraph) -> T, unset: &T) {
    let mut lost = Vec::new();
    for (kind, body) in stories(inner) {
        let d = open(&body);
        let first = story_para(&d, "STORY").map(|p| get(&p));
        assert!(first.as_ref().is_some_and(|v| v != unset), "{kind}: the reader gives {first:?}");
        let back = import(export(&d).as_bytes()).unwrap();
        let again = story_para(&back, "STORY").map(|p| get(&p));
        if again != first {
            lost.push(format!("{kind}: {first:?} became {again:?}"));
        }
    }
    assert!(lost.is_empty(), "lost on save: {lost:#?}");
}

fn tabs(p: &Paragraph) -> Vec<(f32, TabAlign, TabLeader)> {
    p.props.tabs.iter().flatten().map(|t| (t.pos, t.align, t.leader)).collect()
}

#[test]
fn tab_stops_are_kept() {
    kept(r"\tqr\tldot\tx5000 STORY\tab 12", tabs, &Vec::new());
}

#[test]
fn the_tab_stop_is_a_right_tab_at_250_points_with_dots() {
    for (kind, body) in stories(r"\tqr\tldot\tx5000 STORY\tab 12") {
        let back = import(export(&open(&body)).as_bytes()).unwrap();
        let p = story_para(&back, "STORY");
        assert_eq!(p.as_ref().map(tabs), Some(vec![(250.0, TabAlign::Right, TabLeader::Dot)]), "{kind}");
    }
}

#[test]
fn the_saved_file_holds_the_tab_stop_once() {
    for (kind, body) in stories(r"\tqr\tldot\tx5000 STORY\tab 12") {
        let out = export(&open(&body));
        assert_eq!(out.matches(r"\tx5000").count(), 1, "{kind}: {out}");
    }
}

#[test]
fn widow_control_off_is_kept() {
    kept(r"\nowidctlpar STORY", |p| p.props.widow_control, &None);
}

#[test]
fn keep_lines_together_is_kept() {
    kept(r"\keep STORY", |p| p.props.keep_lines, &None);
}

#[test]
fn paragraph_shading_is_kept() {
    kept(r"\cbpat2 STORY", |p| p.props.shading, &None);
}

#[test]
fn character_spacing_is_kept() {
    kept(r"\expnd4\expndtw20 STORY", |p| p.runs.iter().filter_map(|r| r.props.spacing).collect::<Vec<_>>(), &Vec::new());
}

#[test]
fn hidden_text_stays_and_stays_hidden() {
    let hidden = |p: &Paragraph| (p.text.contains("secret"), p.runs.iter().any(|r| r.props.hidden == Some(true)));
    kept(r"STORY {\v secret} end", hidden, &(false, false));
}

#[test]
fn the_same_paragraphs_in_the_body_are_kept() {
    let d = open(r"\pard\plain\f0\fs24\tqr\tldot\tx5000 BODY\tab 12\par\pard\plain\f0\fs24\nowidctlpar\keep\cbpat2 second\par");
    let back = import(export(&d).as_bytes()).unwrap();
    let (a, b) = (d.body[1].as_para().unwrap(), back.body[1].as_para().unwrap());
    assert_eq!(tabs(back.body[0].as_para().unwrap()), vec![(250.0, TabAlign::Right, TabLeader::Dot)]);
    assert_eq!((b.props.widow_control, b.props.keep_lines, b.props.shading), (Some(false), Some(true), a.props.shading));
    assert!(a.props.shading.is_some());
}

#[test]
fn a_plain_note_and_comment_still_round_trip() {
    for (kind, body) in stories("STORY plain") {
        let back = import(export(&open(&body)).as_bytes()).unwrap();
        let p = story_para(&back, "STORY plain");
        assert!(p.is_some(), "{kind}");
        assert_eq!(p.map(|p| (tabs(&p), p.props.widow_control, p.props.keep_lines, p.props.shading)), Some((Vec::new(), None, None, None)), "{kind}");
    }
}
