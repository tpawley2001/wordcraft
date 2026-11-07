//! Where View › Window › Arrange All and View Side by Side put the windows: outer window
//! rectangles in points that share their edges exactly (no gaps, no overlaps).

/// Edge `k` of `n` equal parts of `size` from `start`, rounded to a whole point.
fn edge(start: f32, size: f32, k: usize, n: usize) -> f32 {
    if n == 0 {
        return start;
    }
    (start + size * k as f32 / n as f32).round()
}

/// Arrange All: `n` windows stacked top to bottom, each the full width of `area` and 1/n of
/// its height, in order.
pub(crate) fn arrange_all(area: egui::Rect, n: usize) -> Vec<egui::Rect> {
    (0..n)
        .map(|k| {
            let top = edge(area.top(), area.height(), k, n);
            let bottom = edge(area.top(), area.height(), k.saturating_add(1), n);
            egui::Rect::from_min_max(egui::pos2(area.left(), top), egui::pos2(area.right(), bottom))
        })
        .collect()
}

/// View Side by Side: the left and right halves of `area`, each full height.
pub(crate) fn side_by_side(area: egui::Rect) -> [egui::Rect; 2] {
    let mid = edge(area.left(), area.width(), 1, 2);
    [egui::Rect::from_min_max(area.min, egui::pos2(mid, area.bottom())), egui::Rect::from_min_max(egui::pos2(mid, area.top()), area.max)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Rect, pos2, vec2};

    fn screen() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1040.0))
    }

    #[test]
    fn no_windows_no_rects() {
        assert!(arrange_all(screen(), 0).is_empty());
    }

    #[test]
    fn one_window_fills_the_area() {
        assert_eq!(arrange_all(screen(), 1), vec![screen()]);
        let off = Rect::from_min_size(pos2(100.0, 50.0), vec2(1920.0, 1040.0));
        assert_eq!(arrange_all(off, 1), vec![off]);
    }

    #[test]
    fn two_windows_split_the_height() {
        let r = arrange_all(screen(), 2);
        assert_eq!(r, vec![Rect::from_min_max(pos2(0.0, 0.0), pos2(1920.0, 520.0)), Rect::from_min_max(pos2(0.0, 520.0), pos2(1920.0, 1040.0))]);
    }

    #[test]
    fn three_windows_share_edges_exactly() {
        let r = arrange_all(screen(), 3);
        let tops: Vec<f32> = r.iter().map(|r| r.top()).collect();
        let heights: Vec<f32> = r.iter().map(|r| r.height()).collect();
        assert_eq!(tops, vec![0.0, 347.0, 693.0]);
        assert_eq!(heights, vec![347.0, 346.0, 347.0]);
        assert!(r.iter().all(|r| r.left() == 0.0 && r.width() == 1920.0));
        for pair in r.windows(2) {
            assert_eq!(pair[0].bottom(), pair[1].top());
        }
    }

    #[test]
    fn an_offset_area_keeps_its_origin() {
        let off = Rect::from_min_size(pos2(100.0, 50.0), vec2(1920.0, 1040.0));
        let r = arrange_all(off, 3);
        let tops: Vec<f32> = r.iter().map(|r| r.top()).collect();
        assert_eq!(tops, vec![50.0, 397.0, 743.0]);
        assert_eq!(r.last().map(|r| r.bottom()), Some(1090.0));
        assert!(r.iter().all(|r| r.left() == 100.0 && r.right() == 2020.0));
        let [a, b] = side_by_side(off);
        assert_eq!((a.left(), a.right(), b.left(), b.right()), (100.0, 1060.0, 1060.0, 2020.0));
        assert!(a.top() == 50.0 && b.bottom() == 1090.0);
    }

    #[test]
    fn side_by_side_halves() {
        let [a, b] = side_by_side(screen());
        assert_eq!((a.left(), a.right(), b.left(), b.right()), (0.0, 960.0, 960.0, 1920.0));
        assert!(a.height() == 1040.0 && b.height() == 1040.0);
        let [a, b] = side_by_side(Rect::from_min_size(pos2(0.0, 0.0), vec2(1366.0, 768.0)));
        assert_eq!((a.width(), b.width()), (683.0, 683.0));
    }
}
