use super::model::ClKind;
use super::proof::{ProofRun, lang_at};
use super::{ParaLayout, SOFT_HYPHEN};

/// Word's default hyphenation zone (0.25"), points.
pub(super) const HYPHENATION_ZONE: f32 = wordcraft_doc::DEFAULT_HYPHENATION_ZONE;

/// How much of their width the spaces of a justified line may give up to fit more text (Word
/// 2013+, compatibility mode 15).
pub(super) const MAX_SPACE_SHRINK: f32 = 0.25;

/// How much of their width the single spaces of a line may give up in a document opened from
/// RTF: just under half (Word keeps the word at 49.2% and wraps it at 50%).
pub(super) const RTF_MAX_SPACE_SHRINK: f32 = 0.5;

/// How far one space `space_w` wide may shrink to keep an overrunning word below compatibility
/// mode 15 with Compress punctuation: down to half the font's average character width `avg_w`
/// (OS/2 `xAvgCharWidth` at the run's size), and not at all when that is wider than the space.
/// Measured in Word (synthetic .docx files, mode 11): 12 pt Times New Roman about .57 pt per
/// space, Arial about .70 pt, Calibri none; for a fixed-pitch font (space = average) half a space.
pub(crate) fn compress_space_room(space_w: f32, avg_w: f32) -> f32 {
    (space_w - avg_w / 2.0).max(0.0)
}

/// How far a single space of one style may shrink in the RTF / Compress punctuation squeeze
/// ([`rtf_space_room`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum SpaceRoom {
    /// A fixed-pitch font: half the space ([`RTF_MAX_SPACE_SHRINK`]).
    Half,
    /// A proportional font: down to half its average character width, in points
    /// ([`compress_space_room`]).
    Average(f32),
    /// A proportional font in a document whose proportional fonts don't squeeze (opened from RTF,
    /// or with `w:balanceSingleByteDoubleByteWidth` and `w:useWord97LineBreakRules`): a single
    /// space in it keeps the whole line from squeezing.
    Blocks,
}

impl SpaceRoom {
    /// The room of a space `adv` wide in this style.
    pub(super) fn room(self, adv: f32) -> f32 {
        match self {
            SpaceRoom::Half => adv * RTF_MAX_SPACE_SHRINK,
            SpaceRoom::Average(avg) => compress_space_room(adv, avg),
            SpaceRoom::Blocks => 0.0,
        }
    }
}

/// Points inside the right edge the space before the overrunning word must start, for widths
/// summed in `f32`: Word wraps the word when that space starts exactly on the edge.
const RTF_EDGE_TOLERANCE: f32 = 0.01;

/// How far a word (with the space before it) may overrun a justified line and still be fitted
/// onto it by shrinking the line's spaces, as a share of its own width.
const MAX_WORD_OVERRUN: f32 = 0.355;

/// How far a justified line's text may run past its right edge, its spaces (`spaces` wide in
/// all) shrinking to make room, when the word now ending the line is `word` wide with the space
/// before it. Measured in Word for Mac (compatibility mode 15, synthetic justified paragraphs):
/// at most a quarter of the spaces' width, and at most 0.355 of the word's own (0.351 to 0.360
/// across words of 2.7 to 80 pt and spaces of 3.3 and 6.7 pt).
pub(super) fn space_room(spaces: f32, word: f32) -> f32 {
    (spaces * MAX_SPACE_SHRINK).min(word.max(0.0) * MAX_WORD_OVERRUN)
}

/// How far a line of a document opened from RTF (or below compatibility mode 15 with Compress
/// punctuation) may run past its right edge, its inner single spaces (up to and including the one
/// before the word) shrinking to make room, when the word ending the line ends `overrun` past the
/// edge and the space before it starts at `space_from`. `room` is the spaces' summed
/// [`SpaceRoom::room`]. Measured in Word (synthetic RTF files, Courier New 8 to 20 pt, left,
/// right, centred and justified alike): the word stays when the space before it starts inside the
/// line and the overrun is less than that room (for fixed pitch, half the width of the line's
/// single spaces), however many there are; the word's own width doesn't limit it. Otherwise
/// nothing (the word wraps). Only single spaces count: a run of two or more keeps its width. In a
/// document opened from RTF the caller squeezes only lines whose single spaces are all in
/// fixed-pitch fonts ([`SpaceRoom::Blocks`]): in Times New Roman an overrun of 0.125 pt wraps.
///
/// With `\wraptrsp` (`Settings::wrap_trail_spaces`, measured for justified and left-aligned lines) `first` is
/// how far the word's first character ends past the edge and the width of the space before the
/// word: Word keeps the word only when that character overruns by less than half that space.
pub(super) fn rtf_space_room(room: f32, overrun: f32, space_from: f32, right_edge: f32, first: Option<(f32, f32)>) -> f32 {
    let first_ok = first.is_none_or(|(over, space)| over < space * 0.5);
    let starts_inside = space_from < right_edge - RTF_EDGE_TOLERANCE;
    if first_ok && starts_inside && overrun > 0.0 && overrun < room { overrun } else { 0.0 }
}

/// How far past its right edge a line may run, as a share of its width, in a document with
/// WordPerfect-style justification (`w:wpJustification`): measured in Word (compatibility mode
/// 11, justified Times New Roman paragraphs), the word stays when it overruns by at most 0.03907
/// to 0.03914 of the line's width, whatever the number of spaces or the word's width.
const WP_LINE_SQUEEZE: f32 = 0.0391;

/// What a WordPerfect-justified line squeezed out of its spaces has left over comes off each
/// character in turn, from the line's end: 1/600 inch.
const WP_CHAR_SQUEEZE: f32 = 0.12;

/// At most this many passes over a squeezed line's characters (each pass takes
/// [`WP_CHAR_SQUEEZE`] off every character).
const WP_MAX_PASSES: usize = 64;

/// How far a justified line from `left` to `right_edge` may run past its edge with
/// WordPerfect-style justification ([`WP_LINE_SQUEEZE`] of its width).
pub(super) fn wp_line_room(left: f32, right_edge: f32) -> f32 {
    let w = right_edge - left;
    if w.is_finite() && w > 0.0 { w * WP_LINE_SQUEEZE } else { 0.0 }
}

/// Squeeze a WordPerfect-justified line that overruns its edge by `-slack` (at most `room`) so
/// that it ends at the edge: the inner spaces after the line's last tab (from `from`) shrink
/// alike, by at most [`MAX_SPACE_SHRINK`] of their width, and what is still over comes off the
/// characters after the tab, [`WP_CHAR_SQUEEZE`] each, from the line's last character leftwards,
/// pass after pass. A line overrunning by more (a word longer than the line) is left alone.
pub(super) fn wp_squeeze_line(pl: &ParaLayout, xs: &mut [f32], (c0, c1, from): (usize, usize, usize), (content_end, slack): (f32, f32), room: f32) {
    let over = -slack;
    if !over.is_finite() || over <= 0.0 || over > room + 0.05 || c1 < c0 {
        return;
    }
    let from = from.clamp(c0, c1);
    let inside = |k: usize| xs.get(k - c0).copied().unwrap_or(f32::MAX) < content_end - 0.01;
    let text_from = (from..c1).find(|k| pl.clusters.get(*k).is_some_and(|c| c.kind != ClKind::Space && c.kind != ClKind::Marker)).unwrap_or(c1);
    let mut take = vec![0.0f32; c1 - c0];
    let spaces: f32 = (text_from..c1)
        .filter(|k| pl.clusters.get(*k).is_some_and(|c| c.kind == ClKind::Space) && inside(*k))
        .filter_map(|k| pl.clusters.get(k))
        .map(|c| c.adv.max(0.0))
        .sum();
    let mut left = over;
    if spaces > 0.0 {
        let ratio = (over / spaces).min(MAX_SPACE_SHRINK);
        for k in text_from..c1 {
            if let Some(c) = pl.clusters.get(k).filter(|c| c.kind == ClKind::Space)
                && inside(k)
                && let Some(t) = take.get_mut(k - c0)
            {
                *t = c.adv.max(0.0) * ratio;
            }
        }
        left -= spaces * ratio;
    }
    // The characters: letters and inner spaces, from the line's last one leftwards.
    let chars: Vec<usize> =
        (from..c1).rev().filter(|k| pl.clusters.get(*k).is_some_and(|c| matches!(c.kind, ClKind::Text | ClKind::Space)) && inside(*k)).collect();
    for _ in 0..WP_MAX_PASSES {
        if left <= 1e-4 {
            break;
        }
        let mut took = false;
        for &k in &chars {
            if left <= 1e-4 {
                break;
            }
            let adv = pl.clusters.get(k).map_or(0.0, |c| c.adv.max(0.0));
            if let Some(t) = take.get_mut(k - c0) {
                let d = WP_CHAR_SQUEEZE.min(left).min(adv - *t).max(0.0);
                if d > 0.0 {
                    *t += d;
                    left -= d;
                    took = true;
                }
            }
        }
        if !took {
            break;
        }
    }
    let mut shift = 0.0;
    for (q, v) in xs.iter_mut().enumerate() {
        *v -= shift;
        shift += take.get(q).copied().unwrap_or(0.0);
    }
}

/// Whether the space cluster `k` stands alone: no space next to it (markers between don't
/// separate them).
fn single_space(pl: &ParaLayout, k: usize) -> bool {
    let is_space = |c: Option<&super::model::Cluster>| c.is_some_and(|c| c.kind == ClKind::Space);
    let before = pl.clusters.get(..k).and_then(|cs| cs.iter().rev().find(|c| c.kind != ClKind::Marker));
    let after = pl.clusters.get(k + 1..).and_then(|cs| cs.iter().find(|c| c.kind != ClKind::Marker));
    !is_space(before) && !is_space(after)
}

/// Take `-slack` (an overflow) out of the line's inner spaces, in proportion to their widths and
/// never more than `cap` of them ([`MAX_SPACE_SHRINK`], or [`RTF_MAX_SPACE_SHRINK`]). Trailing
/// spaces hang and are left alone; with `skip_leading`, so are the spaces before the line's
/// first character. With `singles_only` (RTF) a run of two or more spaces keeps its width and
/// only single spaces shrink. With `rooms` (each style's [`SpaceRoom`], RTF) each space gives up
/// the same share of its own room instead of `cap` of its width.
pub(super) fn shrink_line_spaces(
    pl: &ParaLayout,
    xs: &mut [f32],
    (c0, c1): (usize, usize),
    (content_end, slack): (f32, f32),
    cap: f32,
    (skip_leading, singles_only): (bool, bool),
    rooms: Option<&[SpaceRoom]>,
) {
    let first = if skip_leading {
        (c0..c1).find(|k| pl.clusters.get(*k).is_some_and(|c| c.kind != ClKind::Space && c.kind != ClKind::Marker)).unwrap_or(c1)
    } else {
        c0
    };
    let spaces: Vec<(usize, f32)> = (first..c1)
        .filter_map(|k| {
            let c = pl.clusters.get(k).filter(|c| c.kind == ClKind::Space)?;
            if singles_only && !single_space(pl, k) {
                return None;
            }
            let room = match rooms {
                Some(r) => r.get(c.style as usize).map_or(0.0, |r| r.room(c.adv)),
                None => c.adv,
            };
            (xs.get(k - c0).copied().unwrap_or(f32::MAX) < content_end - 0.01).then_some((k, room))
        })
        .collect();
    let total: f32 = spaces.iter().map(|s| s.1).sum();
    if total <= 0.0 || !slack.is_finite() {
        return;
    }
    let ratio = (-slack / total).clamp(0.0, if rooms.is_some() { 1.0 } else { cap });
    let mut take = 0.0;
    let mut next = spaces.iter().peekable();
    for k in c0..c1 {
        if let Some(v) = xs.get_mut(k - c0) {
            *v -= take;
        }
        if let Some((_, adv)) = next.next_if(|s| s.0 == k) {
            take += adv * ratio;
        }
    }
    if let Some(v) = xs.last_mut() {
        *v -= take;
    }
}

/// The hyphen glyph and advance in a style (cached per paragraph).
pub(super) fn hyphen_glyph(pl: &ParaLayout, style: u32, cache: &mut Vec<(u32, u32, f32)>) -> (u32, u32, f32) {
    if let Some(h) = cache.iter().find(|h| h.0 == style) {
        return *h;
    }
    let h = pl
        .styles
        .get(style as usize)
        .and_then(|st| {
            let g = wordcraft_fonts::shape(&st.face, "-", &[], |c| c).into_iter().next()?;
            let k = st.size / st.face.upem.max(1.0) as f32 * st.rc.scale / 100.0;
            Some((style, g.gid, g.x_advance as f32 * k))
        })
        .unwrap_or((style, 0, 0.0));
    cache.push(h);
    h
}

/// Clusters after which a line may end with a hyphen: soft hyphens always, and dictionary or
/// pattern hyphenation points of each word, in the language of its run ([`ProofRun`]), when
/// automatic hyphenation is on (words in capitals only with `caps`). Both are found in `text`, the paragraph as laid out (without what
/// the layout leaves out, [`left_out`]), and `to_para` maps the end of a char there to the
/// paragraph's text.
pub(super) fn hyphenation_points(
    text: &str,
    to_para: impl Fn(usize) -> Option<usize>,
    pl: &ParaLayout,
    auto: bool,
    caps: bool,
    runs: &[ProofRun],
) -> Vec<u32> {
    let mut bytes: Vec<usize> = text.char_indices().filter(|(_, c)| *c == SOFT_HYPHEN).map(|(i, c)| i + c.len_utf8()).collect();
    // Vietnamese words are single syllables and never take a hyphen: no automatic hyphenation in
    // a Vietnamese paragraph, nor in a word written with a Vietnamese-only letter (`Nguyễn`).
    if auto && !wordcraft_proof::vi::looks_vietnamese(text) {
        let lim = wordcraft_proof::hyphen::Limits::default();
        let mut start: Option<usize> = None;
        for (i, c) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
            let word_char = c.is_alphabetic() || c == '\'' || c == '\u{2019}';
            match (start, word_char) {
                (None, true) => start = Some(i),
                (Some(a), false) => {
                    // The patterns are for Latin-script languages: never hyphenate Persian/Arabic words.
                    // Words in capitals (`NASA`, `README`) only when the document allows it.
                    let all_caps = |w: &str| w.chars().filter(|c| c.is_alphabetic()).all(char::is_uppercase);
                    if let Some(w) = text.get(a..i)
                        && w.chars().count() >= lim.min_word
                        && !w.chars().any(wordcraft_fonts::is_rtl)
                        && !wordcraft_proof::vi::is_vietnamese_word(w)
                        && (caps || !all_caps(w))
                    {
                        // The word's language: its first letter's run, found by its place in `p.text`.
                        let first = w.chars().next().map_or(1, char::len_utf8);
                        let lang = to_para(a + first).map(|e| lang_at(runs, e.saturating_sub(first))).unwrap_or_default();
                        let offs: Vec<usize> = w.char_indices().map(|(o, _)| o).collect();
                        // The hyphenator drops every point of an all-capitals word (it takes it for an
                        // acronym); Word hyphenates it where the lower-case word breaks. Only when each
                        // letter lowercases to one char, so the char offsets still line up (`ß`, `İ`).
                        let lower = (caps && all_caps(w) && w.chars().any(char::is_alphabetic) && w.chars().all(|c| c.to_lowercase().count() == 1))
                            .then(|| w.to_lowercase());
                        let ask = lower.as_deref().unwrap_or(w);
                        for pt in wordcraft_proof::hyphen::hyphen_points_in(ask, &lim, lang) {
                            if let Some(o) = offs.get(pt) {
                                bytes.push(a + o);
                            }
                        }
                    }
                    start = None;
                }
                _ => {}
            }
        }
    }
    let mut bytes: Vec<usize> = bytes.into_iter().filter_map(to_para).collect();
    if bytes.is_empty() {
        return Vec::new();
    }
    bytes.sort_unstable();
    let mut out: Vec<u32> = pl
        .clusters
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c.kind, ClKind::Text | ClKind::Marker) && bytes.binary_search(&c.end).is_ok())
        .map(|(i, _)| i as u32)
        .collect();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitals_that_lowercase_to_several_chars_lay_out_without_panicking() {
        // `İ` lowercases to two chars: such a word keeps the old behaviour (no lower-case lookup).
        crate::test_fonts();
        for caps in [true, false] {
            let mut d = wordcraft_doc::Document::from_text("İNTERNATİONALİZATİON STRAßENBAHN INTERNATIONALIZATION ''");
            d.settings.auto_hyphenation = true;
            d.settings.hyphenate_caps = caps;
            let l = crate::layout(&d, &mut crate::LayoutCache::new(), &crate::LayoutOptions::default());
            assert!(!l.pages.is_empty());
        }
    }

    #[test]
    fn a_fixed_pitch_space_keeps_half_its_width() {
        for adv in [4.8_f32, 7.2, 12.0] {
            assert!((SpaceRoom::Half.room(adv) - adv * RTF_MAX_SPACE_SHRINK).abs() < 1e-6);
            // A fixed-pitch font's space is its average width: the same room either way.
            assert!((SpaceRoom::Average(adv).room(adv) - SpaceRoom::Half.room(adv)).abs() < 1e-6);
        }
        assert_eq!(SpaceRoom::Blocks.room(7.2), 0.0);
        assert_eq!(SpaceRoom::Average(f32::INFINITY).room(7.2), 0.0, "no average: no room");
    }

    #[test]
    fn the_overrunning_word_stays_within_the_summed_room() {
        // Ten 12 pt Times New Roman spaces (3 pt, average 4.81 pt): about 5.95 pt of room.
        let room: f32 = (0..10).map(|_| SpaceRoom::Average(821.0 / 2048.0 * 12.0).room(3.0)).sum();
        assert!((room - 5.947).abs() < 0.01, "{room}");
        assert_eq!(rtf_space_room(room, 5.8, 400.0, 432.0, None), 5.8, "kept");
        assert_eq!(rtf_space_room(room, 6.1, 400.0, 432.0, None), 0.0, "wraps");
        assert_eq!(rtf_space_room(room, 1.0, 432.0, 432.0, None), 0.0, "the space starts at the edge");
        // Calibri-like proportions: no room, the word wraps however small the overrun.
        let none: f32 = (0..10).map(|_| SpaceRoom::Average(6.14).room(2.71)).sum();
        assert_eq!(rtf_space_room(none, 0.05, 400.0, 432.0, None), 0.0);
    }
}
