//! A missing SimSun keeps Word's line height. Measured in Word's PDFs: SimSun's font descriptor
//! gives `/Ascent` 859, `/Descent` -141; 10.5, 12 and 14 pt single-spaced lines are 13.62, 15.56
//! and 18.16 pt apart (1.297 em), the first baseline about one em below the top of the body: the
//! extra 0.297 em is split evenly above and below. NSimSun and SimHei measured the same.

use wordcraft_fonts::word::{missing_font_line, resolve};

const WORD_LINE: (f64, f64) = (1.007, 0.290);

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn a_missing_simsun_takes_words_line() {
    for name in ["SimSun", "宋体", "simsun", "NSimSun", "新宋体", "SimHei", "黑体"] {
        let r = resolve(name, false, false);
        let line = missing_font_line(name, &r);
        if r.substituted {
            let line = line.unwrap_or_else(|| panic!("a missing {name} has Word's line"));
            assert!(close(line, WORD_LINE), "{name}: {line:?}");
            // 12 pt lines are 15.56 pt apart.
            assert!(((line.0 + line.1) * 12.0 - 15.564).abs() < 0.01, "{name}: {line:?}");
        } else {
            assert_eq!(line, None, "an installed {name} keeps its own line");
        }
    }
}

#[test]
fn guard_unmeasured_chinese_fonts_are_unchanged() {
    for name in ["FangSong", "KaiTi", "Microsoft YaHei", "MingLiU", "MS Mincho", "SimSun-ExtB"] {
        let r = resolve(name, false, false);
        if r.substituted {
            assert_eq!(missing_font_line(name, &r), None, "{name}");
        }
    }
}
