//! Text in a WMF or EMF picture set in a symbol font (Symbol, Wingdings, Webdings) that is not
//! installed draws its Unicode symbols, as body text does, not the font's own codes as empty boxes.

use wordcraft_metafile::{HAlign, Text, VAlign};

use crate::{metafile_drawn_text, metafile_text_run};

fn run(font: &str, text: &str) -> Text {
    Text {
        text: text.to_string(),
        x: 0.0,
        y: 0.0,
        size: 12.0,
        cell: false,
        font: font.to_string(),
        bold: false,
        italic: false,
        color: [0, 0, 0, 255],
        angle: 0.0,
        halign: HAlign::Left,
        valign: VAlign::Baseline,
        dx: Vec::new(),
    }
}

#[test]
fn wingdings_codes_draw_as_unicode_symbols() {
    crate::test_fonts();
    // The bullet of a slide picture: stored as U+F0A7, or as the 8-bit code U+00A7.
    for stored in ["\u{F0A7}", "\u{A7}"] {
        assert_eq!(metafile_drawn_text(&run("Wingdings", stored)), "▪", "{stored:?}");
    }
    assert_eq!(metafile_drawn_text(&run("wingdings", "\u{F0A7}\u{F0A7}")), "▪▪");
}

#[test]
fn symbol_font_letters_draw_as_greek() {
    crate::test_fonts();
    assert_eq!(metafile_drawn_text(&run("Symbol", "a")), "α");
    assert_eq!(metafile_drawn_text(&run("Symbol", "\u{F061}")), "α");
}

#[test]
fn other_fonts_and_unmapped_codes_are_unchanged() {
    crate::test_fonts();
    let plain = "a\u{A7}\u{F0A7}";
    assert_eq!(metafile_drawn_text(&run("Arial", plain)), plain);
    // Outside the symbol font's code ranges nothing is mapped.
    assert_eq!(metafile_drawn_text(&run("Wingdings", "\u{2022}")), "\u{2022}");
    assert_eq!(metafile_drawn_text(&run("Wingdings", "")), "");
}

#[test]
fn one_advance_per_character_survives_the_mapping() {
    crate::test_fonts();
    let t = run("Wingdings", "\u{A7}\u{A7}\u{A7}");
    let drawn = metafile_drawn_text(&t);
    assert_eq!(drawn.chars().count(), t.text.chars().count());
    // The shaped glyphs' clusters index the drawn text, whose UTF-8 length differs from the stored text's.
    let (_, shaped) = metafile_text_run(&t);
    assert!(!shaped.is_empty());
    for g in &shaped {
        assert!(g.cluster < drawn.len() && drawn.is_char_boundary(g.cluster), "cluster {} in {drawn:?}", g.cluster);
    }
}
