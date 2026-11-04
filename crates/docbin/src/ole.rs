//! Embedded OLE objects' print presentation ([MS-DOC] §2.6.1 sprmCFOle2, sprmCPicLocation;
//! [MS-OLEDS] presentation streams). An `EMBED` field's code holds a `\x01` with sprmCFOle2 = 1
//! whose sprmCPicLocation is the id of the object's storage `ObjectPool/_<id>`. When that storage
//! holds a `\x03EPRINT` stream (a plain EMF), Word prints that picture instead of the field
//! result's Data stream picture.
//!
//! Word 97-2003 also writes objects (Equation Editor equations) whose reference is the `\x01` in
//! the field's result, with no picture in the Data stream. Word draws those from the storage: its
//! `\x03EPRINT` when there is one, else the Windows Metafile in `\x03META` (after an 8-byte
//! METAFILEPICT16 header), at the goal size in `\x03PIC`.

use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::{media, officeart, sprm};

/// sprmCFOle2: the character is an OLE object reference.
const C_F_OLE2: u16 = 0x080A;
/// sprmCPicLocation: for an OLE object reference, its storage id.
const C_PIC_LOCATION: u16 = 0x6A03;
/// Most object storages looked at in one document.
const MAX_OBJECTS: usize = 5_000;
/// `EMR_HEADER` record type and the " EMF" signature at byte offset 40.
const EMR_HEADER: u32 = 1;
const ENHMETA_SIGNATURE: u32 = 0x464D_4520;
/// Smallest `EMR_HEADER` (the fields up to `szlMillimeters`).
const MIN_HEADER: u32 = 88;
/// `\x03META`: the METAFILEPICT16 header (mm, xExt, yExt, hMF) before the metafile.
const META_PICT_HEADER: usize = 8;
/// `\x03PIC`: dxaGoal, dyaGoal (twips) and mx, my (per mille) offsets, and the bytes needed.
const PIC_DXA_GOAL: usize = 20;
const PIC_DYA_GOAL: usize = 24;
const PIC_MX: usize = 44;
const PIC_MY: usize = 48;
const PIC_MIN_LEN: usize = 52;

/// The print presentations (EMF bytes) of the document's embedded objects, by storage id, and
/// the pictures of objects referenced from a field result.
#[derive(Default)]
pub(crate) struct Prints {
    by_id: HashMap<u32, Vec<u8>>,
    /// Storage id → (WMF bytes from `\x03META`, for objects without an EPRINT).
    metas: HashMap<u32, Vec<u8>>,
    /// Storage id → goal size in points from `\x03PIC`.
    goals: HashMap<u32, (f32, f32)>,
}

/// The picture of an object referenced from a field result: its bytes, media extension and
/// display size in points.
pub(crate) struct ResultObject<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) ext: &'static str,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

impl Prints {
    /// Read every `ObjectPool/_<id>/\x03EPRINT` that is a well-formed EMF, each no larger than
    /// the per-picture cap and all together within the document's media byte cap.
    pub(crate) fn read<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>) -> Self {
        let mut prints = Prints::default();
        let ids: Vec<u32> = match comp.read_storage("/ObjectPool") {
            Ok(entries) => entries.filter(|e| e.is_storage()).filter_map(|e| storage_id(e.name())).take(MAX_OBJECTS).collect(),
            Err(_) => return prints,
        };
        let mut total = 0usize;
        for id in ids {
            if let Some(goal) = pic_goal(comp, id) {
                prints.goals.insert(id, goal);
            }
            let (bytes, is_meta) = match eprint(comp, id) {
                Some(b) => (b, false),
                None => match meta(comp, id) {
                    Some(b) => (b, true),
                    None => continue,
                },
            };
            let Some(t) = total.checked_add(bytes.len()).filter(|&t| t <= media::MAX_MEDIA_BYTES) else { break };
            total = t;
            if is_meta {
                prints.metas.insert(id, bytes);
            } else {
                prints.by_id.insert(id, bytes);
            }
        }
        prints
    }

    /// The picture of object `id` referenced from a field result: its EPRINT (else its META
    /// metafile) at its PIC goal size. `None` without a usable picture or size.
    pub(crate) fn result_object(&self, id: u32) -> Option<ResultObject<'_>> {
        let &(w, h) = self.goals.get(&id)?;
        let (bytes, ext) = match self.by_id.get(&id) {
            Some(emf) => (emf.as_slice(), "emf"),
            None => (self.metas.get(&id)?.as_slice(), "wmf"),
        };
        Some(ResultObject { bytes, ext, w, h })
    }

    /// The print presentation of the object in storage `id`.
    pub(crate) fn get(&self, id: u32) -> Option<&[u8]> {
        self.by_id.get(&id).map(Vec::as_slice)
    }
}

/// The id of an object storage named `_<decimal id>`.
fn storage_id(name: &str) -> Option<u32> {
    let digits = name.strip_prefix('_')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The object's `\x03EPRINT` stream, if it is a well-formed EMF within the per-picture cap.
fn eprint<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, id: u32) -> Option<Vec<u8>> {
    let mut s = comp.open_stream(format!("/ObjectPool/_{id}/\u{3}EPRINT")).ok()?;
    let cap = officeart::MAX_METAFILE as u64;
    if s.len() > cap {
        return None;
    }
    let mut v = Vec::new();
    s.by_ref().take(cap.saturating_add(1)).read_to_end(&mut v).ok()?;
    (v.len() as u64 <= cap && is_emf(&v)).then_some(v)
}

/// The Windows Metafile in the object's `\x03META` stream (after its METAFILEPICT16 header), if
/// it is well-formed and within the per-picture cap.
fn meta<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, id: u32) -> Option<Vec<u8>> {
    let mut s = comp.open_stream(format!("/ObjectPool/_{id}/\u{3}META")).ok()?;
    let cap = (officeart::MAX_METAFILE as u64).saturating_add(META_PICT_HEADER as u64);
    if s.len() > cap {
        return None;
    }
    let mut v = Vec::new();
    s.by_ref().take(cap.saturating_add(1)).read_to_end(&mut v).ok()?;
    if v.len() as u64 > cap {
        return None;
    }
    let wmf = v.get(META_PICT_HEADER..)?;
    is_wmf(wmf).then(|| wmf.to_vec())
}

/// The object's goal size in points from its `\x03PIC` stream: dxaGoal x dyaGoal twips scaled
/// by mx/my per mille. `None` when the stream is missing or short, or a size is 0.
fn pic_goal<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, id: u32) -> Option<(f32, f32)> {
    let s = comp.open_stream(format!("/ObjectPool/_{id}/\u{3}PIC")).ok()?;
    let mut v = Vec::new();
    s.take(PIC_MIN_LEN as u64).read_to_end(&mut v).ok()?;
    let u32_at = |at: usize| v.get(at..at + 4).and_then(|s| s.try_into().ok()).map(u32::from_le_bytes);
    let (dxa, dya, mx, my) = (u32_at(PIC_DXA_GOAL)?, u32_at(PIC_DYA_GOAL)?, u32_at(PIC_MX)?, u32_at(PIC_MY)?);
    let pt = |goal: u32, scale: u32| f64::from(goal) * f64::from(scale) / 1000.0 / 20.0;
    let (w, h) = (pt(dxa, mx), pt(dya, my));
    // Hostile sizes are refused: at most ten times the largest page Word allows (22 in = 1584 pt).
    let ok = |x: f64| x > 0.0 && x <= 15_840.0;
    (ok(w) && ok(h)).then_some((w as f32, h as f32))
}

/// A Windows Metafile: its META_HEADER has type 1 or 2, headerSize 9, version 0x0100 or 0x0300,
/// and a file size (in 16-bit words) that fits in the bytes there are ([MS-WMF] §2.3.2.2).
fn is_wmf(b: &[u8]) -> bool {
    let u16_at = |at: usize| b.get(at..at + 2).and_then(|s| s.try_into().ok()).map(u16::from_le_bytes);
    let size = b.get(6..10).and_then(|s| s.try_into().ok()).map(u32::from_le_bytes);
    let (Some(typ), Some(header), Some(version), Some(size)) = (u16_at(0), u16_at(2), u16_at(4), size) else {
        return false;
    };
    matches!(typ, 1 | 2) && header == 9 && matches!(version, 0x0100 | 0x0300) && (size as u64).saturating_mul(2) <= b.len() as u64
}

/// An EMF: it starts with an `EMR_HEADER` record carrying the signature, and the file size the
/// header declares fits in the bytes there are.
fn is_emf(b: &[u8]) -> bool {
    let u32_at = |at: usize| b.get(at..at + 4).and_then(|s| s.try_into().ok()).map(u32::from_le_bytes);
    let (Some(typ), Some(size), Some(sig), Some(n_bytes)) = (u32_at(0), u32_at(4), u32_at(40), u32_at(48)) else {
        return false;
    };
    typ == EMR_HEADER && size >= MIN_HEADER && size as usize <= b.len() && sig == ENHMETA_SIGNATURE && n_bytes as usize <= b.len()
}

/// The storage id of an OLE object reference character: its CHPX has sprmCFOle2 = 1 and a
/// sprmCPicLocation.
pub(crate) fn object_id(chpx: &[u8]) -> Option<u32> {
    let mut ole = false;
    let mut id = None;
    for prl in sprm::iter(chpx) {
        match (prl.op, prl.operand) {
            (C_F_OLE2, [v, ..]) => ole = *v == 1,
            (C_PIC_LOCATION, [b0, b1, b2, b3, ..]) => id = Some(u32::from_le_bytes([*b0, *b1, *b2, *b3])),
            _ => {}
        }
    }
    id.filter(|_| ole)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_names_parse_only_as_decimal_ids() {
        assert_eq!(storage_id("_1345470949"), Some(1_345_470_949));
        assert_eq!(storage_id("_"), None);
        assert_eq!(storage_id("_+1"), None);
        assert_eq!(storage_id("_99999999999"), None);
        assert_eq!(storage_id("1"), None);
        assert_eq!(storage_id("_１"), None);
    }

    #[test]
    fn short_or_lying_headers_are_not_wmf() {
        assert!(!is_wmf(&[]));
        let mut h = vec![0u8; 18];
        h[0] = 1;
        h[2] = 9;
        h[4..6].copy_from_slice(&0x0300u16.to_le_bytes());
        h[6] = 9;
        assert!(is_wmf(&h));
        h[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(!is_wmf(&h));
        h[6..10].copy_from_slice(&9u32.to_le_bytes());
        h[2] = 8;
        assert!(!is_wmf(&h));
        assert!(!is_wmf(&h[..9]));
    }

    #[test]
    fn short_or_lying_headers_are_not_emf() {
        assert!(!is_emf(&[]));
        let mut h = vec![0u8; 88];
        h[0] = 1;
        h[4] = 88;
        h[40..44].copy_from_slice(&ENHMETA_SIGNATURE.to_le_bytes());
        h[48] = 88;
        assert!(is_emf(&h));
        h[48..52].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(!is_emf(&h));
        h[48..52].copy_from_slice(&88u32.to_le_bytes());
        h[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(!is_emf(&h));
    }
}
