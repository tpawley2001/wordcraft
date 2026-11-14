//! A missing MS Mincho, MS Gothic (and their proportional and UI faces) or Shruti keeps Word's line
//! height. Measured in Word's PDFs: the Japanese faces give `/Ascent` 859, `/Descent` -141 and
//! 10, 10.5, 12 and 14 pt single-spaced lines 12.96, 13.64, 15.57 and 18.16 pt apart (1.297 em),
//! SimSun's line. Shruti gives `/Ascent` 1021, `/Descent` -660 and 10 pt lines 16.80 pt apart
//! (1.681 em), the first baseline 1.021 em below the top of the body.

use wordcraft_fonts::word::{missing_font_line, missing_font_line_leading, resolve};

const JAPANESE_LINE: (f64, f64) = (1.007, 0.290);
const SHRUTI_LINE: (f64, f64) = (1.021, 0.660);

const JAPANESE: [&str; 11] = [
    "MS Mincho",
    "ＭＳ 明朝",
    "MS PMincho",
    "ＭＳ Ｐ明朝",
    "MS Gothic",
    "ＭＳ ゴシック",
    "MS PGothic",
    "ＭＳ Ｐゴシック",
    "MS UI Gothic",
    "MS MINCHO",
    " ms gothic ",
];

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

/// Checks `name`'s line (with and without "no leading") against `want`, and `size` pt lines
/// against `pitch`, when the font is missing; an installed font keeps its own line.
fn check(name: &str, want: (f64, f64), size: f64, pitch: f64) {
    let r = resolve(name, false, false);
    let line = missing_font_line(name, &r);
    if r.substituted {
        let line = line.unwrap_or_else(|| panic!("a missing {name} has Word's line"));
        assert!(close(line, want), "{name}: {line:?}");
        assert!(((line.0 + line.1) * size - pitch).abs() < 0.01, "{name}: {line:?}");
        for leading in [true, false] {
            let l = missing_font_line_leading(name, &r, leading).unwrap_or_else(|| panic!("{name} leading {leading}"));
            assert!(close(l, want), "{name} leading {leading}: {l:?}");
        }
    } else {
        assert_eq!(line, None, "an installed {name} keeps its own line");
        assert_eq!(missing_font_line_leading(name, &r, false), None, "{name}");
    }
}

#[test]
fn a_missing_japanese_font_takes_words_line() {
    for name in JAPANESE {
        // 12 pt lines are 15.56 pt apart.
        check(name, JAPANESE_LINE, 12.0, 15.564);
    }
}

#[test]
fn a_missing_shruti_takes_words_line() {
    for name in ["Shruti", "shruti"] {
        // 10 pt lines are 16.80 pt apart.
        check(name, SHRUTI_LINE, 10.0, 16.81);
    }
}

#[test]
fn guard_unmeasured_fonts_are_unchanged() {
    for name in ["Yu Mincho", "Latha"] {
        let r = resolve(name, false, false);
        if r.substituted {
            assert_eq!(missing_font_line(name, &r), None, "{name}");
        }
    }
}
