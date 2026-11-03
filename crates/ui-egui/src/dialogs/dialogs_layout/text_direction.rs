//! Text Direction dialog (Layout › Text Direction › Text Direction Options…, the table context menu's
//! Text Direction…): Word's Orientation tiles and Preview for the cell, text box or main document.

use wordcraft_doc::props::TextDirection;

use crate::WordApp;

/// What the dialog turns, from where the caret is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDirectionTarget {
    MainDocument,
    TableCell,
    TextBox,
}

#[derive(Clone, Debug)]
pub struct TextDirectionForm {
    pub target: TextDirectionTarget,
    pub value: TextDirection,
}

impl TextDirectionForm {
    pub fn new(_app: &WordApp) -> Self {
        // Stub: the task fills this in.
        Self { target: TextDirectionTarget::MainDocument, value: TextDirection::Horizontal }
    }

    pub fn title(&self) -> &'static str {
        ""
    }

    pub fn choices(&self) -> [TextDirection; 3] {
        [TextDirection::Horizontal; 3]
    }

    pub fn ok(self, _app: &mut WordApp) -> Result<(), String> {
        Err("not implemented".to_owned())
    }
}

#[cfg(test)]
mod tests;
