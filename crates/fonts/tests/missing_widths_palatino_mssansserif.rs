//! A missing Palatino Linotype or Microsoft Sans Serif keeps Word's
//! advance widths (1/1000 em, read from the width arrays of Word's PDF output), whatever face
//! draws it. A style without a table of its own takes the regular widths; characters not measured
//! keep the substitute's.

use wordcraft_fonts::word_widths::missing_font_advance;

fn eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-9,
        (None, None) => true,
        _ => false,
    }
}

#[test]
fn values_by_style() {
    let cases: &[(&str, bool, bool, char, Option<f64>)] = &[
        ("Palatino Linotype", false, false, ' ', Some(0.250)),
        ("Palatino Linotype", false, false, '@', Some(0.683)),
        ("Palatino Linotype", false, false, 'a', Some(0.500)),
        ("Palatino Linotype", false, false, '®', Some(0.747)),
        ("Palatino Linotype", true, false, ' ', Some(0.250)),
        ("Palatino Linotype", true, false, 'E', Some(0.606)),
        ("Palatino Linotype", true, false, 'c', Some(0.444)),
        ("Palatino Linotype", true, false, '®', Some(0.747)),
        ("Palatino Linotype", false, true, ' ', Some(0.250)),
        ("Palatino Linotype", false, true, 'G', Some(0.722)),
        ("Palatino Linotype", false, true, 'e', Some(0.389)),
        ("Palatino Linotype", false, true, '®', Some(0.747)),
        ("Palatino Linotype", true, true, ' ', Some(0.250)),
        ("Palatino Linotype", true, true, 'O', Some(0.833)),
        ("Palatino Linotype", true, true, 'k', Some(0.556)),
        ("Palatino Linotype", true, true, 'z', Some(0.500)),
        // Palatino Linotype: not in Word's PDFs, the substitute's advance
        ("Palatino Linotype", false, false, 'ƒ', None),
        ("Microsoft Sans Serif", false, false, ' ', Some(0.266)),
        ("Microsoft Sans Serif", false, false, 'B', Some(0.667)),
        ("Microsoft Sans Serif", false, false, 'g', Some(0.556)),
        ("Microsoft Sans Serif", false, false, 'z', Some(0.500)),
        // Microsoft Sans Serif: no Bold table, the regular widths
        ("Microsoft Sans Serif", true, false, '2', Some(0.556)),
        // Microsoft Sans Serif: no Italic table, the regular widths
        ("Microsoft Sans Serif", false, true, '2', Some(0.556)),
        // Microsoft Sans Serif: no BoldItalic table, the regular widths
        ("Microsoft Sans Serif", true, true, '2', Some(0.556)),
        // Microsoft Sans Serif: not in Word's PDFs, the substitute's advance
        ("Microsoft Sans Serif", false, false, 'A', None),
        // The characters where Word's widths differ from the drawn metric twin's (P052 / Nimbus Sans).
        ("Palatino Linotype", false, false, '/', Some(0.320)),
        ("Palatino Linotype", false, false, '\'', Some(0.208)),
        ("Palatino Linotype", false, false, '=', Some(0.500)),
        ("Palatino Linotype", false, true, 'V', Some(0.658)),
        ("Palatino Linotype", true, false, 'K', Some(0.770)),
        ("Microsoft Sans Serif", false, false, 'i', Some(0.228)),
        ("Microsoft Sans Serif", false, false, 'l', Some(0.228)),
        // Microsoft Sans Serif: no Bold / Italic table, the regular widths
        ("Microsoft Sans Serif", true, true, 'i', Some(0.228)),
    ];
    for &(family, bold, italic, c, want) in cases {
        let got = missing_font_advance(family, bold, italic, c);
        assert!(eq(got, want), "{family:?} b={bold} i={italic} {c:?}: got {got:?}, want {want:?}");
    }
}

#[test]
fn family_names_match_case_insensitively_and_trimmed() {
    for (a, b) in [("Palatino Linotype", " palatino linotype "), ("Microsoft Sans Serif", "MICROSOFT SANS SERIF")] {
        let want = missing_font_advance(a, false, false, 'a');
        assert!(want.is_some(), "{a:?} 'a' has a width");
        assert!(eq(want, missing_font_advance(b, false, false, 'a')), "{a:?} vs {b:?}");
    }
}

#[test]
fn other_families_are_not_these() {
    // Palatino and Book Antiqua keep their metric-twin substitute's widths; MS Sans Serif is another font.
    for f in ["Palatino", "Book Antiqua", "MS Sans Serif", "Palatino Linotype Bold"] {
        assert!(missing_font_advance(f, false, false, '/').is_none(), "{f:?}");
    }
}
