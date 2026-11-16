//! Plain text: UTF-8 (with or without BOM), UTF-16 LE/BE (BOM or sniffed), else the legacy
//! encoding the bytes look like (GBK/GB18030, Big5, Shift_JIS, EUC-KR, Windows code pages…;
//! #778), Windows-1252 when nothing fits better.
//! Export writes UTF-8 with CRLF line ends ([`export_with`]: the encoding, line ends and
//! character substitution of Word's File Conversion dialog); list paragraphs get their label and a tab, table
//! cells are separated by tabs.

use wordcraft_doc::Document;

pub use wordcraft_doc::encoding::cp1252;

use crate::model::{self, FBlock, Inline, ListCounter};

fn utf16(b: &[u8], le: bool) -> String {
    let units: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| if le { u16::from_le_bytes(*c) } else { u16::from_be_bytes(*c) }).collect();
    String::from_utf16_lossy(&units)
}

/// UTF-16 without a BOM, told by its zero bytes: `Some(true)` little-endian, `Some(false)`
/// big-endian.
fn unmarked_utf16(b: &[u8]) -> Option<bool> {
    if b.len() < 4 || !b.len().is_multiple_of(2) {
        return None;
    }
    let sample = b.get(..b.len().min(512))?;
    let even_zero = sample.iter().step_by(2).filter(|x| **x == 0).count();
    let odd_zero = sample.iter().skip(1).step_by(2).filter(|x| **x == 0).count();
    let half = sample.len() / 2;
    if odd_zero * 10 >= half * 4 && even_zero * 10 < half {
        return Some(true);
    }
    if even_zero * 10 >= half * 4 && odd_zero * 10 < half {
        return Some(false);
    }
    None
}

/// Does `b` look like plain text, so a file with an extension WordCraft doesn't know (a
/// playlist, a log, a config file…) can open as text the way Word's text import opens it?
///
/// Text has a BOM, or reads as UTF-16 by its zero bytes, or has no NUL byte and next to no
/// other control characters in its first 64 KB. PDFs are refused by their signature (they
/// start as text). An empty file is text.
pub fn is_text(b: &[u8]) -> bool {
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) || b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        return true;
    }
    if b.starts_with(b"%PDF-") {
        return false;
    }
    let sample = b.get(..b.len().min(64 << 10)).unwrap_or(b);
    // Tab, line feed, form feed, carriage return, DOS end-of-file (^Z) and escape appear in text;
    // other controls in more than 1 of 100 characters mean binary.
    let odd = |c: u32| c == 0 || (c < 0x20 && !matches!(c, 0x09 | 0x0A | 0x0C | 0x0D | 0x1A | 0x1B)) || c == 0x7F;
    if let Some(le) = unmarked_utf16(sample) {
        let units = sample.as_chunks::<2>().0;
        let n = units.iter().filter(|c| odd(u32::from(if le { u16::from_le_bytes(**c) } else { u16::from_be_bytes(**c) }))).count();
        return n.saturating_mul(100) <= units.len();
    }
    if sample.contains(&0) {
        return false;
    }
    sample.iter().filter(|c| odd(u32::from(**c))).count().saturating_mul(100) <= sample.len()
}

/// Decode text bytes: BOMs first, then UTF-16 without BOM (by its zero bytes), UTF-8, and the
/// legacy encoding the bytes look like ([`legacy`]) as the last resort.
pub fn decode(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    if let Some(le) = unmarked_utf16(b) {
        return utf16(b, le);
    }
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => legacy(b),
    }
}

/// Most bytes looked at to guess a legacy encoding.
const SNIFF: usize = 1 << 20;

/// Bytes that aren't UTF-8 in the legacy encoding they look like (chardetng, the detector
/// Firefox uses for unlabelled pages): GBK/GB18030 for Simplified Chinese, Big5, Shift_JIS or
/// EUC-JP, EUC-KR, or a single-byte Windows / ISO code page; Windows-1252 as before when the
/// guess is a single-byte Western one.
pub fn legacy(b: &[u8]) -> String {
    let enc = legacy_guess(b);
    if enc == encoding_rs::WINDOWS_1252 {
        return b.iter().map(|c| cp1252(*c)).collect();
    }
    enc.decode_without_bom_handling(b).0.into_owned()
}

/// chardetng's guess for bytes that aren't UTF-8.
fn legacy_guess(b: &[u8]) -> &'static encoding_rs::Encoding {
    let mut det = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    det.feed(b.get(..SNIFF).unwrap_or(b), b.len() <= SNIFF);
    det.guess(None, chardetng::Utf8Detection::Deny)
}

/// Decode bytes in a named encoding (a WHATWG label such as `gbk`, `gb18030`, `big5`,
/// `shift_jis`, `euc-kr`, `utf-16le`, `windows-1251`); `None` when the label isn't known.
pub fn decode_as(b: &[u8], label: &str) -> Option<String> {
    let enc = encoding_rs::Encoding::for_label(label.trim().as_bytes())?;
    Some(enc.decode(b).0.into_owned())
}

/// The encoding [`decode`] would read `b` in: a BOM, UTF-16 by its zero bytes, UTF-8 (pure
/// ASCII too), else the legacy guess ([`legacy`]): `Windows` for windows-1252, else its
/// encoding_rs name in lower case.
pub fn detect(b: &[u8]) -> TxtEncoding {
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return TxtEncoding::Utf8;
    }
    if b.starts_with(&[0xFF, 0xFE]) {
        return TxtEncoding::Utf16Le;
    }
    if b.starts_with(&[0xFE, 0xFF]) {
        return TxtEncoding::Utf16Be;
    }
    if let Some(le) = unmarked_utf16(b) {
        return if le { TxtEncoding::Utf16Le } else { TxtEncoding::Utf16Be };
    }
    if std::str::from_utf8(b).is_ok() {
        return TxtEncoding::Utf8;
    }
    let enc = legacy_guess(b);
    if enc == encoding_rs::WINDOWS_1252 { TxtEncoding::Windows } else { TxtEncoding::Label(enc.name().to_ascii_lowercase()) }
}

/// `b` read in `enc`; a BOM matching `enc` is dropped (EF BB BF for UTF-8, FF FE for UTF-16
/// LE, FE FF for UTF-16 BE). Windows reads with [`cp1252`] as [`decode`] does; an unknown
/// label reads as Windows.
pub fn decode_with(b: &[u8], enc: &TxtEncoding) -> String {
    match target(enc) {
        Target::Utf8 => String::from_utf8_lossy(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)).into_owned(),
        Target::Utf16(true) => utf16(b.strip_prefix(&[0xFF, 0xFE]).unwrap_or(b), true),
        Target::Utf16(false) => utf16(b.strip_prefix(&[0xFE, 0xFF]).unwrap_or(b), false),
        Target::Ibm437 => b.iter().map(|c| ibm437_char(*c)).collect(),
        Target::Rs(e) if e == encoding_rs::WINDOWS_1252 => b.iter().map(|c| cp1252(*c)).collect(),
        Target::Rs(e) => e.decode_without_bom_handling(b).0.into_owned(),
    }
}

/// [`import_on`] with the encoding chosen instead of guessed.
pub fn import_with(bytes: &[u8], enc: &TxtEncoding, page: &wordcraft_doc::SectionProps) -> Document {
    let mut doc = text_document(&decode_with(bytes, enc));
    doc.last_section = page.clone();
    doc
}

/// A document with one paragraph per line.
pub fn import(bytes: &[u8]) -> Document {
    text_document(&decode(bytes))
}

/// One paragraph per line of decoded `text`; control characters other than tab and form feed
/// are dropped.
fn text_document(text: &str) -> Document {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let clean: String = text.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\u{000C}')).collect();
    Document::from_text(&clean)
}

/// [`import`] onto `page`: plain text gives no page size (#1381).
pub fn import_on(bytes: &[u8], page: &wordcraft_doc::SectionProps) -> Document {
    let mut doc = import(bytes);
    doc.last_section = page.clone();
    doc
}

// Plain text has no drawing container: include its story at the anchor position.
fn flatten_box(inline: &mut Inline, depth: usize) {
    if depth >= 8 {
        return;
    }
    let text = match inline {
        Inline::Shape { blocks, .. } => {
            let mut lines = Vec::new();
            blocks_text(blocks, &mut lines, &mut ListCounter::default(), depth + 1);
            lines.join("\n").replace("\r\n", "\n")
        }
        Inline::Group { members, .. } => {
            let mut text = String::new();
            for member in members {
                flatten_box(&mut member.inline, depth + 1);
                if let Inline::Text(t, _) = &member.inline {
                    text.push_str(t);
                }
            }
            text
        }
        _ => return,
    };
    *inline = Inline::Text(text, Default::default());
}

fn blocks_text(blocks: &[FBlock], out: &mut Vec<String>, lists: &mut ListCounter, depth: usize) {
    for b in blocks {
        match b {
            FBlock::Para(p) => {
                let mut line = String::new();
                match p.list {
                    Some(li) => {
                        line.push_str(&"\t".repeat(li.level as usize));
                        line.push_str(&lists.label(li));
                        line.push('\t');
                    }
                    None => lists.reset(),
                }
                let mut p = p.clone();
                for inline in &mut p.inlines {
                    flatten_box(inline, depth);
                }
                line.push_str(&p.shown_text().replace('\n', "\r\n").replace(['\u{000C}', '\u{000E}'], ""));
                out.push(line);
            }
            FBlock::Table(t) => {
                lists.reset();
                for row in &t.rows {
                    let cells: Vec<String> = row
                        .iter()
                        .map(|c| {
                            if depth > 8 {
                                return String::new();
                            }
                            let mut v = Vec::new();
                            blocks_text(&c.blocks, &mut v, &mut ListCounter::default(), depth + 1);
                            v.join(" ")
                        })
                        .collect();
                    out.push(cells.join("\t"));
                }
            }
        }
    }
}

/// The document's lines as plain text (a line break inside a paragraph is a CRLF).
fn lines(doc: &Document) -> Vec<String> {
    let flow = model::from_doc_with(doc, model::FlowOpts { boxes: true, ..Default::default() });
    let mut lines = Vec::new();
    blocks_text(&flow.blocks, &mut lines, &mut ListCounter::default(), 0);
    lines
}

/// UTF-8 text, CRLF line ends.
pub fn export(doc: &Document) -> Vec<u8> {
    lines(doc).join("\r\n").into_bytes()
}

/// The encoding a plain-text save writes (Word's File Conversion dialog).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxtEncoding {
    /// The system's ANSI code page: windows-1252.
    Windows,
    /// The OEM code page: IBM437.
    MsDos,
    Utf8,
    Utf16Le,
    Utf16Be,
    /// A WHATWG label encoding_rs knows ("windows-1251", "shift_jis", "gbk"...).
    Label(String),
}

/// What ends each line of a plain-text save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnd {
    CrLf,
    Cr,
    Lf,
    LfCr,
}

/// The File Conversion choices for a plain-text save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TxtSaveOptions {
    pub encoding: TxtEncoding,
    pub line_end: LineEnd,
    pub allow_substitution: bool,
}

impl Default for TxtSaveOptions {
    /// What Word's File Conversion dialog shows first: Windows (Default), CR/LF, no substitution.
    fn default() -> Self {
        TxtSaveOptions { encoding: TxtEncoding::Windows, line_end: LineEnd::CrLf, allow_substitution: false }
    }
}

impl LineEnd {
    /// The characters that end a line.
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnd::CrLf => "\r\n",
            LineEnd::Cr => "\r",
            LineEnd::Lf => "\n",
            LineEnd::LfCr => "\n\r",
        }
    }
}

/// IBM437 (the MS-DOS code page of Western systems), bytes 0x80..=0xFF; below is ASCII.
const IBM437_HIGH: [char; 128] = [
    '\u{00C7}', '\u{00FC}', '\u{00E9}', '\u{00E2}', '\u{00E4}', '\u{00E0}', '\u{00E5}', '\u{00E7}', '\u{00EA}', '\u{00EB}', '\u{00E8}', '\u{00EF}',
    '\u{00EE}', '\u{00EC}', '\u{00C4}', '\u{00C5}', '\u{00C9}', '\u{00E6}', '\u{00C6}', '\u{00F4}', '\u{00F6}', '\u{00F2}', '\u{00FB}', '\u{00F9}',
    '\u{00FF}', '\u{00D6}', '\u{00DC}', '\u{00A2}', '\u{00A3}', '\u{00A5}', '\u{20A7}', '\u{0192}', '\u{00E1}', '\u{00ED}', '\u{00F3}', '\u{00FA}',
    '\u{00F1}', '\u{00D1}', '\u{00AA}', '\u{00BA}', '\u{00BF}', '\u{2310}', '\u{00AC}', '\u{00BD}', '\u{00BC}', '\u{00A1}', '\u{00AB}', '\u{00BB}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}', '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}',
    '\u{255D}', '\u{255C}', '\u{255B}', '\u{2510}', '\u{2514}', '\u{2534}', '\u{252C}', '\u{251C}', '\u{2500}', '\u{253C}', '\u{255E}', '\u{255F}',
    '\u{255A}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256C}', '\u{2567}', '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}',
    '\u{2558}', '\u{2552}', '\u{2553}', '\u{256B}', '\u{256A}', '\u{2518}', '\u{250C}', '\u{2588}', '\u{2584}', '\u{258C}', '\u{2590}', '\u{2580}',
    '\u{03B1}', '\u{00DF}', '\u{0393}', '\u{03C0}', '\u{03A3}', '\u{03C3}', '\u{00B5}', '\u{03C4}', '\u{03A6}', '\u{0398}', '\u{03A9}', '\u{03B4}',
    '\u{221E}', '\u{03C6}', '\u{03B5}', '\u{2229}', '\u{2261}', '\u{00B1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00F7}', '\u{2248}',
    '\u{00B0}', '\u{2219}', '\u{00B7}', '\u{221A}', '\u{207F}', '\u{00B2}', '\u{25A0}', '\u{00A0}',
];

/// The IBM437 byte for `c`, if it has one.
fn ibm437(c: char) -> Option<u8> {
    if c.is_ascii() {
        return u8::try_from(c).ok();
    }
    let i = IBM437_HIGH.iter().position(|x| *x == c)?;
    u8::try_from(i).ok()?.checked_add(0x80)
}

/// The character IBM437 byte `b` stands for.
fn ibm437_char(b: u8) -> char {
    match b.checked_sub(0x80) {
        None => char::from(b),
        Some(i) => IBM437_HIGH.get(usize::from(i)).copied().unwrap_or(char::REPLACEMENT_CHARACTER),
    }
}

/// What an encoding resolves to for reading and writing.
enum Target {
    Utf8,
    Utf16(bool),
    Ibm437,
    Rs(&'static encoding_rs::Encoding),
}

fn target(e: &TxtEncoding) -> Target {
    match e {
        TxtEncoding::Windows => Target::Rs(encoding_rs::WINDOWS_1252),
        TxtEncoding::MsDos => Target::Ibm437,
        TxtEncoding::Utf8 => Target::Utf8,
        TxtEncoding::Utf16Le => Target::Utf16(true),
        TxtEncoding::Utf16Be => Target::Utf16(false),
        TxtEncoding::Label(l) => match encoding_rs::Encoding::for_label(l.trim().as_bytes()) {
            None => Target::Rs(encoding_rs::WINDOWS_1252),
            Some(enc) if enc == encoding_rs::UTF_16LE => Target::Utf16(true),
            Some(enc) if enc == encoding_rs::UTF_16BE => Target::Utf16(false),
            Some(enc) if enc.output_encoding() == encoding_rs::UTF_8 => Target::Utf8,
            Some(enc) => Target::Rs(enc.output_encoding()),
        },
    }
}

/// The closest character(s) to `c` (Allow character substitution), when there is one.
fn substitute(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{2033}' => "\"",
        '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{2032}' => "'",
        '\u{2013}' | '\u{2014}' => "-",
        '\u{2026}' => "...",
        '\u{00A0}' => " ",
        '\u{2022}' => "*",
        _ => return None,
    })
}

/// A single-byte or multi-byte legacy encoder that writes characters one at a time.
struct Legacy {
    target: Target,
    encoder: Option<encoding_rs::Encoder>,
}

impl Legacy {
    fn new(target: Target) -> Legacy {
        let encoder = match &target {
            Target::Rs(enc) => Some(enc.new_encoder()),
            _ => None,
        };
        Legacy { target, encoder }
    }

    /// Append `s` to `out`; `false` (and nothing written) when a character can't be held.
    fn put(&mut self, s: &str, out: &mut Vec<u8>) -> bool {
        match (&self.target, &mut self.encoder) {
            (Target::Ibm437, _) => {
                let Some(bytes) = s.chars().map(ibm437).collect::<Option<Vec<u8>>>() else { return false };
                out.extend(bytes);
                true
            }
            (_, Some(encoder)) => {
                let mut buf = vec![0u8; encoder.max_buffer_length_from_utf8_without_replacement(s.len()).unwrap_or(64).clamp(16, 1 << 20)];
                let (result, _, written) = encoder.encode_from_utf8_without_replacement(s, &mut buf, false);
                match result {
                    encoding_rs::EncoderResult::InputEmpty => {
                        out.extend(buf.get(..written).unwrap_or(&[]));
                        true
                    }
                    _ => false,
                }
            }
            _ => {
                out.extend(s.as_bytes());
                true
            }
        }
    }

    /// Ends the output (an ISO-2022-JP stream returns to ASCII).
    fn finish(&mut self, out: &mut Vec<u8>) {
        if let Some(encoder) = &mut self.encoder {
            let mut buf = [0u8; 16];
            let (_, _, written) = encoder.encode_from_utf8_without_replacement("", &mut buf, true);
            out.extend(buf.get(..written).unwrap_or(&[]));
        }
    }
}

/// The plain text [`export_with`] encodes, lines joined with `\n`.
pub fn export_text(doc: &Document) -> String {
    lines(doc).join("\n").replace("\r\n", "\n")
}

/// [`export`]'s text encoded per `o`. A character the encoding can't hold is one `?`, after
/// Allow character substitution has tried its closest match. Unicode encodings start with a
/// byte order mark.
pub fn export_with(doc: &Document, o: &TxtSaveOptions) -> Vec<u8> {
    encode_text(&export_text(doc), o)
}

/// `text` (lines joined with `\n`) encoded per `o`.
pub fn encode_text(text: &str, o: &TxtSaveOptions) -> Vec<u8> {
    let text = if o.line_end == LineEnd::Lf { text.to_string() } else { text.replace('\n', o.line_end.as_str()) };
    match target(&o.encoding) {
        Target::Utf8 => {
            let mut out = vec![0xEF, 0xBB, 0xBF];
            out.extend(text.as_bytes());
            out
        }
        Target::Utf16(le) => {
            let mut out = if le { vec![0xFF, 0xFE] } else { vec![0xFE, 0xFF] };
            for u in text.encode_utf16() {
                out.extend(if le { u.to_le_bytes() } else { u.to_be_bytes() });
            }
            out
        }
        t => {
            let mut enc = Legacy::new(t);
            let mut out = Vec::with_capacity(text.len());
            let mut one = [0u8; 4];
            for c in text.chars() {
                if enc.put(c.encode_utf8(&mut one), &mut out) {
                    continue;
                }
                let subbed = o.allow_substitution && substitute(c).is_some_and(|s| enc.put(s, &mut out));
                if !subbed {
                    enc.put("?", &mut out);
                }
            }
            enc.finish(&mut out);
            out
        }
    }
}

/// Char indices (not byte offsets) in `text` of the characters that won't save correctly in
/// `o.encoding`, after substitution when it is allowed: the File Conversion preview shows them
/// in red. Empty for the Unicode encodings.
pub fn unencodable(text: &str, o: &TxtSaveOptions) -> Vec<usize> {
    let t = target(&o.encoding);
    if matches!(t, Target::Utf8 | Target::Utf16(_)) {
        return Vec::new();
    }
    let mut enc = Legacy::new(t);
    let mut sink = Vec::new();
    let mut one = [0u8; 4];
    let mut bad = Vec::new();
    for (i, c) in text.chars().enumerate() {
        sink.clear();
        if enc.put(c.encode_utf8(&mut one), &mut sink) {
            continue;
        }
        if !(o.allow_substitution && substitute(c).is_some_and(|s| enc.put(s, &mut sink))) {
            bad.push(i);
        }
    }
    bad
}
