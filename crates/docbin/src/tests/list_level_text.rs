//! A list level's number text ([MS-DOC] §2.9.150 LVL `xst`) is UTF-16. Only the symbol-font
//! private-use range U+F000–U+F0FF carries flag bits to strip; every other character is itself.

use crate::list::bullet_char;

#[test]
fn symbol_font_bullets_keep_their_mapping() {
    let cases: [(u16, char); 5] = [(0xF0B7, '\u{2022}'), (0xF0A7, '\u{25AA}'), (0xF06F, '\u{25CB}'), (0xF077, '\u{25CF}'), (0xF078, '\u{25CF}')];
    for (stored, want) in cases {
        assert_eq!(bullet_char(stored), want, "stored {stored:#06X}");
    }
}

#[test]
fn unicode_bullets_are_kept() {
    let cases: [(u16, char); 3] = [(0x25CF, '\u{25CF}'), (0x2022, '\u{2022}'), (0x2013, '\u{2013}')];
    for (stored, want) in cases {
        assert_eq!(bullet_char(stored), want, "stored {stored:#06X}");
    }
}

#[test]
fn east_asian_number_text_is_kept() {
    let cases: [(u16, char); 4] = [(0x7B2C, '\u{7B2C}'), (0x7AE0, '\u{7AE0}'), (0xFF08, '\u{FF08}'), (0x3007, '\u{3007}')];
    for (stored, want) in cases {
        assert_eq!(bullet_char(stored), want, "stored {stored:#06X}");
    }
}

#[test]
fn plain_letters_are_unchanged() {
    assert_eq!(bullet_char(0x0041), 'A');
    assert_eq!(bullet_char(0x002E), '.');
}
