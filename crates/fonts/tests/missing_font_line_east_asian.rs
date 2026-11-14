//! Missing East Asian and Thai fonts keep Word's single line (Microsoft 365 Word's PDFs of synthetic
//! documents, Latin text, 12 and 24 pt, single spacing, no grid) instead of their substitute's
//! (Carlito, 1.2207 em; Leelawadee UI: Noto Sans Thai, 1.511 em).

use wordcraft_fonts::word::{missing_font_line, resolve};

// (family, Word's (ascent, descent) in em, Word's mean 12 pt and 24 pt line pitch in pt)
const WORD: &[(&str, (f64, f64), f64, f64)] = &[
    ("Batang", (1.011, 0.290), 15.613, 31.226),
    ("Gulim", (1.011, 0.290), 15.613, 31.226),
    ("Leelawadee UI", (1.081, 0.249), 15.962, 31.925),
    ("Malgun Gothic", (1.291, 0.439), 20.752, 41.515),
    ("Meiryo", (1.286, 0.664), 23.402, 46.807),
    ("Microsoft JhengHei", (1.276, 0.454), 20.752, 41.515),
    ("Microsoft YaHei", (1.256, 0.460), 20.578, 41.179),
    ("PMingLiU", (0.951, 0.350), 15.613, 31.226),
    ("Yu Gothic", (1.181, 0.492), 20.076, 40.152),
];

#[test]
fn missing_east_asian_and_thai_fonts_take_words_line() {
    for &(family, want, p12, p24) in WORD {
        let r = resolve(family, false, false);
        let line = missing_font_line(family, &r);
        if !r.substituted {
            assert_eq!(line, None, "an installed {family} keeps its own line");
            continue;
        }
        let (a, d) = line.unwrap_or_else(|| panic!("a missing {family} has Word's line"));
        assert!((a - want.0).abs() < 1e-3 && (d - want.1).abs() < 1e-3, "{family}: {a} {d}");
        assert!(((a + d) * 12.0 - p12).abs() < 0.05, "{family} 12 pt pitch {}", (a + d) * 12.0);
        assert!(((a + d) * 24.0 - p24).abs() < 0.08, "{family} 24 pt pitch {}", (a + d) * 24.0);
    }
}
