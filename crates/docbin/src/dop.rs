//! The Document Properties (`Dop`, [MS-DOC] §2.7) in the Table stream, located by the FIB's
//! `fcDop`/`lcbDop`. The `DopBase` document options and the compatibility options the layout
//! engine models are read; every option whose containing structure lies beyond `lcbDop` keeps its
//! default, except that Dop95/Dop97 predate HTML paragraph auto spacing and therefore sum paragraph spacing.

use wordcraft_doc::Settings;
use wordcraft_doc::section::{MAX_NOTE_START, NotePos, NoteRestart};

/// Offset of `Dop95.copts80` (after the 84-byte `DopBase`, [MS-DOC] §2.7, Dop95); its first two
/// bytes are `Copts60` (§2.7.2).
const OFF_COPTS60: usize = 84;
/// `Copts60.fExpShRtn` (bit N, the 14th bit; [MS-DOC] §2.7.2 Copts60).
const F_EXP_SH_RTN: u16 = 1 << 13;
/// `Copts60.fNoTabForInd` (bit A, the 1st bit; [MS-DOC] §2.7.2 Copts60): no implicit tab stop at
/// a hanging indent.
const F_NO_TAB_FOR_IND: u16 = 1 << 0;
/// `Copts60.fWrapTrailSpaces` (bit D, the 4th bit; [MS-DOC] §2.7.2 Copts60): spaces past the
/// right edge wrap to the next line.
const F_WRAP_TRAIL_SPACES: u16 = 1 << 3;
/// Offset of `Dop95.copts80`'s second u16 (after `Copts60`; [MS-DOC] §2.7.3 Copts80).
const OFF_COPTS80_HI: usize = 86;
/// `Copts80.f2ptExtLeadingOnly` (bit G of the u16 at Dop offset 86; [MS-DOC] §2.7.3).
const F_2PT_EXT_LEADING_ONLY: u16 = 1 << 6;
/// `Copts80.fNoExtLeading` (bit D of the u16 at Dop offset 86; [MS-DOC] §2.7.3): "Don't add
/// leading (extra space) between rows of text".
const F_NO_EXT_LEADING: u16 = 1 << 3;
/// `Copts80.fWPJust` (bit O of the u16 at Dop offset 86; [MS-DOC] §2.7.3): WordPerfect-style
/// justification.
const F_WP_JUST: u16 = 1 << 14;
/// Size of `Dop95` ([MS-DOC] §2.7, Dop95): `DopBase` (84) + `Copts80` (4).
const DOP95_LEN: usize = 88;
/// Offset of `Dop2000.copts` ([MS-DOC] §2.7, Dop2000): after `dop97` (500 bytes), the two
/// `ilvlLast*` bytes, `istdClickParaType` (2 bytes) and the 32-bit web-option flags.
const OFF_COPTS: usize = 508;
/// Offset of the second 32-bit word of `Copts` (the flags after `copts80`, [MS-DOC] §2.7.1).
const OFF_COPTS_FLAGS: usize = OFF_COPTS + 4;
/// `Copts.fFtnLayoutLikeWW8` (bit B of the word after `copts80`; [MS-DOC] §2.7.1): lay out
/// footnotes as Word 6.x/95/97 did.
const F_FTN_LAYOUT_LIKE_WW8: u32 = 1 << 1;
/// `Copts.fDontUseHTMLParagraphAutoSpacing` (bit C of the word after `copts80`; [MS-DOC] §2.7.1).
const F_DONT_USE_HTML_AUTO_SPACING: u32 = 1 << 2;
/// `Copts.fAllowSpaceOfSameStyleInTable` (bit V of the word after `copts80`; [MS-DOC] §2.7.1).
const F_ALLOW_SPACE_OF_SAME_STYLE_IN_TABLE: u32 = 1 << 21;
/// Size of `Dop2000` ([MS-DOC] §2.7, Dop2000).
const DOP2000_LEN: usize = 544;
/// Size of `Dop2003` ([MS-DOC] §2.7, Dop2003); the Word 2003 options of `Copts` are only meaningful in
/// a DOP at least this long.
const DOP2003_LEN: usize = 616;
/// Size of `Dop2007` ([MS-DOC] §2.7, Dop2007): a shorter DOP is a Word 97-2003 file.
const DOP2007_LEN: usize = 674;
/// `Copts.fSplitPgBreakAndParaMark` (bit 27 of the word after `copts80`; [MS-DOC] §2.7.1): the
/// paragraph mark after a page break starts the new page.
const F_SPLIT_PG_BREAK_AND_PARA_MARK: u32 = 1 << 27;

/// Bytes of the `DopBase` ([MS-DOC] §2.7.3) holding every option `apply_base` reads.
const DOPBASE_READ_LEN: usize = 20;
/// `DopBase.fFacingPages` (byte 0, bit A; [MS-DOC] §2.7.3): Different odd and even pages.
const OFF_FACING: usize = 0;
const F_FACING_PAGES: u8 = 0x01;
/// The byte of `DopBase` holding `fHyphCapitals`, `fAutoHyphen` and `fRevMarking` ([MS-DOC] §2.7.3).
const OFF_HYPH_REV: usize = 5;
/// `DopBase.fHyphCapitals` ([MS-DOC] §2.7.3).
const F_HYPH_CAPITALS: u8 = 0x08;
/// `DopBase.fAutoHyphen` ([MS-DOC] §2.7.3).
const F_AUTO_HYPHEN: u8 = 0x10;
/// `DopBase.fRevMarking` ([MS-DOC] §2.7.3): Track Changes is on.
const F_REV_MARKING: u8 = 0x80;
/// The byte of `DopBase` holding `fMirrorMargins` ([MS-DOC] §2.7.3).
const OFF_MIRROR: usize = 6;
/// `DopBase.fMirrorMargins` ([MS-DOC] §2.7.3).
const F_MIRROR_MARGINS: u8 = 0x20;
/// `DopBase.dxaTab` ([MS-DOC] §2.7.3): the default tab stop interval in twips.
const OFF_DXA_TAB: usize = 10;
/// `DopBase.dxaHotZ` ([MS-DOC] §2.7.3): the hyphenation zone in twips.
const OFF_DXA_HOT_Z: usize = 14;
/// `DopBase.cConsecHypLim` ([MS-DOC] §2.7.3): consecutive hyphenated lines, 0 = no limit.
const OFF_CONSEC_HYP_LIM: usize = 16;
/// The largest default tab or hyphenation zone taken from a file: 22 inches in twips.
const MAX_DXA: u16 = 31_680;

/// Apply the `DopBase` document options ([MS-DOC] §2.7.3): the default tab stop, Different odd
/// and even pages, mirror margins, the hyphenation options and Track Changes. A DOP shorter than
/// the fields read leaves every option at its default; a zero or out-of-range `dxaTab`/`dxaHotZ`
/// keeps its default.
fn apply_base(dop: &[u8], settings: &mut Settings) {
    if dop.len() < DOPBASE_READ_LEN {
        return;
    }
    let byte = |at: usize| dop.get(at).copied().unwrap_or(0);
    settings.even_odd_headers = byte(OFF_FACING) & F_FACING_PAGES != 0;
    let hyph_rev = byte(OFF_HYPH_REV);
    settings.hyphenate_caps = hyph_rev & F_HYPH_CAPITALS != 0;
    settings.auto_hyphenation = hyph_rev & F_AUTO_HYPHEN != 0;
    settings.track_changes = hyph_rev & F_REV_MARKING != 0;
    settings.mirror_margins = byte(OFF_MIRROR) & F_MIRROR_MARGINS != 0;
    if let Some(tab) = u16_at(dop, OFF_DXA_TAB).filter(|t| (1..=MAX_DXA).contains(t)) {
        settings.default_tab = f32::from(tab) / 20.0;
    }
    if let Some(zone) = u16_at(dop, OFF_DXA_HOT_Z).filter(|z| (1..=MAX_DXA).contains(z)) {
        settings.hyphenation_zone = f32::from(zone) / 20.0;
    }
    if let Some(limit) = u16_at(dop, OFF_CONSEC_HYP_LIM) {
        settings.consecutive_hyphen_limit = u32::from(limit);
    }
}

/// The last version whose DOP holds the document-wide note options (Word 2000, nFib 0x00D9).
/// Later versions keep them in each section's sprms and the DOP values are to be ignored
/// ([MS-DOC] §2.7.2 DopBase: fpc, rncFtn, nFtn, rncEdn, nEdn, epc; §2.7.4 Dop97).
const NOTE_OPTIONS_LAST_NFIB: u16 = 0x00D9;
/// `DopBase.fpc` (byte 0, bits 5-6; [MS-DOC] §2.7.2): 2 = beneath text.
const FPC_BENEATH_TEXT: u8 = 2;
/// `DopBase` u16 holding `rncFtn` (bits 0-1) and `nFtn` (bits 2-15) ([MS-DOC] §2.7.2).
const OFF_FTN: usize = 2;
/// `DopBase` u16 holding `rncEdn` (bits 0-1) and `nEdn` (bits 2-15) ([MS-DOC] §2.7.2).
const OFF_EDN: usize = 52;
/// `DopBase` byte holding `epc` (bits 0-1; [MS-DOC] §2.7.2): 0 = end of section.
const OFF_EPC: usize = 54;
/// `Dop97.nfcFtnRef` and `Dop97.nfcEdnRef` ([MS-DOC] §2.7.4): MSONFC number formats.
const OFF_NFC_FTN_REF: usize = 492;
const OFF_NFC_EDN_REF: usize = 494;
/// Size of `Dop97` ([MS-DOC] §2.7.4).
const DOP97_LEN: usize = 500;

/// `rnc` ([MS-DOC] §2.7.2): 1 restarts each section, 2 each page; 0 (continuous) is the default.
fn note_restart(rnc: u16) -> Option<NoteRestart> {
    match rnc & 3 {
        1 => Some(NoteRestart::EachSect),
        2 => Some(NoteRestart::EachPage),
        _ => None,
    }
}

/// The start number in bits 2-15; 1 (the default) and 0 stay unset.
fn note_start(word: u16) -> Option<u32> {
    Some(u32::from(word >> 2)).filter(|n| (2..=MAX_NOTE_START).contains(n))
}

/// Apply the Word 97/2000 document-wide footnote and endnote options ([MS-DOC] §2.7.2, §2.7.4).
/// Values equal to Word's defaults stay unset; fields beyond the DOP's length are skipped.
fn apply_notes(dop: &[u8], settings: &mut Settings) {
    if let Some(&b0) = dop.first()
        && let Some(ftn) = u16_at(dop, OFF_FTN)
    {
        if (b0 >> 5) & 3 == FPC_BENEATH_TEXT {
            settings.footnote_pr.pos = Some(NotePos::BeneathText);
        }
        settings.footnote_pr.num_restart = note_restart(ftn);
        settings.footnote_pr.num_start = note_start(ftn);
    }
    if let Some(edn) = u16_at(dop, OFF_EDN)
        && let Some(&epc) = dop.get(OFF_EPC)
    {
        settings.endnote_pr.num_restart = note_restart(edn);
        settings.endnote_pr.num_start = note_start(edn);
        if epc & 3 == 0 {
            settings.endnote_pr.pos = Some(NotePos::SectEnd);
        }
    }
    if dop.len() >= DOP97_LEN {
        let nfc = |at| u16_at(dop, at).and_then(|v| u8::try_from(v).ok()).map(crate::list::num_format);
        if let Some(f) = nfc(OFF_NFC_FTN_REF) {
            settings.footnote_format = f;
        }
        if let Some(f) = nfc(OFF_NFC_EDN_REF) {
            settings.endnote_format = f;
        }
    }
}

/// Apply the DOP's compatibility options to `settings`. A DOP outside the Table stream is
/// ignored (the defaults stay). A valid Dop95/Dop97 sums paragraph spacing because HTML
/// paragraph auto spacing was introduced with Dop2000. A DOP shorter than Dop2007 splits a page
/// break from its paragraph mark. Files written by Word 2000 or earlier (`version`, the FIB's
/// nFib) also carry the document-wide note options.
pub(crate) fn apply(table: &[u8], pair: Option<(u32, u32)>, settings: &mut Settings, version: u16) {
    let Some((fc, lcb)) = pair else { return };
    let (Ok(fc), Ok(lcb)) = (usize::try_from(fc), usize::try_from(lcb)) else { return };
    let Some(dop) = fc.checked_add(lcb).and_then(|end| table.get(fc..end)) else { return };
    if version <= NOTE_OPTIONS_LAST_NFIB {
        apply_notes(dop, settings);
    }
    apply_base(dop, settings);
    if dop.len() >= DOP95_LEN
        && let Some(copts60) = u16_at(dop, OFF_COPTS60)
    {
        settings.do_not_expand_shift_return = copts60 & F_EXP_SH_RTN != 0;
        settings.no_tab_hang_ind = copts60 & F_NO_TAB_FOR_IND != 0;
        settings.wrap_trail_spaces = copts60 & F_WRAP_TRAIL_SPACES != 0;
    }
    if dop.len() >= DOP95_LEN
        && let Some(copts80) = u16_at(dop, OFF_COPTS80_HI)
    {
        settings.ext_leading_2pt_only = copts80 & F_2PT_EXT_LEADING_ONLY != 0;
        settings.no_leading = copts80 & F_NO_EXT_LEADING != 0;
        settings.wp_justification = copts80 & F_WP_JUST != 0;
    }
    if (DOP95_LEN..DOP2000_LEN).contains(&dop.len()) {
        settings.sum_paragraph_spacing = true;
    }
    if dop.len() >= DOP2000_LEN
        && let Some(flags) = u32_at(dop, OFF_COPTS_FLAGS)
    {
        settings.sum_paragraph_spacing = flags & F_DONT_USE_HTML_AUTO_SPACING != 0;
        settings.footnote_layout_like_ww8 = flags & F_FTN_LAYOUT_LIKE_WW8 != 0;
        if dop.len() >= DOP2003_LEN {
            settings.allow_space_of_same_style_in_table = flags & F_ALLOW_SPACE_OF_SAME_STYLE_IN_TABLE != 0;
        }
    }
    // Word 97-2003 always split a page break from the paragraph mark after it; from Dop2007 on
    // the option is a `Copts` flag. An empty DOP (`lcbDop` 0) is no DOP.
    settings.split_pg_break_and_para_mark =
        !dop.is_empty() && (dop.len() < DOP2007_LEN || u32_at(dop, OFF_COPTS_FLAGS).is_some_and(|flags| flags & F_SPLIT_PG_BREAK_AND_PARA_MARK != 0));
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([*s.first()?, *s.get(1)?]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([*s.first()?, *s.get(1)?, *s.get(2)?, *s.get(3)?]))
}
