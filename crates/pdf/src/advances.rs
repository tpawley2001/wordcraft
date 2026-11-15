//! A copy of a font with other horizontal advances for some glyphs, for text in a missing font
//! laid out with Word's widths for it: the stand-in's glyphs keep their outlines but declare the
//! widths the layout spaced them by, so PDF readers that rebuild words from glyph positions see
//! no gaps or overlaps inside words.
//!
//! The copy is a single-face OpenType file (a collection's face is taken out of it) whose `hmtx`
//! has one advance per glyph (`hhea.numberOfHMetrics` = `maxp.numGlyphs`). Device metrics
//! (`hdmx`, `LTSH`), metric variations (`HVAR`) and a signature (`DSIG`) would contradict the new
//! advances and are left out. Table layout follows the OpenType font file specification.

use std::collections::HashMap;

/// Most tables a font file may list; real fonts have a few dozen.
const MAX_TABLES: usize = 512;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let b = d.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([*b.first()?, *b.get(1)?]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let b = d.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?]))
}

/// The OpenType table checksum: the sum of the table's big-endian 32-bit words, zero padded.
fn checksum(t: &[u8]) -> u32 {
    t.chunks(4).fold(0u32, |sum, c| {
        let mut w = [0u8; 4];
        for (d, s) in w.iter_mut().zip(c) {
            *d = *s;
        }
        sum.wrapping_add(u32::from_be_bytes(w))
    })
}

/// The sfnt version and tables (tag, bytes) of `data`'s face `index`.
fn tables(data: &[u8], index: u32) -> Option<(u32, Vec<([u8; 4], &[u8])>)> {
    // A collection lists its faces' table directories; a plain font is one.
    let dir = if data.get(0..4)? == b"ttcf" {
        let n = u32_at(data, 8)?;
        if index >= n {
            return None;
        }
        usize::try_from(u32_at(data, 12usize.checked_add(usize::try_from(index).ok()?.checked_mul(4)?)?)?).ok()?
    } else {
        0
    };
    let version = u32_at(data, dir)?;
    let count = usize::from(u16_at(data, dir.checked_add(4)?)?);
    if count > MAX_TABLES {
        return None;
    }
    let mut tables: Vec<([u8; 4], &[u8])> = Vec::with_capacity(count);
    for i in 0..count {
        let rec = dir.checked_add(12)?.checked_add(i.checked_mul(16)?)?;
        let tag: [u8; 4] = data.get(rec..rec.checked_add(4)?)?.try_into().ok()?;
        let off = usize::try_from(u32_at(data, rec.checked_add(8)?)?).ok()?;
        let len = usize::try_from(u32_at(data, rec.checked_add(12)?)?).ok()?;
        tables.push((tag, data.get(off..off.checked_add(len)?)?));
    }
    Some((version, tables))
}

/// `data`'s face `index` with the advances in `advances` (glyph id to font units) and its own
/// for every other glyph. `None` when the font can't be read as an OpenType file.
pub(crate) fn with_advances(data: &[u8], index: u32, advances: &HashMap<u16, u16>) -> Option<Vec<u8>> {
    let (version, tables) = tables(data, index)?;
    let table = |tag: &[u8; 4]| tables.iter().find(|(t, _)| t == tag).map(|(_, d)| *d);
    let glyphs = usize::from(u16_at(table(b"maxp")?, 4)?);
    let hhea = table(b"hhea")?;
    let hmtx = table(b"hmtx")?;
    let long = usize::from(u16_at(hhea, 34)?);
    if long == 0 || glyphs == 0 {
        return None;
    }
    // One advance and side bearing per glyph; glyphs past the long metrics share the last advance.
    let mut new_hmtx = Vec::with_capacity(glyphs.checked_mul(4)?);
    let mut max_advance = 0u16;
    let last_advance = u16_at(hmtx, (long - 1).checked_mul(4)?)?;
    for g in 0..glyphs {
        let (own, lsb) = if g < long {
            (u16_at(hmtx, g * 4)?, u16_at(hmtx, g * 4 + 2).unwrap_or(0))
        } else {
            (last_advance, u16_at(hmtx, long * 4 + (g - long) * 2).unwrap_or(0))
        };
        let advance = u16::try_from(g).ok().and_then(|g| advances.get(&g).copied()).unwrap_or(own);
        max_advance = max_advance.max(advance);
        new_hmtx.extend_from_slice(&advance.to_be_bytes());
        new_hmtx.extend_from_slice(&lsb.to_be_bytes());
    }
    let mut new_hhea = hhea.to_vec();
    new_hhea.get_mut(10..12)?.copy_from_slice(&max_advance.to_be_bytes());
    new_hhea.get_mut(34..36)?.copy_from_slice(&u16::try_from(glyphs).ok()?.to_be_bytes());

    let mut out_tables: Vec<([u8; 4], std::borrow::Cow<'_, [u8]>)> = tables
        .iter()
        .filter(|(tag, _)| !matches!(tag, b"HVAR" | b"hdmx" | b"LTSH" | b"DSIG"))
        .map(|(tag, d)| {
            let d = match tag {
                b"hmtx" => std::borrow::Cow::Owned(std::mem::take(&mut new_hmtx)),
                b"hhea" => std::borrow::Cow::Owned(std::mem::take(&mut new_hhea)),
                b"head" => {
                    // The whole-file checksum adjustment is worked out once the file is written.
                    let mut h = d.to_vec();
                    if let Some(adj) = h.get_mut(8..12) {
                        adj.copy_from_slice(&[0; 4]);
                    }
                    std::borrow::Cow::Owned(h)
                }
                _ => std::borrow::Cow::Borrowed(*d),
            };
            (*tag, d)
        })
        .collect();
    out_tables.sort_by_key(|(tag, _)| *tag);
    let n = out_tables.len();
    let n16 = u16::try_from(n).ok()?;
    // searchRange etc.: the largest power of two not over the table count.
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow.checked_mul(2).is_some_and(|p| p <= n16) {
        pow *= 2;
        log += 1;
    }
    let total = out_tables.iter().try_fold(12 + 16 * n, |sum: usize, (_, d)| sum.checked_add(d.len().checked_add(3)? & !3))?;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&n16.to_be_bytes());
    out.extend_from_slice(&(pow.saturating_mul(16)).to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&(n16.saturating_mul(16).saturating_sub(pow.saturating_mul(16))).to_be_bytes());
    let mut offset = 12 + 16 * n;
    let mut head_at = None;
    for (tag, d) in &out_tables {
        if tag == b"head" {
            head_at = Some(offset);
        }
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(d).to_be_bytes());
        out.extend_from_slice(&u32::try_from(offset).ok()?.to_be_bytes());
        out.extend_from_slice(&u32::try_from(d.len()).ok()?.to_be_bytes());
        offset = offset.checked_add(d.len().checked_add(3)? & !3)?;
    }
    for (_, d) in &out_tables {
        out.extend_from_slice(d);
        out.resize(out.len().checked_add(3)? & !3, 0);
    }
    if let Some(at) = head_at.and_then(|h| h.checked_add(8)) {
        let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out.get_mut(at..at.checked_add(4)?)?.copy_from_slice(&adj.to_be_bytes());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hmtx_advance(font: &[u8], g: u16) -> u16 {
        let (_, t) = tables(font, 0).unwrap();
        let get = |tag: &[u8; 4]| t.iter().find(|(x, _)| x == tag).unwrap().1;
        let long = u16_at(get(b"hhea"), 34).unwrap();
        assert!(g < long, "every glyph has its own advance");
        u16_at(get(b"hmtx"), usize::from(g) * 4).unwrap()
    }

    #[test]
    fn the_copy_has_the_new_advances_and_keeps_the_others() {
        let face = wordcraft_fonts::FontDb::global().face("Source Sans 3", "Regular");
        let (a, b) = (u16::try_from(face.glyph_for('T')).unwrap(), u16::try_from(face.glyph_for('O')).unwrap());
        let font = with_advances(face.data(), face.index(), &HashMap::from([(a, 123)])).unwrap();
        assert_eq!(hmtx_advance(&font, a), 123);
        assert_eq!(f64::from(hmtx_advance(&font, b)), face.advance(u32::from(b)));
        assert_eq!(checksum(&font), 0xB1B0_AFBA, "the head checksum adjustment balances the file");
    }

    #[test]
    fn junk_is_not_a_font() {
        let m = HashMap::new();
        assert!(with_advances(b"", 0, &m).is_none());
        assert!(with_advances(b"ttcf\0\0\0\0\0\0\0\x05", 9, &m).is_none());
        assert!(with_advances(&[0, 1, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0], 0, &m).is_none());
        let mut cut = wordcraft_fonts::FontDb::global().face("Source Sans 3", "Regular").data().to_vec();
        cut.truncate(300);
        assert!(with_advances(&cut, 0, &m).is_none());
    }
}
