//! Embedded objects kept whole (see `ole_embed_keep`): the `\x01` in the field code with the
//! picture in the Data stream, the ProgID from `\x01CompObj` when the field code names no class,
//! several objects numbered in order, and the cases that keep only the picture.

use std::io::Write;

use wordcraft_doc::para::InlineObject;

use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_OLE2: u16 = 0x080A;
const F_OBJ: u16 = 0x0856;
const F_SPEC: u16 = 0x0855;

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

fn meta() -> Vec<u8> {
    let mut v = Vec::new();
    for x in [8i16, 1746, 370, 0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&wmf());
    v
}

/// `\x03PIC` with goal size 1440 x 720 twips at 100 %.
fn pic() -> Vec<u8> {
    let mut v = vec![0u8; 100];
    v[20..24].copy_from_slice(&1440u32.to_le_bytes());
    v[24..28].copy_from_slice(&720u32.to_le_bytes());
    v[44..48].copy_from_slice(&1000u32.to_le_bytes());
    v[48..52].copy_from_slice(&1000u32.to_le_bytes());
    v
}

/// A `\x01CompObj` stream ([MS-OLEDS] §2.3.8) whose ProgID string is `prog`.
fn comp_obj(prog: &str) -> Vec<u8> {
    let mut v = vec![0u8; 28];
    let user = b"Synthetic Object\0";
    v.extend_from_slice(&(user.len() as u32).to_le_bytes());
    v.extend_from_slice(user);
    v.extend_from_slice(&0xFFFF_FFFEu32.to_le_bytes());
    v.extend_from_slice(&3u32.to_le_bytes());
    v.extend_from_slice(&(prog.len() as u32 + 1).to_le_bytes());
    v.extend_from_slice(prog.as_bytes());
    v.push(0);
    v
}

fn object_char(id: u32) -> Vec<u8> {
    let id = id.to_le_bytes();
    grpprl(&[(F_SPEC, &[1]), (F_OBJ, &[1]), (F_OLE2, &[1]), (PIC_LOCATION, &id)])
}

/// `doc` with storage `ObjectPool/_<id>` holding `streams`.
fn with_storage(doc: Vec<u8>, id: u32, streams: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(doc)).expect("cfb open");
    if !comp.is_storage("/ObjectPool") {
        comp.create_storage("/ObjectPool").expect("ObjectPool");
    }
    comp.create_storage(format!("/ObjectPool/_{id}")).expect("object storage");
    for (name, data) in streams {
        comp.create_stream(format!("/ObjectPool/_{id}/{name}")).expect("stream").write_all(data).expect("write");
    }
    comp.flush().expect("flush");
    comp.into_inner().into_inner()
}

/// A result-form object storage: its picture and goal size, and `extra` streams.
fn result_streams(extra: &[(&'static str, Vec<u8>)]) -> Vec<(&'static str, Vec<u8>)> {
    let mut v = vec![("\u{3}META", meta()), ("\u{3}PIC", pic())];
    v.extend(extra.iter().cloned());
    v
}

/// The body's inline objects, in order.
fn objects(d: &wordcraft_doc::Document) -> Vec<InlineObject> {
    d.body.iter().filter_map(|b| b.as_para()).flat_map(|p| p.objects.iter().cloned()).collect()
}

fn embedded(d: &wordcraft_doc::Document) -> Vec<std::sync::Arc<wordcraft_doc::graphic::Embedded>> {
    objects(d)
        .into_iter()
        .filter_map(|o| match o {
            InlineObject::Image { ole, .. } => ole,
            _ => None,
        })
        .collect()
}

fn fields(d: &wordcraft_doc::Document) -> Vec<String> {
    objects(d)
        .into_iter()
        .filter_map(|o| match o {
            InlineObject::Field { instr, .. } => Some(instr),
            _ => None,
        })
        .collect()
}

/// The target of the object's `kind` relationship.
fn target(e: &wordcraft_doc::graphic::Embedded, kind: &str) -> String {
    e.refs.iter().find(|r| r.kind.ends_with(kind)).map(|r| r.target.clone()).expect("relationship")
}

/// One result-form `EMBED` field with code `code` (`\x01` at the result).
fn result_doc(code: &'static str, id: u32) -> Vec<u8> {
    let text: &'static str = Box::leak(format!("\u{13}{code}\u{14}\u{1}\u{15}\r").into_boxed_str());
    let obj = text.chars().count() as u32 - 3;
    let spec = StructSpec {
        text,
        chpx_runs: vec![(0, Vec::new()), (obj, object_char(id)), (obj + 1, Vec::new())],
        ..structured_spec(text, text.chars().count() as u32)
    };
    struct_doc(&spec)
}

#[test]
fn an_object_in_the_field_code_with_a_data_stream_picture_is_kept() {
    const ID: u32 = 1_345_470_949;
    // Code `\x01` at CP 24, separator 25, result picture 26, end 27.
    const TEXT: &str = "\u{13} EMBED Word.Document.8 \u{1}\u{14}\u{1}\u{15}\r";
    let png: Vec<u8> = [b"\x89PNG\r\n\x1a\n".as_slice(), &[0x5Au8; 16]].concat();
    let mut picf = vec![0u8; 0x44];
    picf[0..4].copy_from_slice(&((0x44 + png.len()) as u32).to_le_bytes());
    picf[4..6].copy_from_slice(&0x44u16.to_le_bytes());
    picf[28..30].copy_from_slice(&1440i16.to_le_bytes());
    picf[30..32].copy_from_slice(&720i16.to_le_bytes());
    picf[32..34].copy_from_slice(&1000u16.to_le_bytes());
    picf[34..36].copy_from_slice(&1000u16.to_le_bytes());
    let spec = StructSpec {
        text: TEXT,
        data_stream: [picf, png.clone()].concat(),
        chpx_runs: vec![
            (0, Vec::new()),
            (24, object_char(ID)),
            (25, Vec::new()),
            (26, grpprl(&[(PIC_LOCATION, &0u32.to_le_bytes()), (F_SPEC, &[1])])),
            (27, Vec::new()),
        ],
        ..structured_spec(TEXT, TEXT.chars().count() as u32)
    };
    let doc = with_storage(struct_doc(&spec), ID, &[("\u{1}Ole", vec![0u8; 20]), ("WordDocument", b"inner document".to_vec())]);
    let d = read(&doc).expect("opens");
    let kept = embedded(&d);
    assert_eq!(kept.len(), 1, "{:?}", objects(&d));
    assert!(kept[0].xml.contains("ProgID=\"Word.Document.8\""), "{}", kept[0].xml);
    // No PIC stream: the original size is the shown picture's (72 x 36 pt).
    assert!(kept[0].xml.contains("w:dxaOrig=\"1440\"") && kept[0].xml.contains("w:dyaOrig=\"720\""), "{}", kept[0].xml);
    let image = target(&kept[0], "/image");
    assert_eq!(d.passthrough.get(&image).map(|b| b.to_vec()), Some(png), "the picture part is the shown picture");
    assert!(fields(&d).is_empty(), "the EMBED field leaves no trace: {:?}", fields(&d));
}

#[test]
fn without_a_class_in_the_field_code_the_prog_id_comes_from_comp_obj() {
    const ID: u32 = 1_009_175_562;
    let doc = with_storage(result_doc(" EMBED ", ID), ID, &result_streams(&[("\u{1}CompObj", comp_obj("Equation.3"))]));
    let d = read(&doc).expect("opens");
    let kept = embedded(&d);
    assert!(kept[0].xml.contains("ProgID=\"Equation.3\""), "{}", kept[0].xml);
    // A switch is not a class name either.
    let doc = with_storage(result_doc(" EMBED \\s ", ID), ID, &result_streams(&[("\u{1}CompObj", comp_obj("Equation.3"))]));
    assert!(embedded(&read(&doc).expect("opens"))[0].xml.contains("ProgID=\"Equation.3\""));
}

#[test]
fn without_any_class_name_the_prog_id_is_package() {
    const ID: u32 = 7;
    for comp in [vec![1u8, 2, 3], comp_obj("not a <prog> id"), comp_obj("")] {
        let doc = with_storage(result_doc(" EMBED ", ID), ID, &result_streams(&[("\u{1}CompObj", comp)]));
        let d = read(&doc).expect("opens");
        assert!(embedded(&d)[0].xml.contains("ProgID=\"Package\""), "{}", embedded(&d)[0].xml);
    }
}

#[test]
fn two_objects_are_numbered_in_order() {
    const A: u32 = 1_000_000_001;
    const B: u32 = 1_000_000_002;
    // Two result-form fields: objects at CPs 23 and 53.
    const TEXT: &str = "\u{13} EMBED Excel.Sheet.8 \u{14}\u{1}\u{15} \u{13} EMBED PowerPoint.Show.8 \u{14}\u{1}\u{15}\r";
    let (first, second) = (23, 53);
    let spec = StructSpec {
        text: TEXT,
        chpx_runs: vec![(0, Vec::new()), (first, object_char(A)), (first + 1, Vec::new()), (second, object_char(B)), (second + 1, Vec::new())],
        ..structured_spec(TEXT, TEXT.chars().count() as u32)
    };
    let doc = with_storage(struct_doc(&spec), A, &result_streams(&[("Workbook", b"first".to_vec())]));
    let doc = with_storage(doc, B, &result_streams(&[("PowerPoint Document", b"second".to_vec())]));
    let d = read(&doc).expect("opens");
    let kept = embedded(&d);
    assert_eq!(kept.len(), 2);
    assert!(kept[0].xml.contains("ProgID=\"Excel.Sheet.8\"") && kept[0].xml.contains(&format!("ObjectID=\"_{A}\"")));
    assert!(kept[1].xml.contains("ProgID=\"PowerPoint.Show.8\"") && kept[1].xml.contains(&format!("ObjectID=\"_{B}\"")));
    assert_eq!(target(&kept[0], "/oleObject"), "word/embeddings/oleObject1.bin");
    assert_eq!(target(&kept[1], "/oleObject"), "word/embeddings/oleObject2.bin");
    for (e, stream, bytes) in [(&kept[0], "/Workbook", b"first".as_slice()), (&kept[1], "/PowerPoint Document", b"second")] {
        let bin = d.passthrough.get(&target(e, "/oleObject")).expect("bin").to_vec();
        let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(bin)).expect("compound file");
        let mut got = Vec::new();
        std::io::Read::read_to_end(&mut comp.open_stream(stream).expect("stream"), &mut got).expect("read");
        assert_eq!(got, bytes);
    }
    let manifest = String::from_utf8(d.passthrough.get("wordcraft:embedded.parts").expect("manifest").to_vec()).expect("utf-8");
    assert!(manifest.contains("P\tword/embeddings/oleObject2.bin\tapplication/vnd.openxmlformats-officedocument.oleObject\n"), "{manifest}");
    assert!(manifest.contains("P\tword/media/oleObject1.wmf\timage/x-wmf\n"), "{manifest}");
    assert!(fields(&d).is_empty());
}

#[test]
fn an_object_without_its_storage_keeps_its_field() {
    // The Data stream picture form with no `ObjectPool` storage: the picture and the field as before.
    let d = read(&result_doc(" EMBED Excel.Sheet.8 ", 42)).expect("opens");
    assert!(embedded(&d).is_empty());
    assert_eq!(fields(&d), vec!["EMBED Excel.Sheet.8".to_string()]);
}

#[test]
fn a_field_that_is_not_embed_keeps_only_the_picture() {
    const ID: u32 = 9;
    let doc = with_storage(result_doc(" LINK Excel.Sheet.8 ", ID), ID, &result_streams(&[]));
    let d = read(&doc).expect("opens");
    assert!(embedded(&d).is_empty());
    assert_eq!(fields(&d).len(), 1);
    assert!(!d.passthrough.keys().any(|k| k.starts_with("word/embeddings/")));
}
