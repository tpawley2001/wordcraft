//! Tab leaders in a font that isn't installed but has Word's character widths (Aptos, Verdana...)
//! are spaced by Word's width of the leader character, not the stand-in's: Microsoft 365 Word's PDF
//! of a table of contents in 12 pt Aptos draws 106 dots across 368.9 pt, every 3.48 pt (Aptos's
//! '.' is 0.286 em: 12 pt is 100 pixels at 600 dpi, 28.6 rounds to 29 pixels = 3.48 pt).

use wordcraft_doc::props::{TabAlign, TabLeader, TabStop};
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

use crate::display::{self, Draw, MissingWidths};
use crate::{LayoutCache, LayoutOptions, layout};

/// "Contents<tab>12" in `font` at `size` pt with a right tab at 400 pt and a dotted leader: the
/// leader's glyph positions and the missing font it is spaced for.
fn leader(font: &str, size: f32) -> (Vec<f32>, Option<MissingWidths>) {
    crate::test_fonts();
    let chr = CharProps { font: Some(font.into()), size: Some(size), ..Default::default() };
    let mut p = Paragraph::with_text("Contents\t12", chr);
    p.props.tabs = Some(vec![TabStop { pos: 400.0, align: TabAlign::Right, leader: TabLeader::Dot }]);
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let items = display::page_display(&d, &l.pages[0], &display::DisplayOptions::default());
    items
        .iter()
        .find_map(|i| match i {
            Draw::Glyphs { text, glyphs, missing_widths, .. } if text.is_empty() => {
                Some((glyphs.iter().map(|g| g.1).collect(), missing_widths.clone()))
            }
            _ => None,
        })
        .expect("a leader")
}

/// Whether `family` is missing here (drawn from a substitute); prints why a test is skipped.
fn missing(family: &str) -> bool {
    crate::test_fonts();
    let r = wordcraft_fonts::word::resolve(family, false, false);
    let missing = r.substituted && r.face.upem > 0.0;
    if !missing {
        eprintln!("skipped: {family} is installed here (or nothing resolves for it)");
    }
    missing
}

fn step(xs: &[f32]) -> f32 {
    assert!(xs.len() > 2, "a leader of several dots: {xs:?}");
    let s = xs[1] - xs[0];
    assert!(xs.windows(2).all(|w| (w[1] - w[0] - s).abs() < 1e-3), "evenly spaced: {xs:?}");
    s
}

#[test]
fn leader_dots_in_a_missing_font_take_words_width() {
    if !missing("Aptos") {
        return;
    }
    // 12 pt: 0.286 em of 100 px = 28.6 -> 29 px = 3.48 pt (Word's PDF); 11 pt: 92 px, 26.3 -> 26 px = 3.12 pt.
    for (size, want) in [(12.0, 3.48), (11.0, 3.12)] {
        let (xs, m) = leader("Aptos", size);
        let got = step(&xs);
        assert!((got - want).abs() < 1e-3, "Aptos {size} pt: dots every {got} pt, Word {want}");
        assert_eq!(m, Some(MissingWidths { family: "Aptos".into(), bold: false, italic: false }), "the PDF declares the same widths");
    }
}

#[test]
fn leader_dots_in_an_installed_font_keep_their_own_width() {
    let (xs, m) = leader("Source Sans 3", 12.0);
    step(&xs);
    assert_eq!(m, None);
}
