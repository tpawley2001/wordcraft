//! A missing MS Mincho (ＭＳ 明朝) or Shruti keeps Word's line: a 12 pt MS Mincho line with a ruby
//! (furigana) guide on an 18 pt line grid takes one grid line, and 10 pt Shruti lines are 16.8 pt
//! apart, as in Word.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::para::{InlineObject, RubyAlign};
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::section::{DocGrid, DocGridType};

/// Three one-line paragraphs in `font` at `size` (the middle one holding a ruby when `ruby`), on
/// a line grid of `grid` points (none when `None`): the height of each paragraph.
fn heights(font: &str, size: f32, text: &str, ruby: bool, grid: Option<f32>) -> Vec<f32> {
    let chr = wordcraft_doc::CharProps { font: Some(font.into()), font_ea: Some(font.into()), size: Some(size), ..Default::default() };
    let mut d = Document::new();
    d.body = (0..3)
        .map(|i| {
            let mut p = wordcraft_doc::Paragraph::with_text(text, chr.clone());
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
            p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            if ruby && i == 1 {
                let obj = InlineObject::Ruby {
                    base: "担当者".into(),
                    ruby: "ふり".into(),
                    align: RubyAlign::Center,
                    size: 5.0,
                    raise: 9.0,
                    base_size: 12.0,
                    lang: "ja-JP".into(),
                    props: Default::default(),
                };
                p.insert_object(text.find(' ').map_or(0, |o| o + 1), obj, &chr).unwrap();
            }
            wordcraft_doc::para_block(p)
        })
        .collect();
    d.settings.compat_mode = 14;
    d.last_section.doc_grid = grid.map(|line_pitch| DocGrid { kind: DocGridType::Lines, line_pitch, char_space: 0 });
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().unwrap();
    page.items
        .iter()
        .filter_map(|it| if let Placed::Lines { para, l0, l1, .. } = it { Some(para.lines[*l0..*l1].iter().map(|x| x.height).sum()) } else { None })
        .collect()
}

fn substituted(font: &str) -> bool {
    crate::test_fonts();
    wordcraft_fonts::word::resolve(font, false, false).substituted
}

#[test]
fn a_ruby_line_in_a_missing_ms_mincho_takes_one_18_pt_grid_line() {
    let h = heights("ＭＳ 明朝", 12.0, "前の文 後の文", true, Some(18.0));
    assert_eq!(h.len(), 3, "{h:?}");
    if !substituted("ＭＳ 明朝") {
        return;
    }
    for x in &h {
        assert!((x - 18.0).abs() < 0.01, "{h:?}");
    }
    assert!((h.iter().sum::<f32>() - 54.0).abs() < 0.01, "{h:?}");
}

#[test]
fn a_missing_ms_mincho_without_a_grid_has_words_line() {
    // Word: 12 pt lines 15.57 pt apart (1.297 em).
    let h = heights("ＭＳ 明朝", 12.0, "前の文 後の文", false, None);
    assert_eq!(h.len(), 3, "{h:?}");
    if !substituted("ＭＳ 明朝") {
        return;
    }
    for x in &h {
        assert!((x - 15.564).abs() < 0.05, "{h:?}");
    }
}

#[test]
fn a_missing_shruti_has_words_line() {
    let h = heights("Shruti", 10.0, "Name of the applicant", false, None);
    assert_eq!(h.len(), 3, "{h:?}");
    if !substituted("Shruti") {
        return;
    }
    for x in &h {
        assert!((x - 16.81).abs() < 0.05, "{h:?}");
    }
}
