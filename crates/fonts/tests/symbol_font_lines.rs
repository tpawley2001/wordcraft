//! Word sizes a line holding a symbol font by that font's own metrics, even where the font isn't
//! installed. The values are those of the fonts Word embeds in its PDFs (hhea ascender/descender,
//! equal to OS/2 usWinAscent/usWinDescent, no line gap, 2048 units per em).

use wordcraft_fonts::word::{missing_font_line, resolve, symbol_font_metrics};

/// (family, ascent em, descent em, tolerance em).
const WORD: &[(&str, f64, f64, f64)] = &[
    ("Webdings", 1638.0 / 2048.0, 410.0 / 2048.0, 0.002),
    ("Wingdings 2", 1727.0 / 2048.0, 432.0 / 2048.0, 0.002),
    ("Wingdings 3", 1900.0 / 2048.0, 432.0 / 2048.0, 0.002),
    ("Wingdings", 1841.0 / 2048.0, 432.0 / 2048.0, 0.007),
    ("Symbol", 2059.0 / 2048.0, 450.0 / 2048.0, 0.007),
];

#[test]
fn symbol_fonts_have_words_line_metrics() {
    for &(family, asc, desc, tol) in WORD {
        let (a, d) = symbol_font_metrics(family).unwrap_or_else(|| panic!("{family} has Word's metrics"));
        assert!((a - asc).abs() <= tol && (d - desc).abs() <= tol, "{family}: ({a}, {d}) vs ({asc:.4}, {desc:.4})");
    }
}

#[test]
fn names_are_matched_trimmed_and_case_insensitively() {
    for (name, family) in [(" webdings ", "Webdings"), ("WINGDINGS 2", "Wingdings 2"), ("wingdings 3", "Wingdings 3")] {
        assert_eq!(symbol_font_metrics(name), symbol_font_metrics(family), "{name}");
    }
    assert_eq!(symbol_font_metrics("Wingdings 4"), None);
    assert_eq!(symbol_font_metrics("Webding"), None);
}

#[test]
fn a_missing_symbol_font_takes_words_line() {
    for &(family, asc, desc, tol) in WORD {
        let r = resolve(family, false, false);
        if !r.substituted {
            continue;
        }
        let (a, d) = missing_font_line(family, &r).unwrap_or_else(|| panic!("a missing {family} has Word's line"));
        assert!((a - asc).abs() <= tol && (d - desc).abs() <= tol, "{family}: ({a}, {d})");
    }
}
