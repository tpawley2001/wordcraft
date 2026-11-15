//! Date-time pictures (`\@`, ECMA-376 §17.16.4.1): runs of the lowercase letter `a`. A run is
//! one token: directly followed by `/p` it is the AM/PM item; otherwise 1 to 3 letters show
//! one `a` and 4 or more show `aa`. Every expected value was measured in Microsoft 365 Word for
//! Windows (en-US document, QUOTE fields with `\@`, 4/30/2019 13:05:07, a Tuesday).

use crate::cmd::insert::format_date_parts;

const HEISEI_LAST: ((i64, usize, u32), (u32, u32, u32), i64) = ((2019, 4, 30), (13, 5, 7), 18_016);
const MORNING: ((i64, usize, u32), (u32, u32, u32), i64) = ((2019, 4, 30), (9, 5, 7), 18_016);

fn check(when: ((i64, usize, u32), (u32, u32, u32), i64), cases: &[(&str, &str)]) {
    let ((y, m, d), hms, days) = when;
    for (picture, word) in cases {
        assert_eq!(format_date_parts(picture, y, m, d, hms, days), *word, "{picture}");
    }
}

#[test]
fn a_runs_alone() {
    check(
        HEISEI_LAST,
        &[("a", "a"), ("aa", "a"), ("aaa", "a"), ("aaaa", "aa"), ("aaaaa", "aa"), ("aaaaaa", "aa")],
    );
}

#[test]
fn a_runs_beside_other_items() {
    check(
        HEISEI_LAST,
        &[
            ("d a", "30 a"),
            ("a d", "a 30"),
            ("d aa d", "30 a 30"),
            ("d aaa d", "30 a 30"),
            ("h:mm aa", "1:05 a"),
            ("dddd aaa", "Tuesday a"),
            ("xa", "xa"),
            ("ax", "ax"),
            ("'a'", "a"),
            ("am", "a5"),
            ("aam", "a5"),
        ],
    );
}

#[test]
fn a_run_before_slash_p_is_am_pm() {
    check(HEISEI_LAST, &[("aa/pp", "PMp"), ("aaa/ppp", "PMpp"), ("a/p", "PM")]);
    check(MORNING, &[("a/p", "AM")]);
}

#[test]
fn uppercase_a_stays_the_japanese_day() {
    check(
        HEISEI_LAST,
        &[("Aa", "三十a"), ("aA", "a三十"), ("AA", "三十三十"), ("AAA", "三十三十三十")],
    );
}
