//! A font the document asks for but that isn't installed keeps Word's line height for that font
//! (measured in Word's PDF), not its substitute's: 10 pt Lucida Console lines are 10.0 pt apart in
//! Word (ascent 0.789 em), while the Courier-New-like substitute is 1.133 em.

use wordcraft_fonts::word::{missing_font_line, resolve, symbol_line};

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn a_missing_lucida_console_takes_words_one_em_line() {
    let r = resolve("Lucida Console", false, false);
    let line = missing_font_line("Lucida Console", &r);
    if r.substituted {
        let line = line.expect("a missing Lucida Console has Word's line");
        assert!(close(line, (0.789, 0.211)), "{line:?}");
        assert!((line.0 + line.1 - 1.0).abs() < 1e-3);
    } else {
        // Installed: its own metrics are used.
        assert_eq!(line, None);
    }
}

#[test]
fn the_family_name_is_matched_trimmed_and_case_insensitively() {
    let r = resolve("Lucida Console", true, true);
    if r.substituted {
        for name in ["lucida console", " LUCIDA CONSOLE ", "Lucida Console"] {
            let line = missing_font_line(name, &r).unwrap_or_else(|| panic!("{name}"));
            assert!(close(line, (0.789, 0.211)), "{name}: {line:?}");
        }
    }
}

#[test]
fn an_installed_face_never_takes_a_borrowed_line() {
    // Same family, but resolved as installed: no override, whatever the table says.
    let mut r = resolve("Lucida Console", false, false);
    r.substituted = false;
    assert_eq!(missing_font_line("Lucida Console", &r), None);
}

#[test]
fn guard_other_families_keep_their_own_line_and_symbols_keep_theirs() {
    for name in ["Arial", "Courier New", "Times New Roman", "Some Font Nobody Has"] {
        let r = resolve(name, false, false);
        assert_eq!(missing_font_line(name, &r), None, "{name}");
    }
    for name in ["Symbol", "Wingdings"] {
        let r = resolve(name, false, false);
        assert_eq!(missing_font_line(name, &r), symbol_line(name, &r), "{name}");
    }
}
