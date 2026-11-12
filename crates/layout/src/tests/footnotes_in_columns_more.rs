//! Footnotes under their own columns before compatibility mode 15 (`compatibilityMode`, ECMA-376
//! Part 1 §17.15.3.4), beyond `tests/footnotes_in_columns.rs`: three columns, unequal columns
//! (`w:cols` with `w:col` widths, §17.6.4), and a note continued from the last column onto the
//! next page.

use crate::*;
use wordcraft_doc::Pos;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::section::Columns;

const PARAS: usize = 160;

fn doc(columns: Columns) -> Document {
    let text: Vec<String> = (1..=PARAS).map(|i| format!("Line {i}")).collect();
    let mut d = Document::from_text(&text.join("\n"));
    d.settings.compat_mode = 14;
    d.last_section = SectionProps { columns, ..SectionProps::default() };
    d
}

fn equal(count: u32) -> Columns {
    Columns { count, space: 36.0, ..Columns::default() }
}

fn lay(d: &Document) -> DocLayout {
    crate::test_fonts();
    layout(d, &mut LayoutCache::new(), &LayoutOptions::default())
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0
}

/// A note of `rows` one-line paragraphs referred to at the end of body paragraph `para`.
fn note(d: &mut Document, para: usize, rows: &[String]) -> u32 {
    let blocks = rows.iter().map(|r| wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(r, Default::default()))).collect();
    let id = d.add_part(wordcraft_doc::PartKind::Footnote, blocks);
    let at = format!("Line {}", para + 1).len();
    d.insert_object(
        &Pos::body(para, at),
        InlineObject::NoteRef { kind: wordcraft_doc::para::NoteKind::Footnote, id, custom: String::new() },
        &Default::default(),
    )
    .unwrap();
    id
}

fn words(n: usize) -> Vec<String> {
    vec![(1..=n).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ")]
}

fn rows(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("Row {i}")).collect()
}

fn body_in(l: &DocLayout, page: usize, x: f32) -> Vec<u32> {
    l.pages[page]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, x: ix, l0: 0, .. } if path.depth() == 0 && near(*ix, x) => Some(path.0[0]),
            _ => None,
        })
        .collect()
}

/// Every placed piece of note `id`: (page, left edge, lines).
fn pieces(l: &DocLayout, id: u32) -> Vec<(usize, f32, usize)> {
    let mut out = Vec::new();
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Lines { story: StoryRef::Part(s), x, l0, l1, .. } = it
                && *s == id
            {
                out.push((pi, *x, l1 - l0));
            }
        }
    }
    out
}

#[test]
fn three_columns_a_note_in_the_middle_one() {
    // 468pt of text: three 132pt columns at 72, 240 and 408.
    let xs = [72.0, 240.0, 408.0];
    let full = body_in(&lay(&doc(equal(3))), 0, xs[0]).len();
    assert!(full > 20);
    let mut d = doc(equal(3));
    let id = note(&mut d, full + 3, &words(20));
    let l = lay(&d);
    let p = pieces(&l, id);
    assert!(!p.is_empty() && p.iter().all(|p| p.0 == 0 && near(p.1, xs[1])), "under the middle column: {p:?}");
    assert_eq!(body_in(&l, 0, xs[0]).len(), full, "column 1 has no notes");
    assert_eq!(body_in(&l, 0, xs[2]).len(), full, "column 3 has no notes");
    assert!(body_in(&l, 0, xs[1]).len() < full, "column 2 ends above its note");
}

#[test]
fn unequal_columns_wrap_each_note_to_its_own_column() {
    // A 300pt column at 72 and a 132pt one at 408.
    let cols = Columns { count: 2, space: 36.0, widths: vec![(300.0, 36.0), (132.0, 0.0)], ..Columns::default() };
    let full = body_in(&lay(&doc(cols.clone())), 0, 72.0).len();
    let mut d = doc(cols);
    let wide = note(&mut d, 3, &words(30));
    let narrow = note(&mut d, full + 3, &words(30));
    let l = lay(&d);
    let (pw, pn) = (pieces(&l, wide), pieces(&l, narrow));
    assert!(!pw.is_empty() && pw.iter().all(|p| p.0 == 0 && near(p.1, 72.0)), "{pw:?}");
    assert!(!pn.is_empty() && pn.iter().all(|p| p.0 == 0 && near(p.1, 408.0)), "{pn:?}");
    let lines = |p: &[(usize, f32, usize)]| p.iter().map(|p| p.2).sum::<usize>();
    assert!(lines(&pn) > lines(&pw), "the same note takes more lines in the narrow column: {} vs {}", lines(&pn), lines(&pw));
}

#[test]
fn a_note_continued_from_the_last_column_goes_on_in_the_first_column_of_the_next_page() {
    let full = body_in(&lay(&doc(equal(2))), 0, 72.0).len();
    let mut d = doc(equal(2));
    // Referred to on column 2's sixth line, with more rows than fit below it.
    let id = note(&mut d, full + 5, &rows(full));
    let l = lay(&d);
    let p = pieces(&l, id);
    assert_eq!(body_in(&l, 0, 72.0).len(), full, "column 1 has no notes");
    assert!(p.iter().any(|p| p.0 == 0) && p.iter().filter(|p| p.0 == 0).all(|p| near(p.1, 324.0)), "starts under column 2: {p:?}");
    let rest: Vec<_> = p.iter().filter(|p| p.0 == 1).collect();
    assert!(!rest.is_empty() && rest.iter().all(|p| near(p.1, 72.0)), "goes on under column 1 of page 2: {p:?}");
    let col1 = body_in(&l, 1, 72.0);
    let last = body_in(&l, 0, 324.0).last().copied();
    assert_eq!(col1.first().copied(), last.map(|b| b + 1), "page 2 goes on with the next line");
    assert!(col1.len() < full && body_in(&l, 1, 324.0).len() == full, "only column 1 of page 2 ends above the rest");
}
