//! Windows ships Century as a regular face only, so Word draws bold Century by emboldening the
//! regular face: the advance widths stay the regular ones. A missing Century is drawn in C059 the
//! same way (C059 Regular, synthetic bold), not in C059 Bold, whose advances are wider.
//! Century Schoolbook has a real bold on Windows and keeps C059 Bold.

use wordcraft_fonts::FontDb;
use wordcraft_fonts::word::resolve;

fn c059_without_century() -> bool {
    let db = FontDb::global();
    if db.has_family("Century") || !db.has_family("C059") {
        eprintln!("Century installed or C059 missing: nothing to check");
        return false;
    }
    true
}

#[test]
fn bold_century_is_c059_regular_emboldened() {
    if !c059_without_century() {
        return;
    }
    let regular = resolve("Century", false, false);
    for family in ["Century", "century", "CENTURY"] {
        let b = resolve(family, true, false);
        assert!(b.substituted, "{family}");
        assert_eq!(b.face.family, "C059", "{family}");
        assert!(b.face.weight < 600.0, "{family}: a regular face, got weight {}", b.face.weight);
        assert!(b.synth_bold, "{family}");
        assert!(!b.synth_italic, "{family}");
        assert_eq!(b.face.weight, regular.face.weight, "{family}");
        assert_eq!(b.face.italic, regular.face.italic, "{family}");
    }
}

#[test]
fn bold_italic_century_is_c059_italic_emboldened() {
    if !c059_without_century() {
        return;
    }
    let r = resolve("Century", true, true);
    assert_eq!(r.face.family, "C059");
    assert!(r.face.italic, "the real C059 Italic face");
    assert!(r.face.weight < 600.0, "weight {}", r.face.weight);
    assert!(r.synth_bold);
    assert!(!r.synth_italic);
}

#[test]
fn regular_and_italic_century_are_unchanged() {
    if !c059_without_century() {
        return;
    }
    let r = resolve("Century", false, false);
    assert_eq!(r.face.family, "C059");
    assert!(!r.synth_bold && !r.synth_italic);
    let i = resolve("Century", false, true);
    assert!(i.face.italic && !i.synth_bold && !i.synth_italic);
}

#[test]
fn century_schoolbook_keeps_the_real_bold() {
    if !c059_without_century() || FontDb::global().has_family("Century Schoolbook") {
        return;
    }
    let b = resolve("Century Schoolbook", true, false);
    assert_eq!(b.face.family, "C059");
    assert!(b.face.weight >= 600.0, "weight {}", b.face.weight);
    assert!(!b.synth_bold);
}
