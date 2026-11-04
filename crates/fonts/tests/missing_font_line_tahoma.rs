//! Missing Tahoma and Lucida Bright keep Word's line height for those fonts (measured in Word's PDFs),
//! not their substitutes': Tahoma ascent 1.000 em + descent 0.207 em, Lucida Bright 0.967 + 0.205 em.

use wordcraft_fonts::word::{missing_font_line, resolve};

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn missing_tahoma_and_lucida_bright_take_words_lines() {
    for (family, want) in [("Tahoma", (1.0, 0.207)), ("Lucida Bright", (0.967, 0.205))] {
        for (bold, italic) in [(false, false), (true, false), (false, true)] {
            let r = resolve(family, bold, italic);
            let line = missing_font_line(family, &r);
            if r.substituted {
                let line = line.unwrap_or_else(|| panic!("a missing {family} has Word's line"));
                assert!(close(line, want), "{family} b={bold} i={italic}: {line:?}");
            } else {
                assert_eq!(line, None, "{family} installed: its own metrics are used");
            }
        }
    }
}

#[test]
fn the_names_are_matched_trimmed_and_case_insensitively() {
    for (name, family, want) in [(" tahoma ", "Tahoma", (1.0, 0.207)), ("LUCIDA BRIGHT", "Lucida Bright", (0.967, 0.205))] {
        let r = resolve(family, false, false);
        if r.substituted {
            let line = missing_font_line(name, &r).unwrap_or_else(|| panic!("{name}"));
            assert!(close(line, want), "{name}: {line:?}");
        }
    }
}

#[test]
fn guard_an_installed_face_and_near_names_take_no_borrowed_line() {
    let mut r = resolve("Tahoma", false, false);
    r.substituted = false;
    assert_eq!(missing_font_line("Tahoma", &r), None);
    for name in ["Lucida Sans", "Lucida Fax", "Tahoma Bold Condensed"] {
        let r = resolve(name, false, false);
        assert_eq!(missing_font_line(name, &r), None, "{name}");
    }
}
