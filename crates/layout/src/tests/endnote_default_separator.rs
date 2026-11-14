//! Without an endnote separator story, Word's built-in endnote separator takes one single-spaced
//! line of the Normal style, as the built-in footnote separator does, and draws its 2 in rule with
//! the bottom edge 0.42 x the Normal size above the first note, 0.05 x the size thick. Measured
//! in Microsoft 365 Word 16.0.20527 from its PDFs (last body baseline to first endnote baseline:
//! Times New Roman 10 pt 19.44, 12 pt 22.32, 20 pt 33.24; a fixed 20 pt room gave 28.00, 28.43,
//! 30.16).
use super::default_note_separator::sep_height;
use super::*;
use crate::notes::NOTE_SEP;
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

/// Normal = `font` at `normal` pt, one body paragraph referring to a one-line endnote, and no
/// endnote separator story.
fn doc(font: &str, normal: f32) -> Document {
    doc_with(font, normal, wordcraft_doc::para::NoteKind::Endnote).0
}

/// The room the built-in FOOTNOTE separator takes in the same document (one Normal line, which
/// Word measures equal to its built-in endnote separator).
fn footnote_room(font: &str, normal: f32) -> f32 {
    crate::test_fonts();
    let (d, id) = doc_with(font, normal, wordcraft_doc::para::NoteKind::Footnote);
    sep_height(&d, id)
}

fn doc_with(font: &str, normal: f32, kind: wordcraft_doc::para::NoteKind) -> (Document, u32) {
    let mut d = Document::from_text("");
    set_normal(&mut d, font, normal);
    d.settings.compat_mode = 15;
    d.body = vec![para_block(single("Body text"))];
    d.endnote_separators.separator = None;
    d.endnote_separators.continuation_separator = None;
    let part = match kind {
        wordcraft_doc::para::NoteKind::Footnote => wordcraft_doc::PartKind::Footnote,
        _ => wordcraft_doc::PartKind::Endnote,
    };
    let id = d.add_part(part, vec![para_block(single("Note"))]);
    d.insert_object(&Pos::body(0, 9), InlineObject::NoteRef { kind, id, custom: String::new() }, &Default::default()).unwrap();
    (d, id)
}

struct Seen {
    body_bottom: f32,
    note_top: f32,
    rule_y: f32,
    rule_w: f32,
    rule_len: f32,
}

fn seen(d: &Document) -> Seen {
    crate::test_fonts();
    let l = lay(d);
    assert_eq!(l.pages.len(), 1, "one page");
    let page = &l.pages[0];
    let body_bottom = page
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, y, para, l0, l1, .. } => item_bottom(*y, para, *l0, *l1),
            _ => None,
        })
        .reduce(f32::max)
        .expect("a body line");
    let note_top = page
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Part(_), y, .. } => Some(*y),
            _ => None,
        })
        .reduce(f32::min)
        .expect("an endnote line");
    let rules: Vec<(f32, f32, f32)> = page
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Rule { x0, y0, x1, y1, border } if (y0 - y1).abs() < 0.01 => Some((*y0, border.width, (x1 - x0).abs())),
            _ => None,
        })
        .collect();
    assert_eq!(rules.len(), 1, "one separator rule: {rules:?}");
    let (rule_y, rule_w, rule_len) = rules[0];
    Seen { body_bottom, note_top, rule_y, rule_w, rule_len }
}

fn check(font: &str, normal: f32) {
    let d = doc(font, normal);
    let s = seen(&d);
    let line = footnote_room(font, normal);
    let room = s.note_top - s.body_bottom;
    assert!((room - line).abs() <= TOL, "{font} {normal} pt: endnote separator takes {room} pt, want the footnote separator's {line}");
    assert!(s.rule_y > s.body_bottom && s.rule_y < s.note_top, "{font} {normal} pt: the rule sits inside the separator");
    let gap = s.note_top - (s.rule_y + s.rule_w / 2.0);
    let want = 0.42 * normal;
    assert!((gap - want).abs() <= TOL, "{font} {normal} pt: rule bottom {gap} pt above the note, want {want}");
    assert!((s.rule_w - normal / 20.0).abs() < 0.01, "{font} {normal} pt: rule {} pt thick, want {}", s.rule_w, normal / 20.0);
    assert!((s.rule_len - 144.0).abs() < 0.01, "{font} {normal} pt: rule {} pt long, want 144", s.rule_len);
}

#[test]
fn built_in_endnote_separator_is_one_normal_line_10pt() {
    check("Times New Roman", 10.0);
}

#[test]
fn built_in_endnote_separator_is_one_normal_line_12pt() {
    check("Times New Roman", 12.0);
}

#[test]
fn built_in_endnote_separator_is_one_normal_line_20pt() {
    check("Times New Roman", 20.0);
}

#[test]
fn built_in_endnote_separator_is_one_normal_line_arial() {
    check("Arial", 12.0);
}

/// The fixed 20 pt room is gone: at 10 pt Times New Roman the note starts higher than it used to.
#[test]
fn built_in_endnote_separator_is_not_20pt() {
    let s = seen(&doc("Times New Roman", 10.0));
    let room = s.note_top - s.body_bottom;
    assert!((room - 20.0).abs() > 5.0, "endnote separator still takes the old 20 pt: {room}");
}

/// A Normal style whose size is not a size: the room falls back to 12 pt, as for footnotes.
#[test]
fn unusable_normal_size_keeps_12pt() {
    let d = doc("Times New Roman", 0.0);
    let s = seen(&d);
    let room = s.note_top - s.body_bottom;
    assert!((room - NOTE_SEP).abs() <= TOL, "fallback room: {room}");
}
