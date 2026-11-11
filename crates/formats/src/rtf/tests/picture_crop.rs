//! Picture crop (`\piccropl`, `\piccropt`, `\piccropr`, `\piccropb`; RTF 1.9.1, "Pictures"): twips cut off each side,
//! measured against the goal size (`\picwgoal`/`\pichgoal`), with the scale applied after. The sizes are what Word
//! shows for the same files.

use super::*;

fn png_hex() -> String {
    let pixels = image::RgbaImage::from_pixel(4, 2, image::Rgba([40, 80, 160, 255]));
    let mut png = Vec::new();
    pixels.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    png.iter().map(|b| format!("{b:02x}")).collect()
}

/// A document holding one inline PNG whose `\pict` group has `words` after its goal-less header.
fn pict(words: &str) -> String {
    format!("{{\\rtf1\\ansi {{\\pict\\pngblip\\picw200\\pich100{words} {}}}X\\par}}", png_hex())
}

/// The first picture's displayed width and height (points) and its crop (left, top, right, bottom fractions).
fn first_picture(f: &Flow) -> (f32, f32, [f32; 4]) {
    f.blocks
        .iter()
        .filter_map(|b| match b {
            FBlock::Para(p) => Some(&p.inlines),
            FBlock::Table(_) => None,
        })
        .flatten()
        .find_map(|i| match i {
            Inline::Image(img) => Some((img.w, img.h, img.crop)),
            _ => None,
        })
        .expect("a picture")
}

fn shown(words: &str) -> (f32, f32, [f32; 4]) {
    first_picture(&parse(pict(words).as_bytes()).unwrap())
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn assert_shown(got: (f32, f32, [f32; 4]), want: (f32, f32, [f32; 4]), what: &str) {
    assert!(close(got.0, want.0) && close(got.1, want.1), "{what}: size {:?}, want {:?}", (got.0, got.1), (want.0, want.1));
    for (side, (g, w)) in got.2.iter().zip(want.2.iter()).enumerate() {
        assert!(close(*g, *w), "{what}: crop side {side} is {g}, want {w}");
    }
}

const GOAL: &str = "\\picwgoal2880\\pichgoal1440";

#[test]
fn an_uncropped_picture_is_unchanged() {
    assert_shown(shown(GOAL), (144.0, 72.0, [0.0; 4]), "no crop words");
}

#[test]
fn crops_are_read_against_the_goal_size_as_word_shows_them() {
    // (words after the goal, Word's displayed size in points, crop fractions left/top/right/bottom)
    let cases: [(&str, (f32, f32), [f32; 4]); 4] = [
        ("\\piccropl720", (108.0, 72.0), [0.25, 0.0, 0.0, 0.0]),
        ("\\piccropt360\\piccropb360", (144.0, 36.0), [0.0, 0.25, 0.0, 0.25]),
        ("\\piccropl720\\picscalex50\\picscaley50", (54.0, 36.0), [0.25, 0.0, 0.0, 0.0]),
        ("\\piccropl720\\piccropr720\\piccropt360\\piccropb180", (72.0, 45.0), [0.25, 0.25, 0.25, 0.125]),
    ];
    for (words, (w, h), crop) in cases {
        assert_shown(shown(&format!("{GOAL}{words}")), (w, h, crop), words);
    }
}

#[test]
fn the_crop_words_may_come_before_the_goal() {
    assert_shown(shown("\\piccropl720\\picwgoal2880\\pichgoal1440"), (108.0, 72.0, [0.25, 0.0, 0.0, 0.0]), "crop first");
}

#[test]
fn a_negative_crop_cuts_nothing_off_that_side() {
    assert_shown(shown(&format!("{GOAL}\\piccropl-720")), (144.0, 72.0, [0.0; 4]), "negative left");
    assert_shown(shown(&format!("{GOAL}\\piccropl-720\\piccropr720")), (108.0, 72.0, [0.0, 0.0, 0.25, 0.0]), "negative left, right 720");
}

#[test]
fn a_crop_that_leaves_nothing_is_ignored_on_that_axis() {
    assert_shown(shown(&format!("{GOAL}\\piccropl2880")), (144.0, 72.0, [0.0; 4]), "the whole width");
    assert_shown(shown(&format!("{GOAL}\\piccropl2000\\piccropr2000")), (144.0, 72.0, [0.0; 4]), "more than the width");
    // The other axis keeps its own crop.
    assert_shown(shown(&format!("{GOAL}\\piccropl2880\\piccropt360")), (144.0, 54.0, [0.0, 0.25, 0.0, 0.0]), "whole width, top 360");
}

#[test]
fn hostile_crops_leave_a_finite_picture() {
    for words in [
        "\\piccropl750",
        "\\piccropl2147483647\\piccropt2147483647\\piccropr2147483647\\piccropb2147483647",
        "\\picwgoal2880\\pichgoal1440\\piccropl2147483647",
        "\\picwgoal2880\\pichgoal1440\\piccropl-2147483647\\piccropb-2147483647",
        "\\picwgoal-5\\pichgoal0\\piccropl1\\piccropt1",
        "\\picwgoal1\\pichgoal1\\piccropl1\\piccropr1\\piccropt1\\piccropb1",
    ] {
        let (w, h, crop) = shown(words);
        assert!(w.is_finite() && h.is_finite() && w >= 1.0 && h >= 1.0, "{words}: size {w} x {h}");
        assert!(crop.iter().all(|c| c.is_finite() && (0.0..0.99).contains(c)), "{words}: crop {crop:?}");
        assert!(crop[0] + crop[2] < 0.99 && crop[1] + crop[3] < 0.99, "{words}: crop {crop:?}");
    }
}

#[test]
fn a_crop_is_written_with_the_uncropped_goal() {
    let doc = import(pict(&format!("{GOAL}\\piccropl720")).as_bytes()).unwrap();
    let out = export(&doc);
    assert!(out.contains("\\picwgoal2880\\pichgoal1440\\piccropl720"), "{}", out.chars().take(600).collect::<String>());
    for w in ["\\piccropt", "\\piccropr", "\\piccropb"] {
        assert!(!out.contains(w), "{w} written for a side with no crop");
    }
}

#[test]
fn every_side_round_trips() {
    let words = format!("{GOAL}\\piccropl720\\piccropr720\\piccropt360\\piccropb180");
    let out = export(&import(pict(&words).as_bytes()).unwrap());
    assert!(
        out.contains("\\picwgoal2880\\pichgoal1440\\piccropl720\\piccropt360\\piccropr720\\piccropb180"),
        "{}",
        out.chars().take(600).collect::<String>()
    );
    assert_shown(first_picture(&parse(out.as_bytes()).unwrap()), (72.0, 45.0, [0.25, 0.25, 0.25, 0.125]), "after a save");
}

#[test]
fn an_uncropped_picture_writes_no_crop_words() {
    let out = export(&import(pict(GOAL).as_bytes()).unwrap());
    assert!(out.contains("\\picwgoal2880\\pichgoal1440"), "{}", out.chars().take(600).collect::<String>());
    assert!(!out.contains("\\piccrop"), "crop words written for an uncropped picture");
}
