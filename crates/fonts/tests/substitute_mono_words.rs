//! Only a whole word "mono", "monospace", "monospaced", or "code" in a missing font's name makes its
//! substitute monospaced: "Lucida Sans Unicode" and "Monotype Corsiva" are proportional, as Word draws them.

use wordcraft_fonts::word::resolve;

fn fam(name: &str) -> String {
    resolve(name, false, false).face.family.clone()
}

#[test]
fn unicode_and_monotype_are_not_monospaced() {
    let mono = fam("Brand Mono");
    let sans = fam("Brand Sans");
    assert_ne!(mono, sans, "test fonts: a mono and a sans substitute must differ here");
    for name in ["Lucida Sans Unicode", "Brand Unicode", "Monotype Corsiva", "Barcode 39"] {
        if resolve(name, false, false).substituted {
            assert_eq!(fam(name), sans, "{name}");
        }
    }
}

#[test]
fn whole_word_mono_and_code_stay_monospaced() {
    let mono = fam("Brand Mono");
    for name in ["Brand Code", "Brand-Mono", "Brand_Code", "Brand Monospace", "Brand Serif Mono"] {
        if resolve(name, false, false).substituted {
            assert_eq!(fam(name), mono, "{name}");
        }
    }
}
