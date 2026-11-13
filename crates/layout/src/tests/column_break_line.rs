//! A line holding only a column break (a paragraph that starts with `w:br w:type="column"`, RTF
//! `\column`) takes no room in the column it ends, below compatibility mode 15: that column is
//! exactly as tall as the text before the break, and the rest of the paragraph is one line at the
//! top of the next column with the paragraph's own line height. Measured in Word's PDFs of a
//! synthetic RTF document (Letter, 1 in margins, 13.8 pt lines): `TOP`, a continuous two-column
//! section `A1 A2 A3 P B1` (P = column break + mark), then a continuous one-column `MARK`. With P
//! at single, exactly 5 pt and exactly 20 pt, `MARK` is three lines below `A1`; at exactly 40 pt
//! the second column (40 + one line) is the taller one. Here every other line is exactly 14 pt.

use super::*;
use wordcraft_doc::para::COLUMN_BREAK;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::section::{Columns, SectionStart};

const LINE: f32 = 14.0;

fn para(text: &str, height: f32) -> Paragraph {
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Exactly(height));
    p
}

fn section(count: u32, start: SectionStart) -> SectionProps {
    SectionProps { start, columns: Columns { count, space: 36.0, ..Columns::default() }, ..SectionProps::default() }
}

/// `TOP`, then the two-column paragraphs `middle` (the last ends the section), then `MARK`.
fn probe(mode: u32, middle: Vec<Paragraph>) -> Document {
    let mut d = Document::new();
    d.settings.compat_mode = mode;
    let mut top = para("TOP", LINE);
    top.section = Some(Box::new(section(1, SectionStart::NextPage)));
    let mut paras = vec![top];
    paras.extend(middle);
    if let Some(last) = paras.last_mut() {
        last.section = Some(Box::new(section(2, SectionStart::Continuous)));
    }
    paras.push(para("MARK", LINE));
    d.last_section = section(1, SectionStart::Continuous);
    d.body = paras.into_iter().map(wordcraft_doc::para_block).collect();
    d
}

/// A1 A2 A3, P (a column break and `after`, at exactly `p` pt), B1.
fn rows(p: f32, after: &str) -> Vec<Paragraph> {
    vec![para("A1", LINE), para("A2", LINE), para("A3", LINE), para(&format!("{COLUMN_BREAK}{after}"), p), para("B1", LINE)]
}

/// Page, x and y of line `l0` of body paragraph `block`.
fn line_at(l: &DocLayout, block: u32, line: usize) -> (usize, f32, f32) {
    l.pages
        .iter()
        .enumerate()
        .find_map(|(pi, pg)| {
            pg.items.iter().find_map(|it| match it {
                Placed::Lines { story: StoryRef::Body, path, l0, x, y, .. } if path.0 == [block] && *l0 == line => Some((pi, *x, *y)),
                _ => None,
            })
        })
        .unwrap_or_else(|| panic!("line {line} of paragraph {block} is laid out"))
}

/// `MARK`'s top below `A1`'s, and `B1`'s below the top of the columns, in a one-page layout.
fn offsets(d: &Document) -> (f32, f32) {
    let l = lay(d);
    assert_eq!(l.pages.len(), 1);
    let n = d.body.len() as u32;
    let (_, _, a1) = line_at(&l, 1, 0);
    let (_, _, mark) = line_at(&l, n - 1, 0);
    let (_, _, b1) = line_at(&l, n - 2, 0);
    (mark - a1, b1 - a1)
}

fn near(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() <= 0.2, "{what}: {got} vs {want}");
}

#[test]
fn the_break_line_adds_nothing_to_the_column_it_ends() {
    for p in [5.0, LINE, 20.0] {
        let (mark, b1) = offsets(&probe(11, rows(p, "")));
        near(mark, 3.0 * LINE, &format!("MARK, P exactly {p}"));
        near(b1, p, &format!("B1, P exactly {p}"));
    }
}

#[test]
fn a_taller_second_column_sets_where_the_next_section_starts() {
    let (mark, b1) = offsets(&probe(11, rows(40.0, "")));
    near(mark, 40.0 + LINE, "MARK");
    near(b1, 40.0, "B1");
    // Text after the break is on the 40 pt line at the top of column 2.
    let d = probe(11, rows(40.0, "X"));
    let (mark, b1) = offsets(&d);
    near(mark, 40.0 + LINE, "MARK after X");
    near(b1, 40.0, "B1 after X");
    let l = lay(&d);
    let (_, a1_x, a1) = line_at(&l, 1, 0);
    let (_, x_x, x_y) = line_at(&l, 4, 1);
    assert!(x_x > a1_x + 100.0, "X is in column 2: {x_x} vs {a1_x}");
    near(x_y, a1, "X at the top of column 2");
}

#[test]
fn a_single_line_before_the_break_and_two_after() {
    let middle = vec![para("A1", LINE), para(&COLUMN_BREAK.to_string(), 40.0), para("B1", LINE), para("B2", LINE)];
    let (mark, _) = offsets(&probe(11, middle));
    near(mark, 40.0 + 2.0 * LINE, "MARK");
}

#[test]
fn a_break_line_at_the_foot_of_a_full_column_still_ends_it() {
    let mut d = Document::new();
    d.settings.compat_mode = 11;
    d.last_section = section(2, SectionStart::NextPage);
    // 46 lines of 14 pt fill 644 of the 648 pt column; the break's line does not fit after them.
    let mut paras: Vec<Paragraph> = (0..46).map(|i| para(&format!("A{i}"), LINE)).collect();
    paras.push(para(&COLUMN_BREAK.to_string(), LINE));
    paras.push(para("B1", LINE));
    d.body = paras.into_iter().map(wordcraft_doc::para_block).collect();
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    let (_, a_x, a_y) = line_at(&l, 0, 0);
    let (pi, b_x, b_y) = line_at(&l, 47, 0);
    assert_eq!(pi, 0);
    assert!(b_x > a_x + 100.0, "B1 is in column 2: {b_x} vs {a_x}");
    // Below the break's paragraph mark, the first line of column 2.
    near(b_y, a_y + LINE, "B1 below the mark at the top of column 2");
}

#[test]
fn mode_15_keeps_the_break_line() {
    // Not measured in mode 15: the break's line keeps its 20 pt there, as before.
    let (mark, b1) = offsets(&probe(15, rows(20.0, "")));
    near(mark, 3.0 * LINE + 20.0, "MARK");
    near(b1, 20.0, "B1");
}

#[test]
fn a_break_after_text_keeps_its_line() {
    let middle = vec![para("A1", LINE), para("A2", LINE), para(&format!("A3{COLUMN_BREAK}"), LINE), para("B1", LINE)];
    let (mark, b1) = offsets(&probe(11, middle));
    // Column 1 keeps A3's line; the mark after the break starts column 2.
    near(mark, 3.0 * LINE, "MARK");
    near(b1, LINE, "B1");
}
