//! What Immersive Reader draws over the pages on screen: Syllables' dots between the syllables
//! of each word, and Line Focus's dark cover over all but a band of lines around the caret.
//! Screen only: the text, its line breaks, printing and saved files are unchanged.

use egui::{Color32, Painter, Rect, pos2, vec2};
use wordcraft_doc::props::TextDirection;
use wordcraft_layout::{DocLayout, Placed};

use super::pages::page_screen_scale;
use crate::WordApp;

/// Line Focus's cover: near-black at 80% opacity.
const FOCUS_COVER: Color32 = Color32::from_rgba_premultiplied(8, 8, 8, 204);

/// One line of text on screen: its top and bottom, and its page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ScreenLine {
    pub page: usize,
    pub top: f32,
    pub bottom: f32,
}

/// Draws Syllables and Line Focus while Immersive Reader is open. `rects` are the pages'
/// screen rectangles, `caret` the caret in page coordinates.
pub(super) fn paint(
    app: &WordApp,
    painter: &Painter,
    layout: &DocLayout,
    rects: &[Rect],
    scale: f32,
    caret: Option<wordcraft_layout::hit::Caret>,
    clip: Rect,
) {
    let v = &app.session.view;
    if !v.immersive {
        return;
    }
    let dark = super::pages::dark_page(app);
    let mut lines = Vec::new();
    for (pi, page) in layout.pages.iter().enumerate() {
        let Some(pr) = rects.get(pi) else { continue };
        if !pr.intersects(clip) {
            continue;
        }
        let s = page_screen_scale(*pr, page, scale);
        for item in &page.items {
            let Placed::Lines { story, path, para, l0, l1, x, y, turn } = item else { continue };
            if *turn != TextDirection::Horizontal {
                continue;
            }
            let Some(first) = para.lines.get(*l0) else { continue };
            let base_top = first.top;
            let text = app.session.doc.para(*story, path).map(|p| p.text.as_str());
            for line in para.lines.get(*l0..*l1).unwrap_or(&[]) {
                let top = pr.min.y + (y + line.top - base_top) * s;
                let bottom = top + line.height * s;
                lines.push(ScreenLine { page: pi, top, bottom });
                if v.syllables
                    && let Some(text) = text
                {
                    let origin = pos2(pr.min.x + x * s, top);
                    syllable_dots(painter, para, line, text, origin, s, dark);
                }
            }
        }
    }
    if v.line_focus > 0 {
        let caret = caret.and_then(|c| {
            let pr = rects.get(c.page)?;
            let page = layout.pages.get(c.page)?;
            let s = page_screen_scale(*pr, page, scale);
            Some((c.page, pr.min.y + (c.top + c.height / 2.0) * s))
        });
        let band = focus_band(&lines, caret, v.line_focus);
        let (top, bottom) = band.unwrap_or((clip.min.y, clip.min.y));
        let above = Rect::from_min_max(clip.min, pos2(clip.max.x, top.clamp(clip.min.y, clip.max.y)));
        let below = Rect::from_min_max(pos2(clip.min.x, bottom.clamp(clip.min.y, clip.max.y)), clip.max);
        for r in [above, below] {
            if r.height() > 0.0 {
                painter.rect_filled(r, 0.0, FOCUS_COVER);
            }
        }
    }
}

/// The top and bottom of Line Focus's band: the caret's line (`n` = 1), or it and one (3) or two
/// (5) lines either side. `caret` is the caret's page and the screen y of its middle. `None`
/// when the caret's line isn't among `lines`.
pub(super) fn focus_band(lines: &[ScreenLine], caret: Option<(usize, f32)>, n: u8) -> Option<(f32, f32)> {
    let (page, y) = caret?;
    let mut sorted: Vec<ScreenLine> = lines.to_vec();
    sorted.sort_by(|a, b| a.top.total_cmp(&b.top));
    let at = sorted.iter().position(|l| l.page == page && l.top <= y && y <= l.bottom)?;
    let r = usize::from(n.saturating_sub(1) / 2);
    let first = sorted.get(at.saturating_sub(r))?;
    let last = sorted.get(at.saturating_add(r).min(sorted.len().saturating_sub(1)))?;
    Some((first.top, last.bottom.max(first.bottom)))
}

/// Middle dots between the syllables of the words on `line` (the paragraph's text is `text`),
/// in the text colour at 60% opacity. `origin` is the screen point of the column's left edge at
/// the line's top.
fn syllable_dots(
    painter: &Painter,
    para: &wordcraft_layout::para::ParaLayout,
    line: &wordcraft_layout::para::Line,
    text: &str,
    origin: egui::Pos2,
    s: f32,
    dark: bool,
) {
    let Some(shown) = text.get(line.start..line.stop) else { return };
    let color = if dark { Color32::from_white_alpha(153) } else { Color32::from_black_alpha(153) };
    let mut start: Option<usize> = None;
    for (i, c) in shown.char_indices().chain(std::iter::once((shown.len(), ' '))) {
        match (start, c.is_alphabetic()) {
            (None, true) => start = Some(i),
            (Some(a), false) => {
                start = None;
                let Some(word) = shown.get(a..i) else { continue };
                let at = line.start + a;
                // The word's language and size: its first cluster's.
                let Some(first) = cluster_at(para, line, at) else { continue };
                let Some(style) = para.clusters.get(first).and_then(|cl| para.styles.get(cl.style as usize)) else { continue };
                for b in wordcraft_engine::cmd::view::syllable_breaks(word, style.rc.lang.as_deref()) {
                    let Some(k) = cluster_at(para, line, at + b) else { continue };
                    let Some(x) = line.cl_left(k) else { continue };
                    let size = (style.size * s).clamp(4.0, 200.0);
                    let p = origin + vec2(x * s, (line.baseline - style.size * 0.3) * s);
                    painter.text(p, egui::Align2::CENTER_CENTER, "\u{00B7}", egui::FontId::proportional(size), color);
                }
            }
            _ => {}
        }
    }
}

/// The cluster on `line` that starts at byte `at` of the paragraph.
fn cluster_at(para: &wordcraft_layout::para::ParaLayout, line: &wordcraft_layout::para::Line, at: usize) -> Option<usize> {
    (line.c0..line.c1).find(|&k| para.clusters.get(k).is_some_and(|c| c.start == at))
}

#[cfg(test)]
mod tests {
    use super::{ScreenLine, focus_band};

    fn lines() -> Vec<ScreenLine> {
        (0..10).map(|i| ScreenLine { page: 0, top: i as f32 * 20.0, bottom: i as f32 * 20.0 + 20.0 }).collect()
    }

    #[test]
    fn line_focus_band_is_one_three_or_five_lines_around_the_caret() {
        let l = lines();
        assert_eq!(focus_band(&l, Some((0, 50.0)), 1), Some((40.0, 60.0)));
        assert_eq!(focus_band(&l, Some((0, 50.0)), 3), Some((20.0, 80.0)));
        assert_eq!(focus_band(&l, Some((0, 50.0)), 5), Some((0.0, 100.0)));
        assert_eq!(focus_band(&l, Some((0, 10.0)), 5), Some((0.0, 60.0)), "the first line keeps the band in the text");
        assert_eq!(focus_band(&l, Some((1, 50.0)), 3), None, "another page");
        assert_eq!(focus_band(&l, None, 3), None);
    }
}
