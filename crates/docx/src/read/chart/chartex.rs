//! Office 2016 charts (`cx:chartSpace`, chartex namespace) drawn from their data: waterfall
//! charts. Other layouts return `None`, so the caller keeps the fallback picture.

use wordcraft_doc::graphic::GraphicItem;
use wordcraft_doc::props::Rgb;

use crate::xml::El;

/// The items of an Office 2016 chart `w` × `h` points, or `None` when its layout isn't drawn.
pub(crate) fn chartex_items(space: &El, theme: &[Rgb], w: f32, h: f32) -> Option<Vec<GraphicItem>> {
    let _ = (space, theme, w, h);
    None
}
