//! A missing Consolas or Comic Sans MS keeps Word's advance widths (1/1000 em, read from the
//! width arrays of Word's PDF output), whatever face draws it. Consolas is fixed-pitch (550 in
//! every style); Comic Sans MS italic takes the upright widths and bold italic the bold ones.

use wordcraft_fonts::word_widths::missing_font_advance;

fn eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-9,
        (None, None) => true,
        _ => false,
    }
}

#[test]
fn consolas_is_550_in_every_style() {
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        for c in ['a', 'W', 'i', ' ', '0', '@', '~', 'é', 'ü', '€', '\u{a0}'] {
            for family in ["Consolas", "consolas", " Consolas "] {
                let got = missing_font_advance(family, bold, italic, c);
                assert!(eq(got, Some(0.55)), "{family:?} b={bold} i={italic} {c:?}: {got:?}");
            }
        }
    }
}

#[test]
fn consolas_has_no_width_outside_winansi_or_for_controls() {
    for c in ['中', 'ж', '\u{7}', '\u{7f}'] {
        let got = missing_font_advance("Consolas", false, false, c);
        assert!(got.is_none(), "{c:?}: {got:?}");
    }
}

#[test]
fn comic_sans_values_by_style() {
    let cases: &[(bool, bool, char, Option<f64>)] = &[
        (false, false, ' ', Some(0.299)),
        (false, false, 'H', Some(0.768)),
        (false, false, 'a', Some(0.512)),
        (false, false, 'é', Some(0.548)),
        (false, false, 'ü', Some(0.520)),
        (true, false, ' ', Some(0.434)),
        (true, false, 'a', Some(0.556)),
        (true, false, 'é', Some(0.559)),
        (true, false, 'Q', Some(0.876)),
        // Italic = upright, bold italic = bold.
        (false, true, 'a', Some(0.512)),
        (false, true, 'ü', Some(0.520)),
        (true, true, 'a', Some(0.556)),
        (true, true, ' ', Some(0.434)),
        // Not in Word's PDFs: the substitute's advance.
        (true, false, 'ü', None),
        (false, false, '~', None),
        (false, false, '中', None),
    ];
    for &(bold, italic, c, want) in cases {
        for family in ["Comic Sans MS", "comic sans ms"] {
            let got = missing_font_advance(family, bold, italic, c);
            assert!(eq(got, want), "{family} b={bold} i={italic} {c:?}: {got:?}, want {want:?}");
        }
    }
}

#[test]
fn comic_sans_words_sum_to_words() {
    // (bold, italic, text, total advance in 1/1000 em from Word's widths)
    let cases: &[(bool, bool, &str, f64)] = &[
        (false, false, "Hello World", 5596.0),
        (false, false, "Page 12", 3471.0),
        (false, false, "Página 12", 4238.0),
        (true, false, "Hello World", 5742.0),
        (false, true, "Hello World", 5596.0),
    ];
    for &(bold, italic, text, want) in cases {
        let sum: f64 = text.chars().map(|c| missing_font_advance("Comic Sans MS", bold, italic, c).unwrap_or_else(|| panic!("{text}: {c:?}"))).sum();
        assert!((sum * 1000.0 - want).abs() < 1e-6, "{text} b={bold} i={italic}: {sum}, want {want}");
    }
}

#[test]
fn other_fonts_unchanged() {
    assert!(eq(missing_font_advance("Verdana", false, false, 'H'), Some(0.751)));
    assert!(missing_font_advance("Comic Sans", false, false, 'a').is_none(), "exact family names only");
    assert!(missing_font_advance("Consolas Bold", false, false, 'a').is_none(), "exact family names only");
}
