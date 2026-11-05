//! A document in Century or Microsoft Sans Serif that isn't installed is drawn in its metric clone
//! (C059 has Century's advance widths; Liberation Sans has Microsoft Sans Serif's), not in a generic
//! substitute, so its lines break as in Word.

use wordcraft_fonts::FontDb;
use wordcraft_fonts::word::{has_named_substitutes, resolve};

#[test]
fn century_and_microsoft_sans_serif_have_named_substitutes() {
    for f in ["Century", "century schoolbook", "Microsoft Sans Serif", "MS Sans Serif"] {
        assert!(has_named_substitutes(f), "{f}");
    }
}

#[test]
fn a_missing_century_is_drawn_in_c059() {
    let db = FontDb::global();
    if db.has_family("Century") || !db.has_family("C059") {
        eprintln!("Century installed or C059 missing: nothing to check");
        return;
    }
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        let r = resolve("Century", bold, italic);
        assert!(r.substituted, "{bold} {italic}");
        assert_eq!(r.face.family, "C059", "{bold} {italic}");
    }
}

#[test]
fn a_missing_microsoft_sans_serif_is_drawn_in_liberation_sans() {
    let db = FontDb::global();
    if db.has_family("Microsoft Sans Serif") || !db.has_family("Liberation Sans") {
        eprintln!("Microsoft Sans Serif installed or Liberation Sans missing: nothing to check");
        return;
    }
    let r = resolve("Microsoft Sans Serif", false, false);
    assert!(r.substituted);
    assert_eq!(r.face.family, "Liberation Sans");
}

#[test]
fn serif_faces_without_a_clone_keep_the_generic_list() {
    assert!(!has_named_substitutes("Cambria"));
    assert!(!has_named_substitutes("Georgia"));
}
