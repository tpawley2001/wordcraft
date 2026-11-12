//! One source paragraph's share of a joined paragraph's layout ([`crate::join`]): the clusters,
//! lines and glyphs that came from its text, with offsets, cluster and object indexes made its
//! own again, so the caret, hit testing and drawing address the paragraph in the document.

use std::ops::Range;

use super::{ClKind, Line, LineEnd, ParaLayout};

/// Where a source paragraph's text sits in the joined paragraph.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Share {
    /// Its byte range in the joined text.
    pub bytes: (usize, usize),
    /// Its range of inline objects in the joined paragraph's `objects`.
    pub objects: (usize, usize),
    /// It is the chain's first paragraph (the list label and drop cap are drawn with it).
    pub first: bool,
    /// It is the chain's last paragraph (its mark ends the joined paragraph).
    pub last: bool,
}

/// Shift a cluster-indexed list into the view's cluster range `k0..k1`.
fn by_cluster<T: Clone>(v: &[(usize, T)], k0: usize, k1: usize) -> Vec<(usize, T)> {
    v.iter().filter(|(k, _)| (k0..k1).contains(k)).map(|(k, t)| (k - k0, t.clone())).collect()
}

/// The part of byte range `r` inside `b0..b1`, made relative to `b0`.
fn clip(r: &Range<usize>, b0: usize, b1: usize) -> Option<Range<usize>> {
    let (s, e) = (r.start.max(b0), r.end.min(b1));
    (s < e).then(|| s - b0..e - b0)
}

impl ParaLayout {
    /// The first line holding cluster `k` (a position at a wrap point belongs to the next line).
    fn line_of_cluster(&self, k: usize) -> usize {
        self.lines.partition_point(|l| l.c1 <= k).min(self.lines.len().saturating_sub(1))
    }

    /// `share`'s view of this joined layout, with the joined line its first line is. `None` when
    /// there is no line to show it on.
    pub(crate) fn share_view(&self, share: Share) -> Option<(ParaLayout, usize)> {
        let (b0, b1) = (share.bytes.0.min(share.bytes.1), share.bytes.1);
        let (o0, o1) = (share.objects.0.min(share.objects.1), share.objects.1);
        let n = self.clusters.len();
        let k0 = if share.first { 0 } else { self.clusters.partition_point(|c| c.start < b0) };
        let k1 = if share.last { n } else { self.clusters.partition_point(|c| c.start < b1) }.max(k0);
        let last_line = self.lines.len().checked_sub(1)?;
        let f = if share.first { 0 } else { self.line_of_cluster(k0) };
        let t = if share.last {
            last_line
        } else if k1 > k0 {
            self.line_of_cluster(k1 - 1)
        } else {
            f
        }
        .max(f);
        let lines = self.lines.get(f..=t)?.iter().map(|l| view_line(l, (k0, k1), (b0, b1), share.last)).collect();
        let own = self.clusters.get(k0..k1).unwrap_or_default();
        let g0 = own.iter().map(|c| c.g0).min().unwrap_or(0);
        let g1 = own.iter().map(|c| c.g1).max().unwrap_or(g0).max(g0);
        let clusters = own
            .iter()
            .map(|c| {
                let mut c = c.clone();
                c.start = c.start.clamp(b0, b1) - b0;
                c.end = c.end.clamp(b0, b1) - b0;
                c.g0 = c.g0.saturating_sub(g0);
                c.g1 = c.g1.saturating_sub(g0);
                if let ClKind::Object(i) = c.kind {
                    c.kind = ClKind::Object(i.saturating_sub(o0));
                }
                c
            })
            .collect();
        let objects = |v: &[usize]| v.iter().filter(|i| (o0..o1).contains(i)).map(|i| i - o0).collect::<Vec<_>>();
        let k_in = |k: u32| usize::try_from(k).is_ok_and(|k| (k0..k1).contains(&k));
        let view = ParaLayout {
            rp: self.rp.clone(),
            substituted: if share.first { self.substituted.clone() } else { Vec::new() },
            styles: self.styles.clone(),
            glyphs: self.glyphs.get(g0 as usize..g1 as usize).map(<[_]>::to_vec).unwrap_or_default(),
            clusters,
            lines,
            label: if share.first { self.label.clone() } else { None },
            height: self.height,
            text_len: b1.saturating_sub(b0),
            joined_cluster_starts: self.joined_cluster_starts.iter().filter(|o| (b0..=b1).contains(*o)).map(|o| o - b0).collect(),
            has_page_fields: self.has_page_fields,
            notes: by_cluster(&self.notes, k0, k1),
            issues: self.issues.iter().filter_map(|(s, e, g)| clip(&(*s..*e), b0, b1).map(|r| (r.start, r.end, *g))).collect(),
            drop_cap: self.drop_cap.filter(|_| share.first).map(|(k, l, w)| (k.min(k1 - k0), l, w)),
            hyph_after: self.hyph_after.iter().filter(|k| k_in(**k)).map(|k| k.saturating_sub(u32::try_from(k0).unwrap_or(u32::MAX))).collect(),
            bidi_levels: if self.bidi_levels.is_empty() { Vec::new() } else { self.bidi_levels.get(b0..b1).map(<[u8]>::to_vec).unwrap_or_default() },
            drop_cap_w: self.drop_cap_w,
            maths: self.maths.iter().filter(|(i, _)| (o0..o1).contains(i)).map(|(i, m)| (i - o0, m.clone())).collect(),
            check_boxes: objects(&self.check_boxes),
            rubies: self
                .rubies
                .iter()
                .filter(|r| (k0..k1).contains(&r.cluster))
                .map(|r| {
                    let mut r = r.clone();
                    r.cluster -= k0;
                    r
                })
                .collect(),
            displays: by_cluster(&self.displays, k0, k1),
            ptabs: self.ptabs.iter().filter(|p| (k0..k1).contains(&p.0)).map(|&(k, a, b, l)| (k - k0, a, b, l)).collect(),
            field_ends: self.field_ends.iter().filter(|k| (k0..k1).contains(*k)).map(|k| k - k0).collect(),
            left_out: self.left_out.iter().filter_map(|r| clip(r, b0, b1)).collect(),
            shown: self.shown.iter().filter(|s| (k0..k1).contains(&s.0)).map(|(k, t, r)| (k - k0, t.clone(), r.clone())).collect(),
            enclosures: by_cluster(&self.enclosures, k0, k1),
            auto_space: by_cluster(&self.auto_space, k0, k1),
        };
        Some((view, f))
    }
}

/// Line `l` of a joined layout as the view of clusters `k0..k1` (bytes `b0..b1`) sees it. Its
/// left and right edges close in on the share's text where other text shares the line, so a
/// click there lands in the paragraph whose text is under it.
fn view_line(l: &Line, (k0, k1): (usize, usize), (b0, b1): (usize, usize), last: bool) -> Line {
    let end = l.c1.max(l.c0);
    let a = k0.max(l.c0).min(end);
    let b = k1.min(end).max(a);
    let (i, j) = (a - l.c0, b - l.c0);
    let xs = l.xs.get(i..=j).map(<[f32]>::to_vec).unwrap_or_else(|| vec![l.left]);
    let vis = if l.vis.is_empty() { Vec::new() } else { l.vis.get(i..j).map(<[_]>::to_vec).unwrap_or_default() };
    let plain = l.vis.is_empty();
    let left = if plain && a > l.c0 { xs.first().copied().unwrap_or(l.left) } else { l.left };
    let right = if plain && b < l.c1 { xs.last().copied().unwrap_or(l.right) } else { l.right };
    // The line goes on past this share's text (or its own mark is gone): it doesn't end here.
    let goes_on = end > b || (l.end == LineEnd::Para && !last);
    Line {
        top: l.top,
        height: l.height,
        baseline: l.baseline,
        text_asc: l.text_asc,
        text_desc: l.text_desc,
        c0: a.saturating_sub(k0),
        c1: b.saturating_sub(k0),
        xs,
        leaders: l.leaders.iter().filter(|(k, _)| (a..b).contains(k)).map(|&(k, ld)| (k.saturating_sub(k0), ld)).collect(),
        end: if goes_on { LineEnd::Wrap } else { l.end },
        start: l.start.clamp(b0, b1) - b0,
        stop: l.stop.clamp(b0, b1) - b0,
        left,
        right,
        hyphen: if goes_on { None } else { l.hyphen },
        beside: l.beside,
        vis,
        rtl: l.rtl,
    }
}
