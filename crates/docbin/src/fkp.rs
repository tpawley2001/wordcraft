//! Character and paragraph formatting lookup: the bin tables (`PlcBteChpx`/`PlcBtePapx`) in
//! the Table stream point at 512-byte FKP pages in the WordDocument stream, which map FC
//! ranges to CHPX (grpprls) and PAPX (istd + grpprl) ([MS-DOC] §2.4.6.1/2.4.6.2).

use crate::DocbinError;

/// Largest bin table (entries) we accept.
const MAX_BINS: usize = 100_000;

/// A parsed `PlcBte*`: FC breakpoints and the FKP page each range lives in.
pub(crate) struct Bins {
    fcs: Vec<u32>,
    /// Page number (× 512 = byte offset in the WordDocument stream).
    pns: Vec<u32>,
}

impl Bins {
    /// Parse the Plc at `table[fc..fc+lcb]` (aFc[n+1] u32s + aPn[n] u32s).
    pub(crate) fn parse(table: &[u8], fc: u32, lcb: u32) -> Result<Bins, DocbinError> {
        let fc = fc as usize;
        let lcb = lcb as usize;
        if lcb < 4 || !(lcb - 4).is_multiple_of(8) {
            return Ok(Bins { fcs: Vec::new(), pns: Vec::new() });
        }
        let end = fc.checked_add(lcb).ok_or_else(|| DocbinError::Malformed("bin table overflow".into()))?;
        let plc = table.get(fc..end).ok_or_else(|| DocbinError::Malformed("bin table outside the Table stream".into()))?;
        let n = (lcb - 4) / 8;
        if n > MAX_BINS {
            return Err(DocbinError::Limit(format!("bin table has {n} entries")));
        }
        let at = |i: usize| -> Option<u32> { plc.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
        let mut fcs = Vec::with_capacity(n + 1);
        for i in 0..=n {
            fcs.push(at(i).ok_or_else(|| DocbinError::Malformed("truncated bin aFc".into()))?);
        }
        let mut pns = Vec::with_capacity(n);
        for i in 0..n {
            // PnBte* keeps the page number in its low bits; the rest must be zero.
            pns.push(at(n + 1 + i).ok_or_else(|| DocbinError::Malformed("truncated bin aPn".into()))? & 0x003F_FFFF);
        }
        Ok(Bins { fcs, pns })
    }

    /// Parse a Word 6.0/95 bin table at `word[fc..fc+lcb]`: aFc[n+1] u32s + aPn[n] **u16**s
    /// (Word 6 paragraph FKPs have smaller BX entries than Word 97).
    pub(crate) fn parse_word6(word: &[u8], fc: u32, lcb: u32) -> Bins {
        let lcb = lcb as usize;
        let empty = Bins { fcs: Vec::new(), pns: Vec::new() };
        if lcb < 4 || !(lcb - 4).is_multiple_of(6) {
            return empty;
        }
        let Some(plc) = (fc as usize).checked_add(lcb).and_then(|end| word.get(fc as usize..end)) else { return empty };
        let n = ((lcb - 4) / 6).min(MAX_BINS);
        let fcs = (0..=n).filter_map(|i| plc.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))).collect();
        let base = (n + 1) * 4;
        let pns = (0..n).filter_map(|i| plc.get(base + i * 2..base + i * 2 + 2).map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])))).collect();
        Bins { fcs, pns }
    }

    /// The page containing `fc`, as a slice of the WordDocument stream.
    fn page<'w>(&self, word: &'w [u8], fc: u32) -> Option<&'w [u8]> {
        let i = self.fcs.partition_point(|&f| f <= fc).checked_sub(1)?;
        let pn = *self.pns.get(i)?;
        let at = pn.checked_mul(512)? as usize;
        word.get(at..at.checked_add(512)?)
    }

    /// The grpprl of the Chpx covering `fc` ([MS-DOC] ChpxFkp: rgfc u32s, rgb offsets ×2,
    /// crun in the last byte).
    pub(crate) fn chpx<'w>(&self, word: &'w [u8], fc: u32) -> &'w [u8] {
        let Some(page) = self.page(word, fc) else { return &[] };
        let Some(&crun) = page.get(511) else { return &[] };
        let crun = crun.min(0x65) as usize;
        let rgfc = |i: usize| -> Option<u32> { page.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
        // rgb array sits right after rgfc (crun+1 u32s); each byte is a half-offset into the
        // region between rgb's end and crun.
        let rgb_at = (crun + 1) * 4;
        let j = (0..crun).rev().find(|&j| rgfc(j).is_some_and(|f| f <= fc));
        let Some(j) = j else { return &[] };
        let Some(&off) = page.get(rgb_at + j) else { return &[] };
        if off == 0 {
            return &[];
        }
        let at = off as usize * 2;
        let Some(&cb) = page.get(at) else { return &[] };
        page.get(at + 1..at + 1 + cb as usize).unwrap_or(&[])
    }

    /// Word 6 PAPX at the paragraph mark's FC: seven-byte BX entries and a word-count
    /// length followed by an ignored style index and one-byte-opcode grpprl.
    pub(crate) fn papx_word6<'w>(&self, word: &'w [u8], fc: u32) -> &'w [u8] {
        self.papx_word6_data(word, fc).unwrap_or(&[])
    }

    fn papx_word6_data<'w>(&self, word: &'w [u8], fc: u32) -> Option<&'w [u8]> {
        let page = self.page(word, fc)?;
        let crun = usize::from(*page.get(511)?);
        let page = page.get(..511)?;
        let rgbx_at = crun.checked_add(1)?.checked_mul(4)?;
        let header_end = rgbx_at.checked_add(crun.checked_mul(7)?)?;
        page.get(..header_end)?;
        let rgfc = |i: usize| -> Option<u32> {
            let at = i.checked_mul(4)?;
            Some(u32::from_le_bytes(page.get(at..at.checked_add(4)?)?.try_into().ok()?))
        };
        let j = (0..crun).find(|&j| rgfc(j).is_some_and(|start| start <= fc) && j.checked_add(1).and_then(rgfc).is_some_and(|end| fc < end))?;
        let off = usize::from(*page.get(rgbx_at.checked_add(j.checked_mul(7)?)?)?);
        if off == 0 {
            return None;
        }
        let at = off.checked_mul(2)?;
        if at < header_end {
            return None;
        }
        let len = usize::from(*page.get(at)?).checked_mul(2)?;
        let data_at = at.checked_add(1)?;
        let data = page.get(data_at..data_at.checked_add(len)?)?;
        data.get(2..)
    }

    /// The (istd, grpprl) of the Papx of the paragraph whose mark is just before `fc`
    /// ([MS-DOC] PapxFkp: rgfc u32s — each the FC *following* a paragraph mark —, rgbx BX
    /// entries (1 offset byte + 12 PHE bytes), crun in the last byte; the PAPX itself is a
    /// PapxInFkp with the cb/cb' size quirk). Query with the FC of the character after the mark.
    pub(crate) fn papx<'w>(&self, word: &'w [u8], fc: u32) -> (u16, &'w [u8]) {
        let Some(page) = self.page(word, fc) else { return (0, &[]) };
        let Some(&crun) = page.get(511) else { return (0, &[]) };
        // BX is 13 bytes; cap crun at what fits before byte 511.
        let max_run = (511usize.saturating_sub((crun as usize + 1) * 4)) / 13;
        let crun = (crun as usize).min(max_run);
        let rgfc = |i: usize| -> Option<u32> { page.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
        let rgbx_at = (crun + 1) * 4;
        let j = (0..crun).rev().find(|&j| rgfc(j).is_some_and(|f| f <= fc));
        let Some(j) = j else { return (0, &[]) };
        let Some(&off) = page.get(rgbx_at + j * 13) else { return (0, &[]) };
        if off == 0 {
            return (0, &[]);
        }
        let at = off as usize * 2;
        let u16at = |o: usize| -> Option<u16> { page.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])) };
        let Some(cb) = page.get(at).copied() else { return (0, &[]) };
        // PapxInFkp: if cb is nonzero the GrpPrlAndIstd (istd u16 + grpprl) is 2×cb-1 bytes;
        // if cb is zero, the next byte cb' (1 byte) gives 2×cb' bytes of it.
        let (data_at, data_len) = if cb != 0 {
            (at + 1, 2 * cb as usize - 1)
        } else {
            let cb2 = page.get(at + 1).copied().unwrap_or(0) as usize;
            (at + 2, 2 * cb2)
        };
        if data_len < 2 {
            return (0, &[]);
        }
        let istd = u16at(data_at).unwrap_or(0);
        (istd, page.get(data_at + 2..data_at + data_len).unwrap_or(&[]))
    }
}

#[cfg(test)]
mod word6_tests {
    use super::Bins;

    #[test]
    fn malformed_papx_defaults_and_run_end_is_exclusive() {
        let bins = Bins { fcs: vec![100, 200], pns: vec![0] };
        let mut page = [0u8; 512];
        page[..4].copy_from_slice(&100u32.to_le_bytes());
        page[4..8].copy_from_slice(&200u32.to_le_bytes());
        page[8] = 250;
        page[500..505].copy_from_slice(&[2, 0, 0, 5, 1]);
        page[511] = 1;
        assert_eq!(bins.papx_word6(&page, 199), &[5, 1]);
        assert!(bins.papx_word6(&page, 200).is_empty());
        for (at, value) in [(511, 255), (511, 0), (8, 255), (8, 0), (8, 1), (500, 255), (500, 0)] {
            let mut bad = page;
            bad[at] = value;
            assert!(bins.papx_word6(&bad, 199).is_empty(), "{at}={value}");
        }
        for len in 0..512 {
            assert!(bins.papx_word6(&page[..len], 199).is_empty());
        }
        let bad_bins = Bins { fcs: vec![100, 200], pns: vec![u32::MAX] };
        assert!(bad_bins.papx_word6(&page, 199).is_empty());
    }
}
