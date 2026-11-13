//! An inline picture stored as a plain Windows Metafile right after its PICF ([MS-DOC] §2.9.190,
//! `mfpf.mm` = 8 MM_ANISOTROPIC; [MS-WMF] §2.3.2.2 META_HEADER, no placeable header), as older
//! writers and OLE servers' field results (charts, clip art) store it. Word draws it at the goal
//! size times the scale; it must not be dropped.

use wordcraft_doc::para::InlineObject;

use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_SPEC: u16 = 0x0855;
const MM_ANISOTROPIC: u16 = 8;

/// META_HEADER (Type 1, HeaderSize 9, Version 0x0300, `size` words, MaxRecord 3) then META_EOF.
fn wmf(size: u32) -> Vec<u8> {
    let mut v = vec![1, 0, 9, 0, 0, 3];
    v.extend_from_slice(&size.to_le_bytes());
    v.extend_from_slice(&[0, 0, 3, 0, 0, 0, 0, 0]);
    v.extend_from_slice(&[3, 0, 0, 0, 0, 0]);
    v
}

/// A PICF with `mm`, goal size (twips) and scale (1000 = 100 %), then `rest`.
fn picf(mm: u16, goal: (i16, i16), scale: (u16, u16), rest: &[u8]) -> Vec<u8> {
    let mut picf = vec![0u8; 0x44];
    picf[0..4].copy_from_slice(&((0x44 + rest.len()) as u32).to_le_bytes());
    picf[4..6].copy_from_slice(&0x44u16.to_le_bytes());
    picf[6..8].copy_from_slice(&mm.to_le_bytes());
    picf[28..30].copy_from_slice(&goal.0.to_le_bytes());
    picf[30..32].copy_from_slice(&goal.1.to_le_bytes());
    picf[32..34].copy_from_slice(&scale.0.to_le_bytes());
    picf[34..36].copy_from_slice(&scale.1.to_le_bytes());
    [picf.as_slice(), rest].concat()
}

/// `text` with the picture character at `pic_cp` pointing at Data offset 0.
fn doc_with(text: &'static str, pic_cp: u32, data: Vec<u8>) -> wordcraft_doc::Document {
    let ccp = text.chars().count() as u32;
    let pic = grpprl(&[(PIC_LOCATION, &0u32.to_le_bytes()), (F_SPEC, &[1])]);
    let spec = StructSpec {
        text,
        data_stream: data,
        chpx_runs: vec![(0, Vec::new()), (pic_cp, pic), (pic_cp + 1, Vec::new())],
        ..structured_spec(text, ccp)
    };
    read(&struct_doc(&spec)).expect("opens")
}

/// Every inline image in the body: (media key, media bytes, w, h).
fn images(doc: &wordcraft_doc::Document) -> Vec<(String, Vec<u8>, f32, f32)> {
    let mut out = Vec::new();
    for p in doc.body.iter().filter_map(|b| b.as_para()) {
        for o in &p.objects {
            if let InlineObject::Image { media, w, h, .. } = o {
                let bytes = doc.media.get(media).map(|b| b.to_vec()).unwrap_or_default();
                out.push((media.clone(), bytes, *w, *h));
            }
        }
    }
    out
}

fn assert_chart(doc: &wordcraft_doc::Document) {
    let imgs = images(doc);
    assert_eq!(imgs.len(), 1, "{imgs:?}");
    let (key, bytes, w, h) = &imgs[0];
    assert!(key.ends_with(".wmf"), "{key}");
    assert_eq!(bytes, &wmf(12));
    assert!((w - 441.95).abs() <= 0.01 && (h - 308.05).abs() <= 0.01, "{w} x {h}");
}

#[test]
fn a_raw_metafile_picture_is_kept_at_its_size() {
    let doc = doc_with("a\u{1}b\r", 1, picf(MM_ANISOTROPIC, (8839, 6161), (1000, 1000), &wmf(12)));
    assert_chart(&doc);
}

#[test]
fn a_raw_metafile_embed_result_is_kept() {
    const TEXT: &str = "\u{13} EMBED MSGraph.Chart.8 \\s \u{14}\u{1}\u{15}\r";
    let pic_cp = TEXT.chars().position(|c| c == '\u{1}').unwrap() as u32;
    let doc = doc_with(TEXT, pic_cp, picf(MM_ANISOTROPIC, (8839, 6161), (1000, 1000), &wmf(12)));
    assert_chart(&doc);
}

#[test]
fn a_metafile_shorter_than_its_header_says_is_dropped() {
    let doc = doc_with("a\u{1}b\r", 1, picf(MM_ANISOTROPIC, (8839, 6161), (1000, 1000), &wmf(1000)));
    assert!(images(&doc).is_empty());
    assert!(doc.body.first().and_then(|b| b.as_para()).is_some_and(|p| p.text.contains('b')));
}
