//! Word 6.0 and Word 95 documents (wIdent 0xA5DC, nFib 101..=105): the main text, its
//! paragraphs and the basic character formatting (font, bold, italic, size), with each run's
//! 8-bit text decoded in the code page of its font's charset.
//!
//! The layout follows the Word 97 one in [MS-DOC] for the shared structures (the Clx, the bin
//! tables, the CHPX FKP pages, the font table's FFN header), with the Word 6 differences
//! measured from real files: everything lives in the WordDocument stream (there is no Table
//! stream), pieces hold 8-bit text at a direct byte offset, bin tables have 16-bit page
//! numbers, FFNs carry an 8-bit name right after `ixchSzAlt`, and sprms have one-byte opcodes.
//! Paragraph alignment, indents, spacing, keeps and tabs are read from PAPX runs.
//! Styles, tables, sections, headers and footers, notes and pictures are
//! not read yet: the body text is all present, the other stories stay out of it.

use std::io::{Read, Seek};

use wordcraft_doc::Document;
use wordcraft_doc::encoding::{MAC_ROMAN_CODE_PAGE, charset_code_page, is_byte_code_page, windows_byte};
use wordcraft_doc::para::Paragraph;
use wordcraft_doc::props::{CharProps, ParaProps};

use crate::fib::Word6Fib;
use crate::fkp::Bins;
use crate::sprm::Prl;
use crate::table::{self, ParaOut};
use crate::{DocbinError, FieldBuild, MAX_TOTAL_CHARS, MAX_WORK, ParaBuild, Work, finish_field, fmt, metadata};

/// Most pieces any real file has; more means a corrupt `PlcPcd`.
const MAX_PIECES: usize = 1_000_000;
/// Most fonts read from the font table.
const MAX_FONTS: usize = 1024;
/// `chs` of a symbol font: its bytes are glyph codes, kept as U+F000 + byte like Word 97 symbols.
const SYMBOL_CHARSET: u8 = 2;
/// `chse` of a file written on the Macintosh.
const CHSE_MAC: u16 = 256;
/// Word 6 sprm opcodes read here.
const SPRM_FTC: u8 = 93;

/// One piece of 8-bit text: CPs `[cp, cp_end)` stored from byte `fc` on.
#[derive(Clone, Copy, Debug)]
struct Piece {
    cp: u32,
    cp_end: u32,
    fc: u32,
}

/// A font table entry: its name and charset.
#[derive(Clone, Debug, Default)]
struct Font {
    name: String,
    chs: u8,
}

/// How a run's bytes become characters.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Decode {
    /// A single-byte code page.
    CodePage(u16),
    /// A symbol font: U+F000 + byte.
    Symbol,
}

/// Read the main text of a Word 6.0/95 document whose WordDocument stream is `word`.
pub(crate) fn read<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, word: &[u8], fib: &Word6Fib) -> Result<Document, DocbinError> {
    let pieces = pieces(word, fib)?;
    let fonts = fonts(word, fib.sttbf_ffn);
    let font_names: Vec<String> = fonts.iter().map(|f| f.name.clone()).collect();
    let chpx = Bins::parse_word6(word, fib.bte_chpx.0, fib.bte_chpx.1);
    let papx = Bins::parse_word6(word, fib.bte_papx.0, fib.bte_papx.1);
    let mac = fib.chse == CHSE_MAC;
    let work = Work::new(MAX_WORK);
    // Every character comes from a byte of the stream; aliased pieces can't multiply it past this.
    let mut budget = (word.len() as u64).saturating_mul(2).min(MAX_TOTAL_CHARS);

    let mut out: Vec<ParaOut> = Vec::new();
    let mut pb = ParaBuild::default();
    let mut field: Option<FieldBuild> = None;
    // The last CHPX resolved (by address in the stream), its properties and decoding.
    let mut memo: Option<(*const u8, usize, CharProps, Decode)> = None;
    let cp_text = fib.ccp.text;
    'pieces: for p in pieces.iter().filter(|p| p.cp < cp_text) {
        let end = p.cp_end.min(cp_text);
        for cp in p.cp..end {
            let Some(fc) = p.fc.checked_add(cp - p.cp) else { break };
            let Some(&b) = word.get(fc as usize) else { break };
            if budget == 0 {
                break 'pieces;
            }
            budget -= 1;
            let grpprl = chpx.chpx(word, fc);
            let key = (grpprl.as_ptr(), grpprl.len());
            let (props, decode) = match &memo {
                Some((ptr, len, props, decode)) if (*ptr, *len) == key => (props.clone(), *decode),
                _ => {
                    let (props, decode) = char_props(grpprl, &fonts, &font_names, mac);
                    memo = Some((key.0, key.1, props.clone(), decode));
                    (props, decode)
                }
            };
            match b {
                0x0D | 0x07 => {
                    pb.mark = props;
                    apply_para_props(&mut pb.props, papx.papx_word6(word, fc));
                    let done = std::mem::take(&mut pb);
                    out.push(para_out(done, cp.saturating_add(1)));
                }
                // Field begin / separator / end: the result text is kept, as for Word 97.
                0x13 => match field.as_mut() {
                    Some(f) => f.depth = f.depth.saturating_add(1),
                    None => field = Some(FieldBuild { depth: 1, instr: String::new(), sep: None }),
                },
                0x14 => {
                    if let Some(f) = field.as_mut()
                        && f.depth == 1
                        && f.sep.is_none()
                    {
                        f.sep = Some(Vec::new());
                    }
                }
                0x15 => {
                    if let Some(mut f) = field.take() {
                        if f.depth > 1 {
                            f.depth -= 1;
                            field = Some(f);
                        } else {
                            finish_field(&mut pb, f, &work);
                        }
                    }
                }
                0x09 => push(&mut pb, field.as_mut(), '\t', props),
                0x0B => push(&mut pb, field.as_mut(), '\n', props),
                0x0C => push(&mut pb, field.as_mut(), '\u{C}', props),
                0x0E => push(&mut pb, field.as_mut(), '\u{E}', props),
                0x1E => push(&mut pb, field.as_mut(), '\u{2011}', props),
                0x1F => push(&mut pb, field.as_mut(), '\u{AD}', props),
                // Pictures, note references and other anchors are not read yet.
                b if b < 0x20 => {}
                b => push(&mut pb, field.as_mut(), decode_byte(decode, b), props),
            }
        }
    }
    if !pb.text.is_empty() {
        out.push(para_out(pb, cp_text));
    }

    let mut doc = Document::new();
    if let Some(core) = metadata::read(comp) {
        doc.core = core;
    }
    doc.body = table::assemble(out);
    doc.ensure_nonempty();
    work.log();
    Ok(doc)
}

fn para_out(done: ParaBuild, end_cp: u32) -> ParaOut {
    ParaOut {
        para: Paragraph { text: done.text, runs: done.runs, props: done.props, mark: done.mark, objects: done.objects, ..Default::default() },
        end_cp,
        row: table::RowInfo::default(),
    }
}

/// Add a visible character, or record it for the field being read.
fn push(pb: &mut ParaBuild, field: Option<&mut FieldBuild>, c: char, props: CharProps) {
    match field {
        Some(f) => match f.sep.as_mut() {
            Some(result) => result.push((c, props)),
            None => f.instr.push(c),
        },
        None => pb.push(c, props),
    }
}

fn decode_byte(decode: Decode, b: u8) -> char {
    match decode {
        Decode::Symbol => char::from_u32(0xF000 + u32::from(b)).unwrap_or('\u{FFFD}'),
        Decode::CodePage(cp) => windows_byte(cp, b),
    }
}

/// How the text of a font with charset `chs` decodes. The ANSI and default charsets use the
/// file's own character set (Windows-1252, or Mac OS Roman for a Macintosh file). The East
/// Asian multibyte code pages are not decoded yet and read as Windows-1252.
fn decode_of(chs: u8, mac: bool) -> Decode {
    let file = if mac { MAC_ROMAN_CODE_PAGE } else { 1252 };
    if chs == SYMBOL_CHARSET {
        return Decode::Symbol;
    }
    match charset_code_page(i32::from(chs)) {
        Some(cp) if is_byte_code_page(cp) => Decode::CodePage(cp),
        Some(_) => Decode::CodePage(1252),
        None => Decode::CodePage(file),
    }
}

/// Character properties and decoding of a CHPX grpprl of Word 6 sprms (one-byte opcodes).
/// Without a font sprm the text decodes in the charset of font 0, the default font.
fn char_props(grpprl: &[u8], fonts: &[Font], names: &[String], mac: bool) -> (CharProps, Decode) {
    let base = CharProps::default();
    let mut props = CharProps::default();
    let mut decode = decode_of(fonts.first().map_or(0, |f| f.chs), mac);
    let mut rest = grpprl;
    while let Some((&op, after)) = rest.split_first() {
        let Some(len) = operand_len(op) else { break };
        let Some(operand) = after.get(..len) else { break };
        rest = after.get(len..).unwrap_or(&[]);
        if op == SPRM_FTC {
            let ftc = match operand {
                [a, b] => usize::from(u16::from_le_bytes([*a, *b])),
                _ => continue,
            };
            if let Some(font) = fonts.get(ftc) {
                props.font = Some(font.name.clone());
                decode = decode_of(font.chs, mac);
            }
            continue;
        }
        if let Some(op97) = word97_op(op) {
            fmt::apply_char(&mut props, &Prl { op: op97, operand }, &base, names);
        }
    }
    (props, decode)
}

/// Translate Word 6 paragraph operands for the shared Word 97 property applier.
/// Unknown opcodes end the group because their operand sizes cannot be inferred.
fn apply_para_props(props: &mut ParaProps, mut rest: &[u8]) {
    while let Some((&op, after)) = rest.split_first() {
        let (len, op97) = match op {
            5 => (1, Some(0x2403)),
            7 => (1, Some(0x2405)),
            8 => (1, Some(0x2406)),
            9 => (1, Some(0x2407)),
            15 => (usize::from(after.first().copied().unwrap_or(0)).saturating_add(1), Some(0xC60D)),
            16 => (2, Some(0x840E)),
            17 => (2, Some(0x840F)),
            19 => (2, Some(0x8411)),
            20 => (4, Some(0x6412)),
            21 => (2, Some(0xA413)),
            22 => (2, Some(0xA414)),
            51 => (1, None),
            2 => (2, None),
            13 | 14 | 24 | 25 => (1, None),
            12 | 23 => (usize::from(after.first().copied().unwrap_or(0)).saturating_add(1), None),
            _ => break,
        };
        let Some(operand) = after.get(..len) else { break };
        rest = after.get(len..).unwrap_or(&[]);
        if op == 51 {
            props.widow_control = operand.first().map(|&v| v != 0);
        } else if let Some(op) = op97 {
            fmt::apply_para(props, &Prl { op, operand });
        }
    }
}

/// Operand size of the Word 6 character sprms 80..=102, which keep the order and sizes of the
/// Word 97 sprms with `ispmd` 0x30..=0x46 ([MS-DOC] §2.6.1): sprmCIstd (2), sprmCKcd (1), the
/// toggles bold … vanish (1), the font (2), underline (1), size/position (3), spacing (2),
/// language (2), colour (1), size (2), size increment (1), position (2), position adjust (1).
/// `None` for anything else, whose size isn't known: the rest of the grpprl is then skipped.
fn operand_len(op: u8) -> Option<usize> {
    Some(match op {
        80 => 2,
        84..=92 => 1,
        93 => 2,
        94 => 1,
        95 => 3,
        96 | 97 => 2,
        98 => 1,
        99 => 2,
        100 => 1,
        101 => 2,
        102 => 1,
        _ => return None,
    })
}

/// The Word 97 sprm with the same operand for a Word 6 character sprm WordCraft applies.
fn word97_op(op: u8) -> Option<u16> {
    Some(match op {
        85 => 0x0835, // bold
        86 => 0x0836, // italic
        87 => 0x0837, // strike
        90 => 0x083A, // small caps
        91 => 0x083B, // caps
        92 => 0x083C, // hidden
        94 => 0x2A3E, // underline
        98 => 0x2A42, // colour index
        99 => 0x4A43, // size in half-points
        _ => return None,
    })
}

/// The pieces of the main text, in CP order: one piece at `fcMin` for a non-complex file, the
/// Clx's piece table for a complex one (Prcs skipped).
fn pieces(word: &[u8], fib: &Word6Fib) -> Result<Vec<Piece>, DocbinError> {
    let total = u32::try_from(fib.ccp.total()?).unwrap_or(u32::MAX);
    if !fib.complex {
        return Ok(vec![Piece { cp: 0, cp_end: total, fc: fib.fc_min }]);
    }
    let (fc, lcb) = (fib.clx.0 as usize, fib.clx.1 as usize);
    let clx = fc
        .checked_add(lcb)
        .and_then(|end| word.get(fc..end))
        .filter(|c| !c.is_empty())
        .ok_or_else(|| DocbinError::Malformed("the Word 6.0/95 FIB has no readable piece table (Clx)".into()))?;
    let mut i = 0usize;
    let plc = loop {
        match clx.get(i) {
            Some(0x01) => {
                let cb = clx
                    .get(i + 1..i + 3)
                    .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
                    .ok_or_else(|| DocbinError::Malformed("truncated Prc".into()))?;
                i = i.saturating_add(3).saturating_add(cb);
            }
            Some(0x02) => {
                let len = clx
                    .get(i + 1..i + 5)
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
                    .ok_or_else(|| DocbinError::Malformed("truncated Pcdt".into()))?;
                let start = i.saturating_add(5);
                break clx.get(start..start.saturating_add(len)).ok_or_else(|| DocbinError::Malformed("PlcPcd is outside the Clx".into()))?;
            }
            Some(other) => return Err(DocbinError::Malformed(format!("unexpected Clx item 0x{other:02X}"))),
            None => return Err(DocbinError::Malformed("Clx ends before the Pcdt".into())),
        }
    };
    if plc.len() < 4 || !(plc.len() - 4).is_multiple_of(12) {
        return Err(DocbinError::Malformed(format!("PlcPcd is {} bytes", plc.len())));
    }
    let n = (plc.len() - 4) / 12;
    if n > MAX_PIECES {
        return Err(DocbinError::Limit(format!("piece table has {n} pieces")));
    }
    let u32at = |at: usize| plc.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let mut out = Vec::with_capacity(n);
    // CPs are kept non-decreasing: an inverted piece in a corrupt file becomes an empty one.
    let mut prev = 0u32;
    for j in 0..n {
        let (Some(a), Some(b), Some(fc)) = (u32at(j * 4), u32at(j * 4 + 4), u32at((n + 1) * 4 + j * 8 + 2)) else { break };
        let cp = a.max(prev);
        let cp_end = b.max(cp);
        prev = cp_end;
        if cp_end > cp {
            out.push(Piece { cp, cp_end, fc });
        }
    }
    Ok(out)
}

/// The font table (`SttbfFfn`, in the WordDocument stream): u16 total size, then FFNs of
/// cbFfnM1, flags, wWeight (2), chs, ixchSzAlt, and an 8-bit NUL-terminated name.
fn fonts(word: &[u8], (fc, lcb): (u32, u32)) -> Vec<Font> {
    let Some(t) = (fc as usize).checked_add(lcb as usize).and_then(|end| word.get(fc as usize..end)) else { return Vec::new() };
    let total = t.get(..2).map_or(0, |b| usize::from(u16::from_le_bytes([b[0], b[1]]))).min(t.len());
    let mut out = Vec::new();
    let mut at = 2usize;
    while at < total && out.len() < MAX_FONTS {
        let Some(&cb) = t.get(at) else { break };
        let ffn = t.get(at + 1..(at + 1 + usize::from(cb)).min(total)).unwrap_or(&[]);
        let chs = ffn.get(3).copied().unwrap_or(0);
        let cp = match decode_of(chs, false) {
            Decode::CodePage(cp) => cp,
            Decode::Symbol => 1252,
        };
        let name = ffn.get(5..).unwrap_or(&[]).iter().take_while(|&&b| b != 0).take(64).map(|&b| windows_byte(cp, b)).collect();
        out.push(Font { name, chs });
        at = at.saturating_add(1).saturating_add(usize::from(cb));
    }
    out
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn charsets_choose_the_code_page() {
        assert_eq!(decode_of(204, false), Decode::CodePage(1251));
        assert_eq!(decode_of(0, false), Decode::CodePage(1252));
        assert_eq!(decode_of(0, true), Decode::CodePage(MAC_ROMAN_CODE_PAGE));
        assert_eq!(decode_of(77, false), Decode::CodePage(MAC_ROMAN_CODE_PAGE));
        assert_eq!(decode_of(2, false), Decode::Symbol);
        assert_eq!(decode_of(238, false), Decode::CodePage(1250));
    }

    #[test]
    fn hostile_grpprls_and_tables_never_panic() {
        let fonts = vec![Font { name: "A".into(), chs: 204 }];
        let names = vec!["A".to_string()];
        for g in [&[][..], &[93], &[93, 0xFF, 0xFF], &[0xFF, 1, 2], &[99, 1], &[85]] {
            let _ = char_props(g, &fonts, &names, false);
        }
        assert!(fonts_of(&[0xFF, 0xFF, 0x30, 0x00]).len() <= MAX_FONTS);
        let fib = Word6Fib { complex: true, clx: (0, 5), ..Default::default() };
        assert!(pieces(&[0x01, 0xFF, 0xFF, 0x02, 0x00], &fib).is_err());
        let fib = Word6Fib { complex: true, clx: (u32::MAX, u32::MAX), ..Default::default() };
        assert!(pieces(&[0u8; 8], &fib).is_err());
    }

    #[test]
    fn paragraph_operand_boundaries_and_skipped_records() {
        use wordcraft_doc::props::Align;
        let mut props = ParaProps::default();
        apply_para_props(&mut props, &[2, 0, 0, 12, 2, 5, 3, 13, 0, 14, 0, 23, 1, 0, 5, 1, 0]);
        assert_eq!(props.align, Some(Align::Center));
        for g in [&[5][..], &[17, 1], &[20, 1, 2, 3], &[15], &[15, 5, 0], &[12, 255], &[23, 255], &[51]] {
            let mut props = ParaProps::default();
            apply_para_props(&mut props, g);
            assert_eq!(props, ParaProps::default());
        }
    }

    fn fonts_of(t: &[u8]) -> Vec<Font> {
        fonts(t, (0, t.len() as u32))
    }
}
