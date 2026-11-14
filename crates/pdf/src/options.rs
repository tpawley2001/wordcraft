use super::{NumeralMode, WriteError, imposition};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PdfError {
    #[error("the document has no pages to export")]
    NoPages,
    #[error("page {0} does not exist")]
    BadPage(usize),
    #[error("PDF writing failed: {0}")]
    Write(String),
}

impl From<WriteError> for PdfError {
    fn from(error: WriteError) -> Self {
        Self::Write(error.to_string())
    }
}

/// Export options.
#[derive(Clone, Debug)]
pub struct PdfOptions {
    /// Physical sheet layout used only by printing.
    pub imposition: imposition::Imposition,
    /// Repeat complete sheet plans (padding never spills into another copy).
    pub sheet_copies: u32,
    /// Collate physical sheet sides when repeating a print job.
    pub sheet_collate: bool,
    /// Pages to export (0-based layout page indexes, in output order); `None` = all.
    pub pages: Option<Vec<usize>>,
    /// Title / author overrides (default: the document properties).
    pub title: Option<String>,
    pub author: Option<String>,
    /// Write a structure tree (tagged PDF, best effort).
    pub tagged: bool,
    /// Show tracked changes as markup (coloured, underlined / struck through).
    pub include_markup: bool,
    /// Print Markup's comments: each page is scaled down to make room on its right for the
    /// comments in balloons joined to their anchors (Word's "Document showing markup").
    pub comments: bool,
    /// Compress content streams.
    pub compress: bool,
    /// Write hyperlinks and in-document links as link annotations. A print job leaves them out:
    /// they print nothing, and a printer driver can't fit a page with annotations to the paper
    /// (#4734).
    pub links: bool,
    /// Print on this paper (width, height in points) instead of each page's own size: the page
    /// is scaled to fit and centred, the paper turned to the page's orientation (Scale to paper
    /// size, #2108). `None` keeps the document's page sizes.
    pub paper: Option<(f32, f32)>,
    /// How [`export`] shows digits (the Numeral setting; callers resolve `System`, see
    /// [`NumeralMode::resolve`]). [`export_layout`] keeps the layout it is given.
    pub numeral: NumeralMode,
    /// Draw only what was filled into the legacy form fields, where the full page has it (Print
    /// only the data from a form, `w:printFormsData`): no other text, page colour, watermark,
    /// borders, shading or pictures. Print jobs only. Applies to the document's own pages: the
    /// appendix of [`crate::export_layout_with_appendix`] is drawn in full.
    pub forms_data_only: bool,
    /// Draw the page background (page colour and page picture). Print jobs follow File › Options ›
    /// Display › Print background colors and images; export keeps it on (the default).
    pub background: bool,
    /// Draw shapes, text boxes, WordArt and drawing canvases (File › Options › Display › Print
    /// drawings created in Word). On by default.
    pub drawings: bool,
}

/// How many characters of the watermark text the PDF draws.
pub const WATERMARK_MAX_CHARS: usize = 200;

impl PdfOptions {
    /// What a page shows when drawn into the PDF: no marks or dimming, tracked changes as markup
    /// only with `include_markup`. The one place that says so; strict export (#6005) checks the
    /// same display list.
    pub fn display_options(&self) -> wordcraft_layout::display::DisplayOptions {
        wordcraft_layout::display::DisplayOptions {
            marks: false,
            dim_header: false,
            dim_body: false,
            markup: self.include_markup,
            placeholders: false,
            forms_data_only: self.forms_data_only,
            hide_drawings: !self.drawings,
            ..Default::default()
        }
    }

    /// How [`crate::export`] lays a document out for these options.
    pub fn layout_options(&self) -> wordcraft_layout::LayoutOptions {
        wordcraft_layout::LayoutOptions {
            hide_deleted: !self.include_markup,
            comment_balloons: self.comments,
            numeral: self.numeral,
            ..Default::default()
        }
    }
}

impl Default for PdfOptions {
    /// Keep ordinary document export unchanged unless a print layout is requested.
    fn default() -> Self {
        PdfOptions {
            imposition: Default::default(),
            sheet_copies: 1,
            sheet_collate: true,
            pages: None,
            title: None,
            author: None,
            tagged: true,
            include_markup: false,
            comments: false,
            compress: true,
            links: true,
            paper: None,
            numeral: NumeralMode::Arabic,
            forms_data_only: false,
            background: true,
            drawings: true,
        }
    }
}
