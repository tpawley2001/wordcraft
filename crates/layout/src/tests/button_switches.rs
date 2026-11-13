//! GOTOBUTTON and MACROBUTTON display text: general switches written before the display text are
//! not shown (and not applied); anything after the first word of display text is text, as Word
//! shows it (ECMA-376 Part 1 §17.16.5.24, §17.16.5.34; §17.16.4.3 general switches).

use crate::fields::{FieldCtx, button_display_text, field_text};

#[test]
fn switches_before_the_display_text_are_not_shown() {
    let cases: &[(&str, &str)] = &[
        ("MACROBUTTON NoMacro Click here", "Click here"),
        (r"MACROBUTTON NoMacro \* MERGEFORMAT Click here", "Click here"),
        (r"MACROBUTTON NoMacro \*MERGEFORMAT Click here", "Click here"),
        (r"MACROBUTTON NoMacro \* MERGEFORMAT \* Upper Click here", "Click here"),
        // The switch is dropped, not applied.
        (r"MACROBUTTON NoMacro \* Upper Click here", "Click here"),
        (r"MACROBUTTON NoMacro \# 0.00 Click here", "Click here"),
        (r#"MACROBUTTON NoMacro \# "0.0 x" Click here"#, "Click here"),
        (r#"MACROBUTTON NoMacro \@ "d MMM" Click here"#, "Click here"),
        (r"MACROBUTTON NoMacro \! Click here", "Click here"),
        (r"GOTOBUTTON bm \* MERGEFORMAT Go there", "Go there"),
        (r"MACROBUTTON NoMacro \* MERGEFORMAT", ""),
        (r"MACROBUTTON MTPlaceRef \* MERGEFORMAT (1)", "(1)"),
    ];
    for (instr, want) in cases {
        assert_eq!(button_display_text(instr).as_deref(), Some(*want), "{instr:?}");
    }
}

#[test]
fn a_switch_before_the_macro_name_is_skipped_too() {
    assert_eq!(button_display_text(r"MACROBUTTON \* MERGEFORMAT NoMacro Click here").as_deref(), Some("Click here"));
}

#[test]
fn a_switch_after_the_display_text_began_is_text() {
    let cases: &[(&str, &str)] = &[
        (r"MACROBUTTON NoMacro Click here \* MERGEFORMAT", r"Click here \* MERGEFORMAT"),
        (r"MACROBUTTON NoMacro Click \* MERGEFORMAT here", r"Click \* MERGEFORMAT here"),
        (r"MACROBUTTON NoMacro \* MERGEFORMAT x \* Upper y", r"x \* Upper y"),
    ];
    for (instr, want) in cases {
        assert_eq!(button_display_text(instr).as_deref(), Some(*want), "{instr:?}");
    }
}

#[test]
fn other_backslash_words_are_text() {
    assert_eq!(button_display_text(r"MACROBUTTON NoMacro \x Click here").as_deref(), Some(r"\x Click here"));
    assert_eq!(button_display_text(r"MACROBUTTON NoMacro C:\Reports\q3").as_deref(), Some(r"C:\Reports\q3"));
}

#[test]
fn the_laid_out_field_shows_the_text_without_its_switch() {
    let ctx = FieldCtx::default();
    assert_eq!(field_text(r"MACROBUTTON MTPlaceRef \* MERGEFORMAT (1)", "stale", &ctx).0, "(1)");
    assert_eq!(field_text(r"GOTOBUTTON bm \* MERGEFORMAT Go there", "", &ctx).0, "Go there");
}
