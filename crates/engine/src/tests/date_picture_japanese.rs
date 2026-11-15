//! Date-time pictures (`\@`, ECMA-376 §17.16.4.1) with the Japanese era and numeral items:
//! `g`/`gg`/`ggg` era, `e`/`ee` (also `n`/`nn`) era year, `A` day, `O` month, `I` minutes and `C`
//! seconds in Japanese numerals. Every expected value was measured in Microsoft 365 Word for
//! Windows (en-US document, QUOTE fields with `\@`, updated on the stated dates and times).

use crate::cmd::insert::format_date_parts;

fn at(picture: &str, (y, m, d): (i64, usize, u32), hms: (u32, u32, u32), days: i64) -> String {
    format_date_parts(picture, y, m, d, hms, days)
}

const SHOWA_LAST: ((i64, usize, u32), (u32, u32, u32), i64) = ((1989, 1, 7), (9, 5, 7), 6_946);
const HEISEI_FIRST: ((i64, usize, u32), (u32, u32, u32), i64) = ((1989, 1, 8), (9, 5, 7), 6_947);
const HEISEI_LAST: ((i64, usize, u32), (u32, u32, u32), i64) = ((2019, 4, 30), (13, 5, 7), 18_016);
const REIWA_FIRST: ((i64, usize, u32), (u32, u32, u32), i64) = ((2019, 5, 1), (13, 5, 7), 18_017);
const TODAY: ((i64, usize, u32), (u32, u32, u32), i64) = ((2026, 11, 15), (9, 25, 37), 20_772);
const MEIJI_LAST: ((i64, usize, u32), (u32, u32, u32), i64) = ((1912, 7, 29), (9, 5, 0), -20_975);
const TAISHO_FIRST: ((i64, usize, u32), (u32, u32, u32), i64) = ((1912, 7, 30), (9, 5, 0), -20_974);

fn check(when: ((i64, usize, u32), (u32, u32, u32), i64), cases: &[(&str, &str)]) {
    let (date, hms, days) = when;
    for (picture, word) in cases {
        assert_eq!(at(picture, date, hms, days), *word, "{picture} on {date:?} {hms:?}");
    }
}

#[test]
fn era_name_items_and_their_grouping() {
    check(SHOWA_LAST, &[("g", "S"), ("gg", "昭"), ("ggg", "昭和"), ("gggg", "昭和S")]);
    check(HEISEI_FIRST, &[("g", "H"), ("gg", "平"), ("ggg", "平成"), ("gggg", "平成H")]);
    check(HEISEI_LAST, &[("g", "H"), ("ggg", "平成")]);
    check(REIWA_FIRST, &[("g", "R"), ("gg", "令"), ("ggg", "令和")]);
    check(MEIJI_LAST, &[("g", "M"), ("gg", "明"), ("ggg", "明治")]);
    check(TAISHO_FIRST, &[("g", "T"), ("gg", "大"), ("ggg", "大正")]);
}

#[test]
fn era_year_items_and_their_grouping() {
    check(SHOWA_LAST, &[("e", "64"), ("ee", "64"), ("eee", "6464"), ("n", "64"), ("nn", "64"), ("gggee", "昭和64")]);
    check(HEISEI_FIRST, &[("e", "1"), ("ee", "01"), ("eee", "011"), ("n", "1"), ("nn", "01"), ("gggee", "平成01")]);
    check(HEISEI_LAST, &[("e", "31"), ("eee", "3131"), ("gggee", "平成31")]);
    check(REIWA_FIRST, &[("e", "1"), ("ee", "01"), ("eee", "011"), ("gggee", "令和01")]);
    check(TODAY, &[("e", "8"), ("ee", "08"), ("eee", "088"), ("n", "8"), ("nn", "08"), ("gggee", "令和08")]);
    check(MEIJI_LAST, &[("e", "45"), ("gggee", "明治45")]);
    check(TAISHO_FIRST, &[("e", "1"), ("gggee", "大正01")]);
}

#[test]
fn first_year_of_an_era_reads_gannen_before_nen() {
    check(SHOWA_LAST, &[("ggge年M月d日", "昭和64年1月7日")]);
    check(HEISEI_FIRST, &[("ggge年M月d日", "平成元年1月8日")]);
    check(HEISEI_LAST, &[("ggge年M月d日", "平成31年4月30日")]);
    check(REIWA_FIRST, &[("ggge年M月d日", "令和元年5月1日")]);
    check(TODAY, &[("ggge年M月d日", "令和8年11月15日")]);
    check(MEIJI_LAST, &[("ggge年M月d日", "明治45年7月29日")]);
    check(TAISHO_FIRST, &[("ggge年M月d日", "大正元年7月30日")]);
}

#[test]
fn japanese_numeral_items() {
    check(SHOWA_LAST, &[("O", "一"), ("C", "七"), ("I", "五"), ("d A O", "7 七 一")]);
    check(HEISEI_LAST, &[("O", "四"), ("C", "七"), ("I", "五"), ("d A O", "30 三十 四")]);
    check(REIWA_FIRST, &[("O", "五"), ("d A O", "1 一 五")]);
    check(TODAY, &[("O", "十一"), ("C", "三十七"), ("I", "二十五"), ("d A O", "15 十五 十一")]);
    check(MEIJI_LAST, &[("O", "七"), ("C", "\u{25CB}"), ("I", "五"), ("d A O", "29 二十九 七")]);
}

#[test]
fn bare_uppercase_a_is_the_day_numeral_not_am_pm() {
    // Word: `A` is the day in Japanese numerals, repeated per letter; `A/P` is that numeral
    // followed by literal `/P`. Measured on 2026-03-04 / 2026-12-31 / 2026-11-01 (11-14) and here.
    check(((2026, 3, 4), (9, 5, 0), 20_516), &[("A", "四"), ("AA", "四四"), ("AAA", "四四四"), ("x A y", "x 四 26")]);
    check(((2026, 12, 31), (23, 5, 0), 20_818), &[("A", "三十一"), ("AA", "三十一三十一"), ("Ad", "三十一31"), ("Am", "三十一5")]);
    check(((2026, 11, 1), (0, 5, 0), 20_758), &[("A", "一"), ("Ad", "一1")]);
    check(SHOWA_LAST, &[("A/P", "七/P")]);
    check(HEISEI_LAST, &[("A/P", "三十/P")]);
}

#[test]
fn lowercase_a_slash_p_is_am_pm_in_capitals() {
    check(SHOWA_LAST, &[("a/p", "AM")]);
    check(HEISEI_LAST, &[("a/p", "PM")]);
    check(TODAY, &[("a/p", "AM")]);
}
