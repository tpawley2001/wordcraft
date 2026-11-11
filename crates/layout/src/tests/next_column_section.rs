//! A section that starts with a next column break (ECMA-376 Part 1 §17.18.77 `nextColumn`, RTF
//! `\sbkcol`, .doc `bkc = 1`) starts in the next column of the same page when the section before
//! has as many columns and did not end in its last one; otherwise on a new page, as Next Page does.
//! Expected pages and x positions measured from Word's PDFs of the same documents in RTF
//! (US Letter, 1" margins, 36pt column gap).

use super::*;
use wordcraft_doc::section::{Columns, SectionStart};

/// One paragraph per entry of `sections` (its text, then its column count); every section after
/// the first starts with a next column break.
fn doc(sections: &[(&str, u32)]) -> Document {
    let text: Vec<&str> = sections.iter().map(|(t, _)| *t).collect();
    let mut d = Document::from_text(&text.join("\n"));
    let props = |i: usize, count: u32| SectionProps {
        start: if i == 0 { SectionStart::NextPage } else { SectionStart::NextColumn },
        columns: Columns { count, space: 36.0, ..Columns::default() },
        ..SectionProps::default()
    };
    let last = sections.len() - 1;
    for (i, (_, count)) in sections.iter().enumerate() {
        if i == last {
            d.last_section = props(i, *count);
        } else {
            d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().section = Some(Box::new(props(i, *count)));
        }
    }
    d
}

/// Page and x of the first line of each body paragraph, in document order.
fn starts(l: &DocLayout, paras: usize) -> Vec<(usize, f32)> {
    (0..paras)
        .map(|i| {
            l.pages
                .iter()
                .enumerate()
                .find_map(|(pi, p)| {
                    p.items.iter().find_map(|it| match it {
                        Placed::Lines { story: StoryRef::Body, path, x, l0: 0, .. } if path.0 == [i as u32] => Some((pi, *x)),
                        _ => None,
                    })
                })
                .unwrap_or_else(|| panic!("paragraph {i} is laid out"))
        })
        .collect()
}

/// The left edge of column `k` of `count` equal columns 36pt apart in a 468pt wide body at 72pt.
fn col_x(count: u32, k: u32) -> f32 {
    let w = (468.0 - 36.0 * (count - 1) as f32) / count as f32;
    72.0 + k as f32 * (w + 36.0)
}

fn check(sections: &[(&str, u32)], want: &[(usize, f32)], pages: usize) {
    let l = lay(&doc(sections));
    let got = starts(&l, sections.len());
    assert_eq!(l.pages.len(), pages, "pages for {sections:?}: {got:?}");
    for (i, ((gp, gx), (wp, wx))) in got.iter().zip(want).enumerate() {
        assert_eq!(gp, wp, "section {i} page for {sections:?}: {got:?}");
        assert!((gx - wx).abs() < 0.5, "section {i} x for {sections:?}: {gx} vs {wx}");
    }
}

#[test]
fn a_next_column_section_with_the_same_columns_starts_in_the_next_column() {
    check(&[("AAA one", 2), ("BBB one", 2)], &[(0, col_x(2, 0)), (0, col_x(2, 1))], 1);
}

#[test]
fn a_next_column_section_starts_at_the_top_of_the_body() {
    let l = lay(&doc(&[("AAA one", 2), ("BBB one", 2)]));
    let ys: Vec<f32> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, y, l0: 0, .. } => Some(*y),
            _ => None,
        })
        .collect();
    assert_eq!(ys.len(), 2, "both sections on page one: {ys:?}");
    assert!((ys[0] - ys[1]).abs() < 0.5, "same top: {ys:?}");
}

#[test]
fn next_column_sections_fill_the_columns_then_go_to_a_new_page() {
    let x = |k| col_x(3, k);
    check(&[("AAA one", 3), ("BBB one", 3), ("CCC one", 3), ("DDD one", 3)], &[(0, x(0)), (0, x(1)), (0, x(2)), (1, x(0))], 2);
    let x = |k| col_x(2, k);
    check(&[("AAA one", 2), ("BBB one", 2), ("CCC one", 2)], &[(0, x(0)), (0, x(1)), (1, x(0))], 2);
}

#[test]
fn a_next_column_section_with_other_columns_starts_a_new_page() {
    check(&[("AAA one", 1), ("BBB one", 2)], &[(0, 72.0), (1, 72.0)], 2);
    check(&[("AAA one", 2), ("BBB one", 3)], &[(0, 72.0), (1, 72.0)], 2);
}

#[test]
fn a_next_column_section_after_one_column_starts_a_new_page() {
    check(&[("AAA one", 1), ("BBB one", 1)], &[(0, 72.0), (1, 72.0)], 2);
}
