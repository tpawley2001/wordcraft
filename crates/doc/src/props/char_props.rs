use super::color::{Rgb, TextColor};
use super::para_props::Border;
use super::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Underline {
    #[default]
    None,
    Single,
    Words,
    Double,
    Thick,
    Dotted,
    Dash,
    DotDash,
    DotDotDash,
    Wave,
    DoubleWave,
}

impl Underline {
    pub const ALL: [Underline; 11] = [
        Underline::None,
        Underline::Single,
        Underline::Words,
        Underline::Double,
        Underline::Thick,
        Underline::Dotted,
        Underline::Dash,
        Underline::DotDash,
        Underline::DotDotDash,
        Underline::Wave,
        Underline::DoubleWave,
    ];
    /// The OOXML `w:u/@w:val` name.
    pub fn ooxml(self) -> &'static str {
        match self {
            Underline::None => "none",
            Underline::Single => "single",
            Underline::Words => "words",
            Underline::Double => "double",
            Underline::Thick => "thick",
            Underline::Dotted => "dotted",
            Underline::Dash => "dash",
            Underline::DotDash => "dotDash",
            Underline::DotDotDash => "dotDotDash",
            Underline::Wave => "wave",
            Underline::DoubleWave => "wavyDouble",
        }
    }
    pub fn from_ooxml(s: &str) -> Underline {
        Underline::ALL.iter().copied().find(|u| u.ooxml() == s).unwrap_or(match s {
            "thickDash" | "dashLong" | "dashedHeavy" | "dashLongHeavy" => Underline::Dash,
            "dottedHeavy" => Underline::Dotted,
            "wavyHeavy" => Underline::Wave,
            "dashDotHeavy" => Underline::DotDash,
            "dashDotDotHeavy" => Underline::DotDotDash,
            _ => Underline::Single,
        })
    }
}

/// OpenType number forms a run asks for ([MS-DOCX] `w14:numForm`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NumForm {
    Default,
    Lining,
    OldStyle,
}

impl NumForm {
    /// The `w14:val` name.
    pub fn ooxml(self) -> &'static str {
        match self {
            NumForm::Default => "default",
            NumForm::Lining => "lining",
            NumForm::OldStyle => "oldStyle",
        }
    }
    /// `None` for a name [MS-DOCX] doesn't define.
    pub fn from_ooxml(s: &str) -> Option<NumForm> {
        [NumForm::Default, NumForm::Lining, NumForm::OldStyle].into_iter().find(|v| v.ooxml() == s)
    }
}

/// OpenType number spacing a run asks for ([MS-DOCX] `w14:numSpacing`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NumSpacing {
    Default,
    Proportional,
    Tabular,
}

impl NumSpacing {
    /// The `w14:val` name.
    pub fn ooxml(self) -> &'static str {
        match self {
            NumSpacing::Default => "default",
            NumSpacing::Proportional => "proportional",
            NumSpacing::Tabular => "tabular",
        }
    }
    /// `None` for a name [MS-DOCX] doesn't define.
    pub fn from_ooxml(s: &str) -> Option<NumSpacing> {
        [NumSpacing::Default, NumSpacing::Proportional, NumSpacing::Tabular].into_iter().find(|v| v.ooxml() == s)
    }
}

/// One stylistic set (`w14:styleSet`): set `id` (1..=20, feature `ssNN`) on or off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StyleSet {
    pub id: u8,
    pub on: bool,
}

impl StyleSet {
    /// The highest stylistic set OpenType defines (`ss20`).
    pub const MAX_ID: u8 = 20;
}

/// The OpenType ligatures a run asks for ([MS-DOCX] `w14:ligatures`): standard (`liga`),
/// contextual (`clig`), historical (`hlig`) and discretional (`dlig`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Ligatures {
    pub standard: bool,
    pub contextual: bool,
    pub historical: bool,
    pub discretional: bool,
}

impl Ligatures {
    /// Every combination of the four flags.
    pub fn all() -> impl Iterator<Item = Ligatures> {
        (0u8..16).map(|b| Ligatures { standard: b & 1 != 0, contextual: b & 2 != 0, historical: b & 4 != 0, discretional: b & 8 != 0 })
    }
    /// The `w14:ligatures/@w14:val` name: `none`, `all`, or the set flags joined in camelCase
    /// (`standardContextual`).
    pub fn ooxml(self) -> String {
        let on: Vec<&str> =
            [(self.standard, "standard"), (self.contextual, "contextual"), (self.historical, "historical"), (self.discretional, "discretional")]
                .into_iter()
                .filter_map(|(on, n)| on.then_some(n))
                .collect();
        match on.len() {
            0 => "none".into(),
            4 => "all".into(),
            _ => on
                .iter()
                .enumerate()
                .flat_map(|(i, n)| n.chars().enumerate().map(move |(j, c)| if i > 0 && j == 0 { c.to_ascii_uppercase() } else { c }))
                .collect(),
        }
    }
    /// `None` for a name [MS-DOCX] doesn't define.
    pub fn from_ooxml(s: &str) -> Option<Ligatures> {
        Ligatures::all().find(|l| l.ooxml() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum VertAlign {
    #[default]
    Baseline,
    Superscript,
    Subscript,
}

/// Word's 16 highlighter colours (plus none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Highlight {
    #[default]
    None,
    Yellow,
    BrightGreen,
    Turquoise,
    Pink,
    Blue,
    Red,
    DarkBlue,
    Teal,
    Green,
    Violet,
    DarkRed,
    DarkYellow,
    Gray50,
    Gray25,
    Black,
    White,
}

impl Highlight {
    pub const ALL: [Highlight; 17] = [
        Highlight::None,
        Highlight::Yellow,
        Highlight::BrightGreen,
        Highlight::Turquoise,
        Highlight::Pink,
        Highlight::Blue,
        Highlight::Red,
        Highlight::DarkBlue,
        Highlight::Teal,
        Highlight::Green,
        Highlight::Violet,
        Highlight::DarkRed,
        Highlight::DarkYellow,
        Highlight::Gray50,
        Highlight::Gray25,
        Highlight::Black,
        Highlight::White,
    ];
    pub fn rgb(self) -> Option<Rgb> {
        Some(match self {
            Highlight::None => return None,
            Highlight::Yellow => Rgb(255, 255, 0),
            Highlight::BrightGreen => Rgb(0, 255, 0),
            Highlight::Turquoise => Rgb(0, 255, 255),
            Highlight::Pink => Rgb(255, 0, 255),
            Highlight::Blue => Rgb(0, 0, 255),
            Highlight::Red => Rgb(255, 0, 0),
            Highlight::DarkBlue => Rgb(0, 0, 128),
            Highlight::Teal => Rgb(0, 128, 128),
            Highlight::Green => Rgb(0, 128, 0),
            Highlight::Violet => Rgb(128, 0, 128),
            Highlight::DarkRed => Rgb(128, 0, 0),
            Highlight::DarkYellow => Rgb(128, 128, 0),
            Highlight::Gray50 => Rgb(128, 128, 128),
            Highlight::Gray25 => Rgb(192, 192, 192),
            Highlight::Black => Rgb(0, 0, 0),
            Highlight::White => Rgb(255, 255, 255),
        })
    }
    /// OOXML `w:highlight/@w:val`.
    pub fn ooxml(self) -> &'static str {
        match self {
            Highlight::None => "none",
            Highlight::Yellow => "yellow",
            Highlight::BrightGreen => "green",
            Highlight::Turquoise => "cyan",
            Highlight::Pink => "magenta",
            Highlight::Blue => "blue",
            Highlight::Red => "red",
            Highlight::DarkBlue => "darkBlue",
            Highlight::Teal => "darkCyan",
            Highlight::Green => "darkGreen",
            Highlight::Violet => "darkMagenta",
            Highlight::DarkRed => "darkRed",
            Highlight::DarkYellow => "darkYellow",
            Highlight::Gray50 => "darkGray",
            Highlight::Gray25 => "lightGray",
            Highlight::Black => "black",
            Highlight::White => "white",
        }
    }
    pub fn from_ooxml(s: &str) -> Highlight {
        Highlight::ALL.iter().copied().find(|h| h.ooxml() == s).unwrap_or(Highlight::None)
    }
    pub fn name(self) -> &'static str {
        match self {
            Highlight::None => "No Color",
            Highlight::Yellow => "Yellow",
            Highlight::BrightGreen => "Bright Green",
            Highlight::Turquoise => "Turquoise",
            Highlight::Pink => "Pink",
            Highlight::Blue => "Blue",
            Highlight::Red => "Red",
            Highlight::DarkBlue => "Dark Blue",
            Highlight::Teal => "Teal",
            Highlight::Green => "Green",
            Highlight::Violet => "Violet",
            Highlight::DarkRed => "Dark Red",
            Highlight::DarkYellow => "Dark Yellow",
            Highlight::Gray50 => "Gray-50%",
            Highlight::Gray25 => "Gray-25%",
            Highlight::Black => "Black",
            Highlight::White => "White",
        }
    }
}

/// The text size, in points, of a run whose size no style or default sets.
pub const DEFAULT_FONT_SIZE: f32 = 11.0;

/// Fit Text (`w:fitText`, ECMA-376 §17.3.2.14): lay the run, or the consecutive runs with the same
/// `id`, out exactly `width` points wide.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FitText {
    /// The region's width, points (`w:val` / 20).
    pub width: f32,
    /// `w:id`: runs next to each other with the same id are one region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
}

/// Character formatting (direct or in a style). `None` = inherit.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CharProps {
    /// Character style id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// The symbol's own font (OOXML `w:sym/@w:font`, ECMA-376 §17.3.3.30), set on a symbol
    /// character from a `w:sym` or Insert › Symbol: it is shown in this font whatever `font` is,
    /// so changing the text's font keeps the symbol. `None` = the run's font.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sym_font: Option<String>,
    /// Points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<Underline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<Rgb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<TextColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Highlight>,
    /// Character shading (background fill).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<Rgb>,
    /// Character border (`w:bdr`); `style: None` = explicitly no border.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert_align: Option<VertAlign>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_caps: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Hidden in Web Layout (`w:webHidden`); distinct from `w:vanish` (`hidden`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_hidden: Option<bool>,
    /// On a paragraph mark: the mark is always hidden, whatever the hidden-text settings, and the
    /// paragraph runs on into the next one (`w:specVanish`, ECMA-376 §17.3.2.36; Word's style
    /// separator sets it with `hidden`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_vanish: Option<bool>,
    /// Snap characters to the document grid (`w:snapToGrid`). Unset inherits; Word's default is on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
    /// Character spacing (expanded/condensed), points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f32>,
    /// Horizontal scale, percent (100 = normal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    /// Raised (+) / lowered (−) position, points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<f32>,
    /// Kerning for fonts at or above this size (points); 0 = off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kern: Option<f32>,
    /// Fit Text (`w:fitText`, ECMA-376 §17.3.2.14): the run, or the consecutive runs sharing its
    /// id, are laid out exactly this wide.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit_text: Option<FitText>,
    /// A mark over (or under) each character (`w:em`, Font › Emphasis mark), as East Asian text
    /// stresses words; `Some(EmphasisMark::None)` turns off one a style sets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emphasis: Option<EmphasisMark>,
    /// OpenType ligatures (`w14:ligatures`). `None` leaves WordCraft's default: standard and
    /// contextual ligatures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ligatures: Option<Ligatures>,
    /// OpenType number forms (`w14:numForm`, [MS-DOCX]); `None` = the font's own figures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_form: Option<NumForm>,
    /// OpenType number spacing (`w14:numSpacing`, [MS-DOCX]); `None` = the font's own spacing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_spacing: Option<NumSpacing>,
    /// OpenType stylistic sets (`w14:stylisticSets`, [MS-DOCX]): sorted by id, one entry per id
    /// in 1..=20. A run's list replaces its style's as a whole.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stylistic_sets: Option<Vec<StyleSet>>,
    /// Use Contextual Alternates (`w14:cntxtAlts`, [MS-DOCX]); `None` = the shaper's default (`calt` on).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_alternates: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emboss: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engrave: Option<bool>,
    /// WordArt text effects: fill, outline, shadow, glow, reflection ([MS-DOCX] `w14:` run
    /// properties). Overlaid effect by effect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_effects: Option<Box<crate::wordart::TextEffects>>,
    /// BCP 47 language tag (proofing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_proof: Option<bool>,
    /// Right-to-left run (OOXML `w:rtl`): its characters are complex script and read right to left.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// Format the whole run with the complex-script properties below (OOXML `w:cs`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cs: Option<bool>,
    /// Complex-script font (Arabic, Persian, Hebrew…; OOXML `w:rFonts/@w:cs`). `None` = `font`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_cs: Option<String>,
    /// East Asian font (Chinese, Japanese and Korean text; OOXML `w:rFonts/@w:eastAsia`). `None` =
    /// inherit, then `font`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_ea: Option<String>,
    /// Complex-script size, points (`w:szCs`). `None` leaves the inherited value unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_cs: Option<f32>,
    /// Complex-script bold / italic (`w:bCs`, `w:iCs`). `None` leaves the inherited value unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold_cs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic_cs: Option<bool>,
    /// BCP 47 language of complex-script text (`w:lang/@w:bidi`, e.g. `fa-IR`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang_bidi: Option<String>,
    /// BCP 47 language of East Asian text (`w:lang/@w:eastAsia`, e.g. `zh-CN`, `ja-JP`, `ko-KR`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang_ea: Option<String>,
    /// East Asian typography: two lines in one and horizontal in vertical (`w:eastAsianLayout`).
    /// Kept for round trip; layout doesn't apply it yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub east_asian_layout: Option<EastAsianLayout>,
    /// Hyperlink target: a URL, or `#bookmark` for an internal link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// A hyperlink's ScreenTip (`w:hyperlink/@w:tooltip`, a HYPERLINK field's `\o`): the text shown on hover
    /// instead of the address. Only meaningful on a run that has `link`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_tip: Option<Box<str>>,
    /// Tracked insertion: index into `Document::revisions`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ins: Option<u32>,
    /// Tracked deletion: index into `Document::revisions`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub del: Option<u32>,
    /// Tracked formatting change (`w:rPrChange`): the formatting before it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fmt_change: Option<Box<PropChange<CharProps>>>,
}

/// East Asian typography settings of a run (ISO/IEC 29500-1 §17.3.2.10, `w:eastAsianLayout`):
/// *two lines in one* sets the run's text on two sub-lines inside one line (warichu), *horizontal
/// in vertical* turns it 90° to read across a vertical line (tate-chu-yoko). Each attribute is
/// `None` when the file doesn't say.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EastAsianLayout {
    /// East Asian typography run id (`w:id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Two lines in one (`w:combine`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combine: Option<bool>,
    /// Brackets around two lines in one (`w:combineBrackets`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combine_brackets: Option<CombineBrackets>,
    /// Horizontal in vertical (`w:vert`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert: Option<bool>,
    /// Compress horizontal-in-vertical text to the line height (`w:vertCompress`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert_compress: Option<bool>,
}

/// A mark over or under each character (`ST_Em`, ECMA-376 §17.18.24): Word's Font › Emphasis
/// mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum EmphasisMark {
    #[default]
    None,
    /// A dot above.
    Dot,
    /// A comma (sesame) above.
    Comma,
    /// A hollow circle above.
    Circle,
    /// A dot below.
    UnderDot,
}

impl EmphasisMark {
    pub const ALL: [EmphasisMark; 5] = [EmphasisMark::None, EmphasisMark::Dot, EmphasisMark::Comma, EmphasisMark::Circle, EmphasisMark::UnderDot];
    /// OOXML `w:em` value.
    pub fn ooxml(self) -> &'static str {
        match self {
            EmphasisMark::None => "none",
            EmphasisMark::Dot => "dot",
            EmphasisMark::Comma => "comma",
            EmphasisMark::Circle => "circle",
            EmphasisMark::UnderDot => "underDot",
        }
    }
    /// The mark an OOXML `w:em` value names; unknown values are none.
    pub fn from_ooxml(v: &str) -> EmphasisMark {
        EmphasisMark::ALL.into_iter().find(|m| m.ooxml() == v.trim()).unwrap_or_default()
    }
}

/// Brackets around two lines in one (`ST_CombineBrackets`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CombineBrackets {
    None,
    Round,
    Square,
    Angle,
    Curly,
}

impl CombineBrackets {
    pub const ALL: [CombineBrackets; 5] =
        [CombineBrackets::None, CombineBrackets::Round, CombineBrackets::Square, CombineBrackets::Angle, CombineBrackets::Curly];
    /// OOXML `w:combineBrackets` value.
    pub fn ooxml(self) -> &'static str {
        match self {
            CombineBrackets::None => "none",
            CombineBrackets::Round => "round",
            CombineBrackets::Square => "square",
            CombineBrackets::Angle => "angle",
            CombineBrackets::Curly => "curly",
        }
    }
    /// The value for `s`, `None` for one the schema doesn't have.
    pub fn from_ooxml(s: &str) -> Option<CombineBrackets> {
        CombineBrackets::ALL.into_iter().find(|b| b.ooxml() == s)
    }
}

/// A tracked formatting change (ECMA-376 §17.13.5: `w:rPrChange`, `w:pPrChange`,
/// `w:tblPrChange`, `w:trPrChange`, `w:tcPrChange`, `w:sectPrChange`): the properties as they
/// were before the change, and the revision (author, date) that made it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropChange<T> {
    /// Index into `Document::revisions`.
    pub rev: u32,
    /// The properties before the change (formatting only: no revision or link marks).
    pub old: T,
}

impl<T> PropChange<T> {
    pub fn boxed(rev: u32, old: T) -> Option<Box<PropChange<T>>> {
        Some(Box::new(PropChange { rev, old }))
    }
}

/// A tracked change of a paragraph's list numbering (`w:numberingChange`, kept for round trip):
/// `original` is the number as it was shown before the change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct NumChange {
    /// Index into `Document::revisions`.
    pub rev: u32,
    pub original: String,
}

impl CharProps {
    /// The same formatting outside a hyperlink: no link target, and no "Hyperlink" character
    /// style that came with it. What a paragraph mark or text typed next to a link gets.
    pub fn unlinked(&self) -> CharProps {
        let mut c = self.clone();
        c.link_tip = None;
        if c.link.take().is_some() && c.style.as_deref() == Some("Hyperlink") {
            c.style = None;
        }
        c
    }

    /// The fields of `self` that differ from `base` (equal ones unset): what `self` adds when
    /// overlaid on `base`.
    pub fn minus(mut self, base: &CharProps) -> CharProps {
        self.minus_text_style(base);
        self.minus_run_visibility(base);
        self.minus_script_style(base);
        self.minus_document_marks(base);
        self
    }

    fn minus_text_style(&mut self, base: &CharProps) {
        drop_same_fields!(self, base; style, font, sym_font, size, bold, italic, underline, underline_color, strike, double_strike, color, highlight,
            shading, border, vert_align, caps, small_caps, hidden, spacing, scale, position, kern, outline, shadow, emboss, engrave, lang, no_proof, rtl);
    }

    fn minus_run_visibility(&mut self, base: &CharProps) {
        drop_same_fields!(self, base; web_hidden, spec_vanish, snap_to_grid, emphasis, fit_text);
    }

    fn minus_script_style(&mut self, base: &CharProps) {
        drop_same_fields!(self, base; cs, font_cs, font_ea, size_cs, bold_cs, italic_cs, lang_bidi, lang_ea, east_asian_layout);
    }

    fn minus_document_marks(&mut self, base: &CharProps) {
        drop_same_fields!(self, base; link, link_tip, ins, del, ligatures, num_form, num_spacing, stylistic_sets, contextual_alternates);
    }

    /// Apply every `Some` field of `patch` on top of `self`.
    ///
    /// Plain and complex-script properties inherit independently. Commands that intend to
    /// format both scripts must set both values explicitly.
    pub fn overlay(&mut self, patch: &CharProps) {
        self.overlay_text_style(patch);
        self.overlay_run_visibility(patch);
        self.overlay_script_style(patch);
        self.overlay_document_marks(patch);
        if let Some(p) = &patch.text_effects {
            self.text_effects.get_or_insert_with(Default::default).overlay(p);
        }
    }

    fn overlay_text_style(&mut self, patch: &CharProps) {
        overlay_fields!(self, patch; style, font, sym_font, size, bold, italic, underline, underline_color, strike, double_strike, color, highlight,
            shading, border, vert_align, caps, small_caps, hidden, spacing, scale, position, kern, outline, shadow, emboss, engrave, lang, no_proof, rtl);
    }

    fn overlay_run_visibility(&mut self, patch: &CharProps) {
        overlay_fields!(self, patch; web_hidden, spec_vanish, snap_to_grid, emphasis, fit_text);
    }

    fn overlay_script_style(&mut self, patch: &CharProps) {
        overlay_fields!(self, patch; cs, font_cs, font_ea, size_cs, bold_cs, italic_cs, lang_bidi, lang_ea, east_asian_layout);
    }

    fn overlay_document_marks(&mut self, patch: &CharProps) {
        overlay_fields!(self, patch; link, link_tip, ins, del, fmt_change, ligatures, num_form, num_spacing, stylistic_sets, contextual_alternates);
    }
    /// The part of the resolved set `self` (a style's base chain with its own settings applied)
    /// that belongs to the style itself: each field `own` (its own settings alone) sets, or that
    /// differs from `inherited` (the base chain alone). Every other field is left unset, so it
    /// comes from the base style. Text effects are kept whole when they differ. Exhaustive: a new
    /// field must be added here.
    pub fn own_part(self, own: &CharProps, inherited: &CharProps) -> CharProps {
        fn keep<T: Clone + PartialEq>(full: Option<T>, own: &Option<T>, inherited: &Option<T>) -> Option<T> {
            if own.is_some() || full != *inherited { full } else { None }
        }
        let CharProps {
            style,
            font,
            sym_font,
            size,
            bold,
            italic,
            underline,
            underline_color,
            strike,
            double_strike,
            color,
            highlight,
            shading,
            border,
            vert_align,
            caps,
            small_caps,
            hidden,
            web_hidden,
            spec_vanish,
            snap_to_grid,
            spacing,
            scale,
            position,
            kern,
            fit_text,
            emphasis,
            ligatures,
            num_form,
            num_spacing,
            stylistic_sets,
            contextual_alternates,
            outline,
            shadow,
            emboss,
            engrave,
            text_effects,
            lang,
            no_proof,
            rtl,
            cs,
            font_cs,
            font_ea,
            size_cs,
            bold_cs,
            italic_cs,
            lang_bidi,
            lang_ea,
            east_asian_layout,
            link,
            link_tip,
            ins,
            del,
            fmt_change,
        } = self;
        CharProps {
            style: keep(style, &own.style, &inherited.style),
            font: keep(font, &own.font, &inherited.font),
            sym_font: keep(sym_font, &own.sym_font, &inherited.sym_font),
            size: keep(size, &own.size, &inherited.size),
            bold: keep(bold, &own.bold, &inherited.bold),
            italic: keep(italic, &own.italic, &inherited.italic),
            underline: keep(underline, &own.underline, &inherited.underline),
            underline_color: keep(underline_color, &own.underline_color, &inherited.underline_color),
            strike: keep(strike, &own.strike, &inherited.strike),
            double_strike: keep(double_strike, &own.double_strike, &inherited.double_strike),
            color: keep(color, &own.color, &inherited.color),
            highlight: keep(highlight, &own.highlight, &inherited.highlight),
            shading: keep(shading, &own.shading, &inherited.shading),
            border: keep(border, &own.border, &inherited.border),
            vert_align: keep(vert_align, &own.vert_align, &inherited.vert_align),
            caps: keep(caps, &own.caps, &inherited.caps),
            small_caps: keep(small_caps, &own.small_caps, &inherited.small_caps),
            hidden: keep(hidden, &own.hidden, &inherited.hidden),
            web_hidden: keep(web_hidden, &own.web_hidden, &inherited.web_hidden),
            spec_vanish: keep(spec_vanish, &own.spec_vanish, &inherited.spec_vanish),
            snap_to_grid: keep(snap_to_grid, &own.snap_to_grid, &inherited.snap_to_grid),
            spacing: keep(spacing, &own.spacing, &inherited.spacing),
            scale: keep(scale, &own.scale, &inherited.scale),
            position: keep(position, &own.position, &inherited.position),
            kern: keep(kern, &own.kern, &inherited.kern),
            fit_text: keep(fit_text, &own.fit_text, &inherited.fit_text),
            emphasis: keep(emphasis, &own.emphasis, &inherited.emphasis),
            ligatures: keep(ligatures, &own.ligatures, &inherited.ligatures),
            num_form: keep(num_form, &own.num_form, &inherited.num_form),
            num_spacing: keep(num_spacing, &own.num_spacing, &inherited.num_spacing),
            stylistic_sets: keep(stylistic_sets, &own.stylistic_sets, &inherited.stylistic_sets),
            contextual_alternates: keep(contextual_alternates, &own.contextual_alternates, &inherited.contextual_alternates),
            outline: keep(outline, &own.outline, &inherited.outline),
            shadow: keep(shadow, &own.shadow, &inherited.shadow),
            emboss: keep(emboss, &own.emboss, &inherited.emboss),
            engrave: keep(engrave, &own.engrave, &inherited.engrave),
            text_effects: keep(text_effects, &own.text_effects, &inherited.text_effects),
            lang: keep(lang, &own.lang, &inherited.lang),
            no_proof: keep(no_proof, &own.no_proof, &inherited.no_proof),
            rtl: keep(rtl, &own.rtl, &inherited.rtl),
            cs: keep(cs, &own.cs, &inherited.cs),
            font_cs: keep(font_cs, &own.font_cs, &inherited.font_cs),
            font_ea: keep(font_ea, &own.font_ea, &inherited.font_ea),
            size_cs: keep(size_cs, &own.size_cs, &inherited.size_cs),
            bold_cs: keep(bold_cs, &own.bold_cs, &inherited.bold_cs),
            italic_cs: keep(italic_cs, &own.italic_cs, &inherited.italic_cs),
            lang_bidi: keep(lang_bidi, &own.lang_bidi, &inherited.lang_bidi),
            lang_ea: keep(lang_ea, &own.lang_ea, &inherited.lang_ea),
            east_asian_layout: keep(east_asian_layout, &own.east_asian_layout, &inherited.east_asian_layout),
            link: keep(link, &own.link, &inherited.link),
            link_tip: keep(link_tip, &own.link_tip, &inherited.link_tip),
            ins: keep(ins, &own.ins, &inherited.ins),
            del: keep(del, &own.del, &inherited.del),
            fmt_change: keep(fmt_change, &own.fmt_change, &inherited.fmt_change),
        }
    }
    pub fn overlaid(mut self, patch: &CharProps) -> CharProps {
        self.overlay(patch);
        self
    }
    pub fn is_empty(&self) -> bool {
        *self == CharProps::default()
    }
    /// The font a character of this run is shown in: the symbol's own font when it has one
    /// ([`Self::sym_font`]), else the run's font.
    pub fn shown_font(&self) -> Option<&str> {
        self.sym_font.as_deref().filter(|f| !f.trim().is_empty()).or(self.font.as_deref())
    }
    /// The same formatting for text typed or painted next to a symbol: without the symbol's own
    /// font.
    pub fn without_symbol(mut self) -> CharProps {
        self.sym_font = None;
        self
    }
    /// Clear direct formatting but keep what isn't "formatting" (links, revisions, character style
    /// is cleared too, as Word's Clear Formatting does).
    pub fn cleared(&self) -> CharProps {
        CharProps {
            link: self.link.clone(),
            link_tip: self.link_tip.clone(),
            ins: self.ins,
            del: self.del,
            fmt_change: self.fmt_change.clone(),
            // A symbol stays the symbol it is.
            sym_font: self.sym_font.clone(),
            ..Default::default()
        }
    }
    /// Just the formatting: without the link, revision marks and tracked formatting change.
    pub fn formatting(&self) -> CharProps {
        CharProps { link: None, link_tip: None, ins: None, del: None, fmt_change: None, ..self.clone() }
    }
}
