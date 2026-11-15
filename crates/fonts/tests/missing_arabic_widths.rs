//! A missing Traditional Arabic keeps Word's advance widths per joining form (font units of 2048
//! per em, read from the width data of Word's PDF output for documents in that font).

use ArabicForm::{Final, Initial, Isolated, Medial};
use wordcraft_fonts::word_widths_arabic::{ArabicForm, arabic_forms, missing_arabic_advance, missing_arabic_run_advance};

const TA: &str = "Traditional Arabic";

fn u(units: u32) -> Option<f64> {
    Some(f64::from(units) / 2048.0)
}

fn close(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-9,
        (None, None) => true,
        _ => false,
    }
}

#[test]
fn joining_forms() {
    let cases: &[(&str, &[ArabicForm])] = &[
        // beh beh beh: dual-joining on both sides in the middle.
        ("\u{628}\u{628}\u{628}", &[Initial, Medial, Final]),
        // alef joins only to the previous letter: alef then beh stay apart.
        ("\u{627}\u{628}", &[Isolated, Isolated]),
        ("\u{628}\u{627}", &[Initial, Final]),
        // A combining fatha between the letters is transparent and itself Isolated.
        ("\u{628}\u{64E}\u{628}", &[Initial, Isolated, Final]),
        // A space breaks joining.
        ("\u{628} \u{628}", &[Isolated, Isolated, Isolated]),
        ("ab", &[Isolated, Isolated]),
        // dal (right-joining) in the middle: beh joins it, the next beh starts again.
        ("\u{628}\u{62F}\u{628}", &[Initial, Final, Isolated]),
    ];
    for &(text, want) in cases {
        assert_eq!(arabic_forms(text), want, "{text:?}");
    }
}

#[test]
fn advances_by_form_and_style() {
    let cases: &[(&str, bool, bool, char, ArabicForm, Option<f64>)] = &[
        (TA, false, false, '\u{628}', Initial, u(379)),
        (TA, false, false, '\u{628}', Final, u(1438)),
        // Isolated = the base letter's entry.
        (TA, false, false, '\u{628}', Isolated, u(1399)),
        (TA, false, false, '\u{644}', Medial, u(416)),
        ("traditional arabic", false, false, '\u{644}', Final, u(907)),
        (TA, true, false, '\u{644}', Final, u(970)),
        (TA, true, false, '\u{644}', Medial, u(500)),
        // Italic takes the upright widths.
        (TA, false, true, '\u{628}', Initial, u(379)),
        (TA, true, true, '\u{644}', Final, u(970)),
        (TA, false, false, ' ', Isolated, u(500)),
        // No Bold entry for fatha: no width (not the regular one).
        (TA, true, false, '\u{64E}', Isolated, None),
        ("Arial", false, false, '\u{628}', Initial, None),
        (TA, false, false, '\u{4E2D}', Isolated, None),
    ];
    for &(family, bold, italic, c, form, want) in cases {
        let got = missing_arabic_advance(family, bold, italic, c, form);
        assert!(close(got, want), "{family} b={bold} i={italic} {c:?} {form:?}: {got:?}, want {want:?}");
    }
}

#[test]
fn run_advances() {
    let cases: &[(&str, bool, &str, Option<f64>)] = &[
        // beh initial 379 + lam medial 416 + meem final 673.
        (TA, false, "\u{628}\u{644}\u{645}", u(1468)),
        // alef isolated 301 + beh isolated 1399.
        (TA, false, "\u{627}\u{628}", u(1700)),
        // meem initial 606 + fatha (mark: 0) + alef final 420.
        (TA, false, "\u{645}\u{64E}\u{627}", u(1026)),
        // beh initial 379 + lam-alef final ligature U+FEFC 1061 (the alef adds nothing).
        (TA, false, "\u{628}\u{644}\u{627}", u(1440)),
        // lam-alef isolated ligature U+FEFB.
        (TA, false, "\u{644}\u{627}", u(946)),
        (TA, false, "\u{628}\u{644}\u{645} \u{645}\u{627} \u{628}\u{644}\u{627}", u(1468 + 500 + 1026 + 500 + 1440)),
        // Bold: beh initial 379 + lam-alef final 1099.
        (TA, true, "\u{628}\u{644}\u{627}", u(379 + 1099)),
        (TA, false, "\u{628}\u{4E2D}", None),
        ("Arial", false, "\u{628}", None),
    ];
    for &(family, bold, text, want) in cases {
        let got = missing_arabic_run_advance(family, bold, false, text);
        assert!(close(got, want), "{family} b={bold} {text:?}: {got:?}, want {want:?}");
    }
}
