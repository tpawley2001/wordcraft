//! Line numbers and the line between columns in sections with columns, measured in Microsoft 365
//! Word (PDF export, Times New Roman 12 pt, letter page, 1-inch margins):
//! - Auto line-number distance is 18 pt in a one-column section but 9 pt in a section with two or
//!   more columns, whatever the column spacing (18, 36 or 72 pt; two or three columns). An
//!   explicit distance is used as is.
//! - Line between (`w:sep`) is drawn only before a column that has text on the page: two columns
//!   with all the text in the first draw no line; three columns with text in the first two draw
//!   one line, between those two.
//! - The line is 0.72 pt wide.

use super::*;
use wordcraft_doc::section::Columns;

fn doc(lines: usize, count: u32, space: f32, separator: bool, numbers: bool) -> Document {
    let text = (1..=lines).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    let mut d = Document::from_text(&text);
    if numbers {
        d.last_section.line_numbers = Some(Default::default());
    }
    d.last_section.columns = Columns { count, space, separator, widths: Vec::new() };
    d
}

/// Left edge of each column of the document's (only) section, with equal columns.
fn column_lefts(d: &Document) -> Vec<f32> {
    let s = &d.last_section;
    let n = s.columns.count.max(1) as f32;
    let w = (s.text_width() - s.columns.space * (n - 1.0)) / n;
    (0..s.columns.count.max(1)).map(|i| s.margin_left + s.gutter + i as f32 * (w + s.columns.space)).collect()
}

/// Right edge of every line number on page 0.
fn number_right_edges(l: &DocLayout) -> Vec<f32> {
    l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story, para, x, .. } if *story == DECOR_STORY => {
                Some(x + para.lines.first().and_then(|l| l.xs.last().copied()).unwrap_or(0.0))
            }
            _ => None,
        })
        .collect()
}

/// Left x of every body line on page 0.
fn body_xs(l: &DocLayout) -> Vec<f32> {
    l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, x, .. } => Some(*x),
            _ => None,
        })
        .collect()
}

/// Vertical rules on page 0: (x, width).
fn rules(l: &DocLayout) -> Vec<(f32, f32)> {
    l.pages[0]
        .items
        .iter()
        .filter_map(|i| if let Placed::Rule { x0, x1, border, .. } = i { (x0 == x1).then_some((*x0, border.width)) } else { None })
        .collect()
}

/// Which columns have text on page 0.
fn columns_with_text(d: &Document, l: &DocLayout) -> Vec<bool> {
    let xs = body_xs(l);
    column_lefts(d).iter().map(|c| xs.iter().any(|x| (x - c).abs() < 0.5)).collect()
}

/// The numbers of each column end `dist` left of that column.
fn assert_numbers_end(d: &Document, dist: f32, what: &str) {
    let l = lay(d);
    let edges = number_right_edges(&l);
    assert!(!edges.is_empty(), "{what}: no line numbers");
    let lefts = column_lefts(d);
    let used: Vec<f32> = lefts.iter().copied().zip(columns_with_text(d, &l)).filter(|(_, t)| *t).map(|(c, _)| c).collect();
    assert!(used.len() >= 2 || d.last_section.columns.count < 2, "{what}: the text reaches a second column");
    for e in &edges {
        assert!(used.iter().any(|c| (e - (c - dist)).abs() < 0.5), "{what}: a number ends at {e}, expected {dist} pt left of one of {used:?}");
    }
    for c in &used {
        assert!(edges.iter().any(|e| (e - (c - dist)).abs() < 0.5), "{what}: no number {dist} pt left of the column at {c}: {edges:?}");
    }
}

#[test]
fn auto_line_number_distance_is_18pt_in_one_column() {
    crate::test_fonts();
    assert_numbers_end(&doc(20, 1, 36.0, false, true), 18.0, "one column");
}

#[test]
fn auto_line_number_distance_is_9pt_in_columns() {
    crate::test_fonts();
    for (count, space) in [(2, 18.0), (2, 36.0), (2, 72.0), (3, 36.0)] {
        assert_numbers_end(&doc(160, count, space, count == 2, true), 9.0, &format!("{count} columns, space {space}"));
    }
}

#[test]
fn an_explicit_line_number_distance_is_kept_in_columns() {
    crate::test_fonts();
    let mut d = doc(120, 2, 36.0, true, true);
    d.last_section.line_numbers.as_mut().unwrap().distance = 18.0;
    assert_numbers_end(&d, 18.0, "2 columns, distance 18 pt");
}

#[test]
fn no_line_between_columns_when_only_the_first_has_text() {
    crate::test_fonts();
    for count in [2, 3] {
        let d = doc(10, count, 36.0, true, false);
        let l = lay(&d);
        assert_eq!(columns_with_text(&d, &l).iter().filter(|t| **t).count(), 1, "{count} columns: text only in the first");
        assert!(rules(&l).is_empty(), "{count} columns, all text in the first: Word draws no line: {:?}", rules(&l));
    }
}

#[test]
fn line_between_columns_only_before_columns_with_text() {
    crate::test_fonts();
    // Three columns: enough text to fill the first and start the second, the third stays empty.
    let d = (40..200)
        .map(|n| doc(n, 3, 18.0, true, false))
        .find(|d| {
            let l = lay(d);
            columns_with_text(d, &l) == [true, true, false] && body_xs(&l).iter().filter(|x| (**x - column_lefts(d)[1]).abs() < 0.5).count() >= 3
        })
        .expect("a length that leaves the third column empty");
    let l = lay(&d);
    let lefts = column_lefts(&d);
    let w = lefts[1] - lefts[0] - 18.0;
    let mid = lefts[0] + w + 9.0;
    let r = rules(&l);
    let [(x, width)] = r[..] else { panic!("one line, between columns 1 and 2 (Word): {r:?}") };
    assert!((x - mid).abs() < 0.5, "midway between columns 1 and 2: {x} vs {mid}");
    assert!((0.68..=0.76).contains(&width), "Word's line is 0.72 pt wide: {width}");
}

#[test]
fn line_between_full_columns_is_0_72pt() {
    crate::test_fonts();
    let d = doc(160, 2, 36.0, true, false);
    let l = lay(&d);
    assert_eq!(columns_with_text(&d, &l), [true, true]);
    let r = rules(&l);
    let [(x, width)] = r[..] else { panic!("one line: {r:?}") };
    let s = &d.last_section;
    assert!((x - (s.margin_left + s.gutter + s.text_width() / 2.0)).abs() < 0.5, "midway: {x}");
    assert!((0.68..=0.76).contains(&width), "Word's line is 0.72 pt wide: {width}");
}
