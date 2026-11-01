//! Narrow compatibility repairs for compound files ([MS-CFB] §2.3 and §2.6.1).

use std::borrow::Cow;

const FREE: u32 = 0xFFFF_FFFF;
const END: u32 = 0xFFFF_FFFE;
const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

struct Header {
    sector_len: usize,
    sectors: usize,
}

impl Header {
    fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 512 || bytes.get(..8)? != MAGIC || u16_at(bytes, 0x1C)? != 0xFFFE || u16_at(bytes, 0x20)? != 6 {
            return None;
        }
        let sector_len = match (u16_at(bytes, 0x1A)?, u16_at(bytes, 0x1E)?) {
            (3, 9) => 512,
            (4, 12) => 4096,
            _ => return None,
        };
        if bytes.len() < sector_len {
            return None;
        }
        Some(Self { sector_len, sectors: bytes.len().div_ceil(sector_len).checked_sub(1)? })
    }

    fn sector_at(&self, sector: u32) -> Option<usize> {
        let sector = usize::try_from(sector).ok()?;
        if sector >= self.sectors {
            return None;
        }
        sector.checked_add(1)?.checked_mul(self.sector_len)
    }

    /// Locate a FAT sector by its position in the DIFAT, without allocating a sector table.
    fn fat_sector(&self, bytes: &[u8], index: usize) -> Option<u32> {
        if index < 109 {
            return u32_at(bytes, 0x4C + index * 4);
        }
        let entries = self.sector_len / 4 - 1;
        let index = index.checked_sub(109)?;
        let steps = index / entries;
        if steps >= self.sectors || steps >= usize::try_from(u32_at(bytes, 0x48)?).ok()? {
            return None;
        }
        let mut next = u32_at(bytes, 0x44)?;
        for _ in 0..steps {
            let at = self.sector_at(next)?;
            next = u32_at(bytes, at.checked_add(entries * 4)?)?;
        }
        let at = self.sector_at(next)?;
        u32_at(bytes, at.checked_add((index % entries) * 4)?)
    }

    fn next_directory_sector(&self, bytes: &[u8], sector: u32) -> Option<u32> {
        let index = usize::try_from(sector).ok()?;
        let entries = self.sector_len / 4;
        let fat = self.fat_sector(bytes, index / entries)?;
        let at = self.sector_at(fat)?;
        u32_at(bytes, at.checked_add((index % entries) * 4)?)
    }
}

/// Repairs stale FAT end markers beyond EOF and garbage in unused directory entries in a
/// copy of the bytes. Returns `Cow::Borrowed` when nothing needs repair or the header cannot
/// be parsed. Never allocates more than `bytes.len()`; all walks are bounded by the file size.
/// Other malformed data is left for the compound-file guard and reader to reject.
pub fn repair(bytes: &[u8]) -> Cow<'_, [u8]> {
    let Some(header) = Header::parse(bytes) else { return Cow::Borrowed(bytes) };
    let mut fixed = Cow::Borrowed(bytes);
    let entries = header.sector_len / 4;
    // More FAT sectors than this is already refused by cfb_guard. Bounding this walk also
    // prevents a cyclic DIFAT or repeated FAT sectors from making repair work unbounded.
    let limit = header.sectors.div_ceil(entries).saturating_add(1);
    for ordinal in 0..limit {
        let Some(fat) = header.fat_sector(bytes, ordinal) else { break };
        if fat == FREE {
            break;
        }
        let Some(at) = header.sector_at(fat) else { break };
        let first = ordinal.saturating_mul(entries);
        for slot in header.sectors.saturating_sub(first).min(entries)..entries {
            let Some(offset) = at.checked_add(slot * 4) else { break };
            if u32_at(bytes, offset) == Some(END)
                && let Some(dst) = fixed.to_mut().get_mut(offset..offset.saturating_add(4))
            {
                dst.copy_from_slice(&FREE.to_le_bytes());
            }
        }
    }

    // Canonical unused entry: all zero except the three NOSTREAM tree links (§2.6.3).
    let mut unused = [0u8; 128];
    if let Some(links) = unused.get_mut(68..80) {
        links.fill(0xFF);
    }
    let Some(mut directory) = u32_at(bytes, 0x30) else { return fixed };
    for _ in 0..header.sectors {
        let Some(at) = header.sector_at(directory) else { break };
        for slot in 0..header.sector_len / 128 {
            let Some(offset) = at.checked_add(slot * 128) else { break };
            let Some(end) = offset.checked_add(128) else { break };
            if bytes.get(offset.saturating_add(66)) == Some(&0)
                && let Some(entry) = bytes.get(offset..end)
                // The writer may encode an empty name with its two-byte terminator.
                && !entry.iter().zip(&unused).enumerate().all(|(i, (&actual, &expected))| actual == expected || (i == 64 && actual == 2))
                && let Some(dst) = fixed.to_mut().get_mut(offset..end)
            {
                dst.copy_from_slice(&unused);
            }
        }
        let Some(next) = header.next_directory_sector(bytes, directory) else { break };
        directory = next;
    }
    fixed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn fixture(size: usize, sectors: usize, fats: usize) -> Vec<u8> {
        let mut bytes = vec![0; size * (sectors + 1)];
        bytes[..8].copy_from_slice(&MAGIC);
        let (version, shift): (u16, u16) = if size == 512 { (3, 9) } else { (4, 12) };
        bytes[0x1A..0x1C].copy_from_slice(&version.to_le_bytes());
        bytes[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes());
        bytes[0x1E..0x20].copy_from_slice(&shift.to_le_bytes());
        bytes[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
        put(&mut bytes, 0x2C, fats as u32);
        put(&mut bytes, 0x30, (fats + 1) as u32);
        put(&mut bytes, 0x44, END);
        for i in 0..109 {
            put(&mut bytes, 0x4C + i * 4, if i < fats { i as u32 } else { FREE });
        }
        bytes[size..size * (fats + 1)].fill(0xFF);
        if fats > 109 {
            put(&mut bytes, 0x44, fats as u32);
            put(&mut bytes, 0x48, 1);
            let at = (fats + 1) * size;
            bytes[at..at + size].fill(0xFF);
            put(&mut bytes, at, 109);
            put(&mut bytes, at + size - 4, END);
        }
        let dir = (fats + 2) * size;
        for entry in bytes[dir..dir + size].as_chunks_mut::<128>().0 {
            entry[68..80].fill(0xFF);
        }
        put(&mut bytes, size + (fats + 1) * 4, END);
        bytes
    }

    #[test]
    fn difat_chain_and_later_directory_sector() {
        let mut bytes = fixture(512, 14000, 110);
        let stale = 110 * 512 + (14000 - 109 * 128) * 4;
        put(&mut bytes, stale, END);
        // Directory sector 111 continues at 112, whose unused entries are all garbage.
        put(&mut bytes, 512 + 111 * 4, 112);
        put(&mut bytes, 512 + 112 * 4, END);
        bytes[113 * 512..114 * 512].fill(37);
        for slot in 0..4 {
            bytes[113 * 512 + slot * 128 + 66] = 0;
        }
        let fixed = repair(&bytes);
        assert_eq!(u32_at(&fixed, stale), Some(FREE));
        assert_eq!(u32_at(&fixed, 113 * 512 + 68), Some(FREE));
        assert_eq!(u16_at(&fixed, 113 * 512 + 64), Some(0));
        assert_eq!(fixed.len(), bytes.len());
        assert!(matches!(repair(&fixed), Cow::Borrowed(_)));
    }

    #[test]
    fn version_four_and_non_marker_values() {
        let mut bytes = fixture(4096, 4, 1);
        assert!(matches!(repair(&bytes), Cow::Borrowed(_)));
        put(&mut bytes, 4096 + 4 * 4, END);
        put(&mut bytes, 4096 + 5 * 4, 12345);
        let fixed = repair(&bytes);
        assert_eq!(u32_at(&fixed, 4096 + 4 * 4), Some(FREE));
        assert_eq!(u32_at(&fixed, 4096 + 5 * 4), Some(12345));
    }

    #[test]
    fn malformed_headers_are_borrowed_and_directory_cycles_terminate() {
        let mut bytes = fixture(512, 4, 1);
        for len in 0..512 {
            assert!(matches!(repair(&bytes[..len]), Cow::Borrowed(_)));
        }
        bytes[0x1E] = 63;
        assert!(matches!(repair(&bytes), Cow::Borrowed(_)));
        bytes[0x1E] = 9;
        put(&mut bytes, 512 + 2 * 4, 2);
        assert!(matches!(repair(&bytes), Cow::Borrowed(_)));
        assert!(matches!(repair(b"not a compound file"), Cow::Borrowed(_)));
    }
}
