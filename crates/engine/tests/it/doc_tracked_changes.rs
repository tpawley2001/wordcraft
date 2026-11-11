//! A Word 97-2003 document with tracked changes (#4187): its deleted text opens as a tracked
//! deletion, not as live text, so Accept All drops it, Reject All keeps it, and a DOCX save
//! writes it inside `w:del` as `w:delText`, never as an ordinary `w:t`.

use std::io::{Read, Write};

use serde_json::json;
use wordcraft_engine::{Session, io};

/// "Keep " plain, "gone " deleted, "new" inserted, ".\r" plain.
const TEXT: &str = "Keep gone new.\r";
const DEL: (u32, u32) = (5, 10);
const INS: (u32, u32) = (10, 13);
/// Where the text starts in the WordDocument stream (past the FIB).
const TEXT_FC: usize = 0x600;
const AUTHOR: &str = "A. Reviewer";

/// A minimal Word 97 FIB ([MS-DOC] §2.5.1): FibBase, 14 `rgw`, 22 `rglw` (ccpText), 0x5D fc/lcb
/// pairs (PlcfBteChpx at 12, Clx at 33, SttbfRMark at 51).
fn fib(ccp_text: u32, pairs: &[(usize, u32, u32)]) -> Vec<u8> {
    let mut v = vec![0u8; 0x9A + 0x5D * 8 + 2];
    let mut put = |at: usize, b: &[u8]| v[at..at + b.len()].copy_from_slice(b);
    put(0x00, &0xA5ECu16.to_le_bytes());
    put(0x02, &0x00C1u16.to_le_bytes());
    put(0x0A, &0x1000u16.to_le_bytes());
    put(0x0C, &0x00BFu16.to_le_bytes());
    put(0x20, &0x000Eu16.to_le_bytes());
    put(0x3E, &0x0016u16.to_le_bytes());
    put(0x40, &0x1000u32.to_le_bytes());
    put(0x4C, &ccp_text.to_le_bytes());
    put(0x98, &0x005Du16.to_le_bytes());
    for (i, fc, lcb) in pairs {
        put(0x9A + i * 8, &fc.to_le_bytes());
        put(0x9A + i * 8 + 4, &lcb.to_le_bytes());
    }
    v
}

fn sprm(op: u16, operand: &[u8]) -> Vec<u8> {
    [&op.to_le_bytes()[..], operand].concat()
}

/// A ChpxFkp page: plain text, the deletion (sprmCFRMarkDel, sprmCIbstRMarkDel = author 0), the
/// insertion (sprmCFRMarkIns, sprmCIbstRMark = author 0), plain text.
fn chpx_page(ccp: u32) -> Vec<u8> {
    let del = [sprm(0x0800, &[1]), sprm(0x4863, &[0, 0])].concat();
    let ins = [sprm(0x0801, &[1]), sprm(0x4804, &[0, 0])].concat();
    let fc = |cp: u32| (TEXT_FC as u32 + 2 * cp).to_le_bytes();
    let mut page = vec![0u8; 512];
    for (i, cp) in [0, DEL.0, INS.0, INS.1, ccp].into_iter().enumerate() {
        page[i * 4..i * 4 + 4].copy_from_slice(&fc(cp));
    }
    // rgb after the five FCs: runs 0 and 3 have no CHPX; run 1's sits at byte 0x40, run 2's at 0x60.
    page[20 + 1] = 0x20;
    page[20 + 2] = 0x30;
    for (at, grpprl) in [(0x40, &del), (0x60, &ins)] {
        page[at] = grpprl.len() as u8;
        page[at + 1..at + 1 + grpprl.len()].copy_from_slice(grpprl);
    }
    page[511] = 4;
    page
}

/// The .doc: one paragraph, one uncompressed piece, and the author table.
fn doc_bytes() -> Vec<u8> {
    let ccp = TEXT.chars().count() as u32;
    let text: Vec<u8> = TEXT.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let page_at = (TEXT_FC + text.len()).div_ceil(512) * 512;
    // Clx: one piece, uncompressed at TEXT_FC.
    let mut plc = Vec::new();
    for cp in [0, ccp] {
        plc.extend_from_slice(&cp.to_le_bytes());
    }
    plc.extend_from_slice(&[0, 0]);
    plc.extend_from_slice(&(TEXT_FC as u32).to_le_bytes());
    plc.extend_from_slice(&[0, 0]);
    let mut table = vec![0u8; 0x40];
    table.push(0x02);
    table.extend_from_slice(&(plc.len() as u32).to_le_bytes());
    table.extend_from_slice(&plc);
    let clx_len = table.len() - 0x40;
    let bte_at = table.len();
    for x in [0u32, 0x7FFF_FFFF, (page_at / 512) as u32] {
        table.extend_from_slice(&x.to_le_bytes());
    }
    // SttbfRMark: an extended STTB of one author.
    let rmark_at = table.len();
    table.extend_from_slice(&[0xFF, 0xFF, 1, 0, 0, 0]);
    let units: Vec<u16> = AUTHOR.encode_utf16().collect();
    table.extend_from_slice(&(units.len() as u16).to_le_bytes());
    table.extend(units.iter().flat_map(|u| u.to_le_bytes()));
    let rmark_len = table.len() - rmark_at;
    let pairs = [(33, 0x40, clx_len as u32), (12, bte_at as u32, 12), (51, rmark_at as u32, rmark_len as u32)];
    let mut word = fib(ccp, &pairs);
    word.resize(TEXT_FC, 0);
    word.extend_from_slice(&text);
    word.resize(page_at, 0);
    word.extend_from_slice(&chpx_page(ccp));

    let mut comp = cfb::CompoundFile::create(std::io::Cursor::new(Vec::new())).expect("create");
    comp.create_stream("/WordDocument").expect("stream").write_all(&word).expect("write");
    comp.create_stream("/0Table").expect("stream").write_all(&table).expect("write");
    comp.flush().expect("flush");
    comp.into_inner().into_inner()
}

fn body_text(s: &Session) -> String {
    s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n")
}

/// Every element `<tag ...>...</tag>` in `xml`.
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{tag} "), format!("</{tag}>"));
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let tail = &rest[start..];
        let end = tail.find(&close).map_or(tail.len(), |e| e + close.len());
        out.push(&tail[..end]);
        rest = &tail[end..];
    }
    out
}

#[test]
fn a_tracked_deletion_is_not_live_text_after_a_docx_save() {
    let doc = io::open_bytes("tracked.doc", &doc_bytes()).expect("opens");

    let mut accepted = Session::new(doc.clone());
    accepted.run("review.acceptAll", &json!({})).expect("accept all");
    assert_eq!(body_text(&accepted), "Keep new.");
    let mut rejected = Session::new(doc.clone());
    rejected.run("review.rejectAll", &json!({})).expect("reject all");
    assert_eq!(body_text(&rejected), "Keep gone .");

    let docx = io::save_bytes("tracked.docx", &doc).expect("saves");
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(docx)).expect("zip");
    let mut body = String::new();
    z.by_name("word/document.xml").expect("document part").read_to_string(&mut body).expect("utf-8");

    let dels = elements(&body, "w:del");
    assert_eq!(dels.len(), 1, "one w:del: {body}");
    let del = dels.first().copied().unwrap_or_default();
    assert!(del.contains("w:author=\"A. Reviewer\""), "{del}");
    assert!(del.contains("<w:delText") && del.contains(">gone </w:delText>"), "{del}");
    let inss = elements(&body, "w:ins");
    assert_eq!(inss.len(), 1, "one w:ins: {body}");
    let ins = inss.first().copied().unwrap_or_default();
    assert!(ins.contains("w:author=\"A. Reviewer\""), "{ins}");
    assert!(ins.contains(">new</w:t>"), "{ins}");
    for t in elements(&body, "w:t").into_iter().chain(body.match_indices("<w:t>").map(|(i, _)| &body[i..])) {
        let t = t.split("</w:t>").next().unwrap_or_default();
        assert!(!t.contains("gone"), "deleted text written as live text: {t}");
    }
}
