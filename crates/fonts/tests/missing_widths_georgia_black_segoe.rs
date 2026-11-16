//! A missing Georgia, Arial Black, Segoe UI, Bookman Old Style, Perpetua or Gill Sans MT keeps
//! Word's advance widths (1/1000 em, read from the width arrays of Word's PDF output), whatever face
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
        ("Georgia", false, false, ' ', Some(0.241)),
        ("Georgia", false, false, 'I', Some(0.390)),
        ("Georgia", false, false, 'd', Some(0.574)),
        ("Georgia", false, false, 'ö', Some(0.539)),
        ("Georgia", true, false, ' ', Some(0.254)),
        ("Georgia", true, false, 'L', Some(0.686)),
        ("Georgia", true, false, 'c', Some(0.531)),
        ("Georgia", true, false, 'y', Some(0.562)),
        ("Georgia", false, true, ' ', Some(0.241)),
        ("Georgia", false, true, 'M', Some(0.927)),
        ("Georgia", false, true, 'j', Some(0.291)),
        ("Georgia", false, true, 'ö', Some(0.537)),
        ("Georgia", true, true, ' ', Some(0.254)),
        ("Georgia", true, true, 'P', Some(0.706)),
        ("Georgia", true, true, 'k', Some(0.641)),
        ("Georgia", true, true, 'ö', Some(0.636)),
        // Georgia: not in Word's PDFs, the substitute's advance
        ("Georgia", false, false, 'q', None),
        ("Arial Black", false, false, ' ', Some(0.333)),
        ("Arial Black", false, false, 'D', Some(0.778)),
        ("Arial Black", false, false, 'c', Some(0.667)),
        ("Arial Black", false, false, 'é', Some(0.667)),
        // Arial Black: no Bold table, the regular widths
        ("Arial Black", true, false, '-', Some(0.333)),
        // Arial Black: no Italic table, the regular widths
        ("Arial Black", false, true, '-', Some(0.333)),
        // Arial Black: no BoldItalic table, the regular widths
        ("Arial Black", true, true, '-', Some(0.333)),
        // Arial Black: not in Word's PDFs, the substitute's advance
        ("Arial Black", false, false, 'ƒ', None),
        ("Segoe UI", false, false, ' ', Some(0.274)),
        ("Segoe UI", false, false, 'C', Some(0.619)),
        ("Segoe UI", false, false, 'c', Some(0.462)),
        ("Segoe UI", false, false, '§', Some(0.448)),
        ("Segoe UI", true, false, ' ', Some(0.276)),
        ("Segoe UI", true, false, 'R', Some(0.653)),
        ("Segoe UI", true, false, 'i', Some(0.284)),
        ("Segoe UI", true, false, 'u', Some(0.605)),
        ("Segoe UI", false, true, ' ', Some(0.275)),
        ("Segoe UI", false, true, ']', Some(0.304)),
        ("Segoe UI", false, true, 'm', Some(0.858)),
        ("Segoe UI", false, true, 'y', Some(0.484)),
        // Segoe UI: no BoldItalic table, the regular widths
        ("Segoe UI", true, true, ',', Some(0.217)),
        // Segoe UI: not in Word's PDFs, the substitute's advance
        ("Segoe UI", false, false, 'H', None),
        ("Bookman Old Style", false, false, ' ', Some(0.320)),
        ("Bookman Old Style", false, false, 'E', Some(0.720)),
        ("Bookman Old Style", false, false, 'g', Some(0.540)),
        ("Bookman Old Style", false, false, 'å', Some(0.580)),
        ("Bookman Old Style", true, false, ' ', Some(0.340)),
        ("Bookman Old Style", true, false, 'T', Some(0.700)),
        ("Bookman Old Style", true, false, 'j', Some(0.340)),
        ("Bookman Old Style", true, false, 'y', Some(0.620)),
        ("Bookman Old Style", false, true, ' ', Some(0.300)),
        ("Bookman Old Style", false, true, 'I', Some(0.320)),
        ("Bookman Old Style", false, true, 'o', Some(0.540)),
        ("Bookman Old Style", false, true, 'u', Some(0.620)),
        ("Bookman Old Style", true, true, ' ', Some(0.340)),
        ("Bookman Old Style", true, true, 'A', Some(0.720)),
        ("Bookman Old Style", true, true, 'M', Some(0.860)),
        ("Bookman Old Style", true, true, 'Y', Some(0.660)),
        // Bookman Old Style: not in Word's PDFs, the substitute's advance
        ("Bookman Old Style", false, false, 'C', None),
        ("Perpetua", false, false, ' ', Some(0.225)),
        ("Perpetua", false, false, 'T', Some(0.563)),
        ("Perpetua", false, false, 'k', Some(0.438)),
        ("Perpetua", false, false, '»', Some(0.417)),
        ("Perpetua", true, false, ' ', Some(0.225)),
        ("Perpetua", true, false, '[', Some(0.365)),
        ("Perpetua", true, false, 'n', Some(0.563)),
        ("Perpetua", true, false, 'y', Some(0.500)),
        // Perpetua: no Italic table, the regular widths
        ("Perpetua", false, true, 'G', Some(0.656)),
        // Perpetua: no BoldItalic table, the regular widths
        ("Perpetua", true, true, 'G', Some(0.656)),
        // Perpetua: not in Word's PDFs, the substitute's advance
        ("Perpetua", false, false, 'A', None),
        ("Gill Sans MT", false, false, ' ', Some(0.278)),
        ("Gill Sans MT", false, false, 'M', Some(0.781)),
        ("Gill Sans MT", false, false, 'h', Some(0.500)),
        ("Gill Sans MT", false, false, 'ø', Some(0.552)),
        ("Gill Sans MT", true, false, ' ', Some(0.278)),
        ("Gill Sans MT", true, false, 'N', Some(0.844)),
        ("Gill Sans MT", true, false, 'i', Some(0.271)),
        ("Gill Sans MT", true, false, 'y', Some(0.510)),
        // Gill Sans MT: no Italic table, the regular widths
        ("Gill Sans MT", false, true, ':', Some(0.219)),
        // Gill Sans MT: no BoldItalic table, the regular widths
        ("Gill Sans MT", true, true, ':', Some(0.219)),
        // Gill Sans MT: not in Word's PDFs, the substitute's advance
        ("Gill Sans MT", false, false, 'X', None),
    ];
    for &(family, bold, italic, c, want) in cases {
        let got = missing_font_advance(family, bold, italic, c);
        assert!(eq(got, want), "{family:?} b={bold} i={italic} {c:?}: got {got:?}, want {want:?}");
    }
}

#[test]
fn family_names_match_case_insensitively_and_trimmed() {
    for (a, b) in [
        ("Georgia", " georgia "),
        ("Arial Black", "ARIAL BLACK"),
        ("Segoe UI", "segoe ui"),
        ("Bookman Old Style", "bookman old style"),
        ("Perpetua", "PERPETUA"),
        ("Gill Sans MT", " gill sans mt "),
    ] {
        let want = missing_font_advance(a, false, false, 'a');
        assert!(want.is_some(), "{a:?} 'a' has a width");
        assert!(eq(want, missing_font_advance(b, false, false, 'a')), "{a:?} vs {b:?}");
    }
}

#[test]
fn other_segoe_ui_families_are_not_segoe_ui() {
    // Segoe UI Light and Semibold are faces of their own: no table here.
    for f in ["Segoe UI Light", "Segoe UI Semibold", "Segoe UI Symbol"] {
        assert!(missing_font_advance(f, false, false, 'a').is_none(), "{f:?}");
    }
}
