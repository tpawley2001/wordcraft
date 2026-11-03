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
        todo!("TextDirectionForm::new")
    }

    pub fn title(&self) -> &'static str {
        todo!("TextDirectionForm::title")
    }

    pub fn choices(&self) -> [TextDirection; 3] {
        todo!("TextDirectionForm::choices")
    }

    pub fn ok(self, _app: &mut WordApp) -> Result<(), String> {
        todo!("TextDirectionForm::ok")
    }
}

#[cfg(test)]
mod tests;
