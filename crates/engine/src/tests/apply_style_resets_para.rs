//! Applying a paragraph style leaves the paragraph as the style defines it: direct indents,
//! alignment, spacing, line spacing, keeps, tabs, borders, shading, outline level and a directly
//! applied bullet or number all go. Measured in Microsoft 365 Word 16.0 (Selection.Style and the
//! Ctrl+Alt+1 / Ctrl+Shift+N / Ctrl+Shift+L keys, caret in the paragraph), 2026-11-14: Heading 1,
//! Normal, List Bullet and Body Text Indent 3 behaved the same.

use serde_json::json;
use wordcraft_doc::props::{Border, Borders, LineSpacing, TabAlign, TabLeader, TabStop};
use wordcraft_doc::{Align, Document, ParaProps, Pos, Rgb};

use crate::Session;

/// One paragraph "Alpha beta gamma delta" with `direct` as its direct paragraph formatting (on
/// top of Normal), the caret at offset 3.
fn para_with(direct: impl Fn(&mut ParaProps)) -> Session {
    let mut s = Session::new(Document::new());
    s.run("text.insert", &json!({"text": "Alpha beta gamma delta"})).unwrap();
    let p = s.doc.para_mut(wordcraft_doc::StoryRef::Body, &Pos::body(0, 0).path).unwrap();
    direct(&mut p.props);
    s.sel = crate::Selection::caret(Pos::body(0, 3));
    s
}

fn props(s: &Session) -> ParaProps {
    s.doc.para_at(&Pos::body(0, 0)).unwrap().props.clone()
}

/// The direct formatting each case sets, by name.
fn cases() -> Vec<(&'static str, Box<dyn Fn(&mut ParaProps)>)> {
    vec![
        (
            "left indent, centred, space before",
            Box::new(|p: &mut ParaProps| {
                p.indent_left = Some(36.0);
                p.align = Some(Align::Center);
                p.space_before = Some(18.0);
            }),
        ),
        (
            "first-line and right indent",
            Box::new(|p: &mut ParaProps| {
                p.indent_first = Some(20.0);
                p.indent_right = Some(40.0);
            }),
        ),
        ("line spacing 1.5", Box::new(|p: &mut ParaProps| p.line_spacing = Some(LineSpacing::Multiple(1.5)))),
        (
            "keeps and widow control",
            Box::new(|p: &mut ParaProps| {
                p.keep_next = Some(true);
                p.keep_lines = Some(true);
                p.page_break_before = Some(true);
                p.widow_control = Some(false);
            }),
        ),
        ("a tab stop", Box::new(|p: &mut ParaProps| p.tabs = Some(vec![TabStop { pos: 100.0, align: TabAlign::default(), leader: TabLeader::default() }]))),
        ("a box border", Box::new(|p: &mut ParaProps| p.borders = Some(Borders::all(Border::single(0.5))))),
        ("yellow shading", Box::new(|p: &mut ParaProps| p.shading = Some(Rgb(255, 255, 0)))),
        ("outline level 4", Box::new(|p: &mut ParaProps| p.outline_level = Some(3))),
    ]
}

/// What applying `style` leaves: the style id, nothing else that `ParaProps::default()` lacks.
fn assert_only_style(s: &Session, style: &str, case: &str) {
    let got = props(s);
    let want = ParaProps { style: Some(style.into()), ..Default::default() };
    assert_eq!(got, want, "{case}, then {style}: only the style should remain");
}

#[test]
fn a_paragraph_style_removes_direct_paragraph_formatting() {
    for (cmd, style) in [("para.heading1", "Heading1"), ("para.normal", "Normal")] {
        for (case, direct) in cases() {
            let mut s = para_with(direct);
            s.run(cmd, &json!({})).unwrap();
            assert_only_style(&s, style, case);
        }
    }
    // Any paragraph style, not just headings and Normal.
    for (case, direct) in cases() {
        let mut s = para_with(direct);
        s.run("para.style", &json!({"style": "Quote"})).unwrap();
        assert_only_style(&s, "Quote", case);
    }
}

#[test]
fn a_paragraph_style_removes_a_direct_bullet_or_number() {
    for list_cmd in ["para.bullets", "para.numbering"] {
        for style in ["Heading1", "Normal", "Quote"] {
            let mut s = para_with(|_| {});
            s.run(list_cmd, &json!({})).unwrap();
            assert!(props(&s).numbering.is_some_and(|n| n.num != 0), "{list_cmd}: expected a list before");
            s.run("para.style", &json!({"style": style})).unwrap();
            let p = props(&s);
            assert!(p.numbering.is_none_or(|n| n.num == 0), "{list_cmd}, then {style}: the bullet or number should go");
            assert_eq!(p.indent_left, None, "{list_cmd}, then {style}: the list's direct indent should go");
            assert_eq!(p.indent_first, None, "{list_cmd}, then {style}: the list's direct hanging indent should go");
            assert_eq!(p.style.as_deref(), Some(style));
        }
    }
}

#[test]
fn a_character_style_leaves_paragraph_formatting_alone() {
    let mut s = para_with(|p| {
        p.indent_left = Some(36.0);
        p.align = Some(Align::Center);
    });
    s.sel = crate::Selection { anchor: Pos::body(0, 0), focus: Pos::body(0, 5) };
    s.run("para.style", &json!({"style": "Strong"})).unwrap();
    let p = props(&s);
    assert_eq!(p.indent_left, Some(36.0));
    assert_eq!(p.align, Some(Align::Center));
}

#[test]
fn a_drop_cap_survives() {
    let mut s = para_with(|p| {
        p.drop_cap = Some(1);
        p.indent_left = Some(36.0);
    });
    s.run("para.heading1", &json!({})).unwrap();
    let p = props(&s);
    assert_eq!(p.drop_cap, Some(1), "a drop cap is an object, not formatting");
    assert_eq!(p.indent_left, None);
}
