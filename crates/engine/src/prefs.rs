use serde::{Deserialize, Serialize};

/// Editing preferences that persist between runs (the front end saves and restores them).
/// File › Options sets most of them (`file.options`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    /// Word Count includes text boxes, footnotes and endnotes (Word's default).
    pub count_notes: bool,
    /// On-screen paper for pages without a document background; excluded from saved documents.
    #[serde(deserialize_with = "lenient_paper_color")]
    pub paper_color: crate::cmd::view::PaperColor,
    /// Track Changes Options: what markup shows and how revisions are drawn (per user, as in
    /// Word).
    pub markup: wordcraft_layout::display::MarkupOptions,
    /// File › Print / Export: Print Markup. PDFs, prints and page images show comments in
    /// balloons beside each page and tracked changes as marked up; HTML, Markdown and text
    /// copies end with the comments (`file.printMarkup`).
    pub print_markup: bool,
    /// File › Options › Display › Print hidden text (Word's default: off). Showing hidden text on
    /// screen does not print it.
    pub print_hidden_text: bool,
    /// File › Options › Advanced › Print field codes instead of their values (off).
    pub print_field_codes: bool,
    /// File › Options › Advanced › Print pages in reverse order (off).
    pub print_reverse: bool,
    /// File › Options › Display › Print document properties (off): a print of the document is
    /// followed by the Document Properties report on its own page.
    pub print_properties: bool,
    /// File › Options › Display › Print background colors and images (off): print jobs leave out
    /// the page colour and page picture unless it is on. Ordinary PDF export always draws them.
    pub print_background: bool,
    /// File › Options › Display › Print drawings created in Word (on): off, print jobs draw nothing
    /// for shapes, text boxes, WordArt and drawing canvases (their space stays).
    pub print_drawings: bool,
    /// File › Options › Advanced › Print on front of the sheet for duplex printing (off): a manual
    /// duplex job prints the fronts in reverse order.
    pub print_duplex_front: bool,
    /// File › Options › Advanced › Print on back of the sheet for duplex printing (off): a manual
    /// duplex job prints the backs in reverse order.
    pub print_duplex_back: bool,
    /// File › Options › Advanced › Save › Always create backup copy (off).
    pub always_create_backup: bool,
    /// Language › Detect language automatically (per user, as in Word).
    pub detect_language: bool,
    /// Set Proofing Language › Set As Default: overrides the interface language for future
    /// blank documents and plain text. Older preferences keep following the interface.
    #[serde(deserialize_with = "lenient_default_language")]
    pub default_language: Option<String>,
    /// Typing over a selection replaces it; off, the typed text goes in front of it.
    pub typing_replaces_selection: bool,
    /// The Insert key switches overtype on and off.
    pub insert_key_overtype: bool,
    /// Overtype: typed characters replace the ones after the caret.
    pub overtype: bool,
    /// The unit rulers and dialogs show lengths in, when the user chose one (File › Options ›
    /// Advanced › Measurement units); `None` follows the region ([`Session::units`]). Saved under
    /// a new key; an older `units` value other than inches (the old default) carries over on load.
    #[serde(rename = "measurementUnits")]
    pub units: Option<wordcraft_geom::Unit>,
    /// The region the user chose for new documents, units and the decimal separator (a locale
    /// tag like `pl-PL`, File › Options › Advanced › Region); `None` follows the system.
    pub region: Option<String>,
    /// A folder searched for Hunspell dictionaries before the system locations (File › Options ›
    /// Language, #1063): an app started from Finder or the Dock doesn't see
    /// `WORDCRAFT_DICTIONARIES`. `None` searches the system locations only.
    pub dictionaries_folder: Option<String>,
    /// The user templates folder File › New lists personal templates from and Save as Word
    /// Template starts in (File › Options › Advanced › File locations, #2852). `None`: the app's
    /// own `Templates` folder in its settings folder.
    pub user_templates_folder: Option<String>,
    /// Formatting marks shown even while Show/Hide ¶ is off.
    pub marks: wordcraft_layout::display::FormattingMarks,
    /// Mark grammar errors as you type (with Check spelling as you type on).
    pub mark_grammar: bool,
    /// Check spelling as you type (File › Options › Proofing). The live value is
    /// `ViewState::proofing`; this is the copy that is saved and restored (#1792).
    pub check_spelling: bool,
    /// Show readability statistics after a spelling check (File › Options › Proofing).
    pub show_readability: bool,
    /// Read Mode's Page Color. The live value is `ViewState::read_page_color`.
    #[serde(deserialize_with = "lenient_read_page_color")]
    pub read_page_color: crate::cmd::view::ReadPageColor,
    pub ignore_uppercase: bool,
    pub ignore_numbers: bool,
    pub ignore_internet: bool,
    /// AutoCorrect Options: replacements, capitalization and AutoFormat As You Type (per user, as
    /// in Word). A damaged saved value reads as the defaults.
    #[serde(deserialize_with = "crate::cmd::tools::lenient_autocorrect")]
    pub autocorrect: crate::cmd::tools::AutoCorrectPrefs,
    /// The Clipboard pane's Options (`edit.clipboardOptions`).
    pub clipboard: ClipboardPrefs,
    /// What the Insert Table dialog opens with (Remember dimensions for new tables).
    #[serde(deserialize_with = "lenient_insert_table")]
    pub insert_table: InsertTablePrefs,
    /// File › Options › Advanced › Display › Style area pane width in Draft and Outline views, in
    /// points (0: no style area, Word's default). An application setting, never saved in documents.
    #[serde(deserialize_with = "lenient_style_area_width")]
    pub style_area_width: f32,
    /// File › Options › Advanced › Font Substitution: the font each missing font is shown and
    /// printed in on this computer, in every document (`font.setSubstitute`). The documents keep
    /// naming the missing fonts.
    #[serde(deserialize_with = "lenient_font_substitutes")]
    pub font_substitutes: wordcraft_layout::FontSubstitutes,
}

/// Widest style area, in points (22 in).
pub const MAX_STYLE_AREA_WIDTH: f32 = 1584.0;

/// Insert Table's remembered size and AutoFit behavior (`insert.table` with `remember`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InsertTablePrefs {
    pub rows: u32,
    pub cols: u32,
    /// `fixed`, `contents` or `window`.
    pub autofit: String,
    /// Fixed column width in points; 0 is Auto.
    pub col_width: f32,
    /// Remember dimensions for new tables is on.
    pub remember: bool,
}

impl Default for InsertTablePrefs {
    fn default() -> Self {
        InsertTablePrefs { rows: 2, cols: 5, autofit: "fixed".into(), col_width: 0.0, remember: false }
    }
}

/// The Clipboard pane's Options (Home › Clipboard › Clipboard Pane › Options), as in Word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClipboardPrefs {
    /// Open the pane when a copy or cut makes it hold a second item.
    pub show_automatically: bool,
    /// Open the pane when the same selection is copied twice in a row.
    pub show_on_copy_twice: bool,
    /// Collect copies and cuts while the pane is hidden.
    pub collect_without_showing: bool,
    /// Report "Item N of 24" when a copy or cut is collected.
    pub show_status: bool,
}

impl Default for ClipboardPrefs {
    fn default() -> Self {
        ClipboardPrefs { show_automatically: false, show_on_copy_twice: false, collect_without_showing: true, show_status: true }
    }
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            paper_color: Default::default(),
            count_notes: true,
            markup: Default::default(),
            print_markup: false,
            print_hidden_text: false,
            print_field_codes: false,
            print_reverse: false,
            print_properties: false,
            print_background: false,
            print_drawings: true,
            print_duplex_front: false,
            print_duplex_back: false,
            always_create_backup: false,
            typing_replaces_selection: true,
            insert_key_overtype: false,
            overtype: false,
            units: Default::default(),
            region: None,
            dictionaries_folder: None,
            user_templates_folder: None,
            marks: Default::default(),
            mark_grammar: true,
            check_spelling: true,
            show_readability: false,
            read_page_color: Default::default(),
            ignore_uppercase: true,
            ignore_numbers: true,
            ignore_internet: true,
            detect_language: true,
            default_language: None,
            autocorrect: Default::default(),
            clipboard: Default::default(),
            insert_table: Default::default(),
            style_area_width: 0.0,
            font_substitutes: Default::default(),
        }
    }
}

impl Prefs {
    /// What the spelling check skips.
    pub fn spelling(&self) -> wordcraft_proof::Options {
        wordcraft_proof::Options {
            ignore_uppercase: self.ignore_uppercase,
            ignore_numbers: self.ignore_numbers,
            ignore_internet: self.ignore_internet,
        }
    }
}

/// Read saved font substitutes ([`Prefs::font_substitutes`]) leniently: pairs that aren't two
/// strings are skipped, and anything else reads as none, without discarding the other
/// preferences.
fn lenient_font_substitutes<'de, D: serde::Deserializer<'de>>(d: D) -> Result<wordcraft_layout::FontSubstitutes, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    let pairs = value.as_array().into_iter().flatten().filter_map(|p| match p.as_array().map(Vec::as_slice) {
        Some([font, family]) => Some((font.as_str()?.to_string(), family.as_str()?.to_string())),
        _ => None,
    });
    Ok(wordcraft_layout::FontSubstitutes::from_pairs(pairs))
}

/// Ignore malformed saved language tags without discarding the other preferences.
fn lenient_default_language<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(value.as_str().and_then(wordcraft_proof::lang::normalize_tag))
}

fn lenient_paper_color<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::cmd::view::PaperColor, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// Ignore an invalid saved Page Color without discarding other preferences.
fn lenient_read_page_color<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::cmd::view::ReadPageColor, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// Ignore a damaged remembered Insert Table size without discarding other preferences.
fn lenient_insert_table<'de, D: serde::Deserializer<'de>>(d: D) -> Result<InsertTablePrefs, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// A damaged or out-of-range saved style area width loads as no style area.
fn lenient_style_area_width<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(value.as_f64().map(|w| w as f32).filter(|w| w.is_finite() && (0.0..=MAX_STYLE_AREA_WIDTH).contains(w)).unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use super::Prefs;
    use crate::cmd::view::ReadPageColor;

    #[test]
    fn read_page_color_loads_from_old_and_damaged_prefs() {
        let old: Prefs = serde_json::from_str(r#"{"checkSpelling":false}"#).unwrap();
        assert_eq!(old.read_page_color, ReadPageColor::None);
        assert!(!old.check_spelling);
        for bad in [r#"{"readPageColor":"unknown","checkSpelling":false}"#, r#"{"readPageColor":42,"checkSpelling":false}"#] {
            let prefs: Prefs = serde_json::from_str(bad).unwrap();
            assert_eq!(prefs.read_page_color, ReadPageColor::None);
            assert!(!prefs.check_spelling);
        }
        let saved: Prefs = serde_json::from_str(r#"{"readPageColor":"sepia"}"#).unwrap();
        assert_eq!(saved.read_page_color, ReadPageColor::Sepia);
        assert_eq!(serde_json::to_value(saved).unwrap()["readPageColor"], "sepia");
    }
}
