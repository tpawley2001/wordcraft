//! A paragraph whose mark is hidden (`w:pPr/w:rPr/w:vanish`, with or without `w:specVanish`, Word's
//! style separator) is laid out on one line with the next paragraph when hidden text isn't shown.
//! Measured in Microsoft 365 Word 16.0.20527 (Letter, 1 in margins): `HEADA` (right-aligned, mark
//! hidden), ` bodyb text` (left), `LAST` -> one right-aligned line `HEADA bodyb text` ending at the
//! right margin, then `LAST` on the second line.
use crate::*;
use wordcraft_doc::para_block;
use wordcraft_doc::props::{Align, LineSpacing};

fn chr() -> CharProps {
    CharProps { font: Some("Arial".into()), size: Some(12.0), ..Default::default() }
}

fn para(text: &str, align: Align) -> Paragraph {
    let mut p = Paragraph::with_text(text, chr());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p.props.align = Some(align);
    p.mark = chr();
    p
}

fn doc(hide_first_mark: bool) -> Document {
    let mut d = Document::new();
    let mut first = para("HEADA", Align::Right);
    if hide_first_mark {
        first.mark.hidden = Some(true);
    }
    d.body = vec![para_block(first), para_block(para(" bodyb text", Align::Left)), para_block(para("LAST", Align::Left))];
    d
}

fn lay_with(d: &Document, show_hidden: bool) -> DocLayout {
    crate::test_fonts();
    layout(d, &mut LayoutCache::new(), &LayoutOptions { show_hidden, ..Default::default() })
}

/// Top of the first line placed for body block `block`.
fn top(l: &DocLayout, block: usize) -> f32 {
    l.pages[0]
        .items
        .iter()
        .find_map(|item| match item {
            Placed::Lines { path, y, .. } if *path == Path::top(block) => Some(*y),
            _ => None,
        })
        .expect("block placed on page 1")
}

#[test]
fn paragraph_with_hidden_mark_joins_the_next_one_on_one_line() {
    let plain = lay_with(&doc(false), false);
    // Not hidden: three lines, LAST on the third.
    let pitch = top(&plain, 1) - top(&plain, 0);
    assert!(pitch > 10.0, "{pitch}");
    assert!((top(&plain, 2) - top(&plain, 0) - 2.0 * pitch).abs() < 0.5);
    // Hidden mark: HEADA and ' bodyb text' share line 1, LAST is on line 2 (Word: y 88.95 vs 105.99).
    let joined = lay_with(&doc(true), false);
    assert!(
        (top(&joined, 2) - top(&joined, 0) - pitch).abs() < 0.5,
        "LAST must be one line below the joined line: {} vs {}",
        top(&joined, 2),
        top(&joined, 0)
    );
}

#[test]
fn hidden_mark_shown_keeps_separate_lines() {
    // With hidden text shown the mark is visible: the paragraphs keep their own lines.
    let plain = lay_with(&doc(false), true);
    let shown = lay_with(&doc(true), true);
    assert!((top(&shown, 2) - top(&plain, 2)).abs() < 0.5);
}
