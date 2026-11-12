//! Word 6.0 / Word 95 `.doc` files (wIdent 0xA5DC, nFib 101..=105), built in code from the FIB
//! layout measured for gap #14: text at fcMin, the Clx at FIB 0x160 inside the WordDocument stream
//! (no Table stream), 8-bit pieces, a font table whose charset gives each run's code page, and
//! CHPX pages of one-byte sprms.

use super::{body_texts, cfb_file, read};

const FC_MIN: usize = 0x300;

fn put16(v: &mut [u8], at: usize, x: u16) {
    v[at..at + 2].copy_from_slice(&x.to_le_bytes());
}
fn put32(v: &mut [u8], at: usize, x: u32) {
    v[at..at + 4].copy_from_slice(&x.to_le_bytes());
}

#[derive(Default)]
struct W6 {
    nfib: u16,
    complex: bool,
    encrypted: bool,
    /// 8-bit main text (and header text after it when `ccp_hdd` > 0).
    text: Vec<u8>,
    ccp_text: u32,
    ccp_hdd: u32,
    /// Pieces as (cp length, byte offset into `text`), in CP order; only when `complex`.
    pieces: Vec<(u32, usize)>,
    prc: bool,
    /// Fonts as (name, charset).
    fonts: Vec<(&'static str, u8)>,
    /// FFID bytes of the fonts, in order; a font past the end gets 0x16 (roman, variable, TrueType).
    ffids: Vec<u8>,
    /// Character runs as (start offset into `text`, Word 6 grpprl).
    chpx: Vec<(usize, Vec<u8>)>,
    /// Paragraph runs as (start offset into `text`, Word 6 grpprl), in one PAPX page with Word 6's
    /// 7-byte BX entries; each PAPX is `cw`, `istd` 0, then the grpprl. An empty grpprl gets
    /// `bOffset` 0 (no properties).
    papx: Vec<(usize, Vec<u8>)>,
    /// Sections as (end CP, Word 6 section grpprl): a PlcSed at FIB 0x88/0x8C, each SEPX as
    /// `cb` u16 + grpprl; an empty grpprl gets `fcSepx` 0xFFFFFFFF (no properties).
    sections: Vec<(u32, Vec<u8>)>,
}

impl W6 {
    fn build(&self) -> Vec<u8> {
        let mut w = vec![0u8; FC_MIN];
        put16(&mut w, 0x00, 0xA5DC);
        put16(&mut w, 0x02, if self.nfib == 0 { 101 } else { self.nfib });
        put16(&mut w, 0x04, 0x6027);
        put16(&mut w, 0x06, 0x0409);
        let mut flags = 0u16;
        if self.complex {
            flags |= 0x0004;
        }
        if self.encrypted {
            flags |= 0x0100;
        }
        put16(&mut w, 0x0A, flags);
        put16(&mut w, 0x0C, 101);
        w.extend_from_slice(&self.text);
        let fc_mac = w.len() as u32;
        put32(&mut w, 0x18, FC_MIN as u32);
        put32(&mut w, 0x1C, fc_mac);
        put32(&mut w, 0x34, self.ccp_text);
        put32(&mut w, 0x3C, self.ccp_hdd);
        // Pad to a page boundary so CHPX pages can follow.
        w.resize(w.len().div_ceil(512) * 512, 0);
        if !self.chpx.is_empty() {
            let pn = (w.len() / 512) as u16;
            let mut page = vec![0u8; 512];
            let crun = self.chpx.len();
            for (i, (start, _)) in self.chpx.iter().enumerate() {
                put32(&mut page, i * 4, (FC_MIN + start) as u32);
            }
            put32(&mut page, crun * 4, fc_mac);
            let mut at = 510usize;
            for (i, (_, grpprl)) in self.chpx.iter().enumerate() {
                let need = 1 + grpprl.len();
                at = (at - need) & !1;
                page[at] = grpprl.len() as u8;
                page[at + 1..at + 1 + grpprl.len()].copy_from_slice(grpprl);
                page[(crun + 1) * 4 + i] = (at / 2) as u8;
            }
            page[511] = crun as u8;
            w.extend_from_slice(&page);
            let mut bte = Vec::new();
            bte.extend_from_slice(&(FC_MIN as u32).to_le_bytes());
            bte.extend_from_slice(&fc_mac.to_le_bytes());
            bte.extend_from_slice(&pn.to_le_bytes());
            let at = w.len();
            w.extend_from_slice(&bte);
            put32(&mut w, 0xB8, at as u32);
            put32(&mut w, 0xBC, bte.len() as u32);
        }
        if !self.papx.is_empty() {
            w.resize(w.len().div_ceil(512) * 512, 0);
            let pn = (w.len() / 512) as u16;
            let mut page = vec![0u8; 512];
            let crun = self.papx.len();
            for (i, (start, _)) in self.papx.iter().enumerate() {
                put32(&mut page, i * 4, (FC_MIN + start) as u32);
            }
            put32(&mut page, crun * 4, fc_mac);
            let mut at = 510usize;
            for (i, (_, grpprl)) in self.papx.iter().enumerate() {
                if grpprl.is_empty() {
                    continue;
                }
                let cw = (2 + grpprl.len()).div_ceil(2);
                at = (at - (1 + cw * 2)) & !1;
                page[at] = cw as u8;
                page[at + 3..at + 3 + grpprl.len()].copy_from_slice(grpprl);
                page[(crun + 1) * 4 + i * 7] = (at / 2) as u8;
            }
            page[511] = crun as u8;
            w.extend_from_slice(&page);
            let mut bte = Vec::new();
            bte.extend_from_slice(&(FC_MIN as u32).to_le_bytes());
            bte.extend_from_slice(&fc_mac.to_le_bytes());
            bte.extend_from_slice(&pn.to_le_bytes());
            let at = w.len();
            w.extend_from_slice(&bte);
            put32(&mut w, 0xC0, at as u32);
            put32(&mut w, 0xC4, bte.len() as u32);
        }
        if !self.sections.is_empty() {
            let mut fcs = Vec::new();
            for (_, grpprl) in &self.sections {
                if grpprl.is_empty() {
                    fcs.push(u32::MAX);
                    continue;
                }
                w.resize(w.len().div_ceil(2) * 2, 0);
                fcs.push(w.len() as u32);
                w.extend_from_slice(&(grpprl.len() as u16).to_le_bytes());
                w.extend_from_slice(grpprl);
            }
            let mut plc = 0u32.to_le_bytes().to_vec();
            for (end, _) in &self.sections {
                plc.extend_from_slice(&end.to_le_bytes());
            }
            for fc in fcs {
                plc.extend_from_slice(&0u16.to_le_bytes()); // fn
                plc.extend_from_slice(&fc.to_le_bytes()); // fcSepx
                plc.extend_from_slice(&0u16.to_le_bytes()); // fnMpr
                plc.extend_from_slice(&0u32.to_le_bytes()); // fcMpr
            }
            let at = w.len();
            w.extend_from_slice(&plc);
            put32(&mut w, 0x88, at as u32);
            put32(&mut w, 0x8C, plc.len() as u32);
        }
        if !self.fonts.is_empty() {
            let mut t = vec![0u8, 0u8];
            for (i, (name, chs)) in self.fonts.iter().enumerate() {
                let ffid = self.ffids.get(i).copied().unwrap_or(0x16);
                let mut ffn = vec![ffid, 0x90, 0x01, *chs, 0];
                ffn.extend_from_slice(name.as_bytes());
                ffn.push(0);
                t.push(ffn.len() as u8);
                t.extend_from_slice(&ffn);
            }
            let total = t.len() as u16;
            put16(&mut t, 0, total);
            let at = w.len();
            w.extend_from_slice(&t);
            put32(&mut w, 0xD0, at as u32);
            put32(&mut w, 0xD4, t.len() as u32);
        }
        if self.complex {
            let mut clx = Vec::new();
            if self.prc {
                clx.extend_from_slice(&[0x01, 0x03, 0x00, 0x55, 0x81, 0x00]);
            }
            let mut plc = Vec::new();
            let mut cp = 0u32;
            plc.extend_from_slice(&cp.to_le_bytes());
            for (len, _) in &self.pieces {
                cp += len;
                plc.extend_from_slice(&cp.to_le_bytes());
            }
            for (_, off) in &self.pieces {
                plc.extend_from_slice(&0u16.to_le_bytes());
                plc.extend_from_slice(&((FC_MIN + off) as u32).to_le_bytes());
                plc.extend_from_slice(&0u16.to_le_bytes());
            }
            clx.push(0x02);
            clx.extend_from_slice(&(plc.len() as u32).to_le_bytes());
            clx.extend_from_slice(&plc);
            let at = w.len();
            w.extend_from_slice(&clx);
            put32(&mut w, 0x160, at as u32);
            put32(&mut w, 0x164, clx.len() as u32);
        }
        cfb_file(&[("WordDocument", &w)])
    }
}

fn plain(text: &str) -> W6 {
    W6 { text: text.as_bytes().to_vec(), ccp_text: text.len() as u32, ..Default::default() }
}

/// The run (by byte offset in the paragraph text) covering the first occurrence of `needle`.
fn run_at<'a>(p: &'a wordcraft_doc::Paragraph, needle: &str) -> &'a wordcraft_doc::para::Run {
    let at = p.text.find(needle).unwrap_or_else(|| panic!("{needle:?} in {:?}", p.text));
    let mut start = 0;
    for r in &p.runs {
        if at < start + r.len {
            return r;
        }
        start += r.len;
    }
    panic!("no run at {at} in {:?}", p.runs)
}

fn first_para(doc: &wordcraft_doc::Document) -> &wordcraft_doc::Paragraph {
    doc.body.iter().find_map(|b| b.as_para()).expect("a paragraph")
}

#[test]
fn word6_plain_text_and_paragraphs() {
    let doc = read(&plain("The quick brown fox\rSecond line\r").build()).expect("Word 6.0 opens");
    assert_eq!(body_texts(&doc), ["The quick brown fox", "Second line"]);
}

#[test]
fn word95_nfib_104_opens() {
    let f = W6 { nfib: 104, ..plain("Word 95\r") }.build();
    assert_eq!(body_texts(&read(&f).expect("Word 95 opens")), ["Word 95"]);
}

#[test]
fn word6_complex_pieces_out_of_order() {
    // Stored "World\rHello " ; CP order is "Hello World\r".
    let f = W6 { complex: true, pieces: vec![(6, 6), (6, 0)], ..plain("World\rHello ") }.build();
    assert_eq!(body_texts(&read(&f).expect("opens")), ["Hello World"]);
}

#[test]
fn word6_prc_before_pcdt_is_skipped() {
    let f = W6 { complex: true, prc: true, pieces: vec![(4, 0)], ..plain("Ahoy\r") }.build();
    // "Ahoy\r" is 5 bytes but the piece is 4: the paragraph mark is not in the text.
    let f2 = W6 { complex: true, prc: true, pieces: vec![(5, 0)], ..plain("Ahoy\r") }.build();
    assert!(read(&f).is_ok());
    assert_eq!(body_texts(&read(&f2).expect("opens")), ["Ahoy"]);
}

#[test]
fn word6_text_code_page_comes_from_the_font_charset() {
    // cp1251 "Компания" in a run whose font is "Times New Roman Cyr" (charset 204), lid en-US.
    let mut text = vec![0xCA, 0xEE, 0xEC, 0xEF, 0xE0, 0xED, 0xE8, 0xFF];
    text.extend_from_slice(b" \xC0\r");
    let f = W6 {
        ccp_text: text.len() as u32,
        text,
        fonts: vec![("Times New Roman", 0), ("Symbol", 2), ("Arial", 0), ("Times New Roman Cyr", 204)],
        chpx: vec![(0, vec![0x5D, 0x03, 0x00])],
        ..Default::default()
    }
    .build();
    let doc = read(&f).expect("opens");
    assert_eq!(body_texts(&doc), ["Компания А"]);
}

#[test]
fn word6_ansi_font_keeps_cp1252() {
    let f = W6 { fonts: vec![("Times New Roman", 0)], chpx: vec![(0, vec![0x5D, 0x00, 0x00])], ..plain("caf\u{E9}\r") };
    // plain() used UTF-8 bytes; store cp1252 instead.
    let f = W6 { text: b"caf\xE9\r".to_vec(), ccp_text: 5, ..f }.build();
    assert_eq!(body_texts(&read(&f).expect("opens")), ["café"]);
}

#[test]
fn word6_bold_and_size_from_chpx() {
    let f = W6 {
        fonts: vec![("Times New Roman", 0), ("Symbol", 2), ("Arial", 0)],
        // "Plain " default, "Bold" = sprm 85 bold + sprm 99 size 24 half-points + sprm 93 Arial.
        chpx: vec![(0, vec![]), (6, vec![0x55, 0x01, 0x63, 0x18, 0x00, 0x5D, 0x02, 0x00]), (10, vec![])],
        ..plain("Plain Bold\r")
    }
    .build();
    let doc = read(&f).expect("opens");
    let p = first_para(&doc);
    assert_eq!(p.text, "Plain Bold");
    let bold = run_at(p, "Bold");
    assert_eq!(bold.props.bold, Some(true), "{:?}", p.runs);
    assert_eq!(bold.props.size, Some(12.0), "{:?}", p.runs);
    assert_eq!(bold.props.font.as_deref(), Some("Arial"), "{:?}", p.runs);
    assert_ne!(run_at(p, "Plain").props.bold, Some(true));
}

#[test]
fn word6_header_text_stays_out_of_body() {
    let f = W6 { ccp_text: 5, ccp_hdd: 8, ..plain("body\rheader\r\r") }.build();
    assert_eq!(body_texts(&read(&f).expect("opens")), ["body"]);
}

#[test]
fn word6_encrypted_is_refused_clearly() {
    let f = W6 { encrypted: true, ..plain("secret\r") }.build();
    let err = read(&f).expect_err("encrypted Word 6 is refused");
    assert!(format!("{err:?}").to_lowercase().contains("password") || matches!(err, super::super::DocbinError::PasswordRequired), "{err:?}");
}

#[test]
fn other_magic_still_refused() {
    let mut f = plain("x\r");
    f.nfib = 101;
    let mut bytes = f.build();
    // Corrupt nothing in the container; a Word 2 magic is still not a Word document we read.
    let w = bytes.windows(2).position(|p| p == [0xDC, 0xA5]).expect("magic");
    bytes[w] = 0x99;
    bytes[w + 1] = 0xA6;
    assert!(read(&bytes).is_err());
}

fn paras(doc: &wordcraft_doc::Document) -> Vec<&wordcraft_doc::Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

#[test]
fn word6_para_alignment_indents_and_spacing() {
    use wordcraft_doc::props::{Align, LineSpacing};
    let text = "Title\rBody\rLast\r";
    let f = W6 {
        papx: vec![
            // "Title": centered (5=1), 12pt before (21=240), 6pt after (22=120), keep with next (8).
            (0, vec![0x05, 0x01, 0x15, 0xF0, 0x00, 0x16, 0x78, 0x00, 0x08, 0x01]),
            // "Body": justified, left 36pt (17=720), right 18pt (16=360), first line -18pt (19=-360),
            // 1.5 lines (20: dyaLine 360, fMultLinespace 1), keep lines (7), widow control off (51=0).
            (6, vec![0x05, 0x03, 0x11, 0xD0, 0x02, 0x10, 0x68, 0x01, 0x13, 0x98, 0xFE, 0x14, 0x68, 0x01, 0x01, 0x00, 0x07, 0x01, 0x33, 0x00]),
            // "Last": right-aligned, exactly 14pt (dyaLine -280), page break before (9).
            (11, vec![0x05, 0x02, 0x14, 0xE8, 0xFE, 0x00, 0x00, 0x09, 0x01]),
        ],
        ..plain(text)
    }
    .build();
    let doc = read(&f).expect("opens");
    let ps = paras(&doc);
    assert_eq!(ps.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(), ["Title", "Body", "Last"]);
    let (t, b, l) = (&ps[0].props, &ps[1].props, &ps[2].props);
    assert_eq!(t.align, Some(Align::Center), "{t:?}");
    assert_eq!(t.space_before, Some(12.0));
    assert_eq!(t.space_after, Some(6.0));
    assert_eq!(t.keep_next, Some(true));
    assert_eq!(b.align, Some(Align::Justify), "{b:?}");
    assert_eq!(b.indent_left, Some(36.0));
    assert_eq!(b.indent_right, Some(18.0));
    assert_eq!(b.indent_first, Some(-18.0));
    assert_eq!(b.line_spacing, Some(LineSpacing::Multiple(1.5)));
    assert_eq!(b.keep_lines, Some(true));
    assert_eq!(b.widow_control, Some(false));
    assert_ne!(b.keep_next, Some(true), "keep with next stays on Title");
    assert_eq!(l.align, Some(Align::Right), "{l:?}");
    assert_eq!(l.line_spacing, Some(LineSpacing::Exactly(14.0)));
    assert_eq!(l.page_break_before, Some(true));
    assert_ne!(l.indent_left, Some(36.0), "indents stay on Body");
}

#[test]
fn word6_para_tabs_match_the_word97_reading() {
    // sprmPChgTabsPapx as stored in a real Word 6 file: len 5, no deletions, one added stop at
    // 720 twips with TBD 0x09 (centre, dotted leader). Same operand as Word 97's 0xC60D.
    let operand = [0x05u8, 0x00, 0x01, 0xD0, 0x02, 0x09];
    let mut grpprl = vec![0x0F];
    grpprl.extend_from_slice(&operand);
    let f = W6 { papx: vec![(0, grpprl)], ..plain("a\tb\r") }.build();
    let doc = read(&f).expect("opens");
    let got = first_para(&doc).props.tabs.clone();
    let mut want = wordcraft_doc::props::ParaProps::default();
    crate::fmt::apply_para(&mut want, &crate::sprm::Prl { op: 0xC60D, operand: &operand });
    assert!(want.tabs.as_ref().is_some_and(|t| t.len() == 1 && t[0].pos == 36.0), "{:?}", want.tabs);
    assert_eq!(got, want.tabs);
}

#[test]
fn word6_para_skips_known_sprms_and_stops_at_unknown_ones() {
    use wordcraft_doc::props::Align;
    let f = W6 {
        papx: vec![
            // In-table (24) and fTtp (25) are skipped by length; centred is still read after them.
            (0, vec![0x18, 0x00, 0x19, 0x00, 0x05, 0x01]),
            // Right-aligned, then an unknown opcode 200: what follows (left indent) is not read.
            (2, vec![0x05, 0x02, 0xC8, 0x11, 0xD0, 0x02]),
            // bOffset 0: no properties.
            (4, vec![]),
        ],
        ..plain("a\rb\rc\r")
    }
    .build();
    let doc = read(&f).expect("opens");
    let ps = paras(&doc);
    assert_eq!(ps.len(), 3);
    assert_eq!(ps[0].props.align, Some(Align::Center));
    assert_eq!(ps[1].props.align, Some(Align::Right));
    assert_eq!(ps[1].props.indent_left, None);
    assert_eq!(ps[2].props.align.unwrap_or(Align::Left), Align::Left);
}

#[test]
fn word6_para_hostile_papx_pages_never_panic() {
    let base = W6 { papx: vec![(0, vec![0x05, 0x01, 0x11, 0xD0, 0x02])], ..plain("one\rtwo\r") }.build();
    let papx_page = base.windows(5).position(|w| w == [0x05, 0x01, 0x11, 0xD0, 0x02]).expect("grpprl") / 512 * 512;
    for (at, byte) in [(511usize, 0xFFu8), (511, 0x00), (8, 0xFF), (8, 0x00)] {
        let mut f = base.clone();
        f[papx_page + at] = byte;
        let _ = read(&f);
    }
    // Truncated operands at the end of a grpprl.
    for g in [vec![0x14, 0x68], vec![0x0F, 0xFF, 0x00], vec![0x11], vec![0x0F]] {
        let _ = read(&W6 { papx: vec![(0, g)], ..plain("x\r") }.build());
    }
}

// Sections (P04-003): Word 6 section sprms are one byte, the Word 97 sprm's ispmd + 133, with the
// Word 97 operand. Grpprls below are the ones measured from real Word 6/95 files.

/// Word6.doc's only section: page 12240 x 15840 twips, left/right 1800, top/bottom 1440, nfcPgn 0.
const LETTER_1800_1440: [u8; 15] = [0xA4, 0xD0, 0x2F, 0xA5, 0xE0, 0x3D, 0xA6, 0x08, 0x07, 0xA7, 0x08, 0x07, 0xA8, 0xA0, 0x05];

#[test]
fn word6_section_page_size_and_margins() {
    let g = [LETTER_1800_1440.as_slice(), &[0xA9, 0xA0, 0x05, 0x93, 0x00]].concat();
    let doc = read(&W6 { sections: vec![(2, g)], ..plain("a\r") }.build()).expect("opens");
    let s = &doc.last_section;
    assert_eq!((s.page_w, s.page_h), (612.0, 792.0));
    assert_eq!((s.margin_left, s.margin_right, s.margin_top, s.margin_bottom), (90.0, 90.0, 72.0, 72.0));
    assert!(!s.landscape);
}

#[test]
fn word6_continuous_landscape_section_ends_on_its_paragraph() {
    // First section: sprmSBkc 0 (continuous), page 15840 x 12240 twips (landscape inferred).
    let first = vec![0x8E, 0x00, 0xA4, 0xE0, 0x3D, 0xA5, 0xD0, 0x2F];
    let doc = read(&W6 { sections: vec![(2, first), (4, Vec::new())], ..plain("a\rb\r") }.build()).expect("opens");
    let paras: Vec<_> = doc.body.iter().filter_map(|b| b.as_para()).collect();
    assert_eq!(paras.len(), 2);
    let s = paras[0].section.as_deref().expect("paragraph a ends the first section");
    assert_eq!(s.start, wordcraft_doc::section::SectionStart::Continuous);
    assert_eq!((s.page_w, s.page_h), (792.0, 612.0));
    assert!(s.landscape);
    assert!(paras[1].section.is_none());
    assert_eq!(doc.last_section, wordcraft_doc::section::SectionProps::default());
}

#[test]
fn word6_section_header_and_footer_distances() {
    // TestMickey.doc: column spacing 708, page numbers start at 1, header/footer 708 twips.
    let g = vec![0x91, 0xC4, 0x02, 0xA1, 0x01, 0x00, 0x9C, 0xC4, 0x02, 0x9D, 0xC4, 0x02];
    let doc = read(&W6 { sections: vec![(2, g)], ..plain("a\r") }.build()).expect("opens");
    let s = &doc.last_section;
    assert!((s.header - 35.4).abs() < 1e-3, "header {}", s.header);
    assert!((s.footer - 35.4).abs() < 1e-3, "footer {}", s.footer);
    assert_eq!(s.page_num_start, Some(1));
}

#[test]
fn word6_hostile_section_tables_never_panic_and_keep_what_was_read() {
    let base = W6 { sections: vec![(2, LETTER_1800_1440.to_vec())], ..plain("a\r") }.build();
    // Page size read, then an unknown opcode: what came before stays.
    let g = [&LETTER_1800_1440[..6], &[250u8, 1, 2, 3]].concat();
    let doc = read(&W6 { sections: vec![(2, g)], ..plain("a\r") }.build()).expect("opens");
    assert_eq!((doc.last_section.page_w, doc.last_section.page_h), (612.0, 792.0));
    // Truncated operands at every length.
    for n in 0..LETTER_1800_1440.len() {
        let _ = read(&W6 { sections: vec![(2, LETTER_1800_1440[..n].to_vec())], ..plain("a\r") }.build());
    }
    // Every byte of the WordDocument stream's PlcSed/SEPX area overwritten: never a panic.
    let sepx = base.windows(LETTER_1800_1440.len()).position(|w| w == LETTER_1800_1440).expect("sepx");
    for at in sepx.saturating_sub(4)..(sepx + LETTER_1800_1440.len() + 40).min(base.len()) {
        for byte in [0x00, 0x7F, 0xFF] {
            let mut f = base.clone();
            f[at] = byte;
            let _ = read(&f);
        }
    }
}

#[test]
fn word6_roman_family_class_fills_roman_fonts() {
    // FFID 0x10 roman, 0x22 swiss, 0x31 modern, 0x96 roman (bit 7 ignored), and a roman FFN
    // with an empty name.
    let f = W6 {
        fonts: vec![("Zzyzx Roman", 0), ("Zzyzx Sans", 0), ("Zzyzx Mono", 0), ("CG Times", 0), ("", 0)],
        ffids: vec![0x10, 0x22, 0x31, 0x96, 0x10],
        ..plain("Ahoy\r")
    }
    .build();
    let doc = read(&f).expect("opens");
    assert_eq!(doc.settings.roman_fonts, ["Zzyzx Roman", "CG Times"]);
}
