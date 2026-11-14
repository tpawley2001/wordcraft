//! Emphasis dots below the text (`w:em w:val="underDot"`, ECMA-376 17.3.2.12): the room Word leaves
//! under the line. Measured 2026-11-14 in Microsoft 365 Word's PDF (single spacing; per size a plain
//! paragraph, an underDot paragraph, a plain paragraph, all the same size): the underDot line is as
//! far from the line above as a plain line, and the next line is about 0.2875 x size further down
//! (Times New Roman 0.273-0.315, Calibri 0.285-0.311 x size at 8-36 pt; Word rounds to 0.12 pt):
//! the same amount Word adds above a line with dot, comma or circle marks.
//! The bundled test fonts differ from those, so the tests pin the rule against the plain line.
use crate::display::{Draw, page_display};
use crate::*;
use wordcraft_doc::para_block;
use wordcraft_doc::props::EmphasisMark;

const FONTS: [&str; 2] = ["Source Serif 4", "Inter"];

/// Baselines of "Alpha", "Plain", "Under" (underDot) and "After", in `font` at `size` pt.
fn baselines(font: &str, size: f32) -> [f32; 4] {
    crate::test_fonts();
    let mut d = Document::new();
    let plain = CharProps { font: Some(font.into()), size: Some(size), ..CharProps::default() };
    let under = CharProps { emphasis: Some(EmphasisMark::UnderDot), ..plain.clone() };
    d.body = vec![
        para_block(Paragraph::with_text("Alpha", plain.clone())),
        para_block(Paragraph::with_text("Plain", plain.clone())),
        para_block(Paragraph::with_text("Under", under)),
        para_block(Paragraph::with_text("After", plain)),
    ];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    let draws = page_display(&d, page, &Default::default());
    let base = |word: &str| {
        draws
            .iter()
            .find_map(|d| match d {
                Draw::Glyphs { glyphs, text, .. } if text.contains(word) => glyphs.first().map(|g| g.2),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{word} is drawn"))
    };
    [base("Alpha"), base("Plain"), base("Under"), base("After")]
}

#[test]
fn the_line_after_dots_below_is_0_2875_of_the_size_further_down() {
    for font in FONTS {
        for size in [8.0f32, 11.0, 14.0, 24.0, 36.0] {
            let [_, p, u, a] = baselines(font, size);
            let got = (a - u) - (u - p);
            let want = 0.2875 * size;
            assert!((got - want).abs() < 0.15, "{font} {size} pt: the next line is {got} pt further down than plain, want {want}");
        }
    }
}

#[test]
fn dots_below_leave_the_line_above_where_it_is() {
    for font in FONTS {
        for size in [8.0f32, 11.0, 36.0] {
            let [al, p, u, _] = baselines(font, size);
            assert!(((u - p) - (p - al)).abs() < 0.05, "{font} {size} pt: underDot line {} below the plain line, plain pitch {}", u - p, p - al);
        }
    }
}
