//! A missing Century keeps Word's line (measured in Microsoft 365 Word's PDFs of synthetic
//! documents): `/Ascent` 986, single-spaced 10, 12, 20 and 36 pt lines 1.2023 em apart, so the
//! descent is the rest of the line. Its C059 substitute has Century's widths but a 1.42 em line.

use wordcraft_fonts::word::{missing_font_line, resolve};

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn missing_century_takes_words_line() {
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        let r = resolve("Century", bold, italic);
        let line = missing_font_line("Century", &r);
        if r.substituted {
            let line = line.expect("a missing Century has Word's line");
            assert!(close(line, (0.986, 0.216)), "b{bold} i{italic}: {line:?}");
            // Word's 12 pt lines are 14.40 to 14.52 pt apart (mean 14.43).
            let pitch = (line.0 + line.1) * 12.0;
            assert!((pitch - 14.43).abs() < 0.05, "{pitch}");
        } else {
            assert_eq!(line, None, "an installed Century keeps its own line");
        }
    }
}

#[test]
fn century_name_matches_trimmed_and_case_insensitively_but_not_other_centuries() {
    let r = resolve("Century", false, false);
    if !r.substituted {
        return;
    }
    for name in ["century", "CENTURY", "  Century "] {
        assert!(missing_font_line(name, &r).is_some_and(|l| close(l, (0.986, 0.216))), "{name}");
    }
    for name in ["Century Schoolbook", "New Century Schoolbook"] {
        assert!(!missing_font_line(name, &r).is_some_and(|l| close(l, (0.986, 0.216))), "{name}");
    }
}
