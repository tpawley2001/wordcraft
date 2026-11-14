//! The footnote separator line sits 0.42 x its font size above the top of the first note's line
//! (rule bottom edge to note line top), whatever the font, measured in Microsoft 365 Word
//! 16.0.20527 from its PDFs (compat modes 11 and 15 alike, Word's 0.12 pt device grid):
//! S = 8 -> 3.28, 10 -> 4.12, 12 -> 5.08, 28 -> 11.68 pt. S is the Normal style's size for the
//! built-in separator, and the size of the run holding the separator mark in a separator story.
use super::*;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Pos, para_block};

const TOL: f32 = 0.15;

fn set_normal(d: &mut Document, font: &str, size: f32) {
    let styles = std::sync::Arc::make_mut(&mut d.styles);
    let normal = styles.get_mut("Normal").expect("the document has a Normal style");
    normal.chr = CharProps { font: Some(font.into()), size: Some(size), ..Default::default() };
    normal.para = ParaProps::default();
}

fn single(text: &str) -> Paragraph {
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p
}

/// Normal = `font` at `normal` pt, one body paragraph referring to a one-line footnote.
fn doc(font: &str, normal: f32) -> Document {
    let mut d = Document::from_text("");
    set_normal(&mut d, font, normal);
    d.settings.compat_mode = 15;
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

/// The gap from the separator rule's bottom edge to the top of the first line below it (the note).
fn gap(d: &Document) -> f32 {
    crate::test_fonts();
    let page = &lay(d).pages[0];
    let rules: Vec<(f32, f32)> = page
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Rule { y0, y1, border, .. } if (y0 - y1).abs() < 0.01 => Some((*y0, border.width)),
            _ => None,
        })
        .collect();
    assert_eq!(rules.len(), 1, "one separator rule: {rules:?}");
    let (ry, w) = rules[0];
    let note_top = page
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { y, .. } if *y > ry => Some(*y),
            _ => None,
        })
        .fold(f32::INFINITY, f32::min);
    assert!(note_top.is_finite(), "a note line below the separator rule at {ry}");
    note_top - (ry + w / 2.0)
}

fn check(font: &str, normal: f32, want: f32) {
    let got = gap(&doc(font, normal));
    assert!((got - want).abs() <= TOL, "{font} {normal} pt: rule bottom {got} pt above the note, want {want}");
}

#[test]
fn built_in_separator_sits_above_the_note_8pt() {
    check("Times New Roman", 8.0, 3.28);
}

#[test]
fn built_in_separator_sits_above_the_note_10pt() {
    check("Times New Roman", 10.0, 4.12);
}

#[test]
fn built_in_separator_sits_above_the_note_12pt() {
    check("Times New Roman", 12.0, 5.08);
}

#[test]
fn built_in_separator_sits_above_the_note_28pt() {
    check("Times New Roman", 28.0, 11.68);
}

#[test]
fn built_in_separator_position_ignores_the_font() {
    check("Arial", 12.0, 5.08);
}

#[test]
fn separator_story_rule_follows_its_runs_size() {
    let mut d = doc("Times New Roman", 10.0);
    let mut sep = single("");
    let run = CharProps { size: Some(24.0), ..Default::default() };
    sep.insert_object(0, InlineObject::note_separator(false), &run).unwrap();
    d.footnote_separators.separator = Some(vec![para_block(sep)]);
    let got = gap(&d);
    assert!((got - 0.42 * 24.0).abs() <= TOL, "24 pt separator run: rule bottom {got} pt above the note, want 10.08");
}
