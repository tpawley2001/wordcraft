//! Missing Palatino Linotype and Franklin Gothic Book keep Word's line height (measured in Word's
//! PDFs of single-spaced documents): 12 pt Palatino Linotype lines are 16.20 pt apart (1.35 em,
//! first baseline 1.05 em below the top of the line), Franklin Gothic Book lines 13.56 pt
//! (1.13 em, ascent 0.917 em). Their substitutes give 1.20 em and 1.22 em.

use wordcraft_fonts::word::{missing_font_line, resolve};

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

const WORD: [(&str, (f64, f64)); 2] = [("Palatino Linotype", (1.05, 0.30)), ("Franklin Gothic Book", (0.917, 0.213))];

#[test]
fn missing_palatino_and_franklin_gothic_take_words_line() {
    for (family, expected) in WORD {
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let r = resolve(family, bold, italic);
            let line = missing_font_line(family, &r);
            if r.substituted {
                let line = line.unwrap_or_else(|| panic!("a missing {family} has Word's line"));
                assert!(close(line, expected), "{family} b{bold} i{italic}: {line:?}");
            } else {
                assert_eq!(line, None, "an installed {family} keeps its own line");
            }
        }
    }
}

#[test]
fn the_new_names_match_trimmed_and_case_insensitively() {
    for (family, expected) in WORD {
        let r = resolve(family, false, false);
        if !r.substituted {
            continue;
        }
        for name in [family.to_lowercase(), family.to_uppercase(), format!("  {family} ")] {
            let line = missing_font_line(&name, &r).unwrap_or_else(|| panic!("{name}"));
            assert!(close(line, expected), "{name}: {line:?}");
        }
    }
}

#[test]
fn guard_lucida_console_and_near_names_are_unchanged() {
    let r = resolve("Lucida Console", false, false);
    if r.substituted {
        assert!(close(missing_font_line("Lucida Console", &r).unwrap(), (0.789, 0.211)));
    }
    // Unmeasured Franklin Gothic faces stay outside the table; Book Antiqua has its own line.
    for (name, expected) in [("Franklin Gothic Medium", None), ("Franklin Gothic Demi", None), ("Book Antiqua", Some((0.964, 0.279)))] {
        let r = resolve(name, false, false);
        if r.substituted {
            assert_eq!(missing_font_line(name, &r), expected, "{name}");
        }
    }
}
