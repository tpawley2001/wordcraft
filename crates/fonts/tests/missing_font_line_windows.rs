//! Missing Verdana, Georgia, Trebuchet MS, Century Gothic, Consolas, Garamond and Comic Sans MS keep
//! Word's line height (measured in Microsoft 365 Word's PDFs of single-spaced documents; the ascent
//! is the embedded font descriptor's `/Ascent`, the descent the rest of the line): Verdana 1.215 em
//! (10 pt lines 12.12 and 12.24 pt apart), Georgia 1.136 em, Trebuchet MS 1.161 em, Century Gothic
//! 1.226 em, Consolas 1.172 em, Garamond 1.125 em, Comic Sans MS 1.394 em (11 pt lines 15.36 and
//! 15.24 pt apart).

use wordcraft_fonts::word::{missing_font_line, resolve};

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

const WORD: [(&str, (f64, f64)); 7] = [
    ("Verdana", (1.005, 0.210)),
    ("Georgia", (0.917, 0.219)),
    ("Trebuchet MS", (0.939, 0.222)),
    ("Century Gothic", (1.006, 0.220)),
    ("Consolas", (0.743, 0.429)),
    ("Garamond", (0.862, 0.263)),
    ("Comic Sans MS", (1.102, 0.292)),
];

#[test]
fn missing_windows_fonts_take_words_line() {
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
fn guard_near_names_and_the_existing_entries_are_unchanged() {
    for name in ["Verdana Pro", "Georgia Pro", "Trebuchet", "Garamond Premier Pro", "Comic Sans", "Consolas Bold", "Century"] {
        let r = resolve(name, false, false);
        if r.substituted {
            assert_eq!(missing_font_line(name, &r), None, "{name}");
        }
    }
    for (family, expected) in [("Tahoma", (1.0, 0.207)), ("Lucida Console", (0.789, 0.211)), ("Palatino Linotype", (1.05, 0.30))] {
        let r = resolve(family, false, false);
        if r.substituted {
            let line = missing_font_line(family, &r).unwrap_or_else(|| panic!("{family}"));
            assert!(close(line, expected), "{family}: {line:?}");
        }
    }
}
