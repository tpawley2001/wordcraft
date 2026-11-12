//! A document in CG Times (the LaserJet-era Times) that isn't installed is drawn in the
//! substitutes of Times New Roman, as Word's own output shows it, not in a generic sans face,
//! so its lines break as in Word.

use wordcraft_fonts::FontDb;
use wordcraft_fonts::word::{has_named_substitutes, resolve};

#[test]
fn cg_times_has_named_substitutes() {
    for f in ["CG Times", "cg times", "CG TIMES"] {
        assert!(has_named_substitutes(f), "{f}");
    }
}

#[test]
fn a_missing_cg_times_is_drawn_in_the_face_times_new_roman_gets() {
    let db = FontDb::global();
    if db.has_family("CG Times") {
        eprintln!("CG Times installed: nothing to check");
        return;
    }
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        let cg = resolve("CG Times", bold, italic);
        let times = resolve("Times New Roman", bold, italic);
        assert!(cg.substituted, "{bold} {italic}");
        assert_eq!(cg.face.family, times.face.family, "{bold} {italic}");
        assert_eq!((cg.synth_bold, cg.synth_italic), (times.synth_bold, times.synth_italic), "{bold} {italic}");
    }
}
