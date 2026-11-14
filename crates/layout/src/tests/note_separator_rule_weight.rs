//! The footnote separator line is drawn 1/20 of the separator's font size thick (measured in
//! Microsoft 365 Word 16.0.20527 from its PDFs, compat modes 11 and 15, Times New Roman): the
//! built-in separator follows the Normal style's size (8 pt -> 0.36, 10 -> 0.48, 12 -> 0.60,
//! 28 -> 1.44 pt in Word's 0.12 pt device grid), and a separator story follows the size of the run
//! holding the separator mark (24 pt -> 1.20 pt). The tests allow for Word's grid.
use super::*;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Pos, para_block};

const TOL: f32 = 0.06;

fn set_normal(d: &mut Document, size: f32) {
    let styles = std::sync::Arc::make_mut(&mut d.styles);
    let normal = styles.get_mut("Normal").expect("the document has a Normal style");
    normal.chr = CharProps { font: Some("Times New Roman".into()), size: Some(size), ..Default::default() };
    normal.para = ParaProps::default();
}

fn single(text: &str) -> Paragraph {
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p
}

/// Normal at `normal` pt, one body paragraph referring to a one-line footnote; no separator story.
fn doc(normal: f32) -> Document {
    let mut d = Document::from_text("");
    set_normal(&mut d, normal);
    d.settings.compat_mode = 11;
    d.body = vec![para_block(single("Body text"))];
    let id = d.add_part(wordcraft_doc::PartKind::Footnote, vec![para_block(single("Note"))]);
    d.insert_object(
        &Pos::body(0, 1),
        InlineObject::NoteRef { kind: wordcraft_doc::para::NoteKind::Footnote, id, custom: String::new() },
        &Default::default(),
    )
    .unwrap();
    d
}

/// The widths of the horizontal rules drawn on page 0 (the separator is the only one).
fn rule_widths(d: &Document) -> Vec<f32> {
    crate::test_fonts();
    lay(d).pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Rule { y0, y1, border, .. } if (y0 - y1).abs() < 0.01 => Some(border.width),
            _ => None,
        })
        .collect()
}

fn check_default(normal: f32, want: f32) {
    let got = rule_widths(&doc(normal));
    assert_eq!(got.len(), 1, "one separator rule: {got:?}");
    assert!((got[0] - want).abs() <= TOL, "Normal {normal} pt: separator {} pt thick, want {want}", got[0]);
}

#[test]
fn built_in_separator_is_a_twentieth_of_normal_8pt() {
    check_default(8.0, 0.4);
}

#[test]
fn built_in_separator_is_a_twentieth_of_normal_12pt() {
    check_default(12.0, 0.6);
}

#[test]
fn built_in_separator_is_a_twentieth_of_normal_28pt() {
    check_default(28.0, 1.4);
}

#[test]
fn separator_story_follows_its_runs_size() {
    let mut d = doc(10.0);
    let mut sep = single("");
    let run = CharProps { size: Some(24.0), ..Default::default() };
    sep.insert_object(0, InlineObject::note_separator(false), &run).unwrap();
    d.footnote_separators.separator = Some(vec![para_block(sep)]);
    let got = rule_widths(&d);
    assert_eq!(got.len(), 1, "one separator rule: {got:?}");
    assert!((got[0] - 1.2).abs() <= TOL, "24 pt separator run: {} pt thick, want 1.2", got[0]);
}
