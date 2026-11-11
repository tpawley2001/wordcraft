//! A missing Univers keeps Word's line and advance widths (measured in Word's PDFs of Univers
//! documents): `/Ascent` 989, single-spaced 12 pt lines 14.88 pt apart (1.24 em), and Univers'
//! own advances (lowercase b d g h n o p q u are 611/1000 em, wider than the substitute's).
//! Italic text takes the regular advances, bold italic the bold ones.

use wordcraft_fonts::word::{missing_font_advance, missing_font_line, resolve};

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn missing_univers_takes_words_line() {
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        let r = resolve("Univers", bold, italic);
        let line = missing_font_line("Univers", &r);
        if r.substituted {
            let line = line.expect("a missing Univers has Word's line");
            assert!(close(line, (0.989, 0.251)), "b{bold} i{italic}: {line:?}");
        } else {
            assert_eq!(line, None, "an installed Univers keeps its own line");
        }
    }
}

#[test]
fn univers_names_match_trimmed_and_case_insensitively() {
    for name in ["univers", "UNIVERS", "  Univers "] {
        assert_eq!(missing_font_advance(name, false, 'n'), Some(0.611), "{name}");
    }
}

#[test]
fn univers_regular_advances() {
    let expected = [
        (' ', 0.278),
        ('a', 0.556),
        ('b', 0.611),
        ('c', 0.5),
        ('k', 0.5),
        ('n', 0.611),
        ('t', 0.333),
        ('w', 0.833),
        ('z', 0.444),
        ('A', 0.722),
        ('V', 0.667),
        ('W', 1.0),
        ('0', 0.556),
        ('@', 1.015),
        ('\'', 0.278),
        ('"', 0.4),
        ('%', 0.889),
    ];
    for (ch, em) in expected {
        let got = missing_font_advance("Univers", false, ch);
        assert!(got.is_some_and(|g| (g - em).abs() < 1e-9), "{ch:?}: {got:?} (Word {em})");
    }
}

#[test]
fn univers_bold_advances() {
    let expected = [(' ', 0.278), ('k', 0.556), ('t', 0.389), ('w', 0.889), ('z', 0.5), ('V', 0.722), ('\'', 0.238), ('/', 0.444), ('#', 0.556)];
    for (ch, em) in expected {
        let got = missing_font_advance("Univers", true, ch);
        assert!(got.is_some_and(|g| (g - em).abs() < 1e-9), "{ch:?}: {got:?} (Word {em})");
    }
}

#[test]
fn guard_no_table_means_none() {
    // Characters Word's PDFs never showed, other families and other Univers faces have no table.
    assert_eq!(missing_font_advance("Univers", false, '{'), None);
    assert_eq!(missing_font_advance("Univers", false, 'é'), None);
    assert_eq!(missing_font_advance("Univers", true, '?'), None);
    for name in ["Arial", "Univers Condensed", "Univers 55", "Verdana", ""] {
        assert_eq!(missing_font_advance(name, false, 'n'), None, "{name}");
    }
}
