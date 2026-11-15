//! Advance widths Word gives some fonts when they aren't installed (see the task spec).

/// The advance (em) Word gives `c` in `family` (bold / italic) when that font isn't installed,
/// or `None` to keep the drawn substitute's advance.
pub fn missing_font_advance(family: &str, bold: bool, italic: bool, c: char) -> Option<f64> {
    let _ = (family, bold, italic, c);
    None
}
