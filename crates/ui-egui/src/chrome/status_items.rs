//! Customize Status Bar: the items the status bar can show, which are shown, and each one's value.

use crate::WordApp;

/// An item of the status bar's Customize Status Bar list, in Word's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatusItem {
    FormattedPageNumber,
    Section,
    PageNumber,
    VerticalPagePosition,
    LineNumber,
    Column,
    WordCount,
    CharacterCount,
    Language,
    TrackChanges,
    CapsLock,
    Overtype,
    SelectionMode,
    ViewShortcuts,
    ZoomSlider,
    Zoom,
}

impl StatusItem {
    pub const ALL: [StatusItem; 16] = [
        StatusItem::FormattedPageNumber,
        StatusItem::Section,
        StatusItem::PageNumber,
        StatusItem::VerticalPagePosition,
        StatusItem::LineNumber,
        StatusItem::Column,
        StatusItem::WordCount,
        StatusItem::CharacterCount,
        StatusItem::Language,
        StatusItem::TrackChanges,
        StatusItem::CapsLock,
        StatusItem::Overtype,
        StatusItem::SelectionMode,
        StatusItem::ViewShortcuts,
        StatusItem::ZoomSlider,
        StatusItem::Zoom,
    ];

    /// The item's name in the list.
    pub fn label(self) -> &'static str {
        todo!("P0604")
    }

    /// Shown before the person changes anything.
    pub fn default_on(self) -> bool {
        todo!("P0604")
    }
}

/// The value the list shows beside `item` ("" when it has none).
pub fn status_item_value(_app: &WordApp, _item: StatusItem) -> String {
    todo!("P0604")
}

/// Is `item` shown on the status bar?
pub fn status_item_shown(_app: &WordApp, _item: StatusItem) -> bool {
    todo!("P0604")
}

/// Show `item` if hidden, hide it if shown.
pub fn toggle_status_item(_app: &mut WordApp, _item: StatusItem) {
    todo!("P0604")
}

#[cfg(test)]
mod tests;
