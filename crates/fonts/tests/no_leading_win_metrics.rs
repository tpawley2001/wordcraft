//! The compatibility option "Don't add leading (extra space) between rows of text" removes only
//! the leading a font has on top of its Windows cell (OS/2 `usWinAscent + usWinDescent`), measured
//! in Word's PDFs of synthetic single-spaced documents (12 pt, without / with the option): Times
//! New Roman 13.80 / 13.29, Arial 13.81 / 13.41, Courier New 13.60 / 13.60, Calibri 14.65 / 14.65
//! and Consolas 14.04 / 14.04 (their line gap is inside the Windows cell). A missing Century
//! Gothic goes from 1.226 em to 1.1916 em (10, 20 and 36 pt lines 11.91, 23.83 and 42.90 pt
//! apart), a missing Lucida Bright from its line to 1.1386 em (11.38, 22.77, 41.00); the removed
//! leading is above the ascent. The other missing-font lines do not change.

use wordcraft_fonts::word::{hhea_line_leading, missing_font_line, missing_font_line_leading, resolve, win_line_leading};

/// One font's hhea ascender, descender and line gap, its OS/2 usWinAscent and usWinDescent (all
/// per 2048 em), and Word's 12 pt pitch without and with the option.
struct Font {
    name: &'static str,
    hhea: (i16, i16, i16),
    win: (u16, u16),
    plain: f64,
    option: f64,
}

const FONTS: [Font; 5] = [
    Font { name: "Times New Roman", hhea: (1825, -443, 87), win: (1825, 443), plain: 13.80, option: 13.29 },
    Font { name: "Arial", hhea: (1854, -434, 67), win: (1854, 434), plain: 13.81, option: 13.41 },
    Font { name: "Courier New", hhea: (1705, -615, 0), win: (1705, 615), plain: 13.60, option: 13.60 },
    Font { name: "Calibri", hhea: (1536, -512, 452), win: (1950, 550), plain: 14.65, option: 14.65 },
    Font { name: "Consolas", hhea: (1521, -527, 350), win: (1884, 514), plain: 14.04, option: 14.04 },
];

fn line(f: &Font, leading: bool) -> (f64, f64) {
    win_line_leading(f.hhea.0, f.hhea.1, f.hhea.2, f.win.0, f.win.1, leading)
}

fn pitch(line: (f64, f64), size: f64) -> f64 {
    (line.0 + line.1) / 2048.0 * size
}

#[test]
fn the_option_gives_words_pitch_for_each_font() {
    for f in &FONTS {
        let got = pitch(line(f, false), 12.0);
        assert!((got - f.option).abs() <= 0.03, "{}: {got} vs Word's {}", f.name, f.option);
    }
}

#[test]
fn without_the_option_the_line_is_the_hhea_line_with_its_gap() {
    for f in &FONTS {
        assert_eq!(line(f, true), hhea_line_leading(f.hhea.0, f.hhea.1, f.hhea.2, true), "{}", f.name);
        let got = pitch(line(f, true), 12.0);
        assert!((got - f.plain).abs() <= 0.03, "{}: {got} vs Word's {}", f.name, f.plain);
    }
}

#[test]
fn a_gap_inside_the_windows_cell_stays_and_sits_above_the_ascent() {
    // Calibri: hhea 1536 + 512 + 452 = 2500 = win 1950 + 550.
    assert_eq!(win_line_leading(1536, -512, 452, 1950, 550, false), (1988.0, 512.0));
    assert_eq!(win_line_leading(1521, -527, 350, 1884, 514, false), (1871.0, 527.0));
    // A Windows cell taller than the whole hhea line: nothing is external leading.
    assert_eq!(win_line_leading(1000, -300, 100, 1200, 400, false), (1100.0, 300.0));
}

#[test]
fn leading_outside_the_windows_cell_comes_off_the_top() {
    // Times New Roman and Arial: the whole gap is outside the cell.
    assert_eq!(win_line_leading(1825, -443, 87, 1825, 443, false), (1825.0, 443.0));
    assert_eq!(win_line_leading(1854, -434, 67, 1854, 434, false), (1854.0, 434.0));
    // Half of the gap is outside: hhea 1000 + 300 + 100 = 1400, win 1030 + 320 = 1350.
    assert_eq!(win_line_leading(1000, -300, 100, 1030, 320, false), (1050.0, 300.0));
}

#[test]
fn unmeasured_shapes_keep_the_ascender_plus_descender() {
    // A Windows cell smaller than ascender + descender: no more than the gap is removed.
    assert_eq!(win_line_leading(1000, -300, 100, 900, 250, false), (1000.0, 300.0));
    // No Windows metrics: the hhea line without its gap.
    assert_eq!(win_line_leading(1825, -443, 87, 0, 0, false), hhea_line_leading(1825, -443, 87, false));
    // A negative gap counts as none, and an extreme descender does not overflow.
    assert_eq!(win_line_leading(1000, -300, -50, 1000, 300, false), (1000.0, 300.0));
    assert_eq!(win_line_leading(1000, -300, -50, 1000, 300, true), (1000.0, 300.0));
    assert_eq!(win_line_leading(1000, i16::MIN, 0, 0, 0, false), (1000.0, 32768.0));
    assert_eq!(win_line_leading(1000, i16::MIN, 40, u16::MAX, u16::MAX, false), (1040.0, 32768.0));
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
}

/// (family, Word's line with the option): the ascent loses the leading, the descent stays.
const LOSE_LEADING: [(&str, (f64, f64)); 2] = [("Century Gothic", (0.9716, 0.220)), ("Lucida Bright", (0.9336, 0.205))];

#[test]
fn missing_century_gothic_and_lucida_bright_lose_their_leading() {
    for (family, expected) in LOSE_LEADING {
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let r = resolve(family, bold, italic);
            let with = missing_font_line_leading(family, &r, true);
            let without = missing_font_line_leading(family, &r, false);
            assert_eq!(with, missing_font_line(family, &r), "{family}: leading keeps the table line");
            if r.substituted {
                let without = without.unwrap_or_else(|| panic!("a missing {family} has Word's line"));
                assert!(close(without, expected), "{family} b{bold} i{italic}: {without:?}");
                let with = with.unwrap_or_else(|| panic!("a missing {family} has Word's line"));
                assert!(close((with.0 + with.1 - without.0 - without.1, with.1), (with.0 - expected.0, expected.1)), "{family}");
            } else {
                assert_eq!(without, None, "an installed {family} keeps its own line");
            }
        }
        let r = resolve(family, false, false);
        if r.substituted {
            for name in [family.to_lowercase(), family.to_uppercase(), format!("  {family} ")] {
                let got = missing_font_line_leading(&name, &r, false).unwrap_or_else(|| panic!("{name}"));
                assert!(close(got, expected), "{name}: {got:?}");
            }
        }
    }
}

#[test]
fn the_other_missing_font_lines_ignore_the_option() {
    for family in [
        "Palatino Linotype",
        "Consolas",
        "Georgia",
        "Aptos",
        "Aptos Display",
        "Franklin Gothic Book",
        "Garamond",
        "Comic Sans MS",
        "Tahoma",
        "Verdana",
        "Trebuchet MS",
        "Lucida Console",
        "Univers",
        "Symbol",
        "Wingdings",
        "Wingdings 2",
        "Wingdings 3",
        "Webdings",
        "A Font Nobody Has",
    ] {
        let r = resolve(family, false, false);
        let with = missing_font_line(family, &r);
        assert_eq!(missing_font_line_leading(family, &r, true), with, "{family}");
        assert_eq!(missing_font_line_leading(family, &r, false), with, "{family}: Word's pitch does not change");
    }
}
