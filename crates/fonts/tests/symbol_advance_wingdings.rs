//! A missing Wingdings, Wingdings 2 or Wingdings 3 keeps the width Word gives each character
//! (measured in Word's own PDF output), as a missing Symbol keeps Symbol's.

use wordcraft_fonts::word::symbol_advance;

fn near(got: Option<f64>, want: f64) -> bool {
    got.is_some_and(|g| (g - want).abs() < 0.0015)
}

#[test]
fn wingdings_codes_take_words_widths() {
    assert!(near(symbol_advance("Wingdings", 0x20), 1.0000), "Wingdings 0x20");
    assert!(near(symbol_advance("Wingdings", 0x6C), 0.7466), "Wingdings 0x6C");
    assert!(near(symbol_advance("Wingdings", 0x6E), 0.7466), "Wingdings 0x6E");
    assert!(near(symbol_advance("Wingdings", 0x71), 0.8911), "Wingdings 0x71");
    assert!(near(symbol_advance("Wingdings", 0xA7), 0.4575), "Wingdings 0xA7");
    assert!(near(symbol_advance("Wingdings", 0xA8), 0.8911), "Wingdings 0xA8");
    assert!(near(symbol_advance("Wingdings", 0xD8), 0.7944), "Wingdings 0xD8");
    assert!(near(symbol_advance("Wingdings", 0xE0), 0.9795), "Wingdings 0xE0");
    assert!(near(symbol_advance("Wingdings", 0xFB), 0.6353), "Wingdings 0xFB");
    assert!(near(symbol_advance("Wingdings", 0xFC), 0.7856), "Wingdings 0xFC");
    assert!(near(symbol_advance("Wingdings 2", 0x50), 0.7656), "Wingdings 2 0x50");
    assert!(near(symbol_advance("Wingdings 2", 0x6C), 0.8916), "Wingdings 2 0x6C");
    assert!(near(symbol_advance("Wingdings 2", 0xA2), 0.8911), "Wingdings 2 0xA2");
    assert!(near(symbol_advance("Wingdings 2", 0xF0), 0.8911), "Wingdings 2 0xF0");
    assert!(near(symbol_advance("Wingdings 3", 0x6C), 0.7051), "Wingdings 3 0x6C");
    assert!(near(symbol_advance("Wingdings 3", 0xD8), 0.7520), "Wingdings 3 0xD8");
    assert!(near(symbol_advance("Wingdings 3", 0xE0), 0.8242), "Wingdings 3 0xE0");
    assert!(near(symbol_advance("Wingdings 3", 0x75), 0.8794), "Wingdings 3 0x75");
}

#[test]
fn family_names_match_like_symbols() {
    assert!(near(symbol_advance("WINGDINGS", 0xA8), 0.8911));
    assert!(near(symbol_advance("Wingdings 3; Arial", 0x6C), 0.7051));
}

#[test]
fn unknown_codes_and_other_fonts_have_no_width() {
    assert_eq!(symbol_advance("Wingdings", 0x7F), None, "the missing-character box keeps its own width");
    assert_eq!(symbol_advance("Wingdings", 0x10), None);
    assert_eq!(symbol_advance("Wingdings 3", 0xF5), None, "not measured");
    assert!(near(symbol_advance("Webdings", 0x6C), 1.0));
    assert!(near(symbol_advance("Webdings", 0x6D), 0.559));
    assert_eq!(symbol_advance("Webdings", 0x7F), None, "the missing-character box keeps its own width");
    assert_eq!(symbol_advance("Wingdings 4", 0x6C), None);
    // Symbol's table is unchanged.
    assert!(near(symbol_advance("Symbol", 0x79), 0.686));
    for c in 0..=u8::MAX {
        let _ = symbol_advance("Wingdings 2", c);
    }
}
