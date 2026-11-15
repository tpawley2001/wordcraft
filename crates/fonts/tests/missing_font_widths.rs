//! A missing Verdana, Tahoma, Century Gothic or SimSun keeps Word's advance widths (1/1000 em,
//! read from the width arrays of Word's PDF output), whatever face draws it.

use wordcraft_fonts::word_widths::missing_font_advance;

fn eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-9,
        (None, None) => true,
        _ => false,
    }
}

#[test]
fn table_values_by_family_and_style() {
    let cases: &[(&str, bool, bool, char, Option<f64>)] = &[
        ("Verdana", false, false, 'H', Some(0.751)),
        ("verdana", true, false, 'W', Some(1.128)),
        // No Tahoma Italic table: the regular widths.
        ("Tahoma", false, true, 'P', Some(0.551)),
        ("Verdana", false, false, ' ', Some(0.352)),
        ("SimSun", false, false, 'a', Some(0.5)),
        ("Arial", false, false, 'H', None),
        ("Verdana", false, false, '中', None),
    ];
    for &(family, bold, italic, c, want) in cases {
        let got = missing_font_advance(family, bold, italic, c);
        assert!(eq(got, want), "{family} b={bold} i={italic} {c:?}: {got:?}, want {want:?}");
    }
}

#[test]
fn run_widths_sum_to_words() {
    // (family, bold, italic, text, total advance in 1/1000 em from the table)
    let cases: &[(&str, bool, bool, &str, f64)] = &[
        ("Verdana", false, false, "Hello World", 5768.0),
        ("Verdana", true, false, "Hello World", 6567.0),
        ("Tahoma", false, false, "Parity test", 4450.0),
        ("Century Gothic", false, false, "Page 12", 3983.0),
        ("SimSun", false, false, "abc 123", 3500.0),
        ("Verdana", false, true, "Quick", 2802.0),
    ];
    for &(family, bold, italic, text, want) in cases {
        let sum: f64 = text.chars().map(|c| missing_font_advance(family, bold, italic, c).unwrap_or_else(|| panic!("{family} has {c:?}"))).sum();
        assert!((sum * 1000.0 - want).abs() < 1e-6, "{family} {text:?}: {}", sum * 1000.0);
    }
}
