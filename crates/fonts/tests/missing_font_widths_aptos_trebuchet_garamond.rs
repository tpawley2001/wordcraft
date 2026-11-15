//! A missing Aptos, Aptos Display, Trebuchet MS or Garamond keeps Word's advance widths (1/1000 em, read from the
//! width arrays of Word's PDF output), whatever face draws it.

use wordcraft_fonts::word_widths::missing_font_advance;

fn eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-9,
        (None, None) => true,
        _ => false,
    }
}

#[test]
fn aptos_trebuchet_and_garamond_table_values() {
    let cases: &[(&str, bool, bool, char, Option<f64>)] = &[
        ("Aptos", false, false, 'H', Some(0.707)),
        ("aptos", false, false, ' ', Some(0.203)),
        ("Aptos", true, false, 'W', Some(0.94)),
        ("Aptos", true, true, 'a', Some(0.557)),
        // Aptos Display has its own widths.
        ("Aptos Display", false, false, ' ', Some(0.193)),
        ("Aptos Display", true, false, 'T', Some(0.499)),
        ("Trebuchet MS", false, false, 'H', Some(0.654)),
        ("trebuchet ms", false, false, ' ', Some(0.301)),
        ("Trebuchet MS", true, false, 'B', Some(0.595)),
        ("Trebuchet MS", false, true, 'e', Some(0.537)),
        ("Garamond", true, false, 'C', Some(0.677)),
        ("GARAMOND", false, false, 'm', Some(0.771)),
        // Not in the Garamond regular table: keep the stand-in's width.
        ("Garamond", false, false, 'z', None),
        ("Trebuchet MS", false, false, '中', None),
    ];
    for &(family, bold, italic, c, want) in cases {
        let got = missing_font_advance(family, bold, italic, c);
        assert!(eq(got, want), "{family} b={bold} i={italic} {c:?}: {got:?}, want {want:?}");
    }
}

#[test]
fn aptos_trebuchet_and_garamond_words_sum() {
    // (family, bold, italic, text, total advance in 1/1000 em from the table)
    let cases: &[(&str, bool, bool, &str, f64)] = &[
        ("Aptos", false, false, "Meeting notes", 6083.0),
        ("Aptos", true, false, "Total 2024", 4620.0),
        ("Aptos", false, true, "Draft", 2151.0),
        ("Aptos Display", true, false, "Chapter 3", 4181.0),
        ("Trebuchet MS", false, false, "Parity test", 4689.0),
        ("Trebuchet MS", true, false, "Annual report", 6507.0),
        ("Trebuchet MS", false, true, "Press", 2307.0),
        ("Garamond", true, false, "Chapter 1", 4032.0),
        ("Garamond", false, false, "Gardam", 3187.0),
    ];
    for &(family, bold, italic, text, want) in cases {
        let sum: f64 = text.chars().map(|c| missing_font_advance(family, bold, italic, c).unwrap_or_else(|| panic!("{family} has {c:?}"))).sum();
        assert!((sum * 1000.0 - want).abs() < 1e-6, "{family} {text:?}: {}", sum * 1000.0);
    }
}
