//! Lines holding Word's symbol fonts are as tall as Word makes them with the real fonts, even where
//! the fonts aren't installed and their characters are drawn from other faces. At 10 pt, single
//! spaced (the fonts Word embeds in its PDFs: hhea = OS/2 win metrics, 2048 units per em):
//! Webdings 10.000 pt, Wingdings 11.099, Wingdings 2 10.542, Wingdings 3 11.387, Symbol 12.251.
//! Word's PDF of a table of symbol codes (Arial Narrow digits followed by one character in each of
//! Webdings, Wingdings, Wingdings 2 and Symbol) has its 10 pt lines 12.25 pt apart.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::Pos;
use wordcraft_doc::props::LineSpacing;

const TOL: f32 = 0.06;

/// Three single-spaced paragraphs (no space before or after) holding `text`.
fn three(text: &str) -> Document {
    let mut d = Document::new();
    d.body = (0..3)
        .map(|_| {
            let mut p = wordcraft_doc::Paragraph::with_text(text, wordcraft_doc::CharProps::default());
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
            p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            wordcraft_doc::para_block(p)
        })
        .collect();
    d
}

fn pitches(d: &Document) -> Vec<f32> {
    let mut c = LayoutCache::new();
    let l = layout(d, &mut c, &LayoutOptions::default());
    let tops: Vec<f32> =
        l.pages.iter().flat_map(|p| &p.items).filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect();
    assert_eq!(tops.len(), 3, "{tops:?}");
    tops.windows(2).map(|w| w[1] - w[0]).collect()
}

fn missing(family: &str) -> bool {
    wordcraft_fonts::word::resolve(family, false, false).substituted
}

/// Three 10 pt paragraphs, each holding only `family` characters (Word's private-use codes).
fn one_font(family: &str) -> Document {
    let text = "\u{F021}\u{F022}\u{F023}\u{F024}\u{F025}";
    let mut d = three(text);
    for i in 0..3 {
        d.format_range(&Pos::body(i, 0), &Pos::body(i, text.len()), &|c| {
            c.font = Some(family.into());
            c.size = Some(10.0);
        })
        .unwrap();
    }
    d
}

#[test]
fn a_line_in_a_missing_symbol_font_is_as_tall_as_words() {
    for (family, want) in [("Webdings", 10.0), ("Wingdings", 11.099), ("Wingdings 2", 10.542), ("Wingdings 3", 11.387), ("Symbol", 12.251)] {
        if !missing(family) {
            continue;
        }
        for pitch in pitches(&one_font(family)) {
            assert!((pitch - want).abs() <= TOL, "{family}: {pitch} pt, Word {want} pt");
        }
    }
}

/// "0034" in Arial Narrow, then one character in each symbol font, as the rows of a symbol table.
#[test]
fn a_line_mixing_symbol_fonts_is_as_tall_as_its_tallest_font() {
    if ["Webdings", "Wingdings", "Wingdings 2", "Symbol"].iter().any(|f| !missing(f)) {
        return;
    }
    let row = "0034\u{F022}\u{F022}\u{F022}\u{F022}";
    let mut d = three(row);
    let sym = "\u{F022}".len();
    for i in 0..3 {
        let fonts = [(0, 4, "Arial Narrow"), (4, 4 + sym, "Webdings"), (4 + sym, 4 + 2 * sym, "Wingdings")];
        let fonts = fonts.into_iter().chain([(4 + 2 * sym, 4 + 3 * sym, "Wingdings 2"), (4 + 3 * sym, 4 + 4 * sym, "Symbol")]);
        for (a, b, family) in fonts {
            d.format_range(&Pos::body(i, a), &Pos::body(i, b), &|c| {
                c.font = Some(family.into());
                c.size = Some(10.0);
            })
            .unwrap();
        }
    }
    for pitch in pitches(&d) {
        assert!((pitch - 12.251).abs() <= TOL, "mixed symbol line: {pitch} pt, Word 12.25 pt");
    }
}
