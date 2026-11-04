//! A document in Arial Narrow that isn't installed is drawn in its metric clone, Liberation Sans Narrow
//! (identical advance widths to Word's Arial Narrow), not in the generic sans substitute (Carlito, about a
//! quarter wider), so its lines break as in Word.

use wordcraft_fonts::FontDb;
use wordcraft_fonts::word::{has_named_substitutes, resolve};

#[test]
fn arial_narrow_has_named_substitutes() {
    for f in ["Arial Narrow", "arial narrow", " ARIAL NARROW ", "Helvetica Narrow"] {
        assert!(has_named_substitutes(f.trim()), "{f}");
    }
}

#[test]
fn a_missing_arial_narrow_is_drawn_in_liberation_sans_narrow() {
    let db = FontDb::global();
    if db.has_family("Arial Narrow") || !db.has_family("Liberation Sans Narrow") {
        eprintln!("Arial Narrow installed or Liberation Sans Narrow missing: nothing to check");
        return;
    }
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        let r = resolve("Arial Narrow", bold, italic);
        assert!(r.substituted, "{bold} {italic}");
        assert_eq!(r.face.family, "Liberation Sans Narrow", "{bold} {italic}");
    }
}

#[test]
fn arial_still_takes_a_full_width_substitute() {
    let db = FontDb::global();
    if db.has_family("Arial") {
        return;
    }
    let r = resolve("Arial", false, false);
    assert!(!r.face.family.contains("Narrow"), "{}", r.face.family);
}
