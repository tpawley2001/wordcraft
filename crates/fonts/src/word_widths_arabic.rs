//! Advance widths Word gives Traditional Arabic when the font isn't installed, per joining form:
//! the substitute draws the text, but each letter keeps the width Word gives its initial, medial,
//! final or isolated form, so Arabic lines break where Word breaks them.

/// The joining form of an Arabic letter in its word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArabicForm {
    /// Joined on neither side (also every non-joining character and combining mark).
    Isolated,
    /// Joined to the next letter only.
    Initial,
    /// Joined on both sides.
    Medial,
    /// Joined to the previous letter only.
    Final,
}

/// How a character joins its neighbours (Unicode `ArabicShaping.txt` joining types).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Joining {
    /// Joins on both sides.
    Dual,
    /// Joins only to the previous letter.
    Right,
    /// Joins both neighbours that can join it, with no forms of its own (tatweel, ZWJ).
    Causing,
    /// Skipped when finding a letter's neighbours (combining marks).
    Transparent,
    /// Breaks joining.
    None,
}

/// A letter's presentation forms: (base, isolated form, form count). With two forms the letter
/// is right-joining (isolated, final); with four dual-joining (isolated, final, initial, medial),
/// the forms at consecutive code points from the isolated one.
const FORMS: &[(u16, u16, u8)] = &[
    (0x0622, 0xFE81, 2),
    (0x0623, 0xFE83, 2),
    (0x0624, 0xFE85, 2),
    (0x0625, 0xFE87, 2),
    (0x0626, 0xFE89, 4),
    (0x0627, 0xFE8D, 2),
    (0x0628, 0xFE8F, 4),
    (0x0629, 0xFE93, 2),
    (0x062A, 0xFE95, 4),
    (0x062B, 0xFE99, 4),
    (0x062C, 0xFE9D, 4),
    (0x062D, 0xFEA1, 4),
    (0x062E, 0xFEA5, 4),
    (0x062F, 0xFEA9, 2),
    (0x0630, 0xFEAB, 2),
    (0x0631, 0xFEAD, 2),
    (0x0632, 0xFEAF, 2),
    (0x0633, 0xFEB1, 4),
    (0x0634, 0xFEB5, 4),
    (0x0635, 0xFEB9, 4),
    (0x0636, 0xFEBD, 4),
    (0x0637, 0xFEC1, 4),
    (0x0638, 0xFEC5, 4),
    (0x0639, 0xFEC9, 4),
    (0x063A, 0xFECD, 4),
    (0x0641, 0xFED1, 4),
    (0x0642, 0xFED5, 4),
    (0x0643, 0xFED9, 4),
    (0x0644, 0xFEDD, 4),
    (0x0645, 0xFEE1, 4),
    (0x0646, 0xFEE5, 4),
    (0x0647, 0xFEE9, 4),
    (0x0648, 0xFEED, 2),
    (0x0649, 0xFEEF, 2),
    (0x064A, 0xFEF1, 4),
    (0x0671, 0xFB50, 2),
    (0x067E, 0xFB56, 4),
    (0x0686, 0xFB7A, 4),
    (0x0698, 0xFB8A, 2),
    (0x06A9, 0xFB8E, 4),
    (0x06AF, 0xFB92, 4),
    (0x06CC, 0xFBFC, 4),
];

const LAM: char = '\u{644}';

fn forms_of(c: char) -> Option<(u16, u8)> {
    let code = u16::try_from(u32::from(c)).ok()?;
    let i = FORMS.binary_search_by_key(&code, |&(k, _, _)| k).ok()?;
    FORMS.get(i).map(|&(_, iso, n)| (iso, n))
}

/// Whether `c` is an alef a preceding lam forms a ligature with.
fn is_alef(c: char) -> bool {
    matches!(c, '\u{622}' | '\u{623}' | '\u{625}' | '\u{627}')
}

fn joining(c: char) -> Joining {
    if let Some((_, n)) = forms_of(c) {
        return if n == 4 { Joining::Dual } else { Joining::Right };
    }
    match u32::from(c) {
        0x0640 | 0x200D => Joining::Causing,
        0x0300..=0x036F
        | 0x0610..=0x061A
        | 0x064B..=0x065F
        | 0x0670
        | 0x06D6..=0x06DC
        | 0x06DF..=0x06E4
        | 0x06E7..=0x06E8
        | 0x06EA..=0x06ED
        | 0xFE20..=0xFE2F => Joining::Transparent,
        0x0620
        | 0x063B..=0x063F
        | 0x066E..=0x066F
        | 0x0678..=0x0687
        | 0x069A..=0x06BF
        | 0x06C1..=0x06C2
        | 0x06CE
        | 0x06D0..=0x06D1
        | 0x06FA..=0x06FC
        | 0x06FF => Joining::Dual,
        0x0672..=0x0673
        | 0x0675..=0x0677
        | 0x0688..=0x0699
        | 0x06C0
        | 0x06C3..=0x06CB
        | 0x06CD
        | 0x06CF
        | 0x06D2..=0x06D3
        | 0x06D5
        | 0x06EE..=0x06EF => Joining::Right,
        _ => Joining::None,
    }
}

/// Whether `c` is a combining mark (drawn over its letter, without width of its own).
fn is_mark(c: char) -> bool {
    joining(c) == Joining::Transparent
}

/// The joining form of every char of `text` (one entry per char): Unicode's Arabic joining
/// (The Unicode Standard, ch. 9.2). Non-joining characters and combining marks are `Isolated`.
pub fn arabic_forms(text: &str) -> Vec<ArabicForm> {
    let types: Vec<Joining> = text.chars().map(joining).collect();
    let mut forms = Vec::with_capacity(types.len());
    // The previous non-transparent char's type.
    let mut prev = Joining::None;
    for (i, &t) in types.iter().enumerate() {
        if t == Joining::Transparent || t == Joining::None {
            forms.push(ArabicForm::Isolated);
            if t == Joining::None {
                prev = Joining::None;
            }
            continue;
        }
        let next = types.get(i + 1..).unwrap_or_default().iter().copied().find(|&n| n != Joining::Transparent).unwrap_or(Joining::None);
        let joins_prev = matches!(prev, Joining::Dual | Joining::Causing);
        let joins_next = matches!(t, Joining::Dual | Joining::Causing) && matches!(next, Joining::Dual | Joining::Right | Joining::Causing);
        forms.push(match (joins_prev, joins_next) {
            (false, false) => ArabicForm::Isolated,
            (false, true) => ArabicForm::Initial,
            (true, true) => ArabicForm::Medial,
            (true, false) => ArabicForm::Final,
        });
        prev = t;
    }
    forms
}

/// The width table for `family` (bold); italic takes the upright one.
fn table(family: &str, bold: bool) -> Option<&'static [(u16, u16)]> {
    if !family.trim().eq_ignore_ascii_case("Traditional Arabic") {
        return None;
    }
    Some(if bold { TRADITIONAL_ARABIC_BOLD } else { TRADITIONAL_ARABIC })
}

/// Whether Word has per-form advances for the missing `family`.
pub fn has_arabic_widths(family: &str) -> bool {
    table(family, false).is_some()
}

fn units(table: &[(u16, u16)], code: u16) -> Option<f64> {
    let i = table.binary_search_by_key(&code, |&(k, _)| k).ok()?;
    table.get(i).map(|&(_, w)| f64::from(w) / 2048.0)
}

/// Word's advance (em) of `c` in `form` in the missing `family`; `None` without a table or entry.
/// A letter's isolated form is its base code point's entry; its other forms are its presentation
/// forms' entries. Characters without forms take their own entry in any form.
pub fn missing_arabic_advance(family: &str, bold: bool, _italic: bool, c: char, form: ArabicForm) -> Option<f64> {
    let table = table(family, bold)?;
    let code = u16::try_from(u32::from(c)).ok()?;
    let code = match (forms_of(c), form) {
        (None, _) | (_, ArabicForm::Isolated) => code,
        (Some((iso, _)), ArabicForm::Final) => iso.checked_add(1)?,
        (Some((iso, 4)), ArabicForm::Initial) => iso.checked_add(2)?,
        (Some((iso, 4)), ArabicForm::Medial) => iso.checked_add(3)?,
        (Some(_), _) => return None,
    };
    units(table, code)
}

/// Word's advance (em) of every char of `text` in the missing `family`, with the text shown
/// before and after it (`before`, `after`), which its first and last letters may join. A lam
/// before an alef (marks between allowed) takes the lam-alef ligature's width and the alef none;
/// combining marks take none. `None` for the whole when `family` has no table, and per char
/// when the table has no width for it.
pub fn missing_arabic_char_advances(family: &str, bold: bool, italic: bool, text: &str, before: &str, after: &str) -> Option<Vec<Option<f64>>> {
    let table = table(family, bold)?;
    // Joining looks past marks only: a few chars of context are plenty.
    const CONTEXT: usize = 16;
    let before: Vec<char> = before.chars().rev().take(CONTEXT).collect::<Vec<_>>().into_iter().rev().collect();
    let after: Vec<char> = after.chars().take(CONTEXT).collect();
    let chars: Vec<char> = before.iter().copied().chain(text.chars()).chain(after.iter().copied()).collect();
    let forms = arabic_forms(&chars.iter().collect::<String>());
    let n = text.chars().count();
    let mut out = Vec::with_capacity(n);
    for i in before.len()..before.len().saturating_add(n) {
        let (Some(&c), Some(&form)) = (chars.get(i), forms.get(i)) else { break };
        let w = if is_mark(c) {
            Some(0.0)
        } else if c == LAM && chars.get(i + 1..).unwrap_or_default().iter().copied().find(|&d| !is_mark(d)).is_some_and(is_alef) {
            // The ligature after the alef it draws: U+FEF5 (madda), FEF7 (hamza above), FEF9
            // (hamza below), FEFB (plain), each followed by its final form.
            let alef = chars.get(i + 1..).unwrap_or_default().iter().copied().find(|&d| !is_mark(d));
            let iso: u16 = match alef {
                Some('\u{622}') => 0xFEF5,
                Some('\u{623}') => 0xFEF7,
                Some('\u{625}') => 0xFEF9,
                _ => 0xFEFB,
            };
            let joined = matches!(form, ArabicForm::Medial | ArabicForm::Final);
            units(table, if joined { iso + 1 } else { iso })
        } else if is_alef(c) && chars.get(..i).unwrap_or_default().iter().rev().copied().find(|&d| !is_mark(d)) == Some(LAM) {
            Some(0.0)
        } else {
            missing_arabic_advance(family, bold, italic, c, form)
        };
        out.push(w);
    }
    Some(out)
}

/// Word's total advance (em) of `text` in the missing `family`: joining forms, lam-alef ligatures,
/// combining marks without width. `None` if a character has no width.
pub fn missing_arabic_run_advance(family: &str, bold: bool, italic: bool, text: &str) -> Option<f64> {
    missing_arabic_char_advances(family, bold, italic, text, "", "")?.into_iter().sum()
}

/// Traditional Arabic advances in font units (2048 per em), sorted by code point.
const TRADITIONAL_ARABIC: &[(u16, u16)] = &[
    (0x0020, 500),
    (0x0021, 569),
    (0x0022, 569),
    (0x0023, 1368),
    (0x0025, 1473),
    (0x0028, 569),
    (0x0029, 569),
    (0x002A, 680),
    (0x002D, 946),
    (0x002E, 569),
    (0x002F, 569),
    (0x003A, 569),
    (0x004D, 1878),
    (0x005F, 1024),
    (0x0064, 1024),
    (0x0072, 682),
    (0x060C, 569),
    (0x061B, 569),
    (0x0621, 613),
    (0x0622, 479),
    (0x0623, 344),
    (0x0624, 719),
    (0x0625, 344),
    (0x0626, 1171),
    (0x0627, 301),
    (0x0628, 1399),
    (0x0629, 528),
    (0x062A, 1399),
    (0x062B, 1399),
    (0x062C, 1098),
    (0x062D, 1098),
    (0x062E, 1098),
    (0x062F, 643),
    (0x0630, 643),
    (0x0631, 680),
    (0x0632, 680),
    (0x0633, 1308),
    (0x0634, 1311),
    (0x0635, 1632),
    (0x0636, 1653),
    (0x0637, 1135),
    (0x0638, 1135),
    (0x0639, 1061),
    (0x063A, 1061),
    (0x0640, 110),
    (0x0641, 1511),
    (0x0642, 1061),
    (0x0643, 983),
    (0x0644, 907),
    (0x0645, 606),
    (0x0646, 870),
    (0x0647, 974),
    (0x0648, 719),
    (0x0649, 1171),
    (0x064A, 1171),
    (0x064B, 408),
    (0x064C, 463),
    (0x064E, 408),
    (0x064F, 414),
    (0x0650, 408),
    (0x0651, 420),
    (0x0653, 480),
    (0x0654, 0),
    (0x0655, 0),
    (0x0660, 946),
    (0x0661, 946),
    (0x0662, 946),
    (0x0663, 946),
    (0x0664, 946),
    (0x0665, 946),
    (0x0666, 946),
    (0x0667, 946),
    (0x0668, 946),
    (0x0669, 946),
    (0x2018, 532),
    (0x2019, 532),
    (0x201C, 981),
    (0x201D, 981),
    (0x2022, 725),
    (0x25CC, 981),
    (0xFBE8, 379),
    (0xFBE9, 379),
    (0xFC08, 870),
    (0xFC09, 1098),
    (0xFC0A, 1098),
    (0xFC0E, 870),
    (0xFC10, 1098),
    (0xFC12, 870),
    (0xFC31, 1098),
    (0xFC32, 1098),
    (0xFC3F, 1061),
    (0xFC40, 1061),
    (0xFC41, 1061),
    (0xFC42, 634),
    (0xFC43, 1098),
    (0xFC44, 1098),
    (0xFC4E, 870),
    (0xFC4F, 1098),
    (0xFC50, 1098),
    (0xFC58, 870),
    (0xFC59, 1098),
    (0xFC5E, 469),
    (0xFC60, 420),
    (0xFC61, 420),
    (0xFC6A, 946),
    (0xFC6D, 1247),
    (0xFC6E, 1098),
    (0xFC6F, 1098),
    (0xFC70, 946),
    (0xFC73, 1247),
    (0xFC74, 1098),
    (0xFC75, 1098),
    (0xFC8E, 1098),
    (0xFC8F, 1098),
    (0xFC91, 946),
    (0xFC94, 1247),
    (0xFC95, 1098),
    (0xFC9C, 946),
    (0xFC9D, 946),
    (0xFC9E, 946),
    (0xFC9F, 643),
    (0xFCA1, 946),
    (0xFCA2, 946),
    (0xFCA3, 946),
    (0xFCA4, 643),
    (0xFCA6, 643),
    (0xFCA8, 1183),
    (0xFCAA, 1183),
    (0xFCAC, 1183),
    (0xFCB0, 1098),
    (0xFCC9, 1000),
    (0xFCCA, 1000),
    (0xFCCB, 1000),
    (0xFCCC, 528),
    (0xFCCD, 719),
    (0xFCCE, 946),
    (0xFCCF, 946),
    (0xFCD0, 946),
    (0xFCD1, 719),
    (0xFCD2, 946),
    (0xFCD3, 946),
    (0xFCD4, 946),
    (0xFCD5, 643),
    (0xFCD8, 969),
    (0xFCDA, 946),
    (0xFCDB, 946),
    (0xFCDC, 946),
    (0xFCDD, 643),
    (0xFD30, 1098),
    (0xFD3E, 1135),
    (0xFD3F, 1135),
    (0xFD88, 983),
    (0xFDF2, 1319),
    (0xFDF4, 1585),
    (0xFDFA, 1818),
    (0xFDFB, 1604),
    (0xFDFC, 1364),
    (0xFDFD, 3562),
    (0xFE82, 535),
    (0xFE84, 448),
    (0xFE86, 719),
    (0xFE88, 420),
    (0xFE8A, 983),
    (0xFE8B, 379),
    (0xFE8C, 379),
    (0xFE8E, 420),
    (0xFE90, 1438),
    (0xFE91, 379),
    (0xFE92, 379),
    (0xFE94, 570),
    (0xFE96, 1438),
    (0xFE97, 379),
    (0xFE98, 379),
    (0xFE9A, 1438),
    (0xFE9B, 379),
    (0xFE9C, 379),
    (0xFE9E, 1020),
    (0xFE9F, 1020),
    (0xFEA0, 1098),
    (0xFEA2, 1020),
    (0xFEA3, 1020),
    (0xFEA4, 1098),
    (0xFEA6, 1020),
    (0xFEA7, 1020),
    (0xFEA8, 1098),
    (0xFEAA, 793),
    (0xFEAC, 793),
    (0xFEAE, 643),
    (0xFEB0, 643),
    (0xFEB2, 1491),
    (0xFEB3, 981),
    (0xFEB4, 1058),
    (0xFEB6, 1491),
    (0xFEB7, 981),
    (0xFEB8, 1058),
    (0xFEB9, 1632),
    (0xFEBA, 1710),
    (0xFEBB, 1182),
    (0xFEBC, 1221),
    (0xFEBE, 1710),
    (0xFEBF, 1182),
    (0xFEC0, 1221),
    (0xFEC1, 1135),
    (0xFEC2, 1171),
    (0xFEC3, 870),
    (0xFEC4, 907),
    (0xFEC6, 1171),
    (0xFEC7, 870),
    (0xFEC8, 907),
    (0xFECA, 793),
    (0xFECB, 756),
    (0xFECC, 606),
    (0xFECE, 793),
    (0xFECF, 756),
    (0xFED0, 606),
    (0xFED2, 1511),
    (0xFED3, 455),
    (0xFED4, 606),
    (0xFED6, 907),
    (0xFED7, 455),
    (0xFED8, 606),
    (0xFEDA, 1399),
    (0xFEDB, 946),
    (0xFEDC, 983),
    (0xFEDE, 907),
    (0xFEDF, 342),
    (0xFEE0, 416),
    (0xFEE2, 673),
    (0xFEE3, 606),
    (0xFEE4, 773),
    (0xFEE6, 907),
    (0xFEE7, 379),
    (0xFEE8, 379),
    (0xFEE9, 528),
    (0xFEEA, 569),
    (0xFEEB, 756),
    (0xFEEC, 719),
    (0xFEEE, 719),
    (0xFEF0, 983),
    (0xFEF2, 983),
    (0xFEF3, 379),
    (0xFEF4, 379),
    (0xFEF5, 946),
    (0xFEF6, 1173),
    (0xFEF7, 946),
    (0xFEF8, 1061),
    (0xFEF9, 946),
    (0xFEFA, 1061),
    (0xFEFB, 946),
    (0xFEFC, 1061),
];

/// Traditional Arabic Bold advances in font units (2048 per em), sorted by code point.
const TRADITIONAL_ARABIC_BOLD: &[(u16, u16)] = &[
    (0x0020, 500),
    (0x0021, 570),
    (0x0022, 570),
    (0x0025, 1835),
    (0x0028, 569),
    (0x0029, 569),
    (0x002D, 1062),
    (0x002E, 570),
    (0x002F, 570),
    (0x003A, 570),
    (0x004D, 1898),
    (0x0064, 1174),
    (0x0072, 811),
    (0x060C, 570),
    (0x0621, 657),
    (0x0623, 379),
    (0x0625, 379),
    (0x0626, 1173),
    (0x0627, 379),
    (0x0628, 1439),
    (0x0629, 570),
    (0x062A, 1439),
    (0x062C, 1136),
    (0x062F, 720),
    (0x0630, 720),
    (0x0631, 681),
    (0x0632, 681),
    (0x0633, 1400),
    (0x0634, 1400),
    (0x0635, 1665),
    (0x0636, 1665),
    (0x0639, 1099),
    (0x0640, 146),
    (0x0641, 1476),
    (0x0642, 1099),
    (0x0644, 984),
    (0x0645, 644),
    (0x0646, 908),
    (0x0648, 774),
    (0x0649, 1173),
    (0x064A, 1173),
    (0x064B, 408),
    (0x064F, 414),
    (0x0653, 621),
    (0x0654, 0),
    (0x0655, 0),
    (0xFBE8, 379),
    (0xFBE9, 420),
    (0xFC08, 908),
    (0xFC09, 1062),
    (0xFC0A, 1062),
    (0xFC0E, 908),
    (0xFC10, 1062),
    (0xFC12, 908),
    (0xFC31, 1099),
    (0xFC32, 1099),
    (0xFC3F, 1173),
    (0xFC40, 1173),
    (0xFC42, 644),
    (0xFC43, 1062),
    (0xFC44, 1062),
    (0xFC4E, 908),
    (0xFC4F, 1062),
    (0xFC50, 1062),
    (0xFC58, 908),
    (0xFC59, 1062),
    (0xFC6A, 1044),
    (0xFC6D, 1212),
    (0xFC6E, 1212),
    (0xFC6F, 1212),
    (0xFC70, 1044),
    (0xFC73, 1212),
    (0xFC74, 1212),
    (0xFC75, 1212),
    (0xFC8E, 1212),
    (0xFC8F, 1212),
    (0xFC91, 1044),
    (0xFC94, 1212),
    (0xFC95, 1212),
    (0xFC9C, 908),
    (0xFC9D, 908),
    (0xFC9E, 908),
    (0xFC9F, 712),
    (0xFCA1, 908),
    (0xFCA2, 908),
    (0xFCA3, 908),
    (0xFCA4, 681),
    (0xFCA6, 681),
    (0xFCA8, 1248),
    (0xFCAA, 1248),
    (0xFCAC, 1248),
    (0xFCB0, 1099),
    (0xFCC9, 1012),
    (0xFCCA, 1012),
    (0xFCCB, 1012),
    (0xFCCC, 633),
    (0xFCCD, 720),
    (0xFCCE, 975),
    (0xFCCF, 975),
    (0xFCD0, 975),
    (0xFCD1, 751),
    (0xFCD2, 908),
    (0xFCD3, 908),
    (0xFCD4, 908),
    (0xFCD5, 681),
    (0xFCD8, 1139),
    (0xFCDA, 908),
    (0xFCDB, 908),
    (0xFCDC, 908),
    (0xFCDD, 681),
    (0xFD30, 1099),
    (0xFD88, 984),
    (0xFDF2, 1416),
    (0xFDF4, 1717),
    (0xFDFA, 1822),
    (0xFDFB, 1692),
    (0xFDFC, 1477),
    (0xFDFD, 3615),
    (0xFE84, 425),
    (0xFE86, 780),
    (0xFE88, 425),
    (0xFE8A, 984),
    (0xFE8B, 379),
    (0xFE8C, 420),
    (0xFE8E, 425),
    (0xFE90, 1476),
    (0xFE91, 379),
    (0xFE92, 420),
    (0xFE94, 607),
    (0xFE96, 1476),
    (0xFE97, 379),
    (0xFE98, 420),
    (0xFE9A, 1476),
    (0xFE9B, 379),
    (0xFE9C, 420),
    (0xFE9E, 1099),
    (0xFE9F, 984),
    (0xFEA0, 1099),
    (0xFEA2, 1099),
    (0xFEA3, 984),
    (0xFEA4, 1099),
    (0xFEA7, 984),
    (0xFEA8, 1099),
    (0xFEAA, 834),
    (0xFEAC, 834),
    (0xFEAE, 720),
    (0xFEB0, 720),
    (0xFEB2, 1580),
    (0xFEB3, 984),
    (0xFEB4, 1140),
    (0xFEB6, 1580),
    (0xFEB7, 984),
    (0xFEB8, 1140),
    (0xFEB9, 1665),
    (0xFEBA, 1740),
    (0xFEBB, 1248),
    (0xFEBC, 1285),
    (0xFEBE, 1740),
    (0xFEBF, 1248),
    (0xFEC0, 1285),
    (0xFEC2, 1212),
    (0xFEC3, 871),
    (0xFEC4, 908),
    (0xFEC8, 908),
    (0xFEC9, 1099),
    (0xFECA, 871),
    (0xFECB, 793),
    (0xFECC, 607),
    (0xFECE, 871),
    (0xFECF, 793),
    (0xFED0, 607),
    (0xFED2, 1513),
    (0xFED3, 556),
    (0xFED4, 607),
    (0xFED6, 947),
    (0xFED7, 556),
    (0xFED8, 607),
    (0xFEDA, 1439),
    (0xFEDB, 908),
    (0xFEDC, 984),
    (0xFEDE, 970),
    (0xFEDF, 379),
    (0xFEE0, 500),
    (0xFEE1, 644),
    (0xFEE2, 773),
    (0xFEE3, 607),
    (0xFEE4, 793),
    (0xFEE6, 947),
    (0xFEE7, 379),
    (0xFEE8, 420),
    (0xFEE9, 570),
    (0xFEEA, 607),
    (0xFEEB, 756),
    (0xFEEC, 720),
    (0xFEEE, 780),
    (0xFEF0, 984),
    (0xFEF2, 984),
    (0xFEF3, 379),
    (0xFEF4, 420),
    (0xFEF5, 984),
    (0xFEF7, 984),
    (0xFEF8, 1099),
    (0xFEF9, 984),
    (0xFEFA, 1099),
    (0xFEFB, 984),
    (0xFEFC, 1099),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_sorted() {
        for t in [TRADITIONAL_ARABIC, TRADITIONAL_ARABIC_BOLD] {
            assert!(t.windows(2).all(|w| w[0].0 < w[1].0));
        }
        assert!(FORMS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn context_joins_across_the_edges() {
        // beh then "beh" (after context beh): the run's beh is medial.
        let w = missing_arabic_char_advances("Traditional Arabic", false, false, "\u{628}", "\u{628}", "\u{628}");
        assert_eq!(w, Some(vec![missing_arabic_advance("Traditional Arabic", false, false, '\u{628}', ArabicForm::Medial)]));
        // An alef after a lam in the text before draws nothing of its own.
        let w = missing_arabic_char_advances("Traditional Arabic", false, false, "\u{627}", "\u{644}", "");
        assert_eq!(w, Some(vec![Some(0.0)]));
        assert_eq!(missing_arabic_char_advances("Arial", false, false, "\u{627}", "", ""), None);
    }
}
