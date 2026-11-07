//! GOTOBUTTON and MACROBUTTON fields show the display text from their code (ECMA-376 Part 1
//! §17.16.5.24, §17.16.5.34), whatever result is stored, as Word does.

use crate::fields::{FieldCtx, button_display_text, field_text};

#[test]
fn button_fields_display_the_text_after_their_target() {
    let cases: &[(&str, Option<&str>)] = &[
        ("GOTOBUTTON Chapter1 Go to chapter", Some("Go to chapter")),
        (" MACROBUTTON NoMacro [Click  here and type] ", Some("[Click  here and type]")),
        ("MACROBUTTON NoMacro \"Quoted text\"", Some("Quoted text")),
        ("gotobutton p3 Page three", Some("Page three")),
        // Switches aren't parsed for these fields: a backslash is text.
        ("MACROBUTTON NoMacro C:\\Reports\\q3", Some("C:\\Reports\\q3")),
        ("MACROBUTTON NoMacro", Some("")),
        ("GOTOBUTTON", Some("")),
        ("PAGE", None),
        ("REF Chapter1", None),
    ];
    for (instr, want) in cases {
        assert_eq!(button_display_text(instr).as_deref(), *want, "{instr:?}");
    }
}

#[test]
fn button_fields_ignore_a_stale_stored_result() {
    let ctx = FieldCtx::default();
    assert_eq!(field_text("GOTOBUTTON _Toc1 Introduction", "", &ctx).0, "Introduction");
    assert_eq!(field_text("MACROBUTTON NoMacro [Type the date]", "stale", &ctx).0, "[Type the date]");
    // Other fields still show their result.
    assert_eq!(field_text("REF Chapter1", "kept", &ctx).0, "kept");
}
