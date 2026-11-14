//! Ctrl+Space, Ctrl+Q and Clear All Formatting reset what Microsoft 365 Word resets (2026-11-14, object
//! model: Range.Font.Reset, Paragraph.Format.Reset, Selection.ClearFormatting).
use super::*;
use wordcraft_doc::props::Highlight;

/// "Hello world" in one paragraph, its first word given a character style, bold and a yellow highlight.
fn styled() -> Session {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "Hello world"}));
    s.doc
        .format_range(&Pos::body(0, 0), &Pos::body(0, 5), &|c| {
            c.style = Some("Emphasis".into());
            c.bold = Some(true);
            c.highlight = Some(Highlight::Yellow);
        })
        .unwrap();
    run(&mut s, "select.all", json!({}));
    s
}

fn char0(s: &Session) -> wordcraft_doc::props::CharProps {
    s.doc.para_at(&Pos::body(0, 0)).unwrap().props_of_char(1).clone()
}

#[test]
fn ctrl_space_drops_the_character_style_and_keeps_the_highlight() {
    let mut s = styled();
    run(&mut s, "format.resetChar", json!({}));
    let c = char0(&s);
    assert_eq!(c.style, None, "Word: Emphasis -> Default Paragraph Font");
    assert_eq!(c.bold, None);
    assert_eq!(c.highlight, Some(Highlight::Yellow), "Word keeps highlighting on Ctrl+Space");
}

#[test]
fn clear_all_formatting_keeps_the_highlight() {
    let mut s = styled();
    run(&mut s, "format.clear", json!({}));
    let c = char0(&s);
    assert_eq!((c.style, c.bold), (None, None));
    assert_eq!(c.highlight, Some(Highlight::Yellow), "Word keeps highlighting on Clear All Formatting");
}

#[test]
fn ctrl_q_removes_a_direct_bullet_or_number_and_keeps_the_style() {
    for cmd in ["para.bullets", "para.numbering"] {
        let mut s = s();
        run(&mut s, "text.insert", json!({"text": "list para"}));
        run(&mut s, "para.heading1", json!({}));
        run(&mut s, cmd, json!({}));
        run(&mut s, "para.alignCenter", json!({}));
        let before = s.doc.para_at(&Pos::body(0, 0)).unwrap().props.numbering;
        assert!(before.is_some_and(|n| n.num != 0), "{cmd}: list applied");
        run(&mut s, "para.reset", json!({}));
        let p = s.doc.para_at(&Pos::body(0, 0)).unwrap().props.clone();
        assert!(p.numbering.is_none_or(|n| n.num == 0), "{cmd}: Word removes the list on Ctrl+Q");
        assert_eq!(p.style.as_deref(), Some("Heading1"), "{cmd}: style kept");
        assert_eq!(p.align, None, "{cmd}: alignment back to the style's");
    }
}
