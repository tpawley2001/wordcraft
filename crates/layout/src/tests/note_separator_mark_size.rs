//! A footnote separator story's line is as tall as a single-spaced line of the larger of the
//! separator character's font size and the paragraph mark's font size (measured in Word 16.0.20527,
//! compat mode 11, Normal Times New Roman 11 pt, footnotes beneath the text). The mark counts here
//! even though the line holds the separator: a 1 pt separator run under an 11 pt mark still takes an
//! 11 pt line, and a 20 pt mark makes a 20 pt line.
use super::default_note_separator::{normal_line, sep_height};
use super::*;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Pos, para_block};

const NORMAL: f32 = 11.0;

/// Set the Normal style to Times New Roman at `size`.
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

/// Normal at 11 pt, one body paragraph referring to a one-line footnote, and a separator story
/// whose separator run is `run` pt and whose paragraph mark is `mark` pt (`None`: unset, so
/// Normal's 11 pt).
fn doc(run: Option<f32>, mark: Option<f32>) -> (Document, u32) {
    let mut d = Document::from_text("");
    set_normal(&mut d, NORMAL);
    d.settings.compat_mode = 11;
    d.body = vec![para_block(single("Body text"))];
    let id = d.add_part(wordcraft_doc::PartKind::Footnote, vec![para_block(single("Note"))]);
    d.insert_object(
        &Pos::body(0, 1),
        InlineObject::NoteRef { kind: wordcraft_doc::para::NoteKind::Footnote, id, custom: String::new() },
        &Default::default(),
    )
    .unwrap();
    let mut sep = single("");
    sep.mark.size = mark;
    let run_props = CharProps { size: run, ..Default::default() };
    sep.insert_object(0, InlineObject::note_separator(false), &run_props).unwrap();
    d.footnote_separators.separator = Some(vec![para_block(sep)]);
    (d, id)
}

/// One single-spaced empty line of Times New Roman at `size`, laid out as body text.
fn line_at(size: f32) -> f32 {
    let mut d = Document::from_text("");
    set_normal(&mut d, size);
    d.settings.compat_mode = 11;
    normal_line(&d)
}

fn check(run: Option<f32>, mark: Option<f32>, want_size: f32) {
    crate::test_fonts();
    let (d, id) = doc(run, mark);
    let got = sep_height(&lay(&d), id);
    let want = line_at(want_size);
    assert!((got - want).abs() < 0.01, "separator run {run:?} pt, mark {mark:?} pt: separator line {got}, want one {want_size} pt line ({want})");
}

/// Word case H: a 1 pt separator run under an unset (11 pt) mark takes an 11 pt line.
#[test]
fn small_separator_run_takes_the_marks_line() {
    check(Some(1.0), None, NORMAL);
}

/// Word case J: a 10 pt separator run under an 11 pt mark takes an 11 pt line.
#[test]
fn ten_point_separator_run_takes_the_eleven_point_mark_line() {
    check(Some(10.0), None, NORMAL);
}

/// Word case K: run and mark both 1 pt: a 1 pt line.
#[test]
fn tiny_run_and_tiny_mark_take_a_tiny_line() {
    check(Some(1.0), Some(1.0), 1.0);
}

/// Word case L: a 20 pt separator run takes a 20 pt line.
#[test]
fn large_separator_run_takes_its_own_line() {
    check(Some(20.0), None, 20.0);
}

/// Word case M: an unset (11 pt) run under a 20 pt mark takes a 20 pt line.
#[test]
fn large_paragraph_mark_makes_the_separator_line_taller() {
    check(None, Some(20.0), 20.0);
}

/// Word case N: an unset (11 pt) run under a 1 pt mark keeps its 11 pt line.
#[test]
fn small_paragraph_mark_does_not_shrink_the_separator_line() {
    check(None, Some(1.0), NORMAL);
}

/// Ordinary text lines are unchanged: a 20 pt mark on an 11 pt text line doesn't make it taller
/// (Word: same pitch as the lines around it).
#[test]
fn a_large_mark_on_a_text_line_still_does_not_count() {
    crate::test_fonts();
    let mut d = Document::from_text("");
    set_normal(&mut d, NORMAL);
    d.settings.compat_mode = 11;
    let mut p = single("Body text");
    p.mark.size = Some(20.0);
    d.body = vec![para_block(p)];
    let l = lay(&d);
    let (y, para) = l.pages[0]
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, para, y, .. } => Some((*y, para.clone())),
            _ => None,
        })
        .unwrap();
    let h = crate::page::item_bottom(y, &para, 0, para.lines.len()).unwrap() - y;
    let want = line_at(NORMAL);
    assert!((h - want).abs() < 0.01, "text line {h}, want {want}");
}
