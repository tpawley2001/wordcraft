//! A missing Yu Mincho, Yu Gothic, MS PGothic, MS Mincho or MS Gothic keeps Word's Latin
//! advance widths (1/1000 em, read from the width arrays of Word's PDF output), whatever face
//! draws it. A style without a table of its own takes the regular widths; characters not measured
//! keep the substitute's.

use wordcraft_fonts::word_widths::missing_font_advance;

fn eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-9,
        (None, None) => true,
        _ => false,
    }
}

#[test]
fn values_by_style() {
    let cases: &[(&str, bool, bool, char, Option<f64>)] = &[
        ("Yu Mincho", false, false, ' ', Some(0.260)),
        ("Yu Mincho", false, false, 'F', Some(0.603)),
        ("Yu Mincho", false, false, 'i', Some(0.287)),
        ("Yu Mincho", false, false, 'v', Some(0.485)),
        // Yu Mincho: no Bold table, the regular widths
        ("Yu Mincho", true, false, '3', Some(0.542)),
        // Yu Mincho: no Italic table, the regular widths
        ("Yu Mincho", false, true, '3', Some(0.542)),
        // Yu Mincho: no BoldItalic table, the regular widths
        ("Yu Mincho", true, true, '3', Some(0.542)),
        // Yu Mincho: not in Word's PDFs, the substitute's advance
        ("Yu Mincho", false, false, 'C', None),
        ("Yu Gothic", false, false, ' ', Some(0.285)),
        ("Yu Gothic", false, false, 'A', Some(0.649)),
        ("Yu Gothic", false, false, 'h', Some(0.580)),
        ("Yu Gothic", false, false, '}', Some(0.386)),
        ("Yu Gothic", true, false, ' ', Some(0.285)),
        ("Yu Gothic", true, false, 'e', Some(0.588)),
        ("Yu Gothic", true, false, 'r', Some(0.392)),
        ("Yu Gothic", true, false, '}', Some(0.404)),
        // Yu Gothic: no Italic table, the regular widths
        ("Yu Gothic", false, true, '.', Some(0.258)),
        // Yu Gothic: no BoldItalic table, the regular widths
        ("Yu Gothic", true, true, '.', Some(0.258)),
        // Yu Gothic: not in Word's PDFs, the substitute's advance
        ("Yu Gothic", false, false, 'B', None),
        ("MS PGothic", false, false, ' ', Some(0.305)),
        ("MS PGothic", false, false, 'S', Some(0.602)),
        ("MS PGothic", false, false, 'h', Some(0.500)),
        ("MS PGothic", false, false, 't', Some(0.352)),
        // MS PGothic: no Bold table, the regular widths
        ("MS PGothic", true, false, 'U', Some(0.641)),
        // MS PGothic: no Italic table, the regular widths
        ("MS PGothic", false, true, 'U', Some(0.641)),
        // MS PGothic: no BoldItalic table, the regular widths
        ("MS PGothic", true, true, 'U', Some(0.641)),
        // MS PGothic: not in Word's PDFs, the substitute's advance
        ("MS PGothic", false, false, 'B', None),
    ];
    for &(family, bold, italic, c, want) in cases {
        let got = missing_font_advance(family, bold, italic, c);
        assert!(eq(got, want), "{family:?} b={bold} i={italic} {c:?}: got {got:?}, want {want:?}");
    }
}

#[test]
fn family_names_match_case_insensitively_and_trimmed() {
    for (a, b) in [
        ("Yu Mincho", " yu mincho "),
        ("Yu Gothic", "YU GOTHIC"),
        ("MS PGothic", "ms pgothic"),
        ("Yu Mincho", "游明朝"),
        ("Yu Gothic", "游ゴシック"),
        ("MS PGothic", "ＭＳ Ｐゴシック"),
    ] {
        let want = missing_font_advance(a, false, false, 'a');
        assert!(want.is_some(), "{a:?} 'a' has a width");
        assert!(eq(want, missing_font_advance(b, false, false, 'a')), "{a:?} vs {b:?}");
    }
}

#[test]
fn ms_mincho_and_ms_gothic_are_half_width_for_ascii() {
    // Fixed-pitch Japanese faces: every printable ASCII character is half an em in every style (Word's PDFs: all
    // 70 measured characters 500/1000). Other characters keep the substitute's advance (many are full-width there).
    for f in ["MS Mincho", "ms gothic", "ＭＳ 明朝", "ＭＳ ゴシック"] {
        for c in [' ', '!', '0', 'A', 'W', 'a', 'm', 'w', '~'] {
            for (b, i) in [(false, false), (true, false), (false, true), (true, true)] {
                let got = missing_font_advance(f, b, i, c);
                assert!(eq(got, Some(0.5)), "{f:?} {c:?} b={b} i={i}: got {got:?}");
            }
        }
        for c in ['é', '×', '°', '—', '漢'] {
            assert!(missing_font_advance(f, false, false, c).is_none(), "{f:?} {c:?}");
        }
    }
}

#[test]
fn other_japanese_faces_are_not_these() {
    // Proportional MS PMincho, Yu Gothic Light / Medium and Meiryo have no table here.
    for f in ["MS PMincho", "Yu Gothic Light", "Yu Gothic Medium", "Meiryo", "MS UI Gothic"] {
        assert!(missing_font_advance(f, false, false, 'a').is_none(), "{f:?}");
    }
}
