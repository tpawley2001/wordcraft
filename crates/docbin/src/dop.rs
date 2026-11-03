//! The Document Properties (`Dop`, [MS-DOC] §2.7) in the Table stream, located by the FIB's
//! `fcDop`/`lcbDop`. Only the compatibility options the layout engine models are read; every
//! option whose containing structure lies beyond `lcbDop` keeps its default.

use wordcraft_doc::Settings;

/// Offset of `Dop95.copts80` (after the 84-byte `DopBase`, [MS-DOC] §2.7, Dop95); its first two
/// bytes are `Copts60` (§2.7.2).
const OFF_COPTS60: usize = 84;
/// `Copts60.fExpShRtn` (bit N, the 14th bit; [MS-DOC] §2.7.2 Copts60).
const F_EXP_SH_RTN: u16 = 1 << 13;
/// Size of `Dop95` ([MS-DOC] §2.7, Dop95): `DopBase` (84) + `Copts80` (4).
const DOP95_LEN: usize = 88;
/// Offset of `Dop2000.copts` ([MS-DOC] §2.7, Dop2000): after `dop97` (500 bytes), the two
/// `ilvlLast*` bytes, `istdClickParaType` (2 bytes) and the 32-bit web-option flags.
const OFF_COPTS: usize = 508;
/// Offset of the second 32-bit word of `Copts` (the flags after `copts80`, [MS-DOC] §2.7.1).
const OFF_COPTS_FLAGS: usize = OFF_COPTS + 4;
/// `Copts.fDontUseHTMLParagraphAutoSpacing` (bit C of the word after `copts80`; [MS-DOC] §2.7.1).
const F_DONT_USE_HTML_AUTO_SPACING: u32 = 1 << 2;
/// `Copts.fAllowSpaceOfSameStyleInTable` (bit V of the word after `copts80`; [MS-DOC] §2.7.1).
const F_ALLOW_SPACE_OF_SAME_STYLE_IN_TABLE: u32 = 1 << 21;
/// Size of `Dop2000` ([MS-DOC] §2.7, Dop2000).
const DOP2000_LEN: usize = 544;
/// Size of `Dop2003` ([MS-DOC] §2.7, Dop2003); the Word 2003 options of `Copts` are only meaningful in
/// a DOP at least this long.
const DOP2003_LEN: usize = 616;

/// Apply the DOP's compatibility options to `settings`. A DOP outside the Table stream is
/// ignored (the defaults stay).
pub(crate) fn apply(table: &[u8], pair: Option<(u32, u32)>, settings: &mut Settings) {
    let Some((fc, lcb)) = pair else { return };
    let (Ok(fc), Ok(lcb)) = (usize::try_from(fc), usize::try_from(lcb)) else { return };
    let Some(dop) = fc.checked_add(lcb).and_then(|end| table.get(fc..end)) else { return };
    if dop.len() >= DOP95_LEN {
        if let Some(copts60) = u16_at(dop, OFF_COPTS60) {
            settings.do_not_expand_shift_return = copts60 & F_EXP_SH_RTN != 0;
        }
    }
    if dop.len() >= DOP2000_LEN {
        if let Some(flags) = u32_at(dop, OFF_COPTS_FLAGS) {
            settings.sum_paragraph_spacing = flags & F_DONT_USE_HTML_AUTO_SPACING != 0;
            if dop.len() >= DOP2003_LEN {
                settings.allow_space_of_same_style_in_table = flags & F_ALLOW_SPACE_OF_SAME_STYLE_IN_TABLE != 0;
            }
        }
    }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([*s.first()?, *s.get(1)?]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([*s.first()?, *s.get(1)?, *s.get(2)?, *s.get(3)?]))
}
