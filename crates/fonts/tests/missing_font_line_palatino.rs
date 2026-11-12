//! A missing font named "Palatino" keeps Word's line height. Word draws it in Palatino Linotype
//! (measured in Word's PDF of a document whose body font is "Palatino": 11 pt single-spaced lines
//! are 14.65 to 14.8 pt apart, 1.35 em, as for Palatino Linotype: 12 pt lines 16.20 pt apart with
//! the first baseline 1.05 em below the top of the line). The substitute's own line is 1.20 em.

use wordcraft_fonts::word::{missing_font_line, resolve};

const WORD_LINE: (f64, f64) = (1.05, 0.30);

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn a_missing_palatino_takes_palatino_linotypes_line() {
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        let r = resolve("Palatino", bold, italic);
        let line = missing_font_line("Palatino", &r);
        if r.substituted {
            let line = line.expect("a missing Palatino has Word's line");
            assert!(close(line, WORD_LINE), "b{bold} i{italic}: {line:?}");
            // 12 pt lines are 16.2 pt apart.
            assert!(((line.0 + line.1) * 12.0 - 16.2).abs() < 0.02, "{line:?}");
        } else {
            assert_eq!(line, None, "an installed Palatino keeps its own line");
        }
    }
}

#[test]
fn the_name_matches_trimmed_and_case_insensitively() {
    for name in ["palatino", "PALATINO", "  Palatino  "] {
        let r = resolve(name, false, false);
        if r.substituted {
            let line = missing_font_line(name, &r).unwrap_or_else(|| panic!("{name:?} has Word's line"));
            assert!(close(line, WORD_LINE), "{name:?}: {line:?}");
        }
    }
}

#[test]
fn palatino_and_palatino_linotype_have_the_same_line() {
    let a = resolve("Palatino", false, false);
    let b = resolve("Palatino Linotype", false, false);
    if a.substituted && b.substituted {
        assert_eq!(missing_font_line("Palatino", &a), missing_font_line("Palatino Linotype", &b));
    }
}

#[test]
fn guard_unmeasured_relatives_are_unchanged() {
    for name in ["Book Antiqua", "Palatino LT Std", "Palatino nova", "Palatin"] {
        let r = resolve(name, false, false);
        if r.substituted {
            assert_eq!(missing_font_line(name, &r), None, "{name}");
        }
    }
}
