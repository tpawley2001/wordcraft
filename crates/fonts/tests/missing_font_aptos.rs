//! Missing Aptos and Aptos Display keep Word's line height (measured in Word's PDFs: descriptor
//! ascent 939, descent -282; 11 pt Aptos at 1.1 lines is 14.76 pt apart = 1.221 em single), not
//! their substitutes'.

use wordcraft_fonts::word::{missing_font_line, resolve};

const APTOS: (f64, f64) = (0.939, 0.282);

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn missing_aptos_and_aptos_display_take_words_line() {
    for family in ["Aptos", "Aptos Display"] {
        for (bold, italic) in [(false, false), (true, false), (false, true)] {
            let mut r = resolve(family, bold, italic);
            r.substituted = true;
            let line = missing_font_line(family, &r).unwrap_or_else(|| panic!("a missing {family} has Word's line"));
            assert!(close(line, APTOS), "{family} b={bold} i={italic}: {line:?}");
        }
    }
}

#[test]
fn the_names_are_matched_trimmed_and_case_insensitively() {
    for (name, family) in [(" aptos ", "Aptos"), ("APTOS DISPLAY", "Aptos Display")] {
        let mut r = resolve(family, false, false);
        r.substituted = true;
        let line = missing_font_line(name, &r).unwrap_or_else(|| panic!("{name}"));
        assert!(close(line, APTOS), "{name}: {line:?}");
    }
}

#[test]
fn guard_an_installed_aptos_and_other_aptos_designs_take_no_borrowed_line() {
    let mut r = resolve("Aptos", false, false);
    r.substituted = false;
    assert_eq!(missing_font_line("Aptos", &r), None);
    for name in ["Aptos Serif", "Aptos Narrow", "Aptos Mono"] {
        let mut r = resolve(name, false, false);
        r.substituted = true;
        assert_eq!(missing_font_line(name, &r), None, "{name}");
    }
}
