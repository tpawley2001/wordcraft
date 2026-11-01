//! Embedded OLE objects print from their print presentation ([MS-DOC] §2.6.1 sprmCFOle2,
//! sprmCPicLocation; [MS-OLEDS] presentation streams). An `EMBED` field's code holds a `\x01`
//! with sprmCFOle2 = 1 whose sprmCPicLocation names the object's storage `ObjectPool/_<id>`; the
//! field result's `\x01` is the picture Word keeps in the Data stream. When that storage holds a
//! `\x03EPRINT` stream (a plain EMF), Word's PDF shows the EPRINT picture, not the Data one
//! (an Excel object's Data picture has light grid lines that Word's print does not draw).
//! The picture keeps the Data stream picture's size (its PICF goal size).

use std::io::Write;

use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_OLE2: u16 = 0x080A;
const F_OBJ: u16 = 0x0856;
const F_SPEC: u16 = 0x0855;

/// The object's storage id: the storage is `ObjectPool/_1345470949`.
const OBJ_ID: u32 = 1_345_470_949;

/// `EMBED` field: code `\x01` at CP 22 (the object), separator 23, result picture 24, end 25.
const TEXT: &str = "\u{13} EMBED Excel.Sheet.8 \u{1}\u{14}\u{1}\u{15}\r";

/// A minimal EMF: EMR_HEADER (108 bytes, " EMF" signature at offset 40, `tag` in the bounds so two
/// files differ) and EMR_EOF (20 bytes).
fn emf(tag: i32) -> Vec<u8> {
    let mut h = vec![0u8; 108];
    h[0..4].copy_from_slice(&1u32.to_le_bytes());
    h[4..8].copy_from_slice(&108u32.to_le_bytes());
    h[16..20].copy_from_slice(&tag.to_le_bytes()); // rclBounds.right
    h[20..24].copy_from_slice(&tag.to_le_bytes()); // rclBounds.bottom
    h[40..44].copy_from_slice(&0x464D_4520u32.to_le_bytes());
    h[44..48].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    h[48..52].copy_from_slice(&128u32.to_le_bytes()); // nBytes
    h[52..56].copy_from_slice(&2u32.to_le_bytes()); // nRecords
    let mut eof = vec![0u8; 20];
    eof[0..4].copy_from_slice(&14u32.to_le_bytes());
    eof[4..8].copy_from_slice(&20u32.to_le_bytes());
    eof[16..20].copy_from_slice(&20u32.to_le_bytes());
    [h, eof].concat()
}

/// A PICF (68 bytes, 1 × 0.5 inch goal size at 100 %) followed by PNG-looking bytes.
fn data_picture() -> (Vec<u8>, Vec<u8>) {
    let png: Vec<u8> = [b"\x89PNG\r\n\x1a\n".as_slice(), &[0x5Au8; 16]].concat();
    let mut picf = vec![0u8; 0x44];
    picf[0..4].copy_from_slice(&((0x44 + png.len()) as u32).to_le_bytes());
    picf[4..6].copy_from_slice(&0x44u16.to_le_bytes());
    picf[28..30].copy_from_slice(&1440i16.to_le_bytes());
    picf[30..32].copy_from_slice(&720i16.to_le_bytes());
    picf[32..34].copy_from_slice(&1000u16.to_le_bytes());
    picf[34..36].copy_from_slice(&1000u16.to_le_bytes());
    ([picf, png.clone()].concat(), png)
}

/// The .doc with the `EMBED` field; `ole_char` is the grpprl of the field code's `\x01`.
fn ole_doc(ole_char: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let (data, png) = data_picture();
    let ccp = TEXT.chars().count() as u32;
    let spec = StructSpec {
        text: TEXT,
        data_stream: data,
        chpx_runs: vec![
            (0, Vec::new()),
            (22, ole_char),
            (23, Vec::new()),
            (24, grpprl(&[(PIC_LOCATION, &0u32.to_le_bytes()), (F_SPEC, &[1])])),
            (25, Vec::new()),
        ],
        ..structured_spec(TEXT, ccp)
    };
    (struct_doc(&spec), png)
}

fn ole_char(id: u32) -> Vec<u8> {
    grpprl(&[(PIC_LOCATION, &id.to_le_bytes()), (F_OLE2, &[1]), (F_OBJ, &[1]), (F_SPEC, &[1])])
}

/// `doc` with `streams` added under `ObjectPool/<storage>/`, as Word lays out embedded objects.
fn with_object_pool(doc: Vec<u8>, storage: &str, streams: &[(&str, &[u8])]) -> Vec<u8> {
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(doc)).expect("cfb open");
    if !comp.is_storage("/ObjectPool") {
        comp.create_storage("/ObjectPool").expect("ObjectPool");
    }
    comp.create_storage(format!("/ObjectPool/{storage}")).expect("object storage");
    for (name, bytes) in streams {
        comp.create_stream(format!("/ObjectPool/{storage}/{name}")).expect("stream").write_all(bytes).expect("write");
    }
    comp.flush().expect("flush");
    comp.into_inner().into_inner()
}

/// The first inline image in the body: (its media bytes, w, h).
fn first_image(bytes: &[u8]) -> (Vec<u8>, f32, f32) {
    let doc = read(bytes).expect("opens");
    for block in &doc.body {
        let Some(p) = block.as_para() else { continue };
        for o in &p.objects {
            if let wordcraft_doc::para::InlineObject::Image { media, w, h, .. } = o {
                return (doc.media.get(media).expect("media entry").to_vec(), *w, *h);
            }
        }
    }
    panic!("no image in {:?}", doc.body)
}

#[test]
fn an_embedded_object_shows_its_eprint_picture_at_the_data_pictures_size() {
    let (doc, _png) = ole_doc(ole_char(OBJ_ID));
    let eprint = emf(500);
    let doc = with_object_pool(doc, &format!("_{OBJ_ID}"), &[("\u{3}EPRINT", &eprint), ("\u{1}Ole", &[0u8; 20])]);
    let (bytes, w, h) = first_image(&doc);
    assert_eq!(bytes, eprint, "the picture is the object's EPRINT presentation");
    assert_eq!((w, h), (72.0, 36.0), "size is the Data picture's PICF goal size (1440 x 720 twips)");
}

#[test]
fn without_an_eprint_stream_the_data_picture_stays() {
    let (doc, png) = ole_doc(ole_char(OBJ_ID));
    let doc = with_object_pool(doc, &format!("_{OBJ_ID}"), &[("\u{1}Ole", &[0u8; 20])]);
    let (bytes, w, h) = first_image(&doc);
    assert_eq!(bytes, png);
    assert_eq!((w, h), (72.0, 36.0));
}

#[test]
fn an_eprint_that_is_not_an_emf_is_ignored() {
    let (doc, png) = ole_doc(ole_char(OBJ_ID));
    let doc = with_object_pool(doc, &format!("_{OBJ_ID}"), &[("\u{3}EPRINT", &[0xEEu8; 300])]);
    assert_eq!(first_image(&doc).0, png);
    // A truncated EMF (header only, nBytes past the stream) is not used either.
    let (doc, png) = ole_doc(ole_char(OBJ_ID));
    let doc = with_object_pool(doc, &format!("_{OBJ_ID}"), &[("\u{3}EPRINT", &emf(500)[..60])]);
    assert_eq!(first_image(&doc).0, png);
}

#[test]
fn the_eprint_of_another_object_or_a_char_without_fole2_is_not_used() {
    // The storage is for a different id.
    let (doc, png) = ole_doc(ole_char(OBJ_ID));
    let doc = with_object_pool(doc, &format!("_{}", OBJ_ID + 1), &[("\u{3}EPRINT", &emf(500))]);
    assert_eq!(first_image(&doc).0, png);
    // The code char has the storage id but no sprmCFOle2: not an OLE object reference.
    let plain = grpprl(&[(PIC_LOCATION, &OBJ_ID.to_le_bytes()), (F_SPEC, &[1])]);
    let (doc, png) = ole_doc(plain);
    let doc = with_object_pool(doc, &format!("_{OBJ_ID}"), &[("\u{3}EPRINT", &emf(500))]);
    assert_eq!(first_image(&doc).0, png);
}
