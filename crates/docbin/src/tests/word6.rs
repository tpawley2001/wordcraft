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
    /// Character runs as (start offset into `text`, Word 6 grpprl).
    chpx: Vec<(usize, Vec<u8>)>,
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
        if !self.fonts.is_empty() {
            let mut t = vec![0u8, 0u8];
            for (name, chs) in &self.fonts {
                let mut ffn = vec![0x16u8, 0x90, 0x01, *chs, 0];
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
