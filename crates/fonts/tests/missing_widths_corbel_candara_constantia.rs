//! A missing Corbel, Candara, Constantia or Tw Cen MT keeps Word's advance widths (1/1000 em,
//! read from the width arrays of Word's PDF output), whatever face draws it. A style without a
//! table of its own takes the regular widths; characters not measured keep the substitute's.

use wordcraft_fonts::word_widths::missing_font_advance;

fn eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-9,
        (None, None) => true,
        _ => false,
    }
}

#[test]
fn corbel_candara_constantia_tw_cen_values_by_style() {
    let cases: &[(&str, bool, bool, char, Option<f64>)] = &[
        ("Corbel", false, false, ' ', Some(0.200)),
        ("Corbel", false, false, 'L', Some(0.521)),
        ("Corbel", false, false, 'f', Some(318.0 / 1000.0)),
        ("Corbel", false, false, '}', Some(0.300)),
        ("Corbel", true, false, ' ', Some(0.206)),
        ("Corbel", true, false, 'P', Some(0.601)),
        ("Corbel", true, false, 'k', Some(0.516)),
        ("Corbel", true, false, 'y', Some(0.514)),
        ("Corbel", false, true, ' ', Some(0.200)),
        ("Corbel", false, true, '[', Some(0.309)),
        ("Corbel", false, true, 'l', Some(0.240)),
        ("Corbel", false, true, 'y', Some(0.470)),
        ("Candara", false, false, ' ', Some(0.217)),
        ("Candara", false, false, 'D', Some(0.643)),
        ("Candara", false, false, 'h', Some(0.542)),
        ("Candara", false, false, 'y', Some(0.465)),
        ("Candara", true, false, ' ', Some(0.217)),
        ("Candara", true, false, 'H', Some(0.653)),
        ("Candara", true, false, 'R', Some(0.596)),
        ("Candara", true, false, 'Y', Some(0.552)),
        ("Constantia", false, false, ' ', Some(0.251)),
        ("Constantia", false, false, 'M', Some(0.905)),
        ("Constantia", false, false, 'h', Some(0.580)),
        ("Constantia", false, false, '|', Some(0.348)),
        ("Constantia", true, false, ' ', Some(0.239)),
        ("Constantia", true, false, ']', Some(0.355)),
        ("Constantia", true, false, 'i', Some(0.332)),
        ("Constantia", true, false, 't', Some(0.390)),
        ("Constantia", false, true, ' ', Some(0.236)),
        ("Constantia", false, true, '[', Some(0.345)),
        ("Constantia", false, true, 'l', Some(0.270)),
        ("Constantia", false, true, 'y', Some(0.480)),
        ("Tw Cen MT", true, false, ' ', Some(0.266)),
        ("Tw Cen MT", true, false, 'P', Some(0.531)),
        ("Tw Cen MT", true, false, 'h', Some(0.531)),
        ("Tw Cen MT", true, false, '|', Some(0.588)),
        // no bold italic table: the regular widths
        ("Corbel", true, true, '/', Some(0.275)),
        ("Candara", true, true, '4', Some(0.533)),
        ("Constantia", true, true, '1', Some(0.313)),
        // no Candara italic table: the regular widths
        ("Candara", false, true, '9', Some(0.548)),
        // not in Word's PDFs: the substitute's advance
        ("Corbel", false, false, 'ƒ', None),
        ("Candara", true, false, 'B', None),
        ("Constantia", false, false, 'B', None),
        // Tw Cen MT: only bold measured; regular and italic keep the substitute's advance
        ("Tw Cen MT", false, false, 'A', None),
        ("Tw Cen MT", false, true, 'A', None),
    ];
    for &(family, bold, italic, c, want) in cases {
        let got = missing_font_advance(family, bold, italic, c);
        assert!(eq(got, want), "{family:?} b={bold} i={italic} {c:?}: got {got:?}, want {want:?}");
    }
}

#[test]
fn family_names_match_case_insensitively_and_trimmed() {
    for (a, b) in [("Corbel", " corbel "), ("Candara", "CANDARA"), ("Constantia", "constantia"), ("Tw Cen MT", " tw cen mt ")] {
        for c in [' ', 'N', 'R'] {
            let want = missing_font_advance(a, true, false, c);
            assert!(want.is_some(), "{a:?} bold {c:?} has a width");
            assert!(eq(want, missing_font_advance(b, true, false, c)), "{a:?} vs {b:?} {c:?}");
        }
    }
}

#[test]
fn a_corbel_word_sums_word_advances() {
    // "Corbel" in Corbel Regular at 1 em.
    let w: f64 = "Corbel".chars().map(|c| missing_font_advance("Corbel", false, false, c).unwrap_or(f64::NAN)).sum();
    assert!((w - 2.72).abs() < 1e-9, "{w}");
}
