//! An embedded OLE object (`EMBED` field whose object char has sprmCFOle2 = 1 and sprmCPicLocation =
//! the `ObjectPool/_<id>` storage id, [MS-DOC] §2.6.1) is kept as an object, not only as its picture:
//! the picture's `ole` holds a `w:object` (ECMA-376 §17.3.3.19) with an `o:OLEObject` whose ProgID is
//! the field code's class name, and the storage is copied into a standalone compound file in
//! `Document::passthrough`, so a .docx save writes `word/embeddings/oleObjectN.bin` as Word does.

use std::io::{Read, Write};

use wordcraft_doc::para::InlineObject;

use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_OLE2: u16 = 0x080A;
const F_OBJ: u16 = 0x0856;
const F_SPEC: u16 = 0x0855;

const OBJ_ID: u32 = 1_345_471_004;

/// `EMBED` field: begin CP 0, code CP 1..21, separator 22, result object char 23, end 24.
const TEXT: &str = "\u{13} EMBED Excel.Sheet.8 \u{14}\u{1}\u{15}\r";
const OBJ_CP: u32 = 23;

const WORKBOOK: &[u8] = b"synthetic workbook stream: copied verbatim, never parsed";
const COMP_OBJ: &[u8] = &[1, 0, 0xFE, 0xFF, 3, 10, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
const OLE: &[u8] = &[1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// A minimal WMF: META_HEADER then META_EOF.
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

/// `\x03META`: METAFILEPICT16 (mm 8, xExt, yExt, hMF 0) then the metafile.
fn meta() -> Vec<u8> {
    let mut v = Vec::new();
    for x in [8i16, 1746, 370, 0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&wmf());
    v
}

/// `\x03PIC` with goal size 2880 x 1440 twips (144 x 72 pt) at 100 %.
fn pic() -> Vec<u8> {
    let mut v = vec![0u8; 100];
    v[0..2].copy_from_slice(&100u16.to_le_bytes());
    v[20..24].copy_from_slice(&2880u32.to_le_bytes());
    v[24..28].copy_from_slice(&1440u32.to_le_bytes());
    v[44..48].copy_from_slice(&1000u32.to_le_bytes());
    v[48..52].copy_from_slice(&1000u32.to_le_bytes());
    v
}

fn object_char() -> Vec<u8> {
    let id = OBJ_ID.to_le_bytes();
    grpprl(&[(F_SPEC, &[1]), (F_OBJ, &[1]), (F_OLE2, &[1]), (PIC_LOCATION, &id)])
}

fn streams() -> Vec<(&'static str, Vec<u8>)> {
    vec![("\u{1}CompObj", COMP_OBJ.to_vec()), ("\u{1}Ole", OLE.to_vec()), ("\u{3}META", meta()), ("\u{3}PIC", pic()), ("Workbook", WORKBOOK.to_vec())]
}

/// The .doc with the field and, when `with_storage`, its `ObjectPool/_<id>` storage.
fn doc(with_storage: bool) -> Vec<u8> {
    let ccp = TEXT.chars().count() as u32;
    let spec =
        StructSpec { text: TEXT, chpx_runs: vec![(0, Vec::new()), (OBJ_CP, object_char()), (OBJ_CP + 1, Vec::new())], ..structured_spec(TEXT, ccp) };
    let bytes = struct_doc(&spec);
    if !with_storage {
        return bytes;
    }
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(bytes)).expect("cfb open");
    if !comp.is_storage("/ObjectPool") {
        comp.create_storage("/ObjectPool").expect("ObjectPool");
    }
    comp.create_storage(format!("/ObjectPool/_{OBJ_ID}")).expect("object storage");
    for (name, data) in streams() {
        comp.create_stream(format!("/ObjectPool/_{OBJ_ID}/{name}")).expect("stream").write_all(&data).expect("write");
    }
    comp.flush().expect("flush");
    comp.into_inner().into_inner()
}

/// Every inline picture's `ole` markup in the body (None for a plain picture).
fn oles(d: &wordcraft_doc::Document) -> Vec<Option<std::sync::Arc<wordcraft_doc::graphic::Embedded>>> {
    let mut out = Vec::new();
    for block in &d.body {
        let Some(p) = block.as_para() else { continue };
        for o in &p.objects {
            if let InlineObject::Image { ole, .. } = o {
                out.push(ole.clone());
            }
        }
    }
    out
}

#[test]
fn an_embedded_object_keeps_its_ole_object_markup() {
    let d = read(&doc(true)).expect("opens");
    let oles = oles(&d);
    assert_eq!(oles.len(), 1, "the object is one picture");
    let ole = oles[0].as_ref().expect("the picture carries its OLE object");
    let xml = &ole.xml;
    assert!(xml.starts_with("<w:object"), "a standalone w:object element: {xml}");
    assert!(xml.contains("<o:OLEObject"), "{xml}");
    assert!(xml.contains("Type=\"Embed\""), "{xml}");
    assert!(xml.contains("ProgID=\"Excel.Sheet.8\""), "the class name from the field code: {xml}");
    assert!(xml.contains(&format!("ObjectID=\"_{OBJ_ID}\"")), "{xml}");
    assert!(xml.contains("w:dxaOrig=\"2880\"") && xml.contains("w:dyaOrig=\"1440\""), "the PIC goal size: {xml}");
}

#[test]
fn the_object_storage_is_kept_as_a_standalone_compound_file() {
    let d = read(&doc(true)).expect("opens");
    let oles = oles(&d);
    let ole = oles[0].as_ref().expect("ole");
    let r = ole.refs.iter().find(|r| r.kind.ends_with("/oleObject")).expect("an oleObject relationship");
    assert!(!r.external, "embedded, not linked");
    assert_eq!(r.target, "word/embeddings/oleObject1.bin");
    assert!(ole.xml.contains(&format!("r:id=\"{}\"", r.id)), "the OLEObject uses the relationship");
    let bin = d.passthrough.get(&r.target).expect("the part's bytes are kept");
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(bin.to_vec())).expect("a compound file");
    for (name, data) in streams() {
        let mut got = Vec::new();
        comp.open_stream(format!("/{name}")).unwrap_or_else(|_| panic!("stream {name:?} at the root")).read_to_end(&mut got).expect("read");
        assert_eq!(got, data, "stream {name:?} is copied byte for byte");
    }
}

#[test]
fn the_picture_is_still_drawn_from_the_storage() {
    let d = read(&doc(true)).expect("opens");
    let mut n = 0;
    for block in &d.body {
        let Some(p) = block.as_para() else { continue };
        for o in &p.objects {
            if let InlineObject::Image { media, w, h, .. } = o {
                assert_eq!(d.media.get(media).expect("media").to_vec(), wmf(), "the META picture, as before");
                assert_eq!((*w, *h), (144.0, 72.0));
                n += 1;
            }
        }
    }
    assert_eq!(n, 1);
}

#[test]
fn without_a_storage_nothing_is_invented() {
    let d = read(&doc(false)).expect("opens");
    assert!(oles(&d).iter().all(Option::is_none), "no object markup without the storage");
    assert!(!d.passthrough.keys().any(|k| k.starts_with("word/embeddings/")), "no embedded part");
}
