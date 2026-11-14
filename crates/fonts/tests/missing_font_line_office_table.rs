//! Twenty missing Office fonts keep Word's single line (Microsoft 365 Word's PDFs of synthetic documents,
//! 12 and 24 pt, single spacing, no grid) instead of their substitute's (Carlito, 1.2207 em).

use wordcraft_fonts::word::{missing_font_line, resolve};

// (family, Word's (ascent, descent) in em, Word's mean 12 pt and 24 pt line pitch in pt)
const WORD: &[(&str, (f64, f64), f64, f64)] = &[
    ("Algerian", (1.096, 0.224), 15.843, 31.685),
    ("Baskerville Old Face", (0.881, 0.260), 13.694, 27.375),
    ("Bauhaus 93", (1.226, 0.234), 17.523, 35.055),
    ("Bodoni MT", (0.917, 0.282), 14.392, 28.783),
    ("Constantia", (0.951, 0.270), 14.654, 29.306),
    ("Cooper Black", (0.918, 0.229), 13.759, 27.528),
    ("Copperplate Gothic Bold", (0.885, 0.228), 13.355, 26.710),
    ("DokChampa", (1.076, 0.269), 16.137, 32.274),
    ("Ebrima", (1.111, 0.247), 16.290, 32.590),
    ("Goudy Old Style", (0.966, 0.234), 14.403, 28.805),
    ("Haettenschweiler", (0.946, 0.121), 12.799, 25.608),
    ("Lucida Sans", (0.967, 0.211), 14.130, 28.270),
    ("Lucida Sans Unicode", (1.097, 0.440), 18.439, 36.891),
    ("Mangal", (1.241, 0.439), 20.163, 40.316),
    ("Nirmala UI", (1.079, 0.251), 15.963, 31.925),
    ("Rockwell", (0.946, 0.229), 14.097, 28.183),
    ("Segoe UI Light", (1.079, 0.251), 15.963, 31.925),
    ("Segoe UI Semibold", (1.079, 0.251), 15.963, 31.925),
    ("Sylfaen", (1.006, 0.311), 15.799, 31.608),
    ("Tw Cen MT", (0.856, 0.233), 13.072, 26.132),
];

#[test]
fn missing_office_fonts_take_words_line() {
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

#[test]
fn segoe_ui_faces_share_segoe_uis_line() {
    let r = resolve("Segoe UI Light", false, false);
    if !r.substituted {
        return;
    }
    for name in ["Segoe UI Light", "Segoe UI Semibold"] {
        assert_eq!(missing_font_line(name, &r), missing_font_line("Segoe UI", &r), "{name}");
    }
}
