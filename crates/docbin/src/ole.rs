//! Embedded OLE objects' print presentation ([MS-DOC] §2.6.1 sprmCFOle2, sprmCPicLocation;
//! [MS-OLEDS] presentation streams). An `EMBED` field's code holds a `\x01` with sprmCFOle2 = 1
//! whose sprmCPicLocation is the id of the object's storage `ObjectPool/_<id>`. When that storage
//! holds a `\x03EPRINT` stream (a plain EMF), Word prints that picture instead of the field
//! result's Data stream picture.

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

/// The print presentations (EMF bytes) of the document's embedded objects, by storage id.
#[derive(Default)]
pub(crate) struct Prints {
    by_id: HashMap<u32, Vec<u8>>,
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
            let Some(bytes) = eprint(comp, id) else { continue };
            let Some(t) = total.checked_add(bytes.len()).filter(|&t| t <= media::MAX_MEDIA_BYTES) else { break };
            total = t;
            prints.by_id.insert(id, bytes);
        }
        prints
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
