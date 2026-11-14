//! Date-time pictures (`\@`, ECMA-376 §17.16.4.1): a run of `h`, `H`, `m` or `s` longer than two
//! letters reads in pieces of at most two (`hhh` is `hh` then `h`), and lowercase `a/p` is one
//! item showing AM or PM. Expected results measured in Microsoft 365 Word for Windows (TIME and
//! QUOTE fields updated at 07:59:48 and on stated times).

use crate::cmd::insert::format_date_parts;

fn at(picture: &str, hms: (u32, u32, u32)) -> String {
    format_date_parts(picture, 2026, 11, 14, hms, 20_406)
}

#[test]
fn long_hour_minute_second_runs_read_two_letters_at_a_time() {
    let morning = (7, 59, 48);
    for (picture, word) in [("hhh", "077"), ("HHH", "077"), ("mmm", "5959"), ("sss", "4848")] {
        assert_eq!(at(picture, morning), word, "{picture} at 07:59:48");
    }
    let afternoon = (13, 5, 9);
    for (picture, word) in [("hhh", "011"), ("HHH", "1313"), ("mmm", "055"), ("sss", "099")] {
        assert_eq!(at(picture, afternoon), word, "{picture} at 13:05:09");
    }
}

#[test]
fn lowercase_a_slash_p_shows_am_or_pm() {
    for (hms, word) in [((7, 59, 48), "7:59 AM"), ((13, 5, 0), "1:05 PM"), ((0, 5, 0), "12:05 AM"), ((12, 30, 0), "12:30 PM")] {
        assert_eq!(at("h:mm a/p", hms), word, "h:mm a/p at {hms:?}");
    }
}

#[test]
fn pictures_word_already_agrees_on_are_unchanged() {
    let morning = (7, 59, 48);
    for (picture, word) in [
        ("h:mm am/pm", "7:59 AM"),
        ("h:mm AM/PM", "7:59 AM"),
        ("h am/pm am/pm", "7 AM AM"),
        ("HH:mm", "07:59"),
        ("hh:mm:ss", "07:59:48"),
        ("H:m:s", "7:59:48"),
        ("h", "7"),
    ] {
        assert_eq!(at(picture, morning), word, "{picture} at 07:59:48");
    }
    assert_eq!(at("h:mm am/pm", (12, 30, 0)), "12:30 PM");
    assert_eq!(at("hh:mm", (0, 5, 0)), "12:05");
    assert_eq!(at("dddd d MMMM yyyy", morning), at("dddd d MMMM yyyy", (13, 5, 9)));
}
