//! Missing Segoe UI, Perpetua, Gill Sans MT and Book Antiqua keep Word's single line (measured in
//! Microsoft 365 Word's PDFs of synthetic documents, 12 and 24 pt, single spacing, no grid), not
//! their substitutes' (Carlito 1.2207 em, P052 1.2 em).

use wordcraft_fonts::word::{missing_font_line, resolve};

// (family, Word's (ascent, descent) in em, Word's mean 12 pt and 24 pt line pitch in pt)
const WORD: &[(&str, (f64, f64), f64, f64)] = &[
    ("Segoe UI", (1.079, 0.251), 15.963, 31.925),
    ("Perpetua", (0.820, 0.326), 13.748, 27.506),
    ("Gill Sans MT", (0.929, 0.231), 13.923, 27.834),
    ("Book Antiqua", (0.964, 0.279), 14.915, 29.819),
];

#[test]
fn missing_fonts_take_words_line() {
    for &(family, want, p12, p24) in WORD {
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let r = resolve(family, bold, italic);
            let line = missing_font_line(family, &r);
            if !r.substituted {
                assert_eq!(line, None, "an installed {family} keeps its own line");
                continue;
            }
            let (a, d) = line.unwrap_or_else(|| panic!("a missing {family} has Word's line"));
            assert!((a - want.0).abs() < 1e-3 && (d - want.1).abs() < 1e-3, "{family} b{bold} i{italic}: {a} {d}");
            assert!(((a + d) * 12.0 - p12).abs() < 0.05, "{family} 12 pt pitch {}", (a + d) * 12.0);
            assert!(((a + d) * 24.0 - p24).abs() < 0.08, "{family} 24 pt pitch {}", (a + d) * 24.0);
        }
    }
}

#[test]
fn book_antiqua_is_not_palatinos_line() {
    let r = resolve("Book Antiqua", false, false);
    if !r.substituted {
        return;
    }
    assert_ne!(missing_font_line("Book Antiqua", &r), missing_font_line("Palatino Linotype", &r));
}

#[test]
fn names_match_trimmed_and_case_insensitively_but_not_other_faces() {
    for &(family, want, _, _) in WORD {
        let r = resolve(family, false, false);
        if !r.substituted {
            continue;
        }
        for name in [family.to_lowercase(), family.to_uppercase(), format!("  {family} ")] {
            assert_eq!(missing_font_line(&name, &r), Some(want), "{name:?}");
        }
    }
    let r = resolve("Segoe UI", false, false);
    if r.substituted {
        for name in ["Gill Sans", "Gill Sans MT Condensed"] {
            assert_ne!(missing_font_line(name, &r), Some((1.079, 0.251)), "{name}");
            assert_ne!(missing_font_line(name, &r), Some((0.929, 0.231)), "{name}");
        }
    }
}
