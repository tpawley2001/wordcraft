//! Embedded OLE objects' print presentation ([MS-DOC] §2.6.1 sprmCFOle2, sprmCPicLocation;
//! [MS-OLEDS] presentation streams). An `EMBED` field's code holds a `\x01` with sprmCFOle2 = 1
//! whose sprmCPicLocation is the id of the object's storage `ObjectPool/_<id>`. When that storage
//! holds a `\x03EPRINT` stream (a plain EMF), Word prints that picture instead of the field
//! result's Data stream picture.
//!
//! Word 97-2003 also writes objects (Equation Editor equations) whose reference is the `\x01` in
//! the field's result, with no picture in the Data stream. Word draws those from the storage: its
//! `\x03EPRINT` when there is one, else the Windows Metafile in `\x03META` (after an 8-byte
//! METAFILEPICT16 header), at the goal size in `\x03PIC`. An object whose only presentation is
//! a QuickDraw `\x03PICT` (Mac Word) is not drawn from it: Word shows a white box framed by a
//! 0.72 pt black line inside its edges, at the METAFILEPICT extents in `\x03PIC` ([`placeholder`]).
//!
//! Each object storage is also kept whole: a copy of it as a standalone compound file, written
//! back by a .docx save as `word/embeddings/oleObjectN.bin` behind a `w:object` (ECMA-376
//! §17.3.3.19) whose `o:OLEObject` names it, so the object stays editable ([`Kept`]).

use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;
use std::sync::Arc;

use wordcraft_doc::graphic::{Embedded, EmbeddedRef};

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
/// `\x03PIC`: the METAFILEPICT mapping mode and extents (HIMETRIC for MM_ISOTROPIC and
/// MM_ANISOTROPIC).
const PIC_MM: usize = 10;
const PIC_X_EXT: usize = 12;
const PIC_Y_EXT: usize = 16;
const MM_ISOTROPIC: u16 = 7;
const MM_ANISOTROPIC: u16 = 8;
/// A PICT-only object's placeholder frame thickness, in points (inside the box edges).
const PLACEHOLDER_FRAME_PT: f64 = 0.72;
/// `\x03PIC`: dxaGoal, dyaGoal (twips) and mx, my (per mille) offsets, and the bytes needed.
const PIC_DXA_GOAL: usize = 20;
const PIC_DYA_GOAL: usize = 24;
const PIC_MX: usize = 44;
const PIC_MY: usize = 48;
const PIC_MIN_LEN: usize = 52;
/// Most entries (streams and storages) in one object storage that is kept.
const MAX_STORAGE_ENTRIES: usize = 4096;
/// Deepest sub-storage path (in names below the object storage) of one object that is kept.
const MAX_STORAGE_DEPTH: usize = 16;
/// Most bytes of `\x01CompObj` read for the object's ProgID.
const MAX_COMP_OBJ: u64 = 4096;
/// `\x01CompObj`: the CompObjHeader before AnsiUserType ([MS-OLEDS] §2.3.8).
const COMP_OBJ_HEADER: usize = 28;
/// Longest ProgID (COM's limit is 39 characters).
const MAX_PROG_ID: usize = 39;
/// The `Document::passthrough` key of the manifest of kept parts the .docx writer reads
/// (`wordcraft_docx::package::EMBEDDED_PARTS`): `P\tpart\tcontent type` lines.
const EMBEDDED_PARTS: &str = "wordcraft:embedded.parts";
const OLE_OBJECT_CT: &str = "application/vnd.openxmlformats-officedocument.oleObject";
const REL_OLE_OBJECT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

/// The print presentations (EMF bytes) of the document's embedded objects, by storage id, and
/// the pictures of objects referenced from a field result.
#[derive(Default)]
pub(crate) struct Prints {
    by_id: HashMap<u32, Vec<u8>>,
    /// Storage id → (WMF bytes from `\x03META`, for objects without an EPRINT).
    metas: HashMap<u32, Vec<u8>>,
    /// Storage id → goal size in points from `\x03PIC` (for a PICT placeholder, its box size).
    goals: HashMap<u32, (f32, f32)>,
    /// Storage id → the storage kept whole, for objects within the caps.
    stored: HashMap<u32, Stored>,
}

/// An object storage kept to be written back: a standalone compound file holding every stream
/// and sub-storage of `ObjectPool/_<id>`, its ProgID from `\x01CompObj`, and its `\x03PIC` goal
/// size in twips (unscaled).
pub(crate) struct Stored {
    bin: Arc<Vec<u8>>,
    prog_id: Option<String>,
    goal_twips: Option<(u32, u32)>,
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
        let mut kept = 0u64;
        for id in ids {
            let pic = pic_stream(comp, id);
            let goal = pic.map(|p| p.goal).and_then(pic_goal);
            if let Some(goal) = goal {
                prints.goals.insert(id, goal);
            }
            let room = (media::MAX_MEDIA_BYTES as u64).saturating_sub(kept);
            if let Some(bin) = copy_storage(comp, id, room) {
                kept = kept.saturating_add(bin.len() as u64);
                let goal_twips = pic.map(|p| (p.goal[0], p.goal[1])).filter(|&(x, y)| x > 0 && y > 0 && x <= MAX_GOAL_TWIPS && y <= MAX_GOAL_TWIPS);
                prints.stored.insert(id, Stored { bin: Arc::new(bin), prog_id: comp_obj_prog_id(comp, id), goal_twips });
            }
            let (bytes, is_meta) = match eprint(comp, id) {
                Some(b) => (b, false),
                None => match meta(comp, id) {
                    Some(b) => (b, true),
                    None => {
                        // Only a QuickDraw PICT: Word's framed box, at the extents (else the goal).
                        let size = pic.and_then(pic_extents).or(goal);
                        match size.filter(|_| has_pict(comp, id)) {
                            Some(size) => {
                                prints.goals.insert(id, size);
                                (placeholder(size), true)
                            }
                            None => continue,
                        }
                    }
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
    /// metafile, else a PICT-only object's placeholder at its extents) at its PIC goal size. `None`
    /// without a usable picture or size.
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

/// Largest `\x03PIC` goal size taken as the object's original size: ten times the largest
/// page Word allows (22 in), in twips.
const MAX_GOAL_TWIPS: u32 = 316_800;

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
    repair_emf_header(&mut v);
    (v.len() as u64 <= cap && is_emf(&v)).then_some(v)
}

/// Repairs an EMF's `EMR_HEADER` counts and NUL-terminates full 32-unit font face names
/// ([MS-EMF] 2.3.4.2, 2.3.7.8, 2.2.13). Nothing changes unless the chain through EOF is valid.
pub(crate) fn repair_emf_header(emf: &mut [u8]) {
    let u32_at = |bytes: &[u8], at: usize| bytes.get(at..at.checked_add(4)?).and_then(|s| s.try_into().ok()).map(u32::from_le_bytes);
    let Some(current_handles) = emf.get(56..58).and_then(|s| s.try_into().ok()).map(u16::from_le_bytes) else { return };
    let mut handles = u32::from(current_handles).max(1);
    let mut records = 0u32;
    let mut offset = 0usize;
    let mut face_terminators = Vec::new();
    loop {
        let Some(tail) = emf.get(offset..) else { return };
        let (Some(typ), Some(size)) = (u32_at(tail, 0), u32_at(tail, 4)) else { return };
        if size < 8 || !size.is_multiple_of(4) || (offset == 0 && (typ != EMR_HEADER || size < MIN_HEADER)) {
            return;
        }
        let Ok(size) = usize::try_from(size) else { return };
        let Some(record) = tail.get(..size) else { return };
        if typ == 82
            && let Some(face) = record.get(40..104)
            && !face.as_chunks::<2>().0.contains(&[0, 0])
        {
            // Word requires a NUL within FaceName's 32 UTF-16 units to open the picture.
            let Some(end) = offset.checked_add(104) else { return };
            if face_terminators.try_reserve(1).is_err() {
                return;
            }
            face_terminators.push(end - 2..end);
        }
        if matches!(typ, 37 | 38 | 39 | 40 | 49 | 82 | 93 | 94 | 95 | 99 | 122) {
            let Some(index) = u32_at(record, 8) else { return };
            // SELECTOBJECT and DELETEOBJECT can name stock objects, which need no handle slot.
            if !matches!(typ, 37 | 40) || index & 0x8000_0000 == 0 {
                handles = handles.max(index.saturating_add(1));
            }
        }
        let (Some(next), Some(count)) = (offset.checked_add(size), records.checked_add(1)) else { return };
        offset = next;
        records = count;
        if typ == 14 {
            break;
        }
    }
    let Ok(bytes) = u32::try_from(offset) else { return };
    // Defer all writes until the complete chain through EOF is valid: a partial repair could
    // leave a damaged presentation looking usable to a strict EMF reader.
    if let Some(fields) = emf.get_mut(48..60) {
        let handles = handles.min(u32::from(u16::MAX)) as u16;
        let [lo, hi] = handles.to_le_bytes();
        for (field, value) in fields.as_chunks_mut::<4>().0.iter_mut().zip([bytes.to_le_bytes(), records.to_le_bytes(), [lo, hi, 0, 0]]) {
            field.copy_from_slice(&value);
        }
    }
    for range in face_terminators {
        if let Some(unit) = emf.get_mut(range) {
            unit.fill(0);
        }
    }
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

/// The fields of an object's `\x03PIC` stream that size its picture.
#[derive(Clone, Copy)]
struct Pic {
    /// dxaGoal, dyaGoal (twips) and mx, my (per mille).
    goal: [u32; 4],
    /// The METAFILEPICT mapping mode and xExt, yExt.
    mm: u16,
    ext: [i32; 2],
}

/// The object's `\x03PIC` fields. `None` when the stream is missing or short.
fn pic_stream<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, id: u32) -> Option<Pic> {
    let s = comp.open_stream(format!("/ObjectPool/_{id}/\u{3}PIC")).ok()?;
    let mut v = Vec::new();
    s.take(PIC_MIN_LEN as u64).read_to_end(&mut v).ok()?;
    let bytes4 = |at: usize| v.get(at..at + 4).and_then(|s| s.try_into().ok());
    let u32_at = |at: usize| bytes4(at).map(u32::from_le_bytes);
    let i32_at = |at: usize| bytes4(at).map(i32::from_le_bytes);
    let mm = v.get(PIC_MM..PIC_MM + 2).and_then(|s| s.try_into().ok()).map(u16::from_le_bytes)?;
    Some(Pic {
        goal: [u32_at(PIC_DXA_GOAL)?, u32_at(PIC_DYA_GOAL)?, u32_at(PIC_MX)?, u32_at(PIC_MY)?],
        mm,
        ext: [i32_at(PIC_X_EXT)?, i32_at(PIC_Y_EXT)?],
    })
}

/// The object's METAFILEPICT extents in points: xExt x yExt HIMETRIC when the mapping mode is
/// MM_ISOTROPIC or MM_ANISOTROPIC. mx/my are not applied. `None` when a size is 0, negative or
/// out of range.
fn pic_extents(pic: Pic) -> Option<(f32, f32)> {
    if !matches!(pic.mm, MM_ISOTROPIC | MM_ANISOTROPIC) {
        return None;
    }
    let pt = |v: i32| f64::from(v) * 72.0 / 2540.0;
    let (w, h) = (pt(pic.ext[0]), pt(pic.ext[1]));
    // The same hostile-size cap as the goal size.
    let ok = |x: f64| x > 0.0 && x <= 15_840.0;
    (ok(w) && ok(h)).then_some((w as f32, h as f32))
}

/// Does the object's storage hold a non-empty `\x03PICT` (a QuickDraw picture, not read)?
fn has_pict<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, id: u32) -> bool {
    comp.open_stream(format!("/ObjectPool/_{id}/\u{3}PICT")).is_ok_and(|s| !s.is_empty())
}

/// A PICT-only object's picture, a plain WMF ([MS-WMF]) of the box's aspect: a white box, then a
/// black frame [`PLACEHOLDER_FRAME_PT`] thick inside its edges as four filled bands. The longer
/// side is 32,000 logical units, so `w` x `h` points map onto the window.
fn placeholder((w, h): (f32, f32)) -> Vec<u8> {
    const SIDE: f64 = 32_000.0;
    let (w, h) = (f64::from(w).max(f64::MIN_POSITIVE), f64::from(h).max(f64::MIN_POSITIVE));
    let k = SIDE / w.max(h);
    let units = |v: f64| (v * k).round().clamp(1.0, SIDE) as i16;
    let (x, y) = (units(w), units(h));
    let band = units(PLACEHOLDER_FRAME_PT).min(x / 2).min(y / 2).max(1);
    // Records: (function, parameters as 16-bit words).
    let rect = |l: i16, t: i16, r: i16, b: i16| (0x041Bu16, vec![b, r, t, l]);
    let records: [(u16, Vec<i16>); 11] = [
        (0x0103, vec![MM_ANISOTROPIC as i16]),
        (0x020B, vec![0, 0]),
        (0x020C, vec![y, x]),
        // NULL_PEN, WHITE_BRUSH (stock objects).
        (0x012D, vec![0x8008u16 as i16]),
        (0x012D, vec![0x8000u16 as i16]),
        rect(0, 0, x, y),
        // BLACK_BRUSH.
        (0x012D, vec![0x8004u16 as i16]),
        rect(0, 0, x, band),
        rect(0, y - band, x, y),
        rect(0, band, band, y - band),
        rect(x - band, band, x, y - band),
    ];
    let mut body = Vec::new();
    let mut max_record = 3u32;
    for (func, params) in records.iter().chain([(0u16, Vec::new())].iter()) {
        let words = 3 + params.len() as u32;
        max_record = max_record.max(words);
        body.extend_from_slice(&words.to_le_bytes());
        body.extend_from_slice(&func.to_le_bytes());
        for p in params {
            body.extend_from_slice(&p.to_le_bytes());
        }
    }
    let total_words = (18 + body.len() as u32) / 2;
    let mut v = Vec::with_capacity(18 + body.len());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&9u16.to_le_bytes());
    v.extend_from_slice(&0x0300u16.to_le_bytes());
    v.extend_from_slice(&total_words.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&max_record.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&body);
    v
}

/// The object's goal size in points from its `\x03PIC` fields: dxaGoal x dyaGoal twips scaled
/// by mx/my per mille. `None` when a size is 0 or out of range.
fn pic_goal([dxa, dya, mx, my]: [u32; 4]) -> Option<(f32, f32)> {
    let pt = |goal: u32, scale: u32| f64::from(goal) * f64::from(scale) / 1000.0 / 20.0;
    let (w, h) = (pt(dxa, mx), pt(dya, my));
    // Hostile sizes are refused: at most ten times the largest page Word allows (22 in = 1584 pt).
    let ok = |x: f64| x > 0.0 && x <= 15_840.0;
    (ok(w) && ok(h)).then_some((w as f32, h as f32))
}

/// A Windows Metafile: its META_HEADER has type 1 or 2, headerSize 9, version 0x0100 or 0x0300,
/// and a file size (in 16-bit words) that fits in the bytes there are ([MS-WMF] §2.3.2.2).
pub(crate) fn is_wmf(b: &[u8]) -> bool {
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

/// A copy of storage `ObjectPool/_<id>` as a standalone compound file: every stream (bytes
/// unchanged) and sub-storage at the same path below the root, and the root's CLSID the
/// storage's. `None` when the storage is missing or unreadable, has more than
/// [`MAX_STORAGE_ENTRIES`] entries or a path deeper than [`MAX_STORAGE_DEPTH`], or its streams
/// hold more than `cap` bytes.
fn copy_storage<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, id: u32, cap: u64) -> Option<Vec<u8>> {
    let root = format!("/ObjectPool/_{id}");
    let mut root_clsid = None;
    let mut storages = Vec::new();
    let mut streams = Vec::new();
    let mut size = 0u64;
    for (n, e) in comp.walk_storage(&root).ok()?.enumerate() {
        if n > MAX_STORAGE_ENTRIES {
            return None;
        }
        let rel = e.path().strip_prefix(&root).ok()?.to_path_buf();
        if rel.components().count() > MAX_STORAGE_DEPTH {
            return None;
        }
        if rel.as_os_str().is_empty() {
            root_clsid = Some(*e.clsid());
        } else if e.is_storage() {
            storages.push((rel, *e.clsid()));
        } else {
            size = size.checked_add(e.len()).filter(|&s| s <= cap)?;
            streams.push((rel, e.len()));
        }
    }
    let mut out = cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new())).ok()?;
    out.set_storage_clsid("/", root_clsid?).ok()?;
    for (rel, clsid) in storages {
        let path = Path::new("/").join(rel);
        out.create_storage(&path).ok()?;
        out.set_storage_clsid(&path, clsid).ok()?;
    }
    for (rel, len) in streams {
        let mut bytes = Vec::new();
        comp.open_stream(Path::new(&root).join(&rel)).ok()?.take(len).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 != len {
            return None;
        }
        out.create_stream(Path::new("/").join(rel)).ok()?.write_all(&bytes).ok()?;
    }
    out.flush().ok()?;
    Some(out.into_inner().into_inner())
}

/// The ProgID in the object's `\x01CompObj` stream ([MS-OLEDS] §2.3.8): after the header, the
/// AnsiUserType and the AnsiClipboardFormat, the length-prefixed ANSI string that holds it.
fn comp_obj_prog_id<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, id: u32) -> Option<String> {
    let s = comp.open_stream(format!("/ObjectPool/_{id}/\u{1}CompObj")).ok()?;
    let mut v = Vec::new();
    s.take(MAX_COMP_OBJ).read_to_end(&mut v).ok()?;
    let u32_at = |at: usize| v.get(at..at.checked_add(4)?).and_then(|s| s.try_into().ok()).map(u32::from_le_bytes);
    // AnsiUserType: a LengthPrefixedAnsiString.
    let len = u32_at(COMP_OBJ_HEADER)? as usize;
    let mut at = COMP_OBJ_HEADER.checked_add(4)?.checked_add(len)?;
    // AnsiClipboardFormat: a marker (a standard format id follows) or a string's length.
    at = match u32_at(at)? {
        0 => at.checked_add(4)?,
        0xFFFF_FFFF | 0xFFFF_FFFE => at.checked_add(8)?,
        n => at.checked_add(4)?.checked_add(n as usize)?,
    };
    let len = u32_at(at)? as usize;
    let start = at.checked_add(4)?;
    let raw = v.get(start..start.checked_add(len)?)?;
    let text = raw.split(|&b| b == 0).next()?;
    prog_id(std::str::from_utf8(text).ok()?)
}

/// `s` as a ProgID: 1 to 39 ASCII letters, digits, `.`, `_` or `-`, starting with a letter.
fn prog_id(s: &str) -> Option<String> {
    let ok = s.len() <= MAX_PROG_ID
        && s.starts_with(|c: char| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    ok.then(|| s.to_string())
}

/// Is `instr` an `EMBED` field code?
pub(crate) fn is_embed(instr: &str) -> bool {
    instr.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("EMBED"))
}

/// The class name an `EMBED` field code names (`EMBED Excel.Sheet.8 \s` → `Excel.Sheet.8`).
fn embed_prog_id(instr: &str) -> Option<String> {
    let mut words = instr.split_whitespace();
    words.next().filter(|w| w.eq_ignore_ascii_case("EMBED"))?;
    words.next().filter(|w| !w.starts_with('\\')).and_then(prog_id)
}

/// The content type of a kept picture part, by its file extension.
fn image_content_type(ext: &str) -> &'static str {
    match ext {
        "emf" => "image/x-emf",
        "wmf" => "image/x-wmf",
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        _ => "application/octet-stream",
    }
}

/// A length in points as written in a VML style: at most two decimals, no trailing zeros.
fn pt(v: f32) -> String {
    let s = format!("{:.2}", if v.is_finite() { v.max(0.0) } else { 0.0 });
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The embedded objects kept while reading: one `w:object` markup per storage id, numbered from
/// 1 in the order the reader meets them, and the parts they use.
#[derive(Default)]
pub(crate) struct Kept {
    by_id: HashMap<u32, Arc<Embedded>>,
    /// (part name, bytes, content type), in order.
    parts: Vec<(String, Arc<Vec<u8>>, &'static str)>,
}

impl Kept {
    /// The `w:object` of the object in storage `id`, shown as media entry `media_key` at `w` x `h`
    /// points; `instr` is its `EMBED` field code. `None` when the storage wasn't kept or the
    /// picture's bytes are gone.
    pub(crate) fn object(
        &mut self,
        prints: &Prints,
        id: u32,
        instr: &str,
        media_key: &str,
        media: &BTreeMap<String, Arc<Vec<u8>>>,
        (w, h): (f32, f32),
    ) -> Option<Arc<Embedded>> {
        if let Some(e) = self.by_id.get(&id) {
            return Some(e.clone());
        }
        let stored = prints.stored.get(&id)?;
        let picture = media.get(media_key)?.clone();
        let n = self.by_id.len().saturating_add(1);
        let ext = media_key.rsplit_once('.').map_or("bin", |(_, e)| e);
        let bin_part = format!("word/embeddings/oleObject{n}.bin");
        let image_part = format!("word/media/oleObject{n}.{ext}");
        let prog = embed_prog_id(instr).or_else(|| stored.prog_id.clone()).unwrap_or_else(|| "Package".to_string());
        let twips = |pt: f32| (f64::from(pt) * 20.0).round().clamp(0.0, f64::from(MAX_GOAL_TWIPS)) as u32;
        let (dxa, dya) = stored.goal_twips.unwrap_or((twips(w), twips(h)));
        let shape = format!("_x0000_i{}", n.saturating_add(1024));
        let xml = format!(
            concat!(
                "<w:object xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" ",
                "xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" ",
                "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" w:dxaOrig=\"{dxa}\" w:dyaOrig=\"{dya}\">",
                "<v:shape id=\"{shape}\" type=\"#_x0000_t75\" style=\"width:{w}pt;height:{h}pt\" o:ole=\"\">",
                "<v:imagedata r:id=\"rIdPic\" o:title=\"\"/></v:shape>",
                "<o:OLEObject Type=\"Embed\" ProgID=\"{prog}\" ShapeID=\"{shape}\" DrawAspect=\"Content\" ObjectID=\"_{id}\" r:id=\"rIdOle\"/>",
                "</w:object>"
            ),
            dxa = dxa,
            dya = dya,
            shape = shape,
            w = pt(w),
            h = pt(h),
            prog = prog,
            id = id,
        );
        let refs = vec![
            EmbeddedRef { id: "rIdPic".into(), kind: REL_IMAGE.into(), target: image_part.clone(), external: false },
            EmbeddedRef { id: "rIdOle".into(), kind: REL_OLE_OBJECT.into(), target: bin_part.clone(), external: false },
        ];
        self.parts.push((bin_part, stored.bin.clone(), OLE_OBJECT_CT));
        self.parts.push((image_part, picture, image_content_type(ext)));
        let e = Arc::new(Embedded { xml, refs });
        self.by_id.insert(id, e.clone());
        Some(e)
    }

    /// Put the kept parts, and the manifest naming their content types, into `passthrough`.
    pub(crate) fn finish(self, passthrough: &mut BTreeMap<String, Arc<Vec<u8>>>) {
        if self.parts.is_empty() {
            return;
        }
        let mut manifest = String::new();
        for (name, bytes, ct) in self.parts {
            manifest.push_str(&format!("P\t{name}\t{ct}\n"));
            passthrough.insert(name, bytes);
        }
        passthrough.insert(EMBEDDED_PARTS.into(), Arc::new(manifest.into_bytes()));
    }
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
    fn placeholders_of_any_size_are_wmf() {
        for size in [(429.0, 106.0), (0.01, 15_840.0), (15_840.0, 15_840.0), (0.0, 0.0), (f32::NAN, 1.0)] {
            let wmf = placeholder(size);
            assert!(is_wmf(&wmf), "{size:?}");
            assert!(wordcraft_metafile::is_metafile(&wmf), "{size:?}");
        }
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

    /// A compound file holding storage `ObjectPool/_5` with sub-storages `nest` levels deep below
    /// it (`s0/s1/…`), a stream of `len` bytes at the bottom and a `\x01Ole` stream; the object
    /// storage's CLSID is all 0x11 and the first sub-storage's all 0x22.
    fn pool(nest: usize, len: usize) -> cfb::CompoundFile<Cursor<Vec<u8>>> {
        let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        comp.create_storage_all("/ObjectPool/_5").unwrap();
        let mut path = String::from("/ObjectPool/_5");
        for i in 0..nest {
            path.push_str(&format!("/s{i}"));
            comp.create_storage(&path).unwrap();
        }
        comp.create_stream(format!("{path}/Data")).unwrap().write_all(&vec![7u8; len]).unwrap();
        comp.create_stream("/ObjectPool/_5/\u{1}Ole").unwrap().write_all(&[1, 2, 3]).unwrap();
        comp.flush().unwrap();
        // CLSIDs are poked into the directory entries (128 bytes each, name first, CLSID at 0x50).
        let mut bytes = comp.into_inner().into_inner();
        for (name, fill) in [("_5", 0x11u8), ("s0", 0x22)] {
            let mut entry: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            entry.extend_from_slice(&[0, 0]);
            let at = (0..bytes.len().saturating_sub(128)).step_by(128).find(|&i| bytes[i..].starts_with(&entry));
            if let Some(at) = at {
                bytes[at + 0x50..at + 0x60].fill(fill);
            }
        }
        cfb::CompoundFile::open(Cursor::new(bytes)).unwrap()
    }

    fn read_stream(comp: &mut cfb::CompoundFile<Cursor<Vec<u8>>>, path: &str) -> Vec<u8> {
        let mut v = Vec::new();
        comp.open_stream(path).unwrap().read_to_end(&mut v).unwrap();
        v
    }

    #[test]
    fn a_storage_is_copied_whole_with_its_clsids() {
        let mut src = pool(2, 5000);
        let bin = copy_storage(&mut src, 5, 1 << 20).expect("copied");
        let mut out = cfb::CompoundFile::open(Cursor::new(bin)).unwrap();
        assert_eq!(read_stream(&mut out, "/s0/s1/Data"), vec![7u8; 5000]);
        assert_eq!(read_stream(&mut out, "/\u{1}Ole"), vec![1, 2, 3]);
        assert_eq!(out.root_entry().clsid().as_bytes(), &[0x11; 16], "the root CLSID is the storage's");
        assert_eq!(out.entry("/s0").unwrap().clsid().as_bytes(), &[0x22; 16]);
        assert!(!out.exists("/ObjectPool"), "nothing but the object's own tree");
    }

    #[test]
    fn a_storage_past_the_caps_is_not_copied() {
        // Bigger than the byte cap (the media cap in a real read).
        let mut src = pool(0, 5000);
        assert!(copy_storage(&mut src, 5, 5002).is_none());
        assert!(copy_storage(&mut src, 5, 5003).is_some());
        // Deeper than the depth cap.
        let mut src = pool(MAX_STORAGE_DEPTH, 1);
        assert!(copy_storage(&mut src, 5, 1 << 20).is_none());
        let mut src = pool(MAX_STORAGE_DEPTH - 1, 1);
        assert!(copy_storage(&mut src, 5, 1 << 20).is_some());
        // A missing storage.
        assert!(copy_storage(&mut src, 6, 1 << 20).is_none());
    }

    #[test]
    fn prog_ids_are_checked_names() {
        assert_eq!(embed_prog_id(" EMBED Excel.Sheet.8 \\s "), Some("Excel.Sheet.8".into()));
        assert_eq!(embed_prog_id("embed Equation.3"), Some("Equation.3".into()));
        assert_eq!(embed_prog_id("EMBED \\s"), None);
        assert_eq!(embed_prog_id("EMBED"), None);
        assert_eq!(embed_prog_id("LINK Excel.Sheet.8"), None);
        assert_eq!(embed_prog_id("EMBED a\"b"), None);
        assert_eq!(embed_prog_id(&format!("EMBED {}", "A".repeat(40))), None);
        assert_eq!(pt(144.0), "144");
        assert_eq!(pt(12.345), "12.35");
        assert_eq!(pt(f32::NAN), "0");
    }
}
