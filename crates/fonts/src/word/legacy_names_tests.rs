//! Windows 3.x font names: Word draws "Helv" in Arial and "Tms Rmn" in Times New Roman
//! (measured in Microsoft 365 Word's PDFs of Word 97-2003 documents that name them).

use super::{has_named_substitutes, substitutes};

#[test]
fn helv_is_drawn_in_arial() {
    assert_eq!(substitutes("Helv").first(), Some(&"Arial"));
    assert!(substitutes("Helv").contains(&"Liberation Sans"), "{:?}", substitutes("Helv"));
    assert_eq!(substitutes("HELV"), substitutes("Helv"));
    assert!(has_named_substitutes("Helv"));
}

#[test]
fn tms_rmn_is_drawn_in_times_new_roman() {
    assert_eq!(substitutes("Tms Rmn").first(), Some(&"Times New Roman"));
    assert!(substitutes("Tms Rmn").contains(&"Liberation Serif"), "{:?}", substitutes("Tms Rmn"));
    assert_eq!(substitutes("tms rmn"), substitutes("Tms Rmn"));
    assert!(has_named_substitutes("Tms Rmn"));
}

#[test]
fn arial_and_times_new_roman_keep_their_lists() {
    assert_eq!(substitutes("Times New Roman").first(), Some(&"Liberation Serif"));
    assert_eq!(substitutes("Arial").first(), Some(&"Liberation Sans"));
}
