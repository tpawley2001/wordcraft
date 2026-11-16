//! ScreenTip style (File › Options › General) and Show shortcut keys in ScreenTips (Advanced ›
//! Display), as in Word.

use super::tooltip::{ScreenTipStyle, screen_tip_lines};

fn tip(style: ScreenTipStyle, shortcuts: bool, label: &str, sc: &str, desc: &str, disabled: Option<&str>) -> Option<(String, Vec<String>)> {
    screen_tip_lines(style, shortcuts, label, sc, desc, disabled)
}

fn lines(title: &str, notes: &[&str]) -> Option<(String, Vec<String>)> {
    Some((title.to_string(), notes.iter().map(|n| n.to_string()).collect()))
}

#[test]
fn feature_descriptions_is_the_default_style() {
    assert_eq!(ScreenTipStyle::default(), ScreenTipStyle::Descriptions);
}

#[test]
fn descriptions_with_and_without_shortcut_keys() {
    use ScreenTipStyle::Descriptions as D;
    assert_eq!(tip(D, true, "Bold", "Ctrl+B", "Home > Font", None), lines("Bold (Ctrl+B)", &["Home > Font"]));
    assert_eq!(tip(D, false, "Bold", "Ctrl+B", "Home > Font", None), lines("Bold", &["Home > Font"]));
    assert_eq!(tip(D, false, "Paste (Ctrl+V)", "Ctrl+V", "", None), lines("Paste (Ctrl+V)", &[]));
}

#[test]
fn no_feature_descriptions_keeps_name_shortcut_and_reason() {
    use ScreenTipStyle::NoDescriptions as N;
    assert_eq!(tip(N, true, "Bold", "Ctrl+B", "Home > Font", None), lines("Bold (Ctrl+B)", &[]));
    assert_eq!(tip(N, true, "Header", "", "Insert", Some("Open a document first")), lines("Header", &["Open a document first"]));
}

#[test]
fn hidden_shows_no_tip() {
    assert_eq!(tip(ScreenTipStyle::Hidden, true, "Bold", "Ctrl+B", "Home > Font", None), None);
}
