//! Field types from the field tables ([MS-DOC] 2.8.25 Plcfld, 2.9.x Fld, the flt enumeration).
//!
//! A field code in the text carries its keyword in the language of the Word that wrote it (a
//! French Word writes `TM` for a table of contents, `RENVOIPAGE` for a page reference). Each field
//! begin also has a type byte in the story's field table, and that byte does not depend on the
//! language. A code whose keyword is not an English field name gets the English name its type
//! byte gives, so the rest of the reader (and the document) sees the field it is.

use std::ops::Range;

/// Most entries read from one field table: one per field character, so far more than any real
/// document has; a hostile length can't make the reader allocate past it.
const MAX_FIELD_ENTRIES: usize = 1_000_000;

/// The field begin character, as the low five bits of `Fld.fldch` name it.
const FIELD_BEGIN: u8 = 0x13;

/// The field end character, as the low five bits of `Fld.fldch` name it.
const FIELD_END: u8 = 0x15;

/// The type bytes of one story's field begins, and the CPs of its field ends, by story-relative
/// CP (ascending).
#[derive(Debug, Default)]
pub(crate) struct FieldTypes {
    begins: Vec<(u32, u8)>,
    ends: Vec<u32>,
}

/// No field table: every look-up finds nothing.
pub(crate) static NO_FIELD_TYPES: FieldTypes = FieldTypes { begins: Vec::new(), ends: Vec::new() };

impl FieldTypes {
    /// Reads the Plcfld at `pair` (`fc`, `lcb`) of the Table stream: `n + 1` CPs, then `n` Fld
    /// structures of two bytes, `n = (lcb - 4) / 6`. Entries are read while their CPs ascend;
    /// from the first that steps back on, none is trusted.
    pub(crate) fn read(table: &[u8], pair: Option<(u32, u32)>) -> Self {
        let Some((fc, lcb)) = pair else { return Self::default() };
        let at = fc as usize;
        let Some(plc) = table.get(at..at.saturating_add(lcb as usize)) else { return Self::default() };
        let total = plc.len().saturating_sub(4) / 6;
        let n = total.min(MAX_FIELD_ENTRIES);
        let Some(flds) = (total + 1).checked_mul(4).and_then(|from| plc.get(from..)) else { return Self::default() };
        let cps = plc.as_chunks::<4>().0.iter().take(n).map(|c| u32::from_le_bytes(*c));
        let mut begins = Vec::new();
        let mut ends = Vec::new();
        let mut last = 0u32;
        for (cp, &[fldch, grffld]) in cps.zip(flds.as_chunks::<2>().0.iter().take(n)) {
            if cp < last {
                break;
            }
            last = cp;
            if fldch & 0x1F == FIELD_BEGIN {
                begins.push((cp, grffld));
            } else if fldch & 0x1F == FIELD_END {
                ends.push(cp);
            }
        }
        Self { begins, ends }
    }

    /// The type byte of the field that begins at story-relative `cp`, if the table has one.
    pub(crate) fn at(&self, cp: u32) -> Option<u8> {
        let i = self.begins.partition_point(|(c, _)| *c < cp);
        self.begins.get(i).filter(|(c, _)| *c == cp).map(|(_, t)| *t)
    }

    /// Whether the table has a field end entry at story-relative `cp`.
    pub(crate) fn ends_at(&self, cp: u32) -> bool {
        let i = self.ends.partition_point(|c| *c < cp);
        self.ends.get(i).is_some_and(|c| *c == cp)
    }
}

/// The English field name of each mapped type byte (the flt enumeration). Values left out keep
/// their code: their text is not a field name (0x02 is a bare bookmark name, 0x22 an expression,
/// FILLIN and ASK codes may start with their prompt) or the type is not one Word writes for a
/// field the reader handles by name.
const NAMES: &[(u8, &str)] = &[
    (0x03, "REF"),
    (0x06, "SET"),
    (0x07, "IF"),
    (0x08, "INDEX"),
    (0x0A, "STYLEREF"),
    (0x0C, "SEQ"),
    (0x0D, "TOC"),
    (0x0E, "INFO"),
    (0x0F, "TITLE"),
    (0x10, "SUBJECT"),
    (0x11, "AUTHOR"),
    (0x12, "KEYWORDS"),
    (0x13, "COMMENTS"),
    (0x14, "LASTSAVEDBY"),
    (0x15, "CREATEDATE"),
    (0x16, "SAVEDATE"),
    (0x17, "PRINTDATE"),
    (0x18, "REVNUM"),
    (0x19, "EDITTIME"),
    (0x1A, "NUMPAGES"),
    (0x1B, "NUMWORDS"),
    (0x1C, "NUMCHARS"),
    (0x1D, "FILENAME"),
    (0x1E, "TEMPLATE"),
    (0x1F, "DATE"),
    (0x20, "TIME"),
    (0x21, "PAGE"),
    (0x23, "QUOTE"),
    (0x25, "PAGEREF"),
    (0x31, "EQ"),
    (0x32, "GOTOBUTTON"),
    (0x33, "MACROBUTTON"),
    (0x38, "LINK"),
    (0x39, "SYMBOL"),
    (0x3A, "EMBED"),
    (0x3B, "MERGEFIELD"),
    (0x40, "DOCVARIABLE"),
    (0x41, "SECTION"),
    (0x42, "SECTIONPAGES"),
    (0x43, "INCLUDEPICTURE"),
    (0x44, "INCLUDETEXT"),
    (0x45, "FILESIZE"),
    (0x46, "FORMTEXT"),
    (0x47, "FORMCHECKBOX"),
    (0x48, "NOTEREF"),
    (0x49, "TOA"),
    (0x51, "ADDIN"),
    (0x53, "FORMDROPDOWN"),
    (0x55, "DOCPROPERTY"),
    (0x58, "HYPERLINK"),
];

/// English field names besides those in [`NAMES`]: a code that starts with any English name keeps
/// its text whatever its type byte says (files carry stale type bytes).
const OTHER_ENGLISH: &[&str] = &[
    "FTNREF",
    "INCLUDE",
    "ASK",
    "FILLIN",
    "DATA",
    "NEXT",
    "NEXTIF",
    "SKIPIF",
    "MERGEREC",
    "DDE",
    "DDEAUTO",
    "GLOSSARY",
    "PRINT",
    "AUTONUMOUT",
    "AUTONUMLGL",
    "AUTONUM",
    "IMPORT",
    "USERNAME",
    "USERINITIALS",
    "USERADDRESS",
    "BARCODE",
    "MERGESEQ",
    "AUTOTEXT",
    "COMPARE",
    "ADVANCE",
    "CONTROL",
    "AUTOTEXTLIST",
    "LISTNUM",
    "HTMLCONTROL",
    "BIDIOUTLINE",
    "ADDRESSBLOCK",
    "GREETINGLINE",
    "SHAPE",
    "CONTACT",
    "TC",
    "XE",
    "TA",
    "RD",
    "PRIVATE",
    "BIBLIOGRAPHY",
    "CITATION",
    "DISPLAYBARCODE",
    "MERGEBARCODE",
    "DATABASE",
    "HTMLACTIVEX",
];

fn is_english(keyword: &str) -> bool {
    keyword.starts_with('=')
        || NAMES.iter().any(|(_, name)| name.eq_ignore_ascii_case(keyword))
        || OTHER_ENGLISH.iter().any(|name| name.eq_ignore_ascii_case(keyword))
}

/// Where the keyword of `code` is that its type byte `flt` renames, and the English name: `None`
/// when there is no type, the keyword is empty or already an English name, or the type is not
/// mapped. The keyword runs from the first non-white character to the first white space, `\`
/// or `"`.
pub(crate) fn english_keyword(code: &str, flt: Option<u8>) -> Option<(Range<usize>, &'static str)> {
    let flt = flt?;
    let start = code.len() - code.trim_start().len();
    let rest = code.get(start..)?;
    let len = rest.find(|c: char| c.is_whitespace() || c == '\\' || c == '"').unwrap_or(rest.len());
    let keyword = rest.get(..len)?;
    if keyword.is_empty() || is_english(keyword) {
        return None;
    }
    let name = NAMES.iter().find(|(t, _)| *t == flt)?.1;
    Some((start..start + len, name))
}

/// `code` with its keyword replaced by the English name its type byte gives ([`english_keyword`]),
/// the rest of it unchanged; `None` when it stays as it is.
pub(crate) fn english_code(code: &str, flt: Option<u8>) -> Option<String> {
    let (at, name) = english_keyword(code, flt)?;
    let (head, tail) = (code.get(..at.start)?, code.get(at.end..)?);
    let mut out = String::with_capacity(head.len() + name.len() + tail.len());
    out.push_str(head);
    out.push_str(name);
    out.push_str(tail);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mapped_byte_names_its_field() {
        assert_eq!(NAMES.len(), 50);
        for &(t, name) in NAMES {
            assert_eq!(english_code("ZZZ", Some(t)).as_deref(), Some(name), "{t:#04x}");
        }
    }

    #[test]
    fn every_other_byte_leaves_the_code() {
        for t in 0..=255u8 {
            if NAMES.iter().all(|(m, _)| *m != t) {
                assert_eq!(english_code("ZZZ", Some(t)), None, "{t:#04x}");
            }
        }
        assert_eq!(english_code("ZZZ", None), None);
    }

    #[test]
    fn the_rest_of_the_code_is_kept_byte_for_byte() {
        assert_eq!(english_code(" TM \\o \"1-3\" ", Some(0x0D)).as_deref(), Some(" TOC \\o \"1-3\" "));
        assert_eq!(english_code("TM\\o", Some(0x0D)).as_deref(), Some("TOC\\o"));
        assert_eq!(english_code("\tNBPAGES", Some(0x1A)).as_deref(), Some("\tNUMPAGES"));
        assert_eq!(english_code("RENVOIPAGE\"x\"", Some(0x25)).as_deref(), Some("PAGEREF\"x\""));
    }

    #[test]
    fn english_and_empty_keywords_win() {
        for code in [" PAGE ", "page", " CONTACT _Con-1 \\c ", "=2+2", " \"Your name\" ", "", "   ", " xe \"a\""] {
            assert_eq!(english_code(code, Some(0x0D)), None, "{code:?}");
        }
    }

    #[test]
    fn a_hostile_table_never_panics() {
        let table = vec![0xFFu8; 64];
        for (fc, lcb) in [(0, 0), (0, 9), (0, 64), (60, 64), (u32::MAX, u32::MAX), (3, 13)] {
            let t = FieldTypes::read(&table, Some((fc, lcb)));
            assert_eq!(t.at(0), None);
            assert!(!t.ends_at(0));
        }
    }

    #[test]
    fn entries_after_a_step_back_are_not_trusted() {
        let mut plc = Vec::new();
        for cp in [5u32, 2, 9, 12] {
            plc.extend_from_slice(&cp.to_le_bytes());
        }
        plc.extend_from_slice(&[0x13, 0x0D, 0x13, 0x0D, 0x13, 0x0D]);
        let t = FieldTypes::read(&plc, Some((0, plc.len() as u32)));
        assert_eq!((t.at(5), t.at(2), t.at(9)), (Some(0x0D), None, None));
    }
}
