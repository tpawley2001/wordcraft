//! Inline pictures: `sprmCPicLocation` → a `PICF` in the Data stream → embedded image bytes
//! ([MS-DOC] §2.9.190 PICF, §1.3.5 Pictures), and the shared store of every picture the
//! document reads (inline and floating).

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::Arc;

use crate::officeart;
use wordcraft_doc::connector::HorizontalRule;
use wordcraft_doc::props::{Align, Rgb};

/// `PICF.cbHeader` — the fixed size of the PICF structure.
const CB_HEADER: usize = 0x44;
/// Offsets inside PICF: `PICMID.dxaGoal/dyaGoal/mx/my` start at 28.
const OFF_PICMID: usize = 28;

/// Most distinct pictures one document materialises from the Data stream.
const MAX_PICTURES: usize = 5_000;
/// Most picture bytes one document copies out of the Data stream, all pictures together.
pub(crate) const MAX_MEDIA_BYTES: usize = 256 << 20;
/// Most bytes one document decodes pictures into (metafiles inflated, DIB pixels), charged
/// per attempt whether it succeeds or not: retained media is capped separately, by
/// `MAX_MEDIA_BYTES` (#3711, #3712).
const MAX_DECODE_WORK: usize = 512 << 20;
/// The decode budget grows with the input: up to this many bytes per byte of the WordDocument
/// and Data streams. An uncompressed DIB declares at most 32 bytes of pixels per file byte (1
/// bit per pixel, 4 bytes charged each), so 64 passes any such picture. RLE8/RLE4 bitmaps
/// reach about 510 times that and rely on the `MAX_DECODE_WORK` floor instead; a decompression
/// bomb (ratios around 1e6) still hits the cap.
const DECODE_WORK_PER_INPUT_BYTE: usize = 64;
/// The most the decode budget grows to, whatever the input.
const MAX_DECODE_WORK_CEILING: usize = 2 << 30;

/// The decode budget for a document whose pictures live in `input` stream bytes, never less
/// than `floor`.
pub(crate) fn decode_work(input: usize, floor: usize) -> usize {
    input.saturating_mul(DECODE_WORK_PER_INPUT_BYTE).clamp(floor, floor.max(MAX_DECODE_WORK_CEILING))
}

/// The document's pictures, keyed by their location in the Data stream: anchors that share a
/// `fc_pic` share one media entry (or one failure), and the total bytes copied are capped, so a
/// file with thousands of anchors on one huge picture costs one copy, not thousands. Pictures of
/// the drawing layer's BLIP store are shared the same way, by store index.
pub(crate) struct Pictures {
    /// fc_pic → the inline picture already materialised (`None`: unreadable or over the cap,
    /// not retried).
    by_fc: HashMap<u32, Option<Inline>>,
    /// BLIP store index → media key (`None`: unreadable, not retried).
    by_bse: HashMap<u32, Option<String>>,
    /// (stream, foDelay, record length) of a BLIP record outside the store → media key: store
    /// entries and inline pictures that alias one record share one decode (`None`: failed, not
    /// retried). The stream is told apart by its address.
    by_delay: HashMap<(usize, usize, usize), Option<String>>,
    /// Embedded object storage id → media key of its print presentation (`None`: over a cap,
    /// not retried).
    by_ole: HashMap<u32, Option<String>>,
    /// The media entries, merged into the document at the end.
    pub(crate) media: BTreeMap<String, Arc<Vec<u8>>>,
    /// Pictures linked to a file that isn't in the document: media key → the file's path
    /// (`Document::linked_media`).
    pub(crate) links: BTreeMap<String, String>,
    total: usize,
    /// The byte cap (`MAX_MEDIA_BYTES`; smaller in tests).
    pub(crate) limit: usize,
    /// Decode work left (`MAX_DECODE_WORK`; smaller in tests).
    pub(crate) work: officeart::Decode,
    /// Anchors whose picture was dropped (only the first few are logged).
    dropped: usize,
    /// Pictures kept as placeholders (formats we can't draw).
    placeholders: usize,
}

impl Default for Pictures {
    fn default() -> Self {
        Pictures {
            by_fc: HashMap::new(),
            by_bse: HashMap::new(),
            by_delay: HashMap::new(),
            by_ole: HashMap::new(),
            media: BTreeMap::new(),
            links: BTreeMap::new(),
            total: 0,
            limit: MAX_MEDIA_BYTES,
            work: officeart::Decode::new(MAX_DECODE_WORK),
            dropped: 0,
            placeholders: 0,
        }
    }
}

/// An inline picture: its media key, display size (points), crop fractions and alt text.
/// `empty` keeps an invisible shape’s space; `rule` carries a horizontal line’s fill and metadata.
#[derive(Clone, Debug)]
pub(crate) struct Inline {
    pub(crate) key: String,
    pub(crate) empty: bool,
    pub(crate) rule: Option<(Rgb, HorizontalRule)>,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) crop: [f32; 4],
    pub(crate) alt: String,
}

impl Pictures {
    /// An empty store whose decode budget scales with the `input` stream bytes (WordDocument
    /// plus Data) the pictures can come from.
    pub(crate) fn for_input(input: usize) -> Self {
        Pictures { work: officeart::Decode::new(decode_work(input, MAX_DECODE_WORK)), ..Pictures::default() }
    }

    /// The inline picture at `fc_pic` in the Data stream, copying its bytes the first time.
    /// `None` when the header is malformed, the picture can't be read, or a picture
    /// count/byte cap is reached.
    pub(crate) fn get(&mut self, data: &[u8], fc_pic: u32) -> Option<&Inline> {
        if !self.by_fc.contains_key(&fc_pic) {
            if self.by_fc.len().saturating_add(self.by_bse.len()) >= MAX_PICTURES {
                return None;
            }
            let pic = self.read(data, fc_pic);
            self.by_fc.insert(fc_pic, pic);
        }
        self.by_fc.get(&fc_pic)?.as_ref()
    }

    /// Read the inline picture at `fc_pic`: nothing is decoded or copied beyond what is left of
    /// the byte cap, and nothing at all once it is used up.
    fn read(&mut self, data: &[u8], fc_pic: u32) -> Option<Inline> {
        let room = self.limit.saturating_sub(self.total);
        if room == 0 {
            return None;
        }
        match locate(data, fc_pic, self)? {
            Found::Range(pic) => {
                let bytes = data.get(pic.range.clone()).filter(|b| b.len() <= room)?.to_vec();
                let key = self.insert(bytes, pic.ext, false)?;
                Some(Inline { key, empty: false, rule: None, w: pic.w, h: pic.h, crop: [0.0; 4], alt: String::new() })
            }
            Found::Shape { w, h, key, props } => Some(Inline { key: key?, empty: false, rule: None, w, h, crop: props.crop, alt: props.alt }),
            Found::Empty { w, h, props } => Some(Inline { key: String::new(), empty: true, rule: None, w, h, crop: [0.0; 4], alt: props.alt }),
            Found::Rule { w, h, props } => Some(Inline {
                key: String::new(),
                empty: false,
                w,
                h,
                crop: [0.0; 4],
                alt: props.alt,
                rule: Some((
                    props.fill_color.unwrap_or(Rgb(0xAC, 0xA8, 0x99)),
                    HorizontalRule {
                        pct: (props.pct_hr.unwrap_or(1000) as f32 / 10.0).clamp(0.0, 100.0),
                        align: match props.align_hr {
                            Some(1) => Align::Center,
                            Some(2) => Align::Right,
                            _ => Align::Left,
                        },
                        standard: props.standard_hr,
                    },
                )),
            }),
            Found::Linked { w, h, path, props } => {
                let key = format!("link{}", self.links.len() + 1);
                self.links.insert(key.clone(), path);
                Some(Inline { key, empty: false, rule: None, w, h, crop: props.crop, alt: props.alt })
            }
        }
    }

    /// The picture of embedded object `id`, drawn from its print presentation `emf` at the
    /// Data stream picture's size and crop `shown`; one media entry per object. `None` when a
    /// picture count/byte cap is reached.
    pub(crate) fn ole(&mut self, id: u32, emf: &[u8], shown: &Inline) -> Option<Inline> {
        let key = self.ole_key(id, emf, "emf")?;
        Some(Inline { key, empty: false, ..shown.clone() })
    }

    /// The picture of embedded object `id` referenced from a field result, drawn from its own
    /// presentation `bytes` (`ext`: emf or wmf) at `w` x `h` points; one media entry per object.
    pub(crate) fn ole_object(&mut self, id: u32, bytes: &[u8], ext: &str, w: f32, h: f32) -> Option<Inline> {
        let key = self.ole_key(id, bytes, ext)?;
        Some(Inline { key, empty: false, rule: None, w, h, crop: [0.0; 4], alt: String::new() })
    }

    /// The media key of object `id`'s presentation, adding `bytes` the first time.
    fn ole_key(&mut self, id: u32, bytes: &[u8], ext: &str) -> Option<String> {
        if !self.by_ole.contains_key(&id) {
            if self.by_fc.len().saturating_add(self.by_bse.len()).saturating_add(self.by_ole.len()) >= MAX_PICTURES {
                return None;
            }
            let key = self.insert(bytes.to_vec(), ext, false);
            self.by_ole.insert(id, key);
        }
        self.by_ole.get(&id)?.clone()
    }

    /// The media key of BLIP store entry `pib` (1-based), decoding it the first time; `delay`
    /// is the stream that holds BLIPs not embedded in the store (the WordDocument stream).
    pub(crate) fn bse(&mut self, pib: u32, bstore: &[&[u8]], delay: &[u8]) -> Option<String> {
        if let Some(hit) = self.by_bse.get(&pib) {
            return hit.clone();
        }
        if self.by_fc.len().saturating_add(self.by_bse.len()) >= MAX_PICTURES {
            return None;
        }
        let fbse = bstore.get((pib as usize).checked_sub(1)?)?;
        let key = self.decode_bse(fbse, delay);
        self.by_bse.insert(pib, key.clone());
        key
    }

    /// Decode a store entry's BLIP, once per delay-stream record however many entries name it.
    fn decode_bse(&mut self, fbse: &[u8], delay: &[u8]) -> Option<String> {
        let (rec, at) = officeart::fbse_record(fbse, delay)?;
        let at = at.map(|(fo, used)| (delay.as_ptr() as usize, fo, used));
        if let Some(hit) = at.and_then(|at| self.by_delay.get(&at)) {
            return hit.clone();
        }
        let room = self.limit.saturating_sub(self.total);
        let key = officeart::blip(&rec, room, &mut self.work).and_then(|b| self.insert(b.bytes, b.ext, b.placeholder));
        if let Some(at) = at {
            self.by_delay.insert(at, key.clone());
        }
        key
    }

    /// Add picture bytes as a new media entry, within the byte cap.
    fn insert(&mut self, bytes: Vec<u8>, ext: &str, placeholder: bool) -> Option<String> {
        let total = self.total.checked_add(bytes.len()).filter(|&t| t <= self.limit)?;
        let key = format!("image{}.{ext}", self.media.len() + 1);
        if placeholder {
            if self.placeholders < 10 {
                log::warn!("docbin: picture `{key}` is in a format WordCraft can't draw ({ext}); it shows as a placeholder");
            }
            self.placeholders = self.placeholders.saturating_add(1);
        }
        self.media.insert(key.clone(), Arc::new(bytes));
        self.total = total;
        Some(key)
    }

    /// Note a picture anchor that could not be materialised.
    pub(crate) fn dropped(&mut self, cp: u32) {
        if self.dropped < 10 {
            log::warn!("docbin: picture at CP {cp} could not be read (or a picture cap was reached) and was dropped");
        }
        self.dropped = self.dropped.saturating_add(1);
    }
}

/// Where a picture's image bytes sit in the Data stream, and its display size in points.
struct Located {
    w: f32,
    h: f32,
    range: Range<usize>,
    ext: &'static str,
}

/// What a `PICFAndOfficeArtData` holds: raw image bytes (older writers), or an
/// `OfficeArtInlineSpContainer` (Word 97 and later) whose BLIP is decoded.
enum Found {
    Range(Located),
    /// A horizontal line using the existing rule shape model, without image bytes.
    Rule {
        w: f32,
        h: f32,
        props: officeart::Props,
    },
    Shape {
        w: f32,
        h: f32,
        key: Option<String>,
        props: officeart::Props,
    },
    /// A picture linked to a file, with no BLIP of its own (an `INCLUDEPICTURE \d` result):
    /// Word keeps its place and size and draws a frame there.
    Linked {
        w: f32,
        h: f32,
        path: String,
        props: officeart::Props,
    },
    /// A shape with no picture, not linked and not a horizontal line (some writers put one
    /// after a text box anchored as a character): Word lays it out as an invisible object of
    /// its size, so its line keeps the room. Only an absent picture (no `OfficeArtFBSE` in
    /// the container) is empty; an FBSE whose BLIP can't be read is a failed decode and stays
    /// a dropped picture with its warning.
    Empty {
        w: f32,
        h: f32,
        props: officeart::Props,
    },
}

/// `PICF.mfpf.mm` values of pictures stored as a shape (`MM_SHAPE`) or a shape with a file
/// name (`MM_SHAPEFILE`).
const MM_SHAPE: u16 = 0x0064;
const MM_SHAPEFILE: u16 = 0x0066;

/// Locate the picture at `fc_pic` in the Data stream ([MS-DOC] §2.9.192 PICFAndOfficeArtData);
/// the BLIP is decoded once per record and charged to the decode budget.
fn locate(data: &[u8], fc_pic: u32, pics: &mut Pictures) -> Option<Found> {
    let base = fc_pic as usize;
    let pic = data.get(base..)?;
    let lcb = u32::from_le_bytes(pic.get(0..4)?.try_into().ok()?) as usize;
    let cb_header = u16::from_le_bytes(pic.get(4..6)?.try_into().ok()?) as usize;
    let cb_header = if (0x20..=0x100).contains(&cb_header) { cb_header } else { CB_HEADER };
    let mm = u16::from_le_bytes(pic.get(6..8)?.try_into().ok()?);
    let m = pic.get(OFF_PICMID..OFF_PICMID + 8)?;
    let dxa_goal = i16::from_le_bytes([m[0], m[1]]) as f32;
    let dya_goal = i16::from_le_bytes([m[2], m[3]]) as f32;
    let mx = u16::from_le_bytes([m[4], m[5]]) as f32;
    let my = u16::from_le_bytes([m[6], m[7]]) as f32;
    let scale = |r: f32| if r > 0.0 { (r / 1000.0).clamp(0.01, 100.0) } else { 1.0 };
    let end = lcb.min(pic.len());
    if mm == MM_SHAPE || mm == MM_SHAPEFILE {
        let mut at = cb_header;
        let mut file = String::new();
        if mm == MM_SHAPEFILE {
            let n = *pic.get(at)? as usize;
            let name = pic.get(at + 1..).unwrap_or_default();
            file = name.iter().take(n).map(|&b| wordcraft_doc::encoding::cp1252(b)).collect();
            at = at.checked_add(1 + n)?;
        }
        let (blip, props) = inline_shape(pic.get(at..end)?, data, pics)?;
        // The goal size is before cropping; the crop fractions take their share off it.
        let [cl, ct, cr, cb] = props.crop;
        let w = (dxa_goal / 20.0 * (1.0 - cl - cr).max(0.01) * scale(mx)).clamp(1.0, 1584.0);
        let h = (dya_goal / 20.0 * (1.0 - ct - cb).max(0.01) * scale(my)).clamp(1.0, 1584.0);
        // A picture horizontal line (one with a BLIP) stays a picture.
        if props.horiz_rule && blip.is_none() {
            let w = props.width_hr.map_or(dxa_goal * scale(mx), |v| v as f32) / 20.0;
            let h = props.height_hr.map_or(dya_goal * scale(my), |v| v as f32) / 20.0;
            return Some(Found::Rule { w: w.clamp(0.0, 1584.0), h: h.clamp(0.0, 1584.0), props });
        }
        return match blip {
            Some(key) => Some(Found::Shape { w, h, key, props }),
            // `Some(None)` (an FBSE whose BLIP is unreadable or over the decode budget) took
            // the arm above: a failed decode, dropped with its warning, never an empty shape.
            // `None` is an absent picture (no FBSE in the container): a picture linked to a
            // file ([MS-ODRAW] §2.3.23.3 `pibFlags` LinkToFile, or a file name) is kept at its
            // size; so is a shape with no picture at all, drawn as nothing.
            None => {
                let linked = props.pib_flags.is_some_and(|f| f & 0x8 != 0) || !props.pib_name.is_empty() || mm == MM_SHAPEFILE;
                if !linked {
                    return Some(Found::Empty { w, h, props });
                }
                let path = if props.pib_name.is_empty() { file } else { props.pib_name.clone() };
                Some(Found::Linked { w, h, path, props })
            }
        };
    }
    let w = (dxa_goal / 20.0 * scale(mx)).clamp(1.0, 1584.0);
    let h = (dya_goal / 20.0 * scale(my)).clamp(1.0, 1584.0);
    let body = pic.get(cb_header..end)?;
    let at = sniff(body)?;
    let image = body.get(at..)?;
    let start = base.checked_add(cb_header)?.checked_add(at)?;
    Some(Found::Range(Located { w, h, range: start..start.checked_add(image.len())?, ext: ext_of(image) }))
}

/// An `OfficeArtInlineSpContainer`: the picture shape, then the `OfficeArtFBSE` records its
/// `pib` indexes (BLIPs embedded, or at `foDelay` in the Data stream). The BLIP is decoded
/// once per record, however many pictures (and FBSE sizes) name it. The key is `None` when the
/// shape has no FBSE (no BLIP to show), `Some(None)` when its BLIP can't be read.
fn inline_shape(buf: &[u8], data: &[u8], pics: &mut Pictures) -> Option<(Option<Option<String>>, officeart::Props)> {
    let mut budget = 10_000usize;
    let recs = officeart::children(buf, &mut budget);
    let sp = recs.iter().find(|r| r.typ == officeart::rt::SP_CONTAINER).map(|r| officeart::shape(r.body, &mut budget))?;
    let fbses: Vec<&[u8]> = recs.iter().filter(|r| r.typ == officeart::rt::FBSE).map(|r| r.body).collect();
    let Some(fbse) = sp.props.pib.and_then(|p| fbses.get((p as usize).checked_sub(1)?)).or(fbses.first()) else {
        return Some((None, sp.props));
    };
    let key = pics.decode_bse(fbse, data);
    Some((Some(key), sp.props))
}

/// Offset of the first embedded raster image inside the payload, skipping the small
/// metafile header some writers put in front of PNG/JPEG data. `None` for metafiles.
fn sniff(body: &[u8]) -> Option<usize> {
    for at in [0usize, 8, 12, 16, 20, 22, 24, 32] {
        if let Some(b) = body.get(at..)
            && (b.starts_with(b"\x89PNG") || b.starts_with(b"\xFF\xD8\xFF") || b.starts_with(b"GIF8") || b.starts_with(b"BM"))
        {
            return Some(at);
        }
    }
    None
}

fn ext_of(b: &[u8]) -> &'static str {
    if b.starts_with(b"\x89PNG") {
        "png"
    } else if b.starts_with(b"\xFF\xD8\xFF") {
        "jpeg"
    } else if b.starts_with(b"GIF8") {
        "gif"
    } else {
        "bmp"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(props: &[(u16, u32)]) -> Inline {
        let mut table = Vec::new();
        for (id, value) in props {
            table.extend_from_slice(&id.to_le_bytes());
            table.extend_from_slice(&value.to_le_bytes());
        }
        let mut shape = Vec::new();
        shape.extend_from_slice(&((props.len() as u16) << 4 | 3).to_le_bytes());
        shape.extend_from_slice(&officeart::rt::TERTIARY_FOPT.to_le_bytes());
        shape.extend_from_slice(&(table.len() as u32).to_le_bytes());
        shape.extend(table);
        let mut data = vec![0u8; CB_HEADER];
        data[4..6].copy_from_slice(&(CB_HEADER as u16).to_le_bytes());
        data[6..8].copy_from_slice(&MM_SHAPE.to_le_bytes());
        data[28..30].copy_from_slice(&1000i16.to_le_bytes());
        data[30..32].copy_from_slice(&500i16.to_le_bytes());
        data[32..34].copy_from_slice(&2000u16.to_le_bytes());
        data[34..36].copy_from_slice(&60u16.to_le_bytes());
        data.extend_from_slice(&15u16.to_le_bytes());
        data.extend_from_slice(&officeart::rt::SP_CONTAINER.to_le_bytes());
        data.extend_from_slice(&(shape.len() as u32).to_le_bytes());
        data.extend(shape);
        let len = data.len() as u32;
        data[0..4].copy_from_slice(&len.to_le_bytes());
        Pictures::default().get(&data, 0).unwrap().clone()
    }

    #[test]
    fn rule_defaults_use_scaled_goal_size_and_grey_fill() {
        let p = rule(&[(0x03BF, (1 << 27) | (1 << 11) | (1 << 13))]);
        let (fill, hr) = p.rule.unwrap();
        assert_eq!(fill, Rgb(0xAC, 0xA8, 0x99));
        assert_eq!(hr.pct, 100.0);
        assert_eq!(hr.align, Align::Left);
        assert!(!hr.standard, "standard flag requires its use bit");
        assert_eq!((p.w, p.h), (100.0, 1.5));
    }

    #[test]
    fn rule_percent_is_bounded_and_flags_require_use_bits() {
        for (value, pct) in [(2000, 100.0), (u32::MAX, 0.0)] {
            let p = rule(&[(0x0393, value), (0x0394, 99), (0x03BF, (1 << 27) | (1 << 11))]);
            let (_, hr) = p.rule.unwrap();
            assert_eq!(hr.pct, pct);
            assert_eq!(hr.align, Align::Left);
        }
        assert!(rule(&[(0x03BF, 1 << 11)]).rule.is_none());
    }
}
