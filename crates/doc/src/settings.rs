use super::{Deserialize, Rgb, Serialize, math, section, styles};

/// A bibliography source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Source {
    /// Unique tag cited by CITATION fields.
    pub tag: String,
    /// book, bookSection, article (journal), periodical, conference, report, website, film, other
    pub kind: String,
    /// "Last, First; Last, First"
    pub author: String,
    /// A corporate author ("World Health Organization"): written as it is, never split into
    /// names, and in place of `author` when set (Create Source › Corporate Author, `b:Corporate`,
    /// #4848).
    pub corporate: String,
    pub title: String,
    pub year: String,
    pub publisher: String,
    pub city: String,
    pub journal: String,
    pub volume: String,
    pub pages: String,
    pub url: String,
    /// More bibliography fields (Create Source › Show All Bibliography Fields).
    pub editor: String,
    pub edition: String,
    pub issue: String,
    pub month: String,
    pub day: String,
    /// Date accessed (websites).
    pub accessed: String,
    pub doi: String,
}

/// A document variable (`w:docVar`, ECMA-376 Part 1 §17.15.1.31; RTF `{\\*\\docvar {name}{value}}`):
/// a named string macros and add-ins set and a `DOCVARIABLE name` field prints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct DocVar {
    pub name: String,
    pub value: String,
}

/// A custom document property (File › Info › Properties › Custom). Citation managers keep
/// their per-document preferences here (Zotero: `ZOTERO_PREF_1`, `ZOTERO_PREF_2`…).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomProp {
    pub name: String,
    /// OOXML variant type: `lpwstr`, `i4`, `r8`, `bool`, `filetime`…, or `raw` when `value` is
    /// the property's XML content kept verbatim (vectors, blobs).
    pub kind: String,
    pub value: String,
    /// Bookmark associated with this property (`linkTarget`), retained without evaluating it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
    /// Declared in an ODF document as a user field (§7.4.7 `text:user-field-decl`) rather than
    /// a document property: written back as one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub odf_user_field: bool,
}

/// An HTML div from the web settings (`w:div`, ECMA-376 §17.15.2.8), as documents saved from
/// HTML carry them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WebDiv {
    /// `w:id`, which `w:divId` refers to.
    pub id: String,
    /// `w:bodyDiv`: the div stands for the HTML body.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub body_div: bool,
    /// `w:marLeft`, `w:marRight`, `w:marTop`, `w:marBottom`, points.
    pub mar_left: f32,
    pub mar_right: f32,
    pub mar_top: f32,
    pub mar_bottom: f32,
    /// `w:divBdr`, kept for saving; not drawn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<crate::props::Borders>,
    /// Divs nested in this one (`w:divsChild`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<WebDiv>,
}

/// The deepest div nesting kept and searched.
pub const MAX_DIV_DEPTH: usize = 32;

/// The (left, right) margins, points, that the div `id` and the divs it is nested in add to the
/// indents of what names it; `None` when no div has that id.
pub fn div_margins(divs: &[WebDiv], id: &str) -> Option<(f32, f32)> {
    fn find(divs: &[WebDiv], id: &str, depth: usize) -> Option<(f32, f32)> {
        if depth > MAX_DIV_DEPTH {
            return None;
        }
        divs.iter().find_map(|d| {
            let own = (d.mar_left, d.mar_right);
            if d.id == id { Some(own) } else { find(&d.children, id, depth + 1).map(|(l, r)| (l + own.0, r + own.1)) }
        })
    }
    find(divs, id.trim(), 0).filter(|(l, r)| l.is_finite() && r.is_finite())
}

/// Document properties (File › Info).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CoreProps {
    pub title: String,
    pub subject: String,
    pub creator: String,
    pub keywords: String,
    pub description: String,
    pub category: String,
    pub last_modified_by: String,
    pub created: String,
    pub modified: String,
    pub revision: u32,
    /// Status (`cp:contentStatus`), e.g. `Final` once the document is marked as final.
    pub content_status: String,
    /// Last printed (`cp:lastPrinted`), a W3CDTF date; kept as read.
    pub last_printed: String,
    /// `dc:language`, kept as read.
    pub language: String,
    /// `cp:version`, kept as read.
    pub version: String,
    /// `dc:identifier`, kept as read.
    pub identifier: String,
    /// `cp:contentType`, kept as read.
    pub content_type: String,
}

/// The Styles pane's options (Style Pane Options…), as the settings part keeps them
/// (`w:stylePaneFormatFilter`, ECMA-376 Part 1 §17.15.1.85; `w:stylePaneSortMethod`, §17.15.1.86).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StylePaneOptions {
    /// The format filter's 16 bits, as read (`w:val`, 4 hex digits). Only the `PANE_*` bits are
    /// interpreted; every other bit is written back unchanged.
    pub filter: u16,
    /// The file wrote the Word 2010 attribute form (`w:allStyles="1"` …) next to `w:val`.
    pub filter_attrs: bool,
    /// `None` when the file had no `w:stylePaneSortMethod`.
    pub sort: Option<StylePaneSort>,
}

/// Format filter bit: all styles (`w:allStyles`).
pub const PANE_ALL_STYLES: u16 = 0x0001;
/// Format filter bit: styles in use (`w:stylesInUse`).
pub const PANE_IN_USE: u16 = 0x0008;

/// How the Styles pane sorts its list (ST_StyleSort, ECMA-376 Part 1 §17.18.86).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StylePaneSort {
    Name,
    Priority,
    Default,
    Font,
    BasedOn,
    Type,
}

impl StylePaneSort {
    /// Every value, in ST_StyleSort's numeric order (`0000`…`0005`).
    pub const ALL: [StylePaneSort; 6] = [Self::Name, Self::Priority, Self::Default, Self::Font, Self::BasedOn, Self::Type];

    /// The value's number (transitional files write it as 4 hex digits).
    pub fn code(self) -> u16 {
        match self {
            Self::Name => 0,
            Self::Priority => 1,
            Self::Default => 2,
            Self::Font => 3,
            Self::BasedOn => 4,
            Self::Type => 5,
        }
    }

    /// The strict schema's name for the value.
    pub fn name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Priority => "priority",
            Self::Default => "default",
            Self::Font => "font",
            Self::BasedOn => "basedOn",
            Self::Type => "type",
        }
    }

    /// A `w:val` as either form writes it (`0003` or `font`); `None` when unknown.
    pub fn parse(v: &str) -> Option<Self> {
        let v = v.trim();
        if let Some(s) = Self::ALL.into_iter().find(|s| s.name() == v) {
            return Some(s);
        }
        if v.is_empty() || v.len() > 4 {
            return None;
        }
        let n = u16::from_str_radix(v, 16).ok()?;
        Self::ALL.into_iter().find(|s| s.code() == n)
    }
}

/// Read-only recommended and Password to modify (`w:writeProtection`, ECMA-376 Part 1
/// §17.15.1.93). Not encryption and not an editing restriction: a document opened read-only can
/// be edited, but Save can't overwrite its file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WriteProtection {
    /// `w:recommended="1"`: Word asks whether to open the document read-only.
    pub recommended: bool,
    /// The password to modify's hash, as the element's attributes (`w:algorithmName`,
    /// `w:hashValue`, `w:saltValue`, `w:spinCount`, or the older `w:cryptAlgorithmSid`,
    /// `w:hash`, `w:salt`…). Empty: no password.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hash: Vec<(String, String)>,
}

impl WriteProtection {
    /// Whether the element says anything: a recommendation or a password.
    pub fn is_empty(&self) -> bool {
        !self.recommended && self.hash.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub track_changes: bool,
    /// Record a tracked cut and paste of a sentence or paragraph as a move (Track moves); off
    /// when the file has `w:doNotTrackMoves` (ECMA-376 §17.15.1.29), and then it is an
    /// ordinary deletion and insertion.
    pub track_moves: bool,
    /// Default tab stop interval, points.
    pub default_tab: f32,
    pub even_odd_headers: bool,
    pub mirror_margins: bool,
    /// Print only the data from a form (`w:printFormsData`, File › Options › Advanced): printing
    /// puts only what was filled into the legacy form fields on the paper, where the full page
    /// has it (for pre-printed forms). PDF export is unaffected.
    pub print_forms_data: bool,
    pub auto_hyphenation: bool,
    /// Hyphenation zone (`w:hyphenationZone`), points: a line is hyphenated only when breaking
    /// it at a word boundary would leave more than this empty at its end.
    pub hyphenation_zone: f32,
    /// Words in capitals may be hyphenated (`w:doNotHyphenateCaps` turns it off).
    pub hyphenate_caps: bool,
    /// Most consecutive lines that may end with a hyphen (`w:consecutiveHyphenLimit`); 0 = no limit.
    pub consecutive_hyphen_limit: u32,
    pub page_color: Option<Rgb>,
    /// Picture or texture page background (Design › Page Color › Fill Effects), drawn over the colour.
    pub page_picture: Option<PagePicture>,
    pub watermark: Option<Watermark>,
    /// Theme fonts (major = headings, minor = body).
    pub major_font: String,
    pub minor_font: String,
    /// Theme colours: dk1 lt1 dk2 lt2 accent1..6 hlink folHlink.
    pub theme_colors: Vec<Rgb>,
    pub theme_name: String,
    pub footnote_format: section::NumFormat,
    pub endnote_format: section::NumFormat,
    /// The document's footnote and endnote placement, start and restart (`w:footnotePr` /
    /// `w:endnotePr` in the settings); sections may override them.
    #[serde(skip_serializing_if = "section::NoteProps::is_empty")]
    pub footnote_pr: section::NoteProps,
    #[serde(skip_serializing_if = "section::NoteProps::is_empty")]
    pub endnote_pr: section::NoteProps,
    /// Restrict Editing, when enforced: `readOnly`, `comments`, `trackedChanges`, `forms`, or
    /// `none` when only formatting is restricted (`w:documentProtection w:edit`).
    pub protection: Option<String>,
    /// Restrict Editing limits formatting to the styles that aren't locked, and forbids direct
    /// formatting (`w:formatting="1"`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub protect_formatting: bool,
    /// The protection password's hash, as the file's `w:documentProtection` attributes
    /// (`w:algorithmName`, `w:hashValue`, `w:saltValue`, `w:spinCount`, or the older
    /// `w:cryptAlgorithmSid`, `w:hash`, `w:salt`…). Empty: no password.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub protect_hash: Vec<(String, String)>,
    /// Read-only recommended and Password to modify (`w:writeProtection`, §17.15.1.93).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_protection: Option<WriteProtection>,
    /// Word compatibility mode the document is laid out in (`compatibilityMode`): 15 for Word
    /// 2013 and later, which places a table's border at the margin rather than its text and lets
    /// justified lines shrink their spaces to fit more text.
    pub compat_mode: u32,
    /// Fonts the document's font table classes as roman (serif, `w:family w:val="roman"`): Word
    /// draws one that isn't installed in a serif face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roman_fonts: Vec<String>,
    /// HTML divs (`w:divs` in the web settings part, ECMA-376 §17.15.2.8): paragraphs and table
    /// rows name one with `div_id`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub web_divs: Vec<WebDiv>,
    /// `w:doNotUseHTMLParagraphAutoSpacing`: the space between two paragraphs is the first's space
    /// after plus the second's space before, not the larger of the two.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sum_paragraph_spacing: bool,
    /// `w:doNotExpandShiftReturn`: a justified line that ends in a manual line break keeps its
    /// natural spacing; by default Word stretches it to the margin like a wrapped line.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub do_not_expand_shift_return: bool,
    /// A document opened from RTF: Word fits a justified line's last word by squeezing the line's
    /// spaces by up to half their width (see the layout crate's `para::justify`). Not saved.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rtf_justification: bool,
    /// `w:wrapTrailSpaces` / RTF `\wraptrsp` (Wrap trailing spaces onto the next line). In a
    /// document opened from RTF it makes Word stricter about squeezing a justified line (see the
    /// layout crate's `para::justify`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrap_trail_spaces: bool,
    /// `Copts80.f2ptExtLeadingOnly` of a Word 97-2003 file: Word adds at most 2 pt of external
    /// leading; a Courier New line is one em tall. Not saved.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ext_leading_2pt_only: bool,
    /// Keep contextual paragraph spacing inside legacy table cells (`allowSpaceOfSameStyleInTable`).
    /// Word ignores the layout override in compatibility mode 15, but the flag still round-trips.
    pub allow_space_of_same_style_in_table: bool,
    /// Drawing grid spacing in points, across and down (View › Gridlines); ECMA-376's default is 1/8 inch.
    pub grid_h: f32,
    pub grid_v: f32,
    /// `w:displayHorizontalDrawingGridEvery`: draw a vertical gridline every N grid units across;
    /// 0 draws none. Absent in a file means 1.
    #[serde(default = "one_u8")]
    pub grid_every_h: u8,
    /// `w:displayVerticalDrawingGridEvery`: draw a horizontal gridline every N grid units down;
    /// 0 draws none. Absent in a file means 1.
    #[serde(default = "one_u8")]
    pub grid_every_v: u8,
    /// `w:doNotUseMarginsForDrawingGridOrigin` is off: the grid starts at the page margins.
    #[serde(default = "yes")]
    pub grid_use_margins: bool,
    /// `w:drawingGridHorizontalOrigin`: the grid's origin from the page's left edge, in points,
    /// when it doesn't use the margins.
    #[serde(default)]
    pub grid_origin_x: f32,
    /// `w:drawingGridVerticalOrigin`: the grid's origin from the page's top edge, in points.
    #[serde(default)]
    pub grid_origin_y: f32,
    /// Equation options (`m:mathPr`), when the document has them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub math: Option<math::MathProps>,
    /// Citation and bibliography style (References › Style) by name, when the document
    /// records one (OOXML's `b:Sources/@StyleName`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bib_style: Option<String>,
    /// The template the document is attached to (Tools › Templates and Add-ins): the target of
    /// `w:attachedTemplate`'s relationship, a path or `file:` URL as the file gives it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_template: Option<String>,
    /// Update the document's styles from the attached template when it is opened (`w:linkStyles`).
    pub link_styles: bool,
    /// Styles pane options (Style Pane Options…): `w:stylePaneFormatFilter` / `w:stylePaneSortMethod`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_pane: Option<StylePaneOptions>,
    /// The zoom the document was saved at (`w:zoom`), restored when it is opened; `None` for 100%
    /// without a fit mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<Zoom>,
    /// Remove personal information when saving (`w:removePersonalInformation`, ECMA-376
    /// §17.15.1.70): the saved file names comment and revision authors "Author" and leaves out
    /// the document's creator and last-modified-by. The open document keeps its real values.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub remove_personal_info: bool,
    /// Remove date and time from annotations when saving (`w:removeDateAndTime`, §17.15.1.69):
    /// comments and tracked changes are saved without `w:date`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub remove_date_time: bool,
    /// The document was opened from ODF: the standard's rules that differ from Word's apply as
    /// it is laid out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub odf: Option<OdfLayout>,
    /// Caption labels of the document's own and AutoCaption (`w:captions`, ECMA-376 §17.15.1.18).
    #[serde(skip_serializing_if = "Captions::is_empty")]
    pub captions: Captions,
    /// Document variables (`w:docVars`, ECMA-376 §17.15.1.32), in file order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub doc_vars: Vec<DocVar>,
}

/// Most caption labels, and most AutoCaption entries, a document keeps.
pub const MAX_CAPTION_ENTRIES: usize = 256;

/// The caption labels every document has without a `w:caption` entry.
pub const BUILTIN_CAPTION_LABELS: [&str; 3] = ["Figure", "Table", "Equation"];

/// Caption settings (References › Insert Caption): labels made with New Label or whose options
/// differ from the defaults (`w:caption`), and the objects that get a caption as they are
/// inserted (`w:autoCaption`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Captions {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<CaptionLabel>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<AutoCaption>,
}

impl Captions {
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty() && self.auto.is_empty()
    }

    /// The label named `name` (any case, as Word matches labels).
    pub fn label(&self, name: &str) -> Option<&CaptionLabel> {
        self.labels.iter().find(|l| l.name.eq_ignore_ascii_case(name))
    }

    /// The label AutoCaption adds for objects of type `object`.
    pub fn auto_label(&self, object: &str) -> Option<&str> {
        self.auto.iter().find(|a| a.object == object).map(|a| a.label.as_str())
    }
}

/// One caption label and its options (`w:caption`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CaptionLabel {
    pub name: String,
    /// `w:pos`: the caption goes above the item (below by default).
    pub above: bool,
    /// `w:numFmt`: `decimal`, `upperLetter`, `lowerLetter`, `upperRoman` or `lowerRoman`.
    pub num_fmt: String,
    /// `w:chapNum`: the number includes the chapter number.
    pub chapter: bool,
    /// `w:heading`: the heading level (1-9) chapters start at; 0 when unset.
    pub heading: u8,
    /// `w:sep`: between the chapter number and the caption number.
    pub separator: CaptionSep,
    /// `w:noLabel`: the caption is the number without the label.
    pub no_label: bool,
}

impl Default for CaptionLabel {
    fn default() -> Self {
        CaptionLabel {
            name: String::new(),
            above: false,
            num_fmt: "decimal".into(),
            chapter: false,
            heading: 0,
            separator: CaptionSep::Hyphen,
            no_label: false,
        }
    }
}

impl CaptionLabel {
    pub fn new(name: &str) -> Self {
        CaptionLabel { name: name.to_string(), ..Default::default() }
    }

    /// Whether every option is the default, so a built-in label needs no `w:caption` entry.
    pub fn is_default(&self) -> bool {
        *self == CaptionLabel::new(&self.name)
    }
}

/// The caption numbering formats a label can have (ST_NumberFormat's values Word offers).
pub const CAPTION_NUM_FORMATS: [&str; 5] = ["decimal", "upperLetter", "lowerLetter", "upperRoman", "lowerRoman"];

/// `w:sep` (ST_ChapterSep).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptionSep {
    #[default]
    Hyphen,
    Period,
    Colon,
    EmDash,
    EnDash,
}

impl CaptionSep {
    pub const ALL: [CaptionSep; 5] = [CaptionSep::Hyphen, CaptionSep::Period, CaptionSep::Colon, CaptionSep::EmDash, CaptionSep::EnDash];

    /// The attribute value.
    pub fn ooxml(self) -> &'static str {
        match self {
            CaptionSep::Hyphen => "hyphen",
            CaptionSep::Period => "period",
            CaptionSep::Colon => "colon",
            CaptionSep::EmDash => "emDash",
            CaptionSep::EnDash => "enDash",
        }
    }

    pub fn from_ooxml(v: &str) -> Option<CaptionSep> {
        CaptionSep::ALL.into_iter().find(|s| s.ooxml() == v)
    }

    /// The character it stands for.
    pub fn text(self) -> &'static str {
        match self {
            CaptionSep::Hyphen => "-",
            CaptionSep::Period => ".",
            CaptionSep::Colon => ":",
            CaptionSep::EmDash => "\u{2014}",
            CaptionSep::EnDash => "\u{2013}",
        }
    }
}

/// AutoCaption for one kind of object (`w:autoCaption`): inserting one adds a caption with `label`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AutoCaption {
    /// The object type, as Word names it (`Microsoft Word Table`).
    pub object: String,
    pub label: String,
}

/// A document opened from ODF, for the layout: rules of the standard that differ from Word's (a
/// paragraph's padding takes room on a side without a border, XSL's box model) apply, and what
/// the file says about laying it out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OdfLayout {
    /// Tab stops are measured from the paragraph's left indent, default ones too (LibreOffice's
    /// `TabsRelativeToIndent` in the file; from the margin when it doesn't say). The document's
    /// explicit stops are held from the margin either way; the default ones are counted from the
    /// indent.
    pub tabs_from_indent: bool,
}

/// A saved zoom: a percentage, or a fit mode (the percentage it last came to).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zoom {
    /// 10–500.
    pub percent: u32,
    pub fit: Option<ZoomFit>,
}

/// View › Zoom's fit modes a document can be saved in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoomFit {
    OnePage,
    PageWidth,
    TextWidth,
}

/// Word's compatibility mode for documents that don't state one.
pub const LEGACY_COMPAT_MODE: u32 = 12;

/// The compatibility mode Word lays out Word 97-2003 binary (`.doc`) files in.
pub const WORD2003_COMPAT_MODE: u32 = 11;

/// The compatibility mode of documents created by Word 2013 and later, and by WordCraft.
pub const COMPAT_MODE_CURRENT: u32 = 15;

/// Word's hyphenation zone when a document doesn't set one (0.25"), points.
pub const DEFAULT_HYPHENATION_ZONE: f32 = 18.0;

/// ECMA-376 §17.15.1.44/45: 180 twentieths of a point when a document doesn't say.
pub const DEFAULT_GRID: f32 = 9.0;

fn one_u8() -> u8 {
    1
}

fn yes() -> bool {
    true
}

/// A picture fill for the page background (`v:background/v:fill` with an image).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PagePicture {
    /// Key into [`Document::media`].
    pub media: String,
    /// `v:fill/@type` (`tile`, `frame`, `tile`...); tiled when `tile`, stretched to the page otherwise.
    pub fill_type: String,
    /// `o:title` of the fill (texture name), kept for round trip.
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Watermark {
    pub text: String,
    pub font: String,
    pub color: Rgb,
    pub diagonal: bool,
    pub semitransparent: bool,
    /// The shape's box (its `style` width and height, points), which the text is stretched to
    /// fill (`v:textpath fitshape`); `None`: the box Word's Design › Watermark gives
    /// ([`Watermark::frame`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<[f32; 2]>,
}

impl Default for Watermark {
    fn default() -> Self {
        Watermark {
            text: "DRAFT".into(),
            font: styles::BODY_FONT.into(),
            color: Rgb(0xC0, 0xC0, 0xC0),
            diagonal: true,
            semitransparent: true,
            size: None,
        }
    }
}

impl Watermark {
    /// The box the text fills on a page `page_w` × `page_h` (points): its own size, else half the
    /// page diagonal across (half the page width when not diagonal), as high as the text's length
    /// makes it. Centred on the margins and, when diagonal, turned 45° counterclockwise (Word's
    /// `rotation:315`).
    pub fn frame(&self, page_w: f32, page_h: f32) -> [f32; 2] {
        if let Some([w, h]) = self.size.filter(|[w, h]| w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0) {
            return [w.min(4000.0), h.min(4000.0)];
        }
        let (pw, ph) = (wordcraft_geom::finite(page_w).clamp(72.0, 4000.0), wordcraft_geom::finite(page_h).clamp(72.0, 4000.0));
        let across = if self.diagonal { (pw * pw + ph * ph).sqrt() } else { pw };
        let width = (across * 0.5).clamp(36.0, 2000.0);
        let chars = self.text.chars().take(200).count().max(1) as f32;
        [width, (width / (0.55 * chars)).clamp(12.0, 400.0)]
    }
}

/// Our default theme colours.
pub const THEME_COLORS: [Rgb; 12] = [
    Rgb(0x00, 0x00, 0x00),
    Rgb(0xFF, 0xFF, 0xFF),
    Rgb(0x0E, 0x28, 0x41),
    Rgb(0xE8, 0xE8, 0xE8),
    Rgb(0x15, 0x60, 0x82),
    Rgb(0xE9, 0x71, 0x32),
    Rgb(0x19, 0x6B, 0x24),
    Rgb(0x0F, 0x9E, 0xD5),
    Rgb(0xA0, 0x2B, 0x93),
    Rgb(0x4E, 0xA7, 0x2E),
    Rgb(0x46, 0x78, 0x86),
    Rgb(0x96, 0x60, 0x7D),
];

impl Default for Settings {
    fn default() -> Self {
        Settings {
            track_changes: false,
            track_moves: true,
            default_tab: 36.0,
            even_odd_headers: false,
            mirror_margins: false,
            print_forms_data: false,
            auto_hyphenation: false,
            hyphenation_zone: DEFAULT_HYPHENATION_ZONE,
            hyphenate_caps: true,
            consecutive_hyphen_limit: 0,
            page_color: None,
            page_picture: None,
            watermark: None,
            major_font: styles::HEADING_FONT.into(),
            minor_font: styles::BODY_FONT.into(),
            theme_colors: THEME_COLORS.to_vec(),
            theme_name: "Craft".into(),
            footnote_format: section::NumFormat::Decimal,
            endnote_format: section::NumFormat::LowerRoman,
            footnote_pr: section::NoteProps::default(),
            endnote_pr: section::NoteProps::default(),
            protection: None,
            protect_formatting: false,
            protect_hash: Vec::new(),
            write_protection: None,
            compat_mode: COMPAT_MODE_CURRENT,
            roman_fonts: Vec::new(),
            web_divs: Vec::new(),
            sum_paragraph_spacing: false,
            do_not_expand_shift_return: false,
            rtf_justification: false,
            wrap_trail_spaces: false,
            ext_leading_2pt_only: false,
            allow_space_of_same_style_in_table: false,
            grid_h: DEFAULT_GRID,
            grid_v: DEFAULT_GRID,
            // Word's blank document: a horizontal gridline every 2 units (0.25 in), none vertical.
            grid_every_h: 0,
            grid_every_v: 2,
            grid_use_margins: true,
            grid_origin_x: 0.0,
            grid_origin_y: 0.0,
            math: None,
            bib_style: None,
            attached_template: None,
            link_styles: false,
            style_pane: None,
            zoom: None,
            remove_personal_info: false,
            remove_date_time: false,
            odf: None,
            captions: Captions::default(),
            doc_vars: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StylePaneSort;

    #[test]
    fn style_pane_sort_reads_both_forms() {
        assert_eq!(StylePaneSort::parse("0003"), Some(StylePaneSort::Font));
        assert_eq!(StylePaneSort::parse("basedOn"), Some(StylePaneSort::BasedOn));
        assert_eq!(StylePaneSort::parse("5"), Some(StylePaneSort::Type));
        for bad in ["", "0006", "size", "00000", "zz"] {
            assert_eq!(StylePaneSort::parse(bad), None, "{bad}");
        }
    }
}
