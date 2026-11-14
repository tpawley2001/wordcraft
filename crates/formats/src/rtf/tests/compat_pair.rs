//! Word's RTF reader turns on `w:balanceSingleByteDoubleByteWidth` and `w:useWord97LineBreakRules`
//! (its own RTF -> DOCX save writes both): a document opened from RTF sets
//! `Settings::balance_sbcs_dbcs_width` and `Settings::use_word97_line_break_rules`, so its
//! proportional fonts don't squeeze spaces, also after a save as DOCX (Microsoft 365 Word
//! 16.0.20527, 2026-11-13).

use super::*;

#[test]
fn every_rtf_document_carries_the_pair() {
    for rtf in [r"{\rtf1 Hello}", r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}\pard\qj\f0\fs24 Some justified text.\par}"] {
        let d = import(rtf.as_bytes()).unwrap();
        assert!(d.settings.balance_sbcs_dbcs_width, "{rtf}");
        assert!(d.settings.use_word97_line_break_rules, "{rtf}");
        assert!(!d.settings.squeezes_proportional_spaces(), "{rtf}");
    }
}
