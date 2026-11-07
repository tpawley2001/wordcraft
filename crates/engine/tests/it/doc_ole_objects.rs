//! A Word 97-2003 embedded object (an `EMBED` field whose object char names storage
//! `ObjectPool/_<id>`, [MS-DOC] §2.6.1) saved as .docx is an object, as Word writes it: a
//! `w:object` with an `o:OLEObject` related to `word/embeddings/oleObject1.bin`, not a picture
//! and an empty `EMBED` field. Reopening and saving again keeps the same bytes.

use std::io::{Read, Write};

use wordcraft_doc::para::InlineObject;
use wordcraft_engine::io;

const OBJ_ID: u32 = 1_345_471_004;
/// `EMBED` field: the object char (in the result) is CP 23.
const TEXT: &str = "\u{13} EMBED Excel.Sheet.8 \u{14}\u{1}\u{15}\r";
const OBJ_CP: u32 = 23;
/// Where the text starts in the WordDocument stream (past the FIB).
const TEXT_FC: usize = 0x600;
const WORKBOOK: &[u8] = b"synthetic workbook stream: copied verbatim, never parsed";

/// A minimal Word 97 FIB ([MS-DOC] §2.5.1): FibBase, 14 `rgw`, 22 `rglw` (ccpText), 0x5D fc/lcb
/// pairs (Clx at 33, PlcfBteChpx at 12).
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

/// A ChpxFkp page: plain text, then the object char's sprms, then plain text.
fn chpx_page(ccp: u32) -> Vec<u8> {
    let id = OBJ_ID.to_le_bytes();
    // sprmCFSpec, sprmCFObj, sprmCFOle2 = 1, sprmCPicLocation = the storage id.
    let object: Vec<u8> =
        [&0x0855u16.to_le_bytes()[..], &[1], &0x0856u16.to_le_bytes(), &[1], &0x080Au16.to_le_bytes(), &[1], &0x6A03u16.to_le_bytes(), &id].concat();
    let fc = |cp: u32| (TEXT_FC as u32 + 2 * cp).to_le_bytes();
    let mut page = vec![0u8; 512];
    for (i, cp) in [0, OBJ_CP, OBJ_CP + 1, ccp].into_iter().enumerate() {
        page[i * 4..i * 4 + 4].copy_from_slice(&fc(cp));
    }
    // rgb: runs 0 and 2 have no CHPX; run 1's sits at byte 0x40 (word offset 0x20).
    page[16 + 1] = 0x20;
    page[0x40] = object.len() as u8;
    page[0x41..0x41 + object.len()].copy_from_slice(&object);
    page[511] = 3;
    page
}

fn wmf() -> Vec<u8> {
    let mut v = Vec::new();
    for x in [1u16, 9, 0x0300] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&12u32.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&7u32.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&3u32.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v
}

/// The object's storage streams: its picture (`\x03META`), goal size (`\x03PIC`) and data.
fn streams() -> Vec<(&'static str, Vec<u8>)> {
    let meta = [[8u8, 0, 0xD2, 6, 0x72, 1, 0, 0].as_slice(), &wmf()].concat();
    let mut pic = vec![0u8; 100];
    pic[20..24].copy_from_slice(&2880u32.to_le_bytes());
    pic[24..28].copy_from_slice(&1440u32.to_le_bytes());
    pic[44..48].copy_from_slice(&1000u32.to_le_bytes());
    pic[48..52].copy_from_slice(&1000u32.to_le_bytes());
    vec![("\u{1}Ole", vec![1, 0, 0, 2, 0, 0, 0, 0]), ("\u{3}META", meta), ("\u{3}PIC", pic), ("Workbook", WORKBOOK.to_vec())]
}

/// The .doc: one paragraph holding the `EMBED` field, and the object's storage.
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
    let pairs = [(33, 0x40, clx_len as u32), (12, bte_at as u32, 12)];
    let mut word = fib(ccp, &pairs);
    word.resize(TEXT_FC, 0);
    word.extend_from_slice(&text);
    word.resize(page_at, 0);
    word.extend_from_slice(&chpx_page(ccp));

    let mut comp = cfb::CompoundFile::create(std::io::Cursor::new(Vec::new())).expect("create");
    comp.create_stream("/WordDocument").expect("stream").write_all(&word).expect("write");
    comp.create_stream("/0Table").expect("stream").write_all(&table).expect("write");
    comp.create_storage_all(format!("/ObjectPool/_{OBJ_ID}")).expect("storage");
    for (name, data) in streams() {
        comp.create_stream(format!("/ObjectPool/_{OBJ_ID}/{name}")).expect("stream").write_all(&data).expect("write");
    }
    comp.flush().expect("flush");
    comp.into_inner().into_inner()
}

fn part(zip_bytes: &[u8], name: &str) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("zip");
    let mut out = Vec::new();
    z.by_name(name).unwrap_or_else(|_| panic!("part {name}")).read_to_end(&mut out).expect("read");
    out
}

fn text(zip_bytes: &[u8], name: &str) -> String {
    String::from_utf8(part(zip_bytes, name)).expect("utf-8")
}

/// The one inline picture's `ole` markup.
fn ole_of(doc: &wordcraft_doc::Document) -> std::sync::Arc<wordcraft_doc::graphic::Embedded> {
    let mut found = doc.body.iter().filter_map(|b| b.as_para()).flat_map(|p| p.objects.iter()).filter_map(|o| match o {
        InlineObject::Image { ole, .. } => ole.clone(),
        _ => None,
    });
    let ole = found.next().expect("an object");
    assert!(found.next().is_none());
    ole
}

#[test]
fn a_doc_object_is_saved_as_an_ole_object_in_docx() {
    let doc = io::open_bytes("object.doc", &doc_bytes()).expect("opens");
    ole_of(&doc);
    let docx = io::save_bytes("object.docx", &doc).expect("saves");

    let types = text(&docx, "[Content_Types].xml");
    assert!(
        types.contains("Extension=\"bin\" ContentType=\"application/vnd.openxmlformats-officedocument.oleObject\"")
            || types.contains("PartName=\"/word/embeddings/oleObject1.bin\" ContentType=\"application/vnd.openxmlformats-officedocument.oleObject\""),
        "{types}"
    );
    let rels = text(&docx, "word/_rels/document.xml.rels");
    assert!(rels.contains("relationships/oleObject\" Target=\"embeddings/oleObject1.bin\""), "{rels}");
    let body = text(&docx, "word/document.xml");
    assert!(body.contains("<o:OLEObject") && body.contains("ProgID=\"Excel.Sheet.8\""), "{body}");
    assert!(!body.contains("EMBED"), "no EMBED field is written: {body}");

    let bin = part(&docx, "word/embeddings/oleObject1.bin");
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(bin.clone())).expect("compound file");
    let mut workbook = Vec::new();
    comp.open_stream("/Workbook").expect("Workbook").read_to_end(&mut workbook).expect("read");
    assert_eq!(workbook, WORKBOOK);

    // Reopened and saved again: still the object, with the same bytes.
    let again = io::open_bytes("object.docx", &docx).expect("reopens");
    let ole = ole_of(&again);
    let target = &ole.refs.iter().find(|r| r.kind.ends_with("/oleObject")).expect("oleObject ref").target;
    assert_eq!(again.passthrough.get(target).map(|b| b.to_vec()), Some(bin.clone()));
    let docx2 = io::save_bytes("object.docx", &again).expect("saves again");
    assert_eq!(part(&docx2, "word/embeddings/oleObject1.bin"), bin);
}
