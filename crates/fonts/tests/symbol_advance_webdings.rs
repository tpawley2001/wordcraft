//! A missing Webdings keeps the width Word gives each character (measured in Word's own PDF
//! output), as a missing Symbol or Wingdings keeps theirs.

use wordcraft_fonts::word::symbol_advance;

fn near(got: Option<f64>, want: f64) -> bool {
    got.is_some_and(|g| (g - want).abs() < 0.0015)
}

#[test]
fn webdings_codes_take_words_widths() {
    for (code, want) in [
        (0x20, 0.500),
        (0x21, 1.000),
        (0x25, 0.990),
        (0x57, 0.537),
        (0x58, 0.537),
        (0x61, 1.000),
        (0x6D, 0.559),
        (0x8D, 1.221),
        (0xBC, 0.672),
        (0xBD, 0.672),
        (0xBF, 0.843),
        (0xC0, 0.517),
        (0xC5, 0.428),
        (0xC6, 0.606),
        (0xCB, 0.663),
        (0xFB, 1.292),
        (0xFF, 1.000),
    ] {
        assert!(near(symbol_advance("Webdings", code), want), "Webdings {code:#X}: {:?}, want {want}", symbol_advance("Webdings", code));
    }
}

#[test]
fn webdings_name_matches_like_the_other_symbol_fonts() {
    assert!(near(symbol_advance("WEBDINGS", 0x20), 0.5));
    assert!(near(symbol_advance(" webdings ; Arial", 0x6D), 0.559));
    assert!(near(symbol_advance("Arial; Webdings", 0x6D), 0.559), "first known name wins");
}

#[test]
fn webdings_unknown_codes_have_no_width() {
    assert_eq!(symbol_advance("Webdings", 0x7F), None, "the missing-character box keeps its own width");
    assert_eq!(symbol_advance("Webdings", 0x10), None);
    assert_eq!(symbol_advance("Webdings 2", 0x6C), None);
    // The other tables are unchanged.
    assert!(near(symbol_advance("Symbol", 0x79), 0.686));
    assert!(near(symbol_advance("Wingdings", 0xA8), 0.8911));
}
