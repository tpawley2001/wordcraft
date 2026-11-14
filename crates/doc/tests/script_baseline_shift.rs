//! Superscript and subscript baseline shift, measured in Microsoft 365 Word for Windows
//! (PDF export, Times New Roman, Arial and Calibri at 8 to 36 pt; 42 measurements, least
//! squares fit: superscript 0.346 x size up, subscript 0.080 x size down; Word's PDF places
//! baselines to about +/-0.5 pt, so the table below is the fitted rule, not single samples).
use wordcraft_doc::StyleSheet;
use wordcraft_doc::props::{CharProps, VertAlign};

fn shift(size: f32, vert_align: VertAlign, position: f32) -> f32 {
    let sheet = StyleSheet::default();
    let c = CharProps { size: Some(size), vert_align: Some(vert_align), position: Some(position), ..Default::default() };
    sheet.resolve_char(None, &c).baseline_shift()
}

#[test]
fn superscript_rises_035_of_its_size() {
    // full size pt -> raise pt (Word measured, three fonts: 8 -> 3.00/3.00/2.52, 11 -> 3.96 x3,
    // 24 -> 8.52/8.52/8.04, 36 -> 12.48/12.96/12.03)
    for (size, want) in [(8.0, 2.8), (10.0, 3.5), (11.0, 3.85), (12.0, 4.2), (16.0, 5.6), (24.0, 8.4), (36.0, 12.6)] {
        let got = shift(size, VertAlign::Superscript, 0.0);
        assert!((got - want).abs() < 0.01, "{size}pt superscript raise {got}, want {want}");
    }
}

#[test]
fn subscript_drops_008_of_its_size() {
    // Word measured: 11 -> 0.96 x3, 24 -> 2.04/2.04/2.52, 36 -> 2.52/2.52/3.00 (we dropped 0.14 x size: 5.04 at 36pt)
    for (size, want) in [(8.0, -0.64), (10.0, -0.8), (11.0, -0.88), (12.0, -0.96), (16.0, -1.28), (24.0, -1.92), (36.0, -2.88)] {
        let got = shift(size, VertAlign::Subscript, 0.0);
        assert!((got - want).abs() < 0.01, "{size}pt subscript drop {got}, want {want}");
    }
}

#[test]
fn position_adds_to_the_script_shift_and_baseline_text_is_unchanged() {
    assert!((shift(11.0, VertAlign::Superscript, 3.0) - (3.85 + 3.0)).abs() < 0.01);
    assert!((shift(11.0, VertAlign::Subscript, -2.0) - (-0.88 - 2.0)).abs() < 0.01);
    assert_eq!(shift(11.0, VertAlign::Baseline, 0.0), 0.0);
    assert_eq!(shift(11.0, VertAlign::Baseline, -4.5), -4.5);
}
