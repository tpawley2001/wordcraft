//! A missing SimSun (宋体) keeps Word's line (1.297 em), so Chinese text on the usual 15.6 pt line
//! grid (`w:docGrid w:type="lines" w:linePitch="312"`) takes one grid line, as in Word. East Asian
//! characters drawn from a run's missing East Asian font take that font's line; the run's Latin
//! characters keep the Latin font's. Measured in Word: 120 one-line paragraphs, Letter, 1-inch
//! margins (648 pt of body), single spacing, no space before or after.

use crate::{DocLayout, Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::section::{DocGrid, DocGridType};

/// 120 paragraphs `L001 <text>` with the Latin font `ascii`, East Asian font `ea`, at `size`, on a
/// line grid of `grid` points (none when `None`): (lines on page 1, pages).
fn lay(ascii: &str, ea: Option<&str>, size: f32, text: &str, grid: Option<f32>) -> (usize, usize) {
    let chr = wordcraft_doc::CharProps { font: Some(ascii.into()), font_ea: ea.map(Into::into), size: Some(size), ..Default::default() };
    let mut d = Document::new();
    d.body = (1..=120)
        .map(|i| {
            let mut p = wordcraft_doc::Paragraph::with_text(&format!("L{i:03} {text}"), chr.clone());
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
            p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            wordcraft_doc::para_block(p)
        })
        .collect();
    d.last_section.doc_grid = grid.map(|line_pitch| DocGrid { kind: DocGridType::Lines, line_pitch, char_space: 0 });
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    (first_page_lines(&l), l.pages.len())
}

fn first_page_lines(l: &DocLayout) -> usize {
    l.pages.first().map_or(0, |p| p.items.iter().map(|it| if let Placed::Lines { l0, l1, .. } = it { l1 - l0 } else { 0 }).sum())
}

/// SimSun is installed here: the layout keeps the installed font's own line.
fn simsun_installed() -> bool {
    crate::test_fonts();
    !wordcraft_fonts::word::resolve("SimSun", false, false).substituted
}

#[test]
fn simsun_without_a_grid_has_words_line() {
    if simsun_installed() {
        return;
    }
    assert_eq!(lay("SimSun", None, 10.5, "中文", None), (47, 3));
    assert_eq!(lay("SimSun", None, 12.0, "中文", None), (41, 3));
    assert_eq!(lay("SimSun", None, 14.0, "中文", None), (35, 4));
}

#[test]
fn simsun_on_a_15_6_pt_grid_takes_one_grid_line_up_to_12_pt() {
    if simsun_installed() {
        return;
    }
    assert_eq!(lay("SimSun", None, 12.0, "中文", Some(15.6)), (41, 3));
    assert_eq!(lay("SimSun", None, 14.0, "中文", Some(15.6)), (20, 6));
    assert_eq!(lay("宋体", None, 12.0, "中文", Some(15.6)), (41, 3));
}

#[test]
fn a_missing_east_asian_font_gives_cjk_text_its_line() {
    if simsun_installed() {
        return;
    }
    assert_eq!(lay("Times New Roman", Some("宋体"), 12.0, "中文", Some(15.6)), (41, 3));
    assert_eq!(lay("Times New Roman", Some("SimSun"), 12.0, "中文", None), (41, 3));
    assert_eq!(lay("Times New Roman", Some("SimSun"), 11.5, "中文", Some(15.6)), (41, 3));
}

#[test]
fn latin_text_keeps_the_latin_fonts_line() {
    // Word: Times New Roman's own line (13.8 pt at 12 pt, 46 lines a page). The pinned test fonts
    // may draw Times New Roman in a substitute, so compare with the Latin font alone.
    crate::test_fonts();
    for grid in [None, Some(15.6)] {
        assert_eq!(lay("Times New Roman", Some("SimSun"), 12.0, "abc", grid), lay("Times New Roman", None, 12.0, "abc", grid), "{grid:?}");
    }
}

#[test]
fn nsimsun_and_simhei_have_simsuns_line() {
    crate::test_fonts();
    for font in ["NSimSun", "SimHei"] {
        if !wordcraft_fonts::word::resolve(font, false, false).substituted {
            continue;
        }
        assert_eq!(lay(font, None, 12.0, "中文", None), (41, 3), "{font}");
    }
}

#[test]
fn guard_latin_text_on_a_grid_is_unchanged() {
    // Times New Roman text keeps its own line: a 12 pt line in Times New Roman (13.8 pt in Word)
    // or its substitute takes whole grid lines of its own height.
    crate::test_fonts();
    let (first, pages) = lay("Times New Roman", None, 12.0, "abc", Some(15.6));
    assert!(first == 41 || first == 20, "{first}");
    assert_eq!(pages, 120_usize.div_ceil(first));
    // Word: 41 lines with the real Times New Roman.
    if !wordcraft_fonts::word::resolve("Times New Roman", false, false).substituted {
        assert_eq!((first, pages), (41, 3));
    }
}
