//! An embedded object whose reference is the `\x01` in its `EMBED` field's RESULT (no `\x01` in the
//! code), as Word 97-2003 writes Equation Editor objects: the result char has sprmCFOle2 = 1 and its
//! sprmCPicLocation is the `ObjectPool/_<id>` storage id, not a Data stream fc ([MS-DOC] §2.6.1).
//! Word draws the object from the storage: `\x03META` holds an 8-byte METAFILEPICT16 header and a
//! Windows Metafile, `\x03PIC` the picture's goal size in twips (dxaGoal at byte 20, dyaGoal at 24,
//! mx/my per mille at 44/48). Measured in Word's PDF: a 1320 x 280 twips goal is drawn 66 pt wide.

use std::io::Write;

use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_OLE2: u16 = 0x080A;
const F_OBJ: u16 = 0x0856;
const F_SPEC: u16 = 0x0855;

const OBJ_ID: u32 = 1_009_175_562;

/// `EMBED` field: code CP 1..17, separator 17, result object char 18, end 19.
const TEXT: &str = "\u{13} EMBED Equation \u{14}\u{1}\u{15}\r";

/// A minimal WMF: META_HEADER (18 bytes) then META_EOF (6 bytes); `tag` goes in maxRecord so two
/// files differ.
fn wmf(tag: u16) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&1u16.to_le_bytes()); // type: memory metafile
    v.extend_from_slice(&9u16.to_le_bytes()); // headerSize in words
    v.extend_from_slice(&0x0300u16.to_le_bytes()); // version
    v.extend_from_slice(&12u32.to_le_bytes()); // size in words (24 bytes)
    v.extend_from_slice(&0u16.to_le_bytes()); // numberOfObjects
    v.extend_from_slice(&u32::from(tag).to_le_bytes()); // maxRecord
    v.extend_from_slice(&0u16.to_le_bytes()); // numberOfMembers
    v.extend_from_slice(&3u32.to_le_bytes()); // META_EOF: size 3 words
    v.extend_from_slice(&0u16.to_le_bytes()); // function 0
    v
}

/// `\x03META`: METAFILEPICT16 (mm 8, xExt, yExt, hMF 0) then the metafile.
fn meta(wmf: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    for x in [8i16, 1746, 370, 0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(wmf);
    v
}

/// `\x03PIC` as Word writes it, with goal size `dxa` x `dya` twips at 100 %.
fn pic(dxa: u32, dya: u32) -> Vec<u8> {
    let mut v = vec![0u8; 100];
    v[0..2].copy_from_slice(&100u16.to_le_bytes());
    v[2..4].copy_from_slice(&2u16.to_le_bytes());
    v[4..8].copy_from_slice(&1u32.to_le_bytes());
    v[10..12].copy_from_slice(&8u16.to_le_bytes());
    v[12..16].copy_from_slice(&1746u32.to_le_bytes());
    v[16..20].copy_from_slice(&370u32.to_le_bytes());
    v[20..24].copy_from_slice(&dxa.to_le_bytes());
    v[24..28].copy_from_slice(&dya.to_le_bytes());
    v[44..48].copy_from_slice(&1000u32.to_le_bytes());
    v[48..52].copy_from_slice(&1000u32.to_le_bytes());
    v
}

/// The object reference char's grpprl, as in the real file.
fn result_char(id: u32, ole2: bool) -> Vec<u8> {
    let mut sprms: Vec<(u16, &[u8])> = vec![(F_SPEC, &[1]), (F_OBJ, &[1])];
    if ole2 {
        sprms.push((F_OLE2, &[1]));
    }
    let id = id.to_le_bytes();
    sprms.push((PIC_LOCATION, &id));
    grpprl(&sprms)
}

/// The .doc with the field; no Data stream picture at all.
fn doc(result_grpprl: Vec<u8>) -> Vec<u8> {
    let ccp = TEXT.chars().count() as u32;
    let spec = StructSpec { text: TEXT, chpx_runs: vec![(0, Vec::new()), (18, result_grpprl), (19, Vec::new())], ..structured_spec(TEXT, ccp) };
    struct_doc(&spec)
}

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

/// Every inline image in the body: (media bytes, w, h).
fn images(bytes: &[u8]) -> Vec<(Vec<u8>, f32, f32)> {
    let doc = read(bytes).expect("opens");
    let mut out = Vec::new();
    for block in &doc.body {
        let Some(p) = block.as_para() else { continue };
        for o in &p.objects {
            if let wordcraft_doc::para::InlineObject::Image { media, w, h, .. } = o {
                out.push((doc.media.get(media).expect("media entry").to_vec(), *w, *h));
            }
        }
    }
    out
}

/// The usual Equation Editor storage: an empty OlePres000, the META picture and the PIC size.
fn equation_storage(doc: Vec<u8>, id: u32, wmf: &[u8], pic: &[u8]) -> Vec<u8> {
    let meta = meta(wmf);
    let empty_pres = [0xFEu8, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0];
    with_object_pool(doc, &format!("_{id}"), &[("\u{1}Ole", &[0u8; 20]), ("\u{2}OlePres000", &empty_pres), ("\u{3}META", &meta), ("\u{3}PIC", pic)])
}

#[test]
fn a_result_object_char_draws_the_objects_metafile_at_its_pic_goal_size() {
    let w = wmf(7);
    let bytes = equation_storage(doc(result_char(OBJ_ID, true)), OBJ_ID, &w, &pic(1320, 280));
    let imgs = images(&bytes);
    assert_eq!(imgs.len(), 1, "the equation object is drawn once");
    let (media, width, height) = &imgs[0];
    assert_eq!(media, &w, "the picture is the WMF after META's 8-byte METAFILEPICT16 header");
    assert_eq!((*width, *height), (66.0, 14.0), "1320 x 280 twips");
}

#[test]
fn a_valid_eprint_in_the_storage_is_preferred_over_meta() {
    // A minimal EMF: EMR_HEADER (108 bytes, " EMF" at 40) and EMR_EOF (20 bytes).
    let mut emf = vec![0u8; 108];
    emf[0..4].copy_from_slice(&1u32.to_le_bytes());
    emf[4..8].copy_from_slice(&108u32.to_le_bytes());
    emf[40..44].copy_from_slice(&0x464D_4520u32.to_le_bytes());
    emf[44..48].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    emf[48..52].copy_from_slice(&128u32.to_le_bytes());
    emf[52..56].copy_from_slice(&2u32.to_le_bytes());
    emf[56..58].copy_from_slice(&1u16.to_le_bytes()); // nHandles (metafile itself)
    let mut eof = vec![0u8; 20];
    eof[0..4].copy_from_slice(&14u32.to_le_bytes());
    eof[4..8].copy_from_slice(&20u32.to_le_bytes());
    eof[16..20].copy_from_slice(&20u32.to_le_bytes());
    emf.extend_from_slice(&eof);
    let bytes = equation_storage(doc(result_char(OBJ_ID, true)), OBJ_ID, &wmf(7), &pic(1240, 240));
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(bytes)).expect("cfb open");
    comp.create_stream(format!("/ObjectPool/_{OBJ_ID}/\u{3}EPRINT")).expect("stream").write_all(&emf).expect("write");
    comp.flush().expect("flush");
    let bytes = comp.into_inner().into_inner();
    let imgs = images(&bytes);
    assert_eq!(imgs.len(), 1);
    assert_eq!(imgs[0].0, emf, "EPRINT is Word's print presentation");
    assert_eq!((imgs[0].1, imgs[0].2), (62.0, 12.0), "still the PIC goal size, 1240 x 240 twips");
}

#[test]
fn without_a_usable_picture_or_size_nothing_is_invented() {
    // META that is not a metafile.
    let bytes = equation_storage(doc(result_char(OBJ_ID, true)), OBJ_ID, &[0xEEu8; 40], &pic(1320, 280));
    assert!(images(&bytes).is_empty(), "a bad META is not drawn");
    // No PIC stream: no size, so no picture.
    let meta = meta(&wmf(7));
    let bytes = with_object_pool(doc(result_char(OBJ_ID, true)), &format!("_{OBJ_ID}"), &[("\u{3}META", &meta)]);
    assert!(images(&bytes).is_empty(), "no PIC, no size");
    // The storage belongs to another object.
    let bytes = equation_storage(doc(result_char(OBJ_ID, true)), OBJ_ID + 1, &wmf(7), &pic(1320, 280));
    assert!(images(&bytes).is_empty(), "another object's storage is not used");
}

#[test]
fn a_result_char_without_fole2_is_not_an_object_reference() {
    // The same location value without sprmCFOle2 is a Data stream fc (far past the end): dropped as before.
    let bytes = equation_storage(doc(result_char(OBJ_ID, false)), OBJ_ID, &wmf(7), &pic(1320, 280));
    assert!(images(&bytes).is_empty());
    // The text around the field survives either way.
    let doc = read(&bytes).expect("opens");
    assert!(!doc.body.is_empty());
}
