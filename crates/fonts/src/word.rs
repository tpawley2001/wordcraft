//! Word-processor font resolution: map a requested family + bold/italic to an available face,
//! with metric-compatible substitutes for common proprietary document fonts, and Word-style line
//! metrics (Windows ascent/descent, which is what Word uses for single line spacing).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use skrifa::raw::TableProvider;

use crate::{FaceRef, FontDb};

/// A requested font resolved to a face.
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub face: FaceRef,
    /// The face isn't bold but bold was asked for: embolden when drawing.
    pub synth_bold: bool,
    /// The face isn't italic but italic was asked for: slant when drawing.
    pub synth_italic: bool,
    /// The requested family wasn't available.
    pub substituted: bool,
}

/// Substitutes to try, in order, for a family that isn't installed. Open fonts that are
/// metric-compatible come first (Carlito ↔ Calibri, Caladea ↔ Cambria, Liberation ↔ Arial /
/// Times New Roman / Courier New / Microsoft Sans Serif, Liberation Sans Narrow ↔ Arial Narrow,
/// C059 ↔ Century), then common system fonts,
/// then our bundled families.
fn substitutes(family: &str) -> &'static [&'static str] {
    substitutes_by_name(family).0
}

/// Does `family` have substitutes of its own, picked by its name (a metric-compatible clone such
/// as Liberation Serif for Times New Roman, or the same design under another name), rather than
/// the generic lists any unknown font gets?
pub fn has_named_substitutes(family: &str) -> bool {
    if family.contains(';') {
        return list_class(family).named;
    }
    substitutes_by_name(charset_base_family(family)).1
}

const SANS: &[&str] = &["Carlito", "Helvetica Neue", "Arial", "Liberation Sans", "Noto Sans", "DejaVu Sans", "Source Sans 3"];

/// [`substitutes`], and whether they were chosen for this family by name.
fn substitutes_by_name(family: &str) -> (&'static [&'static str], bool) {
    if family.contains(';') {
        for cand in family.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let (subs, named) = substitutes_by_name(cand);
            if named || is_thai_family(cand) || subs != SANS {
                return (subs, named);
            }
        }
        if let Some(first) = family.split(';').map(str::trim).find(|s| !s.is_empty()) {
            return substitutes_by_name(first);
        }
    }
    const SERIF: &[&str] = &["Caladea", "Times New Roman", "Liberation Serif", "Georgia", "Noto Serif", "DejaVu Serif", "Source Serif 4"];
    const MONO: &[&str] = &["Courier New", "Liberation Mono", "Menlo", "Consolas", "DejaVu Sans Mono", "JetBrains Mono"];
    const ARIAL: &[&str] = &["Liberation Sans", "Helvetica", "Helvetica Neue", "Arimo", "Source Sans 3"];
    // Liberation Sans Narrow has Arial Narrow's advance widths; Nimbus Sans Narrow is the next best.
    const ARIAL_NARROW: &[&str] = &[
        "Arial Narrow",
        "Liberation Sans Narrow",
        "Nimbus Sans Narrow",
        "Helvetica Narrow",
        "Liberation Sans",
        "Helvetica",
        "Helvetica Neue",
        "Arimo",
        "Source Sans 3",
    ];
    const ARIAL_UNICODE: &[&str] = &["Arial", "Liberation Sans", "Helvetica", "Helvetica Neue", "Arimo", "Source Sans 3"];
    const TIMES: &[&str] = &["Liberation Serif", "Times", "Tinos", "Source Serif 4"];
    // Word's symbol fonts (Symbol, Wingdings…) are proprietary: their characters are drawn as their
    // Unicode equivalents (`wordcraft_doc::symbol_fonts`) from these faces or any face that has them.
    const SYMBOL: &[&str] = &["Apple Symbols", "Segoe UI Symbol", "DejaVu Sans", "Noto Sans Symbols", "Noto Sans Symbols 2", "Source Sans 3"];
    // Persian / Arabic document fonts. Naskh (book) faces such as B Nazanin, B Lotus, B Zar and
    // Traditional Arabic, then the sans faces (B Yekan, B Titr, IRANSans) people use for
    // headings and screens. Open fonts with full Persian coverage first.
    const NASKH: &[&str] = &["Noto Naskh Arabic", "XB Zar", "XB Niloofar", "Amiri", "Vazirmatn", "Noto Sans Arabic", "DejaVu Sans", "Source Serif 4"];
    const ARABIC_SANS: &[&str] = &["Vazirmatn", "Sahel", "Shabnam", "Samim", "Noto Sans Arabic", "Noto Naskh Arabic", "DejaVu Sans", "Source Sans 3"];
    // Hebrew document fonts: platform-specific fonts first, then common open source fallbacks.
    // Windows: Microsoft Office fonts (David, Miriam) are common; Segoe UI is standard.
    // macOS: Helvetica has good Hebrew support; Geeza Pro is included by default.
    // Linux: Noto Sans Hebrew is the best option.
    #[cfg(windows)]
    const HEBREW: &[&str] = &["Noto Sans Hebrew", "David", "Miriam", "Segoe UI", "Arial", "Tahoma", "DejaVu Sans", "Source Sans 3"];
    #[cfg(target_os = "macos")]
    const HEBREW: &[&str] = &["Helvetica", "Noto Sans Hebrew", "Arial", "Tahoma", "Geeza Pro", "DejaVu Sans", "Source Sans 3"];
    #[cfg(not(any(windows, target_os = "macos")))]
    const HEBREW: &[&str] = &["Noto Sans Hebrew", "Segoe UI", "Arial", "Tahoma", "DejaVu Sans", "Source Sans 3"];
    // Thai document fonts (TH Sarabun New, Cordia New, the UPC families…): Thai faces, every one
    // with Thai letters; [`substitute`] prefers one with Latin letters and digits too, so a run
    // mixing Thai and Latin keeps one font at one size, as Word substitutes Cordia New for the
    // whole run (#1549). Windows' own Thai fonts first, then the Mac's, then the open Noto and Thai
    // Linux Working Group families. The Mac's Thonburi and Sathu carry Apple's `morx` tables
    // instead of OpenType ones; the shaper applies those, so their marks stack as they should.
    #[cfg(windows)]
    const THAI: &[&str] = &["Cordia New", "Leelawadee UI", "Leelawadee", "Tahoma", "Noto Sans Thai", "Browallia New", "Angsana New"];
    #[cfg(target_os = "macos")]
    const THAI: &[&str] = &["Thonburi", "Ayuthaya", "Sathu", "Krungthep", "Noto Sans Thai", "Tahoma"];
    #[cfg(not(any(windows, target_os = "macos")))]
    const THAI: &[&str] = &["Noto Sans Thai", "Laksaman", "Garuda", "Waree", "Tlwg Typo", "Loma", "Umpush", "Noto Serif Thai"];
    // Serif Thai document fonts (Angsana New, EucrosiaUPC, Norasi…) prefer a serif Thai face.
    #[cfg(windows)]
    const THAI_SERIF: &[&str] = &["Angsana New", "AngsanaUPC", "Noto Serif Thai", "Cordia New", "Leelawadee UI", "Tahoma", "Noto Sans Thai"];
    #[cfg(target_os = "macos")]
    const THAI_SERIF: &[&str] = &["Sathu", "Noto Serif Thai", "Thonburi", "Ayuthaya", "Krungthep", "Noto Sans Thai", "Tahoma"];
    #[cfg(not(any(windows, target_os = "macos")))]
    const THAI_SERIF: &[&str] = &["Noto Serif Thai", "Norasi", "Kinnari", "Noto Sans Thai", "Laksaman", "Garuda", "Waree"];
    // Khmer document fonts (Khmer OS …, Khmer Mool1…): a missing one is drawn in the system's
    // Khmer font, as Word does, not in whatever Khmer font the document happens to load: the Khmer
    // OS fonts are wider, so lines wrap and boxes overflow where Word's don't.
    #[cfg(windows)]
    const KHMER: &[&str] = &["Leelawadee UI", "Khmer UI", "DaunPenh", "MoolBoran", "Noto Sans Khmer"];
    #[cfg(target_os = "macos")]
    const KHMER: &[&str] = &["Khmer Sangam MN", "Khmer MN", "Noto Sans Khmer"];
    #[cfg(not(any(windows, target_os = "macos")))]
    const KHMER: &[&str] = &["Noto Sans Khmer", "Noto Serif Khmer"];
    // Palatino Linotype / Book Antiqua: macOS ships the same design as "Palatino"; open
    // look-alikes with matching metrics (TeX Gyre Pagella, URW P052 / Palladio) on Linux.
    const PALATINO: &[&str] = &[
        "Palatino Linotype",
        "Palatino",
        "Book Antiqua",
        "TeX Gyre Pagella",
        "P052",
        "URW Palladio L",
        "Palladio",
        "Caladea",
        "Times New Roman",
        "Liberation Serif",
        "Georgia",
        "Noto Serif",
        "DejaVu Serif",
        "Source Serif 4",
    ];
    // Century: URW's C059 has its advance widths (also packaged as Century Schoolbook L and TeX
    // Gyre Schola), then the generic serif list.
    const CENTURY: &[&str] = &[
        "C059",
        "Century Schoolbook L",
        "TeX Gyre Schola",
        "New Century Schoolbook",
        "Caladea",
        "Times New Roman",
        "Liberation Serif",
        "Georgia",
        "Noto Serif",
        "DejaVu Serif",
        "Source Serif 4",
    ];
    let f = family.to_ascii_lowercase();
    if is_thai_serif(&f) {
        return (THAI_SERIF, true);
    }
    if is_thai(&f) {
        return (THAI, true);
    }
    if is_hebrew(&f) {
        return (HEBREW, true);
    }
    if is_khmer(&f) {
        return (KHMER, true);
    }
    if is_persian_naskh(&f) {
        return (NASKH, true);
    }
    if is_persian_sans(&f) {
        return (ARABIC_SANS, true);
    }
    match f.as_str() {
        "palatino linotype" | "book antiqua" | "palatino" => (PALATINO, true),
        "arial" | "helvetica" | "arial nova" => (ARIAL, true),
        // Microsoft Sans Serif has Arial's advance widths (to within 1/1000 em).
        "microsoft sans serif" | "ms sans serif" => (ARIAL, true),
        "century" | "century schoolbook" | "new century schoolbook" => (CENTURY, true),
        "arial narrow" | "helvetica narrow" => (ARIAL_NARROW, true),
        "arial unicode ms" => (ARIAL_UNICODE, true),
        "times new roman" | "times" | "cg times" => (TIMES, true),
        "courier new" | "courier" | "consolas" | "lucida console" | "cascadia code" | "cascadia mono" => (MONO, true),
        "symbol" | "wingdings" | "wingdings 2" | "wingdings 3" | "segoe ui symbol" | "webdings" => (SYMBOL, true),
        // Serif faces with no open clone of their own: the generic serif list, as for any font.
        "cambria"
        | "cambria math"
        | "georgia"
        | "garamond"
        | "constantia"
        | "baskerville old face"
        | "bookman old style"
        | "sitka text"
        | "sitka"
        | "aptos serif" => (SERIF, false),
        _ if f.contains("mono") || f.contains("code") => (MONO, false),
        _ if f.contains("serif") && !f.contains("sans") => (SERIF, false),
        _ => (SANS, false),
    }
}

/// Ascent and descent (em) of Word's symbol fonts, which aren't open fonts. Their characters are
/// drawn from other faces (`wordcraft_doc::symbol_fonts`), but Word sizes a line by the font
/// itself: a Symbol bullet makes its line taller than the text alone. The values are those Word's
/// embedded fonts give in hhea and OS/2 win metrics (unitsPerEm 2048, lineGap 0).
pub fn symbol_font_metrics(family: &str) -> Option<(f64, f64)> {
    family.split(';').map(str::trim).filter(|f| !f.is_empty()).find_map(|f| match f.to_ascii_lowercase().as_str() {
        "symbol" | "symbol (pcl6)" => Some((1.005, 0.216)),
        "wingdings" => Some((0.899, 0.211)),
        "wingdings 2" => Some((0.8433, 0.2109)),
        "wingdings 3" => Some((0.9277, 0.2109)),
        "webdings" => Some((0.7998, 0.2002)),
        _ => None,
    })
}

/// Advance (em) of the missing-character box Word draws for a code a symbol font has no glyph
/// for (Symbol 0x7F-0x9F, say): the font's own empty box, never another font's glyph. Measured
/// from the application's PDF output (line width less the letters around the box, Times New
/// Roman at 10 and 20 pt); no font file was read. Symbol: 1224/2048 em; the others 1024/2048.
pub fn notdef_advance(family: &str) -> Option<f64> {
    family.split(';').map(str::trim).filter(|f| !f.is_empty()).find_map(|f| match f.to_ascii_lowercase().as_str() {
        "symbol" | "symbol (pcl6)" => Some(1224.0 / 2048.0),
        "wingdings" | "wingdings 2" | "wingdings 3" | "webdings" => Some(1024.0 / 2048.0),
        _ => None,
    })
}

/// Advance (em) of Symbol code `code` (0x20-0xFE) when the Symbol font itself isn't installed and
/// its characters are drawn from another face: Word sizes them by Symbol, not by the face drawn.
/// `family` is matched as [`notdef_advance`] matches it ("Symbol", "Symbol (PCL6)"); other fonts
/// (Wingdings and the like included) and codes Symbol has no glyph for give `None`.
///
/// Source: the published Adobe font metrics for the PDF core font Symbol (Symbol.afm, "Font Metrics
/// for PDF Core 14 Fonts", free to use, copy and distribute); no font file was read. Fifteen codes
/// measured in the application's PDF output agree to the unit (0x44, 0x47, 0x57, 0x59, 0x61, 0x71,
/// 0x72, 0x74, 0x79, 0x7E, 0xA3, 0xB0, 0xB1, 0xB9, 0xBA). The other codes are unverified there (the
/// bracket pieces 0xE9-0xFB measured about 389 for 384 in runs whose size could not be pinned).
pub fn symbol_advance(family: &str, code: u8) -> Option<f64> {
    let symbol = family.split(';').map(str::trim).any(|f| f.eq_ignore_ascii_case("symbol") || f.eq_ignore_ascii_case("symbol (pcl6)"));
    if !symbol {
        return None;
    }
    let w = SYMBOL_WIDTHS.get(usize::from(code)).copied().unwrap_or(0);
    (w > 0).then(|| f64::from(w) / 1000.0)
}

/// Symbol's advances in 1/1000 em by code ([`symbol_advance`]); 0 is no glyph.
#[rustfmt::skip]
const SYMBOL_WIDTHS: [u16; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    250, 333, 713, 500, 549, 833, 778, 439, 333, 333, 500, 549, 250, 549, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 549, 549, 549, 444,
    549, 722, 667, 722, 612, 611, 763, 603, 722, 333, 631, 722, 686, 889, 722, 722,
    768, 741, 556, 592, 611, 690, 439, 768, 645, 795, 611, 333, 863, 333, 658, 500,
    500, 631, 549, 549, 494, 439, 521, 411, 603, 329, 603, 549, 549, 576, 521, 549,
    549, 521, 549, 603, 439, 576, 713, 686, 493, 686, 494, 480, 200, 480, 549, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    750, 620, 247, 549, 167, 713, 500, 753, 753, 753, 753, 1042, 987, 603, 987, 603,
    400, 549, 411, 549, 549, 713, 494, 460, 549, 549, 549, 549, 1000, 603, 1000, 658,
    823, 686, 795, 987, 768, 768, 823, 768, 768, 713, 713, 713, 713, 713, 713, 713,
    768, 713, 790, 790, 890, 823, 549, 250, 713, 603, 603, 1042, 987, 603, 987, 603,
    494, 329, 790, 790, 786, 713, 384, 384, 384, 384, 384, 384, 494, 494, 494, 494,
    0, 329, 274, 686, 686, 686, 384, 384, 384, 384, 384, 384, 494, 494, 494, 0,
];

/// The (ascent, descent) in em Word gives a line with a run in a symbol font, when that isn't the
/// resolved face's own: the font isn't installed (`resolved` is a substitute), or the installed
/// face is one em tall, as Apple's Symbol is (hhea 0.701 / -0.299, no gap). Word for Mac spaces
/// such lines like Windows' Symbol: 1.222 em at 11 and 20 pt on synthetic documents (#2681).
pub fn symbol_line(family: &str, resolved: &Resolved) -> Option<(f64, f64)> {
    let m = symbol_font_metrics(family)?;
    (resolved.substituted || one_em(line_metrics(&resolved.face), resolved.face.upem)).then_some(m)
}

/// (ascent, descent) in em that Word gives a line in `family` when that font isn't installed
/// here and its substitute's own line differs: Word measures the line with the real font.
/// Symbol fonts follow [`symbol_line`].
pub fn missing_font_line(family: &str, resolved: &Resolved) -> Option<(f64, f64)> {
    missing_font_line_leading(family, resolved, true)
}

/// [`missing_font_line`] under the compatibility option "Don't add leading (extra space) between
/// rows of text": `leading` is false when the option applies. Word then takes the external
/// leading off the top of Century Gothic's and Lucida Bright's lines; the other fonts' lines stay.
pub fn missing_font_line_leading(family: &str, resolved: &Resolved, leading: bool) -> Option<(f64, f64)> {
    if let Some(m) = symbol_line(family, resolved) {
        return Some(m);
    }
    if !resolved.substituted {
        return None;
    }
    if !leading && let Some(m) = missing_font_metrics_no_leading(family) {
        return Some(m);
    }
    missing_font_metrics(family)
}

/// Word's single line, (ascent, descent) in em, with "Don't add leading between rows of text",
/// for the missing fonts whose line it changes (see [`missing_font_metrics`]).
fn missing_font_metrics_no_leading(family: &str) -> Option<(f64, f64)> {
    match family.trim().to_ascii_lowercase().as_str() {
        "century gothic" => Some((0.9716, 0.220)),
        "lucida bright" => Some((0.9336, 0.205)),
        _ => None,
    }
}

/// Word's single line, (ascent, descent) in em, for fonts whose substitute spaces lines
/// differently. Measured in Word's PDFs: Lucida Console's font descriptor gives `/Ascent` 789,
/// and consecutive single-spaced 10 pt lines are 10.0 pt apart (one em), so the descent is the
/// rest of the em (its Courier-New-like substitute is 1.133 em).
/// Palatino Linotype: 12 pt lines 16.20 pt apart (1.35 em); a 12 pt header at 36 pt has its first
/// baseline at 48.60 pt, so the ascent is 1.05 em (its P052 substitute is 1.20 em).
/// A font named "Palatino" is drawn by Word in Palatino Linotype and has its line (11 pt lines
/// 14.65 to 14.8 pt apart).
/// Franklin Gothic Book: 12 pt lines 13.56 pt apart (1.13 em); `/Ascent` 917, the descent is the
/// rest of the line (its Carlito substitute is 1.2207 em).
/// Tahoma: `/Ascent` 1000, `/Descent` -207; 12 pt lines 14.43 pt apart (1.207 em).
/// Lucida Bright: `/Ascent` 967, `/Descent` -205; 18 pt lines 21.12 pt apart (1.172 em). With
/// "Don't add leading between rows of text", 10, 20 and 36 pt lines 11.38, 22.77 and 41.00 pt apart
/// (1.1386 em), the first line moved up as much: the leading comes off the ascent.
/// Aptos and Aptos Display: `/Ascent` 939, `/Descent` -282; 11 pt at 1.1 lines is 14.76 pt apart.
/// Verdana: `/Ascent` 1005, the descent the rest of the line; 10 pt lines 12.12 and 12.24 pt apart (1.215 em).
/// Georgia: `/Ascent` 917, the descent the rest of the line; 11 pt lines 12.48 pt apart (1.136 em).
/// Trebuchet MS: `/Ascent` 939, the descent the rest of the line; 24 pt lines 27.84 pt apart (1.161 em).
/// Century Gothic: `/Ascent` 1006, the descent the rest of the line; 10 pt lines 12.24 and 12.36 pt apart (1.226 em).
/// With "Don't add leading between rows of text", 10, 20 and 36 pt lines 11.91, 23.83 and 42.90 pt
/// apart (1.1916 em), the first line moved up as much: the leading comes off the ascent.
/// Consolas: `/Ascent` 743, the descent the rest of the line; 8 pt lines 9.36 pt apart (1.172 em).
/// Garamond: `/Ascent` 862, the descent the rest of the line; 20 pt lines 22.44 to 22.58 pt apart (1.125 em).
/// Comic Sans MS: `/Ascent` 1102, the descent the rest of the line; 11 pt lines 15.36 and 15.24 pt apart (1.394 em).
/// Univers: `/Ascent` 989, the descent the rest of the line; 12 pt lines 14.88 pt apart (1.24 em).
/// SimSun: `/Ascent` 859, `/Descent` -141; 10.5, 12 and 14 pt lines 13.62, 15.56 and 18.16 pt apart (1.297 em), the extra split evenly above and below.
/// MS Mincho, MS PMincho, MS Gothic, MS PGothic and MS UI Gothic (also as written in Japanese): SimSun's descriptor and line; 10, 10.5, 12 and 14 pt lines 12.96, 13.64, 15.57 and 18.16 pt apart (1.297 em).
/// Shruti: `/Ascent` 1021, `/Descent` -660; 10, 12 and 14 pt lines 16.80, 20.16 and 23.57 pt apart (1.681 em), the first baseline 1.021 em below the top of the body.
/// Segoe UI: `/Ascent` 1079, the descent the rest of the line; 12 and 24 pt lines 15.96 and 31.93 pt apart (1.330 em).
/// Perpetua: `/Ascent` 820, the descent the rest of the line; 12 and 24 pt lines 13.75 and 27.51 pt apart (1.146 em).
/// Gill Sans MT: `/Ascent` 929, the descent the rest of the line; 12 and 24 pt lines 13.92 and 27.83 pt apart (1.160 em).
/// Book Antiqua: first baseline 0.962–0.966 em below the top (not `/Ascent` 923); 12 and 24 pt lines 14.92 and 29.82 pt apart (1.2427 em).
fn missing_font_metrics(family: &str) -> Option<(f64, f64)> {
    match family.trim().to_ascii_lowercase().as_str() {
        "lucida console" => Some((0.789, 0.211)),
        "palatino linotype" | "palatino" => Some((1.05, 0.30)),
        "franklin gothic book" => Some((0.917, 0.213)),
        "tahoma" => Some((1.0, 0.207)),
        "lucida bright" => Some((0.967, 0.205)),
        "aptos" | "aptos display" => Some((0.939, 0.282)),
        "verdana" => Some((1.005, 0.210)),
        "georgia" => Some((0.917, 0.219)),
        "trebuchet ms" => Some((0.939, 0.222)),
        "century gothic" => Some((1.006, 0.220)),
        "consolas" => Some((0.743, 0.429)),
        "garamond" => Some((0.862, 0.263)),
        "comic sans ms" => Some((1.102, 0.292)),
        "univers" => Some((0.989, 0.251)),
        "simsun" | "宋体" | "nsimsun" | "新宋体" | "simhei" | "黑体" => Some((1.007, 0.290)),
        "ms mincho"
        | "ＭＳ 明朝"
        | "ms pmincho"
        | "ＭＳ Ｐ明朝"
        | "ms gothic"
        | "ＭＳ ゴシック"
        | "ms pgothic"
        | "ＭＳ Ｐゴシック"
        | "ms ui gothic" => Some((1.007, 0.290)),
        "shruti" => Some((1.021, 0.660)),
        "segoe ui" => Some((1.079, 0.251)),
        "perpetua" => Some((0.820, 0.326)),
        "gill sans mt" => Some((0.929, 0.231)),
        "book antiqua" => Some((0.964, 0.279)),
        _ => None,
    }
}

/// The advance, in em, Word gives `ch` in `family` when that font is missing (its substitute's
/// advance differs); `None` when there is no table for the family or the character. Italic takes
/// the upright advances (Word draws a missing italic as slanted upright text), bold italic the
/// bold ones. Matching is exact: "Univers Condensed" is another face.
pub fn missing_font_advance(family: &str, bold: bool, ch: char) -> Option<f64> {
    let table = match (family.trim().to_ascii_lowercase().as_str(), bold) {
        ("univers", false) => UNIVERS,
        ("univers", true) => UNIVERS_BOLD,
        _ => return None,
    };
    table.binary_search_by_key(&ch, |&(c, _)| c).ok().and_then(|i| table.get(i)).map(|&(_, w)| f64::from(w) / 1000.0)
}

/// Univers advances (1/1000 em), sorted by character: the widths Word's PDFs of Univers
/// documents give the WinAnsi characters they use.
const UNIVERS: &[(char, u16)] = &[
    (' ', 278),
    ('"', 400),
    ('$', 556),
    ('%', 889),
    ('&', 778),
    ('\'', 278),
    ('(', 333),
    (')', 333),
    (',', 278),
    ('-', 333),
    ('.', 278),
    ('/', 278),
    ('0', 556),
    ('1', 556),
    ('2', 556),
    ('3', 556),
    ('4', 556),
    ('5', 556),
    ('6', 556),
    ('7', 556),
    ('8', 556),
    ('9', 556),
    (':', 278),
    (';', 278),
    ('=', 584),
    ('>', 584),
    ('?', 500),
    ('@', 1015),
    ('A', 722),
    ('B', 667),
    ('C', 667),
    ('D', 722),
    ('E', 611),
    ('F', 556),
    ('G', 778),
    ('H', 722),
    ('I', 278),
    ('J', 556),
    ('K', 667),
    ('L', 556),
    ('M', 944),
    ('N', 778),
    ('O', 778),
    ('P', 611),
    ('Q', 778),
    ('R', 667),
    ('S', 667),
    ('T', 611),
    ('U', 778),
    ('V', 667),
    ('W', 1000),
    ('X', 722),
    ('Y', 667),
    ('Z', 611),
    ('[', 333),
    (']', 333),
    ('_', 500),
    ('a', 556),
    ('b', 611),
    ('c', 500),
    ('d', 611),
    ('e', 556),
    ('f', 333),
    ('g', 611),
    ('h', 611),
    ('i', 278),
    ('j', 278),
    ('k', 500),
    ('l', 278),
    ('m', 944),
    ('n', 611),
    ('o', 611),
    ('p', 611),
    ('q', 611),
    ('r', 389),
    ('s', 500),
    ('t', 333),
    ('u', 611),
    ('v', 556),
    ('w', 833),
    ('x', 556),
    ('y', 556),
    ('z', 444),
];

/// Univers Bold advances (1/1000 em), sorted by character, as [`UNIVERS`].
const UNIVERS_BOLD: &[(char, u16)] = &[
    (' ', 278),
    ('#', 556),
    ('&', 778),
    ('\'', 238),
    ('(', 333),
    (')', 333),
    (',', 278),
    ('-', 333),
    ('.', 278),
    ('/', 444),
    ('0', 556),
    ('1', 556),
    ('2', 556),
    ('3', 556),
    ('4', 556),
    ('5', 556),
    ('6', 556),
    ('7', 556),
    ('8', 556),
    ('9', 556),
    (':', 278),
    ('=', 584),
    ('A', 722),
    ('B', 667),
    ('C', 667),
    ('D', 722),
    ('E', 611),
    ('F', 556),
    ('G', 778),
    ('H', 722),
    ('I', 278),
    ('J', 556),
    ('K', 667),
    ('L', 556),
    ('M', 944),
    ('N', 778),
    ('O', 778),
    ('P', 611),
    ('Q', 778),
    ('R', 667),
    ('S', 667),
    ('T', 611),
    ('U', 778),
    ('V', 722),
    ('W', 1000),
    ('X', 722),
    ('Y', 667),
    ('Z', 611),
    ('a', 556),
    ('b', 611),
    ('c', 500),
    ('d', 611),
    ('e', 556),
    ('f', 333),
    ('g', 611),
    ('h', 611),
    ('i', 278),
    ('j', 278),
    ('k', 556),
    ('l', 278),
    ('m', 944),
    ('n', 611),
    ('o', 611),
    ('p', 611),
    ('q', 611),
    ('r', 389),
    ('s', 500),
    ('t', 389),
    ('u', 611),
    ('v', 556),
    ('w', 889),
    ('x', 556),
    ('y', 556),
    ('z', 500),
];

/// Is a line's (ascent, descent) one `upem` em tall?
fn one_em((ascent, descent): (f64, f64), upem: f64) -> bool {
    upem > 0.0 && ((ascent + descent) / upem - 1.0).abs() < 0.005
}

/// The font a Windows 3.1-era charset name stands for: `Times New Roman Cyr`, `Arial CE`,
/// `Courier New Baltic`, `Arial (Hebrew)`… are aliases Windows keeps (its FontSubstitutes) for
/// the font itself, and old Russian and Central European documents name them. Other names are
/// returned unchanged.
pub fn charset_base_family(family: &str) -> &str {
    const SUFFIXES: &[&str] = &[" cyr", " ce", " greek", " tur", " baltic", " (arabic)", " (hebrew)", " (vietnamese)"];
    let lower = family.trim_end().to_ascii_lowercase();
    SUFFIXES
        .iter()
        .find(|s| lower.ends_with(*s) && lower.len() > s.len())
        .and_then(|s| family.trim_end().get(..lower.len() - s.len()))
        .map_or(family, str::trim_end)
}

/// Fonts made to measure alike (the same advance widths and line height), so text drawn in one
/// wraps and paginates as in the others: Carlito for Calibri, Caladea for Cambria, Liberation and
/// Croscore fonts for Arial, Times New Roman and Courier New, the Palatino look-alikes for Palatino
/// Linotype and Book Antiqua, and Arial and Helvetica (drawn to Helvetica's widths) for one
/// another. The one list of them: a missing font is drawn in its first installed twin
/// ([`substitute`]), and text drawn in a twin doesn't change the layout ([`metric_compatible`]).
/// Not macOS Times or Courier: their advance widths differ from Times New Roman's and Courier
/// New's.
const METRIC_TWINS: &[&[&str]] = &[
    &["Arial", "Liberation Sans", "Helvetica", "Arimo"],
    &["Times New Roman", "Liberation Serif", "Tinos"],
    &["Courier New", "Liberation Mono", "Cousine"],
    &["Calibri", "Carlito"],
    &["Cambria", "Caladea"],
    &["Palatino Linotype", "Palatino", "Book Antiqua", "TeX Gyre Pagella", "P052", "URW Palladio L", "Palladio"],
];

/// The fonts that measure like `family` ([`METRIC_TWINS`]), in the order they are tried, without
/// `family` itself.
fn metric_twins(family: &str) -> impl Iterator<Item = &'static str> + '_ {
    let f = family.trim();
    METRIC_TWINS
        .iter()
        .filter(move |g| g.iter().any(|t| t.eq_ignore_ascii_case(f)))
        .flat_map(|g| g.iter().copied())
        .filter(move |t| !t.eq_ignore_ascii_case(f))
}

/// Whether `substitute` was made to measure like `family` ([`METRIC_TWINS`]), so text drawn in it
/// wraps and paginates as in `family`.
pub fn metric_compatible(family: &str, substitute: &str) -> bool {
    let s = substitute.trim();
    family.split(';').map(str::trim).filter(|f| !f.is_empty()).any(|f| {
        let base = charset_base_family(f);
        base.eq_ignore_ascii_case(s) || metric_twins(base).any(|t| t.eq_ignore_ascii_case(s))
    })
}

/// Naskh-style Persian/Arabic document fonts (the "B …" family names come from the common
/// Persian font packs; matched by name only).
fn is_persian_naskh(f: &str) -> bool {
    const NAMES: &[&str] = &[
        "b nazanin",
        "b lotus",
        "b zar",
        "b mitra",
        "b badr",
        "b yagut",
        "b roya",
        "b compset",
        "b nazanin bold",
        "b lotus bold",
        "nazanin",
        "lotus",
        "zar",
        "mitra",
        "badr",
        "yagut",
        "roya",
        "ir nazanin",
        "ir lotus",
        "ir zar",
        "ir mitra",
        "ir badr",
        "irnazanin",
        "irlotus",
        "irzar",
        "irmitra",
        "irbadr",
        "traditional arabic",
        "simplified arabic",
        "arabic typesetting",
        "sakkal majalla",
        "adobe arabic",
        "andalus",
        "times new roman (arabic)",
    ];
    NAMES.contains(&f)
}

/// Sans Persian/Arabic fonts for headings and screens.
fn is_persian_sans(f: &str) -> bool {
    const NAMES: &[&str] = &[
        "b yekan",
        "b titr",
        "b homa",
        "b koodak",
        "b traffic",
        "b jadid",
        "b elham",
        "b kamran",
        "b davat",
        "b ferdosi",
        "yekan",
        "titr",
        "homa",
        "koodak",
        "traffic",
        "ir titr",
        "ir homa",
        "irtitr",
        "irhoma",
        "iransans",
        "iransansweb",
        "iran sans",
        "iranyekan",
        "iran yekan",
        "iranyekanweb",
        "dana",
        "estedad",
        "dubai",
        "aldhabi",
        "urdu typesetting",
        "arabic transparent",
    ];
    NAMES.contains(&f) || f.starts_with("iransans") || f.starts_with("iranyekan")
}

/// Thai document fonts: Word's Thai families (Cordia New, Angsana New, Browallia New, the UPC
/// families, Leelawadee), the Thai national fonts (TH Sarabun New and the other "TH …" fonts),
/// the Mac's, the Thai Linux Working Group's and Noto's. Matched by name only.
fn is_thai(f: &str) -> bool {
    const NAMES: &[&str] = &[
        "cordia new",
        "cordiaupc",
        "angsana new",
        "angsanaupc",
        "browallia new",
        "browalliaupc",
        "leelawadee",
        "leelawadee ui",
        "leelawadee ui semilight",
        "th sarabun new",
        "th sarabunpsk",
        "th sarabun psk",
        "th sarabun",
        "thonburi",
        "ayuthaya",
        "sathu",
        "krungthep",
        "silom",
        "sukhumvit set",
        "norasi",
        "kinnari",
        "garuda",
        "laksaman",
        "loma",
        "purisa",
        "sawasdee",
        "umpush",
        "waree",
        "tlwg typo",
        "tlwg typist",
        "tlwg mono",
    ];
    NAMES.contains(&f) || f.ends_with("upc") || f.starts_with("th ") || f.contains("thai")
}

/// Serif Thai document fonts.
fn is_thai_serif(f: &str) -> bool {
    const NAMES: &[&str] = &["angsana new", "angsanaupc", "eucrosiaupc", "jasmineupc", "kodchiangupc", "dilleniaupc", "norasi", "kinnari"];
    NAMES.contains(&f) || (f.contains("thai") && f.contains("serif"))
}

/// Is `family` a Thai document font ([`is_thai`])? A missing one is drawn in a Thai face for the
/// whole run, never in a Latin substitute (#1549).
pub fn is_thai_family(family: &str) -> bool {
    if family.contains(';') {
        return list_class(family).thai;
    }
    let f = family.trim().to_ascii_lowercase();
    is_thai(&f) || is_thai_serif(&f)
}

/// What the name-based predicates answer about one semicolon-separated fallback list. Shaping asks
/// them for every piece of every run set in the list, so the answer is worked out once per list
/// rather than once per piece.
#[derive(Clone, Copy, Default)]
struct ListClass {
    /// [`is_thai_family`].
    thai: bool,
    /// [`has_named_substitutes`].
    named: bool,
}

/// [`ListClass`] of the semicolon-separated `family`, memoised by its text.
fn list_class(family: &str) -> ListClass {
    let key = family_list_key(family);
    let generation = FontDb::global().generation();
    {
        let mut c = fallback_list_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if c.generation != generation {
            *c = FallbackListMemo { generation, ..Default::default() };
        } else if let Some((cached, class)) = c.classes.get(&key)
            && cached == family
        {
            return *class;
        }
    }
    let class = ListClass { thai: classify_thai(family), named: classify_named(family) };
    let mut c = fallback_list_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if c.generation == generation {
        if c.classes.len() >= FAMILY_CACHE_MAX {
            c.classes.clear();
        }
        c.classes.insert(key, (family.to_string(), class));
    }
    class
}

/// Whether any family of the list is a Thai document font: what [`is_thai_family`] answers.
fn classify_thai(family: &str) -> bool {
    family.split(';').map(str::trim).filter(|f| !f.is_empty()).any(|f| {
        let f = f.to_ascii_lowercase();
        is_thai(&f) || is_thai_serif(&f)
    })
}

/// Whether any family of the list has substitutes of its own: what [`has_named_substitutes`]
/// answers.
fn classify_named(family: &str) -> bool {
    family.split(';').map(str::trim).filter(|f| !f.is_empty()).any(|f| substitutes_by_name(charset_base_family(f)).1)
}

/// Khmer document fonts: the Khmer OS family, Khmer Mool1 and the like, Windows' and the Mac's
/// own and Noto's. Matched by name only.
fn is_khmer(f: &str) -> bool {
    f.starts_with("khmer")
        || f.contains("khmer")
        || matches!(f, "daunpenh" | "moolboran" | "limon" | "battambang" | "bokor" | "content" | "dangrek" | "angkor")
}

/// Hebrew document fonts.
fn is_hebrew(f: &str) -> bool {
    const NAMES: &[&str] = &["david", "miriam", "noto sans hebrew", "arial hebrew", "calibri hebrew"];
    NAMES.contains(&f) || f.contains("hebrew")
}

/// The first installed substitute for a missing `family`. For a Thai font, the first with Latin
/// letters and digits as well as Thai ones (a Thai face without Latin would leave a mixed run in
/// two fonts, #1549), else the first with Thai letters, else the first installed.
fn substitute(db: &FontDb, family: &str) -> Option<&'static str> {
    // A twin draws the text as the font would: Carlito for Calibri, Helvetica for Arial on a Mac.
    if let Some(twin) = metric_twins(family).find(|t| db.has_family(t)) {
        return Some(twin);
    }
    let mut installed = substitutes(family).iter().copied().filter(|s| db.has_family(s));
    if !is_thai_family(family) {
        return installed.next();
    }
    let installed: Vec<&'static str> = installed.collect();
    let covers = |name: &str, chars: &[char]| {
        let face = db.face(name, "Regular");
        face.named(name) && chars.iter().all(|c| face.covers(*c))
    };
    installed
        .iter()
        .copied()
        .find(|s| covers(s, &['\u{0E19}', 'A', 'z', '0', '9']))
        .or_else(|| installed.iter().copied().find(|s| covers(s, &['\u{0E19}'])))
        .or_else(|| installed.first().copied())
}

/// Windows ships Century only as Regular, and Word's PDF embeds that face for bold runs with
/// regular advances (C059 Bold is about 11–13% wider), so substitutes must use synthetic bold.
fn windows_has_no_bold(family: &str) -> bool {
    family.eq_ignore_ascii_case("century")
}

fn style_name(bold: bool, italic: bool) -> &'static str {
    match (bold, italic) {
        (false, false) => "Regular",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (true, true) => "Bold Italic",
    }
}

type Memo = (u64, HashMap<(String, bool, bool), Resolved>);

fn resolve_cache() -> &'static Mutex<Memo> {
    static CACHE: OnceLock<Mutex<Memo>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new((0, HashMap::new())))
}

const FAMILY_CACHE_MAX: usize = 4096;
/// Family names longer than this (bytes) are resolved but never remembered as string keys.
const FAMILY_KEY_MAX: usize = 256;
/// Maximum number of direct `FontDb::has_family` catalog scans per uncached fallback list
/// for candidate names not present in [`FontDb::families`].
const MAX_DIRECT_CANDIDATE_PROBES: usize = 32;

/// Memoised fallback-list resolutions and installed-family lookups for the current [`FontDb::generation`].
#[derive(Default)]
struct FallbackListMemo {
    generation: u64,
    families: std::collections::HashSet<String>,
    probed: HashMap<String, bool>,
    lists: HashMap<(u64, usize), (String, (String, bool))>,
    long_resolved: HashMap<(u64, usize, bool, bool), (String, Resolved)>,
    classes: HashMap<(u64, usize), (String, ListClass)>,
}

/// Shared cache for fallback-list resolutions and candidate availability checks.
fn fallback_list_cache() -> &'static Mutex<FallbackListMemo> {
    static CACHE: OnceLock<Mutex<FallbackListMemo>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(FallbackListMemo::default()))
}

/// Hash and byte length of `family`, used as a compact cache key for fallback lists.
fn family_list_key(family: &str) -> (u64, usize) {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    family.hash(&mut h);
    (h.finish(), family.len())
}

/// Whether `cand` is an installed family in `db`, using the memoised family set and bounded
/// direct catalog probes so long fallback lists do not rescan the font catalog for every candidate.
fn candidate_has_family(db: &FontDb, cand: &str, direct_probes: &mut usize) -> bool {
    let cand = cand.trim();
    if cand.is_empty() || cand.len() > FAMILY_KEY_MAX {
        return false;
    }
    let generation = db.generation();
    let lower = cand.to_ascii_lowercase();
    {
        let mut c = fallback_list_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if c.generation != generation {
            *c = FallbackListMemo { generation, ..Default::default() };
        }
        if c.families.is_empty() {
            c.families = db.families().into_iter().map(|f| f.to_ascii_lowercase()).collect();
        }
        if c.families.contains(&lower) {
            return true;
        }
        if let Some(&hit) = c.probed.get(&lower) {
            return hit;
        }
    }
    if *direct_probes == 0 {
        return false;
    }
    *direct_probes -= 1;
    let hit = db.has_family(cand);
    let mut c = fallback_list_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if c.generation == generation {
        if c.probed.len() >= FAMILY_CACHE_MAX {
            c.probed.clear();
        }
        c.probed.insert(lower, hit);
    }
    hit
}

/// Record a resolved `(family, substituted)` pair for a semicolon-separated fallback list.
fn remember_list_resolution(generation: u64, key: (u64, usize), family: &str, result: (String, bool)) -> (String, bool) {
    let mut c = fallback_list_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if c.generation == generation {
        if c.lists.len() >= FAMILY_CACHE_MAX {
            c.lists.clear();
        }
        c.lists.insert(key, (family.to_string(), result.clone()));
    }
    result
}

/// Resolve a single family or semicolon-separated fallback list to `(family_name, substituted)`.
fn resolve_family(db: &FontDb, family: &str) -> (String, bool) {
    if !family.contains(';') {
        let base = if family.trim().eq_ignore_ascii_case("Symbol (PCL6)") { "Symbol" } else { charset_base_family(family) };
        return if db.has_family(family) {
            (family.to_string(), false)
        } else if base != family && db.has_family(base) {
            (base.to_string(), false)
        } else {
            (substitute(db, base).unwrap_or(crate::FALLBACK_FAMILY).to_string(), true)
        };
    }

    let generation = db.generation();
    let list_key = family_list_key(family);
    {
        let mut c = fallback_list_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if c.generation != generation {
            *c = FallbackListMemo { generation, ..Default::default() };
        } else if let Some((cached_family, r)) = c.lists.get(&list_key)
            && cached_family == family
        {
            return r.clone();
        }
    }

    let candidates: Vec<&str> = family.split(';').map(str::trim).filter(|s| !s.is_empty() && s.len() <= FAMILY_KEY_MAX).collect();
    if candidates.is_empty() {
        return remember_list_resolution(generation, list_key, family, (crate::FALLBACK_FAMILY.to_string(), true));
    }

    let mut direct_probes = MAX_DIRECT_CANDIDATE_PROBES;

    // 1. First installed family in the list:
    for &cand in &candidates {
        let base = if cand.eq_ignore_ascii_case("Symbol (PCL6)") { "Symbol" } else { charset_base_family(cand) };
        if candidate_has_family(db, cand, &mut direct_probes) {
            return remember_list_resolution(generation, list_key, family, (cand.to_string(), false));
        }
        if base != cand && candidate_has_family(db, base, &mut direct_probes) {
            return remember_list_resolution(generation, list_key, family, (base.to_string(), false));
        }
    }

    // 2. None installed: fall back to an installed substitute for an entry in the list rather than generic Carlito.
    // Try Thai substitutes before metric twins of later candidates:
    for &cand in &candidates {
        let base = if cand.eq_ignore_ascii_case("Symbol (PCL6)") { "Symbol" } else { charset_base_family(cand) };
        if is_thai_family(base)
            && let Some(sub) = substitute(db, base)
        {
            return remember_list_resolution(generation, list_key, family, (sub.to_string(), true));
        }
        let mut twin_probes = MAX_DIRECT_CANDIDATE_PROBES;
        if let Some(twin) = metric_twins(base).find(|t| candidate_has_family(db, t, &mut twin_probes)) {
            return remember_list_resolution(generation, list_key, family, (twin.to_string(), true));
        }
    }

    // Try candidates with named or category substitutes (Thai, Hebrew, Khmer, Persian, serif, mono):
    for &cand in &candidates {
        let base = if cand.eq_ignore_ascii_case("Symbol (PCL6)") { "Symbol" } else { charset_base_family(cand) };
        let (subs, named) = substitutes_by_name(base);
        if (named || is_thai_family(base) || subs != SANS)
            && let Some(sub) = substitute(db, base)
        {
            return remember_list_resolution(generation, list_key, family, (sub.to_string(), true));
        }
    }

    // Fall back to the first candidate's substitute:
    if let Some(&first) = candidates.first() {
        let base = if first.eq_ignore_ascii_case("Symbol (PCL6)") { "Symbol" } else { charset_base_family(first) };
        if let Some(sub) = substitute(db, base) {
            return remember_list_resolution(generation, list_key, family, (sub.to_string(), true));
        }
    }

    remember_list_resolution(generation, list_key, family, (crate::FALLBACK_FAMILY.to_string(), true))
}

/// Resolve `family` with bold/italic to a face (memoised until user fonts are added or removed,
/// [`FontDb::generation`]).
pub fn resolve(family: &str, bold: bool, italic: bool) -> Resolved {
    let cache = resolve_cache();
    let db = FontDb::global();
    let generation = db.generation();
    let cacheable = family.len() <= FAMILY_KEY_MAX;
    let key = (if cacheable { family.to_string() } else { String::new() }, bold, italic);
    let long_key = (!cacheable && family.contains(';')).then(|| {
        let (h, len) = family_list_key(family);
        (h, len, bold, italic)
    });
    {
        let mut c = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if c.0 != generation {
            *c = (generation, HashMap::new());
        }
        if cacheable && let Some(r) = c.1.get(&key) {
            return *r;
        }
    }
    if let Some(lk) = long_key {
        let mut c = fallback_list_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if c.generation != generation {
            *c = FallbackListMemo { generation, ..Default::default() };
        } else if let Some((cached_family, r)) = c.long_resolved.get(&lk)
            && cached_family == family
        {
            return *r;
        }
    }
    let (fam, substituted) = resolve_family(db, family);
    // A substitute for a family Windows ships without a bold face is drawn with synthetic bold.
    let first = family.split(';').map(str::trim).next().unwrap_or(family);
    let base = if first.eq_ignore_ascii_case("Symbol (PCL6)") { "Symbol" } else { charset_base_family(first) };
    let force_synth_bold = substituted && windows_has_no_bold(base);
    let face = db.face(&fam, style_name(bold && !force_synth_bold, italic));
    let r = Resolved {
        synth_bold: bold && (force_synth_bold || face.weight < 600.0),
        synth_italic: italic && !face.italic,
        substituted,
        face: FaceRef::of(&face),
    };
    if cacheable {
        let mut c = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if c.0 == generation {
            if c.1.len() >= FAMILY_CACHE_MAX {
                c.1.clear();
            }
            c.1.insert(key, r);
        }
    } else if let Some(lk) = long_key {
        let mut c = fallback_list_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if c.generation == generation {
            if c.long_resolved.len() >= FAMILY_CACHE_MAX {
                c.long_resolved.clear();
            }
            c.long_resolved.insert(lk, (family.to_string(), r));
        }
    }
    r
}

#[cfg(test)]
/// A line's (ascent, descent) from hhea values, both positive: the line gap goes above the ascent.
fn hhea_line(ascender: i16, descender: i16, line_gap: i16) -> (f64, f64) {
    hhea_line_leading(ascender, descender, line_gap, true)
}

/// A line's (ascent, descent) from hhea values, both positive. With `leading` the line gap goes
/// above the ascent; without it (the compatibility option "Don't add leading (extra space)
/// between rows of text") the line is the ascender plus the descender only.
pub fn hhea_line_leading(ascender: i16, descender: i16, line_gap: i16, leading: bool) -> (f64, f64) {
    let gap = if leading { f64::from(line_gap.max(0)) } else { 0.0 };
    (f64::from(ascender) + gap, f64::from(descender).abs())
}

/// [`hhea_line_leading`] for a face that also has OS/2 Windows metrics (`usWinAscent`,
/// `usWinDescent`). Without `leading`, only the external leading goes: what the hhea line
/// (ascender + descender + gap) has on top of the Windows cell (win ascent + descent), never more
/// than the gap, taken off the top. A font whose gap lies inside its Windows cell (Calibri,
/// Consolas) keeps its line; Times New Roman loses its whole gap. A missing or empty Windows
/// cell, or one smaller than ascender + descender, removes the whole gap.
pub fn win_line_leading(ascender: i16, descender: i16, line_gap: i16, win_ascent: u16, win_descent: u16, leading: bool) -> (f64, f64) {
    if leading {
        return hhea_line_leading(ascender, descender, line_gap, true);
    }
    let gap = f64::from(line_gap.max(0));
    let (asc, desc) = (f64::from(ascender), f64::from(descender).abs());
    let cell = f64::from(win_ascent) + f64::from(win_descent);
    let ext = if cell <= 0.0 { gap } else { (asc + desc + gap - cell).clamp(0.0, gap) };
    (asc + gap - ext, desc)
}

/// The line (ascent, descent) in units of an `upem` em that Word gives the classic PostScript
/// families: their Windows counterparts' spacing, though Apple's own Times and Courier are one em
/// tall (hhea 0.75 / -0.25, no gap). Measured in Word for Mac on synthetic documents (single
/// spacing, 11 and 20 pt): Times lines lie exactly as far apart as Times New Roman's (hhea 1825 /
/// -443, gap 87 per 2048), Courier's as Courier New's (1705 / -615, no gap). On Windows, Word
/// draws a "Times" document in Times New Roman anyway.
fn borrowed_line(family: &str, upem: f64, leading: bool) -> Option<(f64, f64)> {
    let (a, d) = match family.trim().to_ascii_lowercase().as_str() {
        "times" => hhea_line_leading(1825, -443, 87, leading),
        "courier" => hhea_line_leading(1705, -615, 0, leading),
        _ => return None,
    };
    Some((a * upem / 2048.0, d * upem / 2048.0))
}

/// Line metrics for a face in font units: (ascent, descent), both positive, the way word
/// processors measure single line spacing: the hhea values with the line gap above the ascent
/// (Word puts the first baseline that far below the line top), falling back to OS/2
/// usWinAscent/usWinDescent; Times and Courier borrow their Windows counterparts' (see
/// [`borrowed_line`]).
pub fn line_metrics(face: &crate::FontFace) -> (f64, f64) {
    face_line(face, true)
}

/// [`line_metrics`] without the line gap: the line Word gives a face under the compatibility
/// option "Don't add leading (extra space) between rows of text": the hhea line without the part of
/// its gap outside the OS/2 Windows cell ([`win_line_leading`]); Times borrows Times New Roman's
/// without its gap. OS/2 win metrics alone have no gap and are the same.
pub fn line_metrics_no_leading(face: &crate::FontFace) -> (f64, f64) {
    face_line(face, false)
}

fn face_line(face: &crate::FontFace, leading: bool) -> (f64, f64) {
    static CACHE: OnceLock<Mutex<HashMap<(u32, bool), (f64, f64)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (face.id(), leading);
    if let Some(v) = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&key) {
        return *v;
    }
    let mut v = (face.ascent.max(1.0), face.descent.abs());
    if let Some(m) = borrowed_line(&face.family, face.upem, leading) {
        v = m;
    } else if let Some(f) = face.skrifa() {
        // hhea ascender + descender + line gap (Word on macOS), the gap above the ascent; OS/2
        // win metrics only when the hhea values are missing, and never more than twice the em.
        // Without leading, only the gap outside the OS/2 Windows cell goes (`win_line_leading`).
        let os2 = f.os2().ok();
        let hhea = f.hhea().ok().map(|h| {
            let (a, d, g) = (h.ascender().to_i16(), h.descender().to_i16(), h.line_gap().to_i16());
            match &os2 {
                Some(o) if !leading => win_line_leading(a, d, g, o.us_win_ascent(), o.us_win_descent(), false),
                _ => hhea_line_leading(a, d, g, leading),
            }
        });
        let win = os2.map(|o| (o.us_win_ascent() as f64, o.us_win_descent() as f64));
        let ok = |m: &(f64, f64)| m.0 > 0.0 && m.0 + m.1 <= face.upem * 2.0;
        if let Some(m) = hhea.filter(ok) {
            v = m;
        } else if let Some(m) = win.filter(ok) {
            v = m;
        }
    }
    cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(key, v);
    v
}

/// Word's OS/2 average character width (em) of `requested` when its text is drawn in the metric
/// twin `face_family` ([`METRIC_TWINS`]), whose own table holds a different average: Times New
/// Roman 821/2048 (Liberation Serif and Tinos store about .55 em), Arial 904/2048 (Liberation Sans
/// and Arimo about .58 em). `None` for the font itself and for any other pair: their own field
/// is used (Carlito's already gives no room, like Calibri's).
pub fn twin_avg_char_width_em(requested: &str, face_family: &str) -> Option<f32> {
    const AVERAGES: &[(&str, f32)] = &[("Times New Roman", 821.0 / 2048.0), ("Arial", 904.0 / 2048.0)];
    let base = charset_base_family(requested.trim()).trim();
    let face = face_family.trim();
    let &(_, avg) = AVERAGES.iter().find(|(name, _)| name.eq_ignore_ascii_case(base))?;
    (!face.eq_ignore_ascii_case(base) && metric_twins(base).any(|t| t.eq_ignore_ascii_case(face))).then_some(avg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_advances_are_symbols_own() {
        let near = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 1e-9);
        assert!(near(symbol_advance("Symbol", 0x79), 0.686));
        assert!(near(symbol_advance("symbol (pcl6)", 0x44), 0.612));
        assert!(near(symbol_advance("SYMBOL", 0x20), 0.250));
        assert!(near(symbol_advance("Symbol", 0xB7), 0.460));
        assert!(near(symbol_advance("Symbol", 0xE5), 0.713));
    }

    #[test]
    fn symbol_advances_only_for_symbols_glyphs() {
        assert_eq!(symbol_advance("Symbol", 0x80), None);
        assert_eq!(symbol_advance("Symbol", 0xFF), None);
        assert_eq!(symbol_advance("Symbol", 0xF0), None);
        assert_eq!(symbol_advance("Symbol", 0x00), None);
        assert_eq!(symbol_advance("Wingdings", 0x79), None);
        assert_eq!(symbol_advance("Times New Roman", 0x79), None);
        assert_eq!(symbol_advance("", 0x79), None);
        // Every code is looked up without a panic.
        let n = (0..=u8::MAX).filter(|c| symbol_advance("Symbol", *c).is_some()).count();
        assert_eq!(n, 189);
    }

    #[test]
    fn missing_font_advance_tables_are_sorted() {
        // `missing_font_advance` binary-searches them: every listed character must be found.
        for table in [UNIVERS, UNIVERS_BOLD] {
            assert!(table.windows(2).all(|w| w[0].0 < w[1].0));
        }
    }

    #[test]
    fn the_line_gap_goes_above_the_ascent() {
        // Times New Roman (hhea 1825 / -443, gap 87), and a font with a large gap: Word puts the
        // first baseline the ascent plus the whole gap below the line top.
        assert_eq!(hhea_line(1825, -443, 87), (1912.0, 443.0));
        assert_eq!(hhea_line(1536, -512, 452), (1988.0, 512.0));
        // A negative gap adds nothing; the most negative descender doesn't overflow.
        assert_eq!(hhea_line(1000, -300, -50), (1000.0, 300.0));
        assert_eq!(hhea_line(1000, i16::MIN, 0), (1000.0, 32768.0));
    }

    #[test]
    fn an_installed_font_with_its_gap_inside_the_windows_cell_keeps_its_line() {
        // Carlito and Calibri have hhea 1536 / -512 / 452 and win 1950 + 550: the option leaves
        // the line as it is. Skipped when neither is installed.
        for family in ["Carlito", "Calibri"] {
            let r = resolve(family, false, false);
            if r.substituted {
                continue;
            }
            let Some(f) = r.face.skrifa() else { continue };
            let (Ok(h), Ok(o)) = (f.hhea(), f.os2()) else { continue };
            let hhea = f64::from(h.ascender().to_i16()) + f64::from(h.descender().to_i16()).abs() + f64::from(h.line_gap().to_i16().max(0));
            let win = f64::from(o.us_win_ascent()) + f64::from(o.us_win_descent());
            if hhea != win {
                continue;
            }
            assert_eq!(line_metrics_no_leading(&r.face), line_metrics(&r.face), "{family}");
        }
    }

    #[test]
    fn missing_century_gothic_and_lucida_bright_lose_their_leading() {
        for (family, plain, option) in [("Century Gothic", 1.226, 1.1916), ("Lucida Bright", 1.172, 1.1386)] {
            let r = resolve(family, false, false);
            if !r.substituted {
                continue;
            }
            let sum = |m: Option<(f64, f64)>| m.map(|(a, d)| a + d);
            let p = sum(missing_font_line_leading(family, &r, true)).unwrap_or(0.0);
            let o = sum(missing_font_line_leading(family, &r, false)).unwrap_or(0.0);
            assert!((p - plain).abs() < 0.001 && (o - option).abs() < 0.001, "{family}: {p} {o}");
        }
        // The other table lines stay.
        for family in ["Consolas", "Palatino Linotype", "Georgia"] {
            let r = resolve(family, false, false);
            assert_eq!(missing_font_line_leading(family, &r, false), missing_font_line(family, &r), "{family}");
        }
    }

    #[test]
    fn times_and_courier_take_their_windows_counterparts_line() {
        // Apple's Times has 2048 units to the em, Times New Roman's line is 1.15 em.
        assert_eq!(borrowed_line("Times", 2048.0, true), Some((1912.0, 443.0)));
        assert_eq!(borrowed_line(" times ", 1000.0, true), Some((1912.0 / 2.048, 443.0 / 2.048)));
        assert_eq!(borrowed_line("Courier", 2048.0, true), Some((1705.0, 615.0)));
        // Only those two: the Windows fonts themselves, and other Mac fonts, keep their own.
        for family in ["Times New Roman", "Courier New", "Helvetica", "Hoefler Text", "Palatino", ""] {
            assert_eq!(borrowed_line(family, 2048.0, true), None, "{family}");
        }
    }

    #[test]
    fn no_leading_lines_leave_out_the_line_gap() {
        // Word's line pitch (pt) with "Don't add leading between rows of text" and without it:
        // size x (ascender + descender [+ gap]) / 2048.
        let pitch = |m: (f64, f64), size: f64| (m.0 + m.1) * size / 2048.0;
        for ((asc, desc, gap), size, without_gap, with_gap) in [
            ((1825, -443, 87), 14.0, 15.50, 16.10),
            ((1825, -443, 87), 12.0, 13.29, 13.80),
            ((1825, -443, 87), 10.0, 11.07, 11.50),
            ((1854, -434, 67), 14.0, 15.64, 16.10),
        ] {
            let bare = pitch(hhea_line_leading(asc, desc, gap, false), size);
            let full = pitch(hhea_line_leading(asc, desc, gap, true), size);
            assert!((bare - without_gap).abs() <= 0.01, "{asc} {size}: {bare} vs {without_gap}");
            assert!((full - with_gap).abs() <= 0.01, "{asc} {size}: {full} vs {with_gap}");
        }
        // The gap comes off the top: the descent is the same either way.
        assert_eq!(hhea_line_leading(1825, -443, 87, false), (1825.0, 443.0));
        // Times borrows Times New Roman's line, without the gap too.
        assert_eq!(borrowed_line("Times", 2048.0, false), Some((1825.0, 443.0)));
    }

    #[test]
    fn family_cache_stays_bounded() {
        for i in 0..(FAMILY_CACHE_MAX + 50) {
            let _ = resolve(&format!("Zz Missing Family {i}"), false, false);
        }
        assert!(resolve_cache().lock().unwrap_or_else(|e| e.into_inner()).1.len() <= FAMILY_CACHE_MAX);
        let long = "L".repeat(10_000);
        let _ = resolve(&long, false, false);
        assert!(!resolve_cache().lock().unwrap_or_else(|e| e.into_inner()).1.keys().any(|k| k.0.len() > 1000));
    }

    #[test]
    fn metric_compatible_substitutes_are_known() {
        assert!(metric_compatible("Calibri", "Carlito"));
        assert!(metric_compatible(" arial ", "Liberation Sans"));
        assert!(metric_compatible("Arial CE", "Helvetica"));
        assert!(metric_compatible("Book Antiqua", "TeX Gyre Pagella"));
        assert!(!metric_compatible("Calibri", "Helvetica Neue"));
        assert!(!metric_compatible("Open Sans", "Carlito"));
        assert!(!metric_compatible("Times New Roman", "Times"));
        assert_eq!(metric_twins("arial").collect::<Vec<_>>(), ["Liberation Sans", "Helvetica", "Arimo"]);
    }

    #[test]
    fn a_missing_font_is_drawn_in_an_installed_twin() {
        // The twins that decide the font box's warning are the faces layout draws in.
        let db = FontDb::global();
        for family in METRIC_TWINS.iter().flat_map(|g| g.iter()) {
            let r = resolve(family, false, false);
            if r.substituted && metric_twins(family).any(|t| db.has_family(t)) {
                assert!(metric_compatible(family, &r.face.family), "{family} → {}", r.face.family);
            }
        }
    }

    #[test]
    fn unknown_family_substitutes() {
        let r = resolve("Definitely Not A Font 123", false, false);
        assert!(r.substituted);
        let b = resolve("Source Serif 4", true, false);
        assert!(!b.substituted);
        assert!(!b.synth_bold);
        let (a, d) = line_metrics(&b.face);
        assert!(a > 0.0 && d >= 0.0);
    }

    #[test]
    fn palatino_family_prefers_palatino_look_alikes_over_generic_serif() {
        for name in ["Palatino Linotype", "Book Antiqua", "palatino"] {
            let list = substitutes(name);
            let pos = |n: &str| list.iter().position(|x| *x == n);
            assert!(pos("Palatino").is_some() && pos("TeX Gyre Pagella").is_some() && pos("P052").is_some(), "{name}");
            // All of them come before the generic serif fallbacks (Caladea, Times New Roman).
            assert!(pos("Palatino") < pos("Caladea") && pos("TeX Gyre Pagella") < pos("Times New Roman"), "{name}");
        }
        // Other serif families are unchanged.
        assert_eq!(substitutes("Georgia").first(), Some(&"Caladea"));
        // Charset aliases are the font itself, so they substitute like it.
        assert_eq!(charset_base_family("Times New Roman Cyr"), "Times New Roman");
        assert_eq!(charset_base_family("Arial CYR"), "Arial");
        assert_eq!(charset_base_family("Courier New CE"), "Courier New");
        assert_eq!(charset_base_family("Arial (Hebrew)"), "Arial");
        assert_eq!(charset_base_family("Cyr"), "Cyr");
        assert_eq!(charset_base_family("Calibri"), "Calibri");
        assert_eq!(substitutes(charset_base_family("Times New Roman Cyr")), substitutes("Times New Roman"));
    }

    #[test]
    fn pcl6_symbol_has_symbols_line_metrics() {
        assert_eq!(symbol_font_metrics("Symbol (PCL6)"), symbol_font_metrics("Symbol"));
        assert_eq!(symbol_font_metrics("Arial"), None);
    }

    #[test]
    fn a_one_em_symbol_face_takes_windows_symbols_line() {
        // Apple's Symbol (2048 units): hhea 1436 / -612, no gap, one em; Windows' Symbol line is 1.221 em.
        assert!(one_em(hhea_line(1436, -612, 0), 2048.0));
        assert!(!one_em((1005.0, 216.0), 1000.0));
        assert!(!one_em((899.0, 211.0), 1000.0));
        assert!(!one_em((0.0, 0.0), 0.0));
        let arial = resolve("Arial", false, false);
        assert_eq!(symbol_line("Arial", &arial), None);
        let symbol = resolve("Symbol", false, false);
        if symbol.substituted {
            assert_eq!(symbol_line("Symbol", &symbol), symbol_font_metrics("Symbol"));
        }
    }

    #[test]
    fn pcl6_symbol_resolves_as_symbol() {
        let actual = resolve("Symbol (PCL6)", false, false);
        let db = FontDb::global();
        if db.has_family("Symbol (PCL6)") {
            assert_eq!(actual.face.id(), db.face("Symbol (PCL6)", "Regular").id());
            assert!(!actual.substituted);
            return;
        }
        let expected = resolve("Symbol", false, false);
        assert_eq!(actual.face.id(), expected.face.id());
        assert_eq!(actual.substituted, expected.substituted);
    }

    #[test]
    fn unicode_arial_is_proportional() {
        for name in ["Arial Unicode MS", "ARIAL UNICODE MS"] {
            let candidates = substitutes(name);
            assert_eq!(candidates.first(), Some(&"Arial"));
            assert_eq!(&candidates[1..], substitutes("Arial"));
        }
        assert_eq!(substitutes("Cascadia Code").first(), Some(&"Courier New"));
    }

    #[test]
    fn symbol_fonts_substitute_symbol_faces() {
        for f in ["Symbol", "Wingdings", "Wingdings 2", "Wingdings 3", "Webdings"] {
            assert!(substitutes(f).contains(&"Noto Sans Symbols 2"), "{f}");
        }
    }

    #[test]
    fn resolves_weights_named_as_their_own_family() {
        // Word stores a Medium face as the family "Inter Medium" (#219).
        let r = resolve("Inter Medium", false, false);
        assert!(!r.substituted && !r.synth_bold && !r.synth_italic);
        assert_eq!((r.face.family.as_str(), r.face.style.as_str()), ("Inter", "Medium"));
        // Bold and italic within it are drawn from the Medium face.
        let b = resolve("Inter Medium", true, true);
        assert_eq!(b.face.style, "Medium");
        assert!(!b.substituted && b.synth_bold && b.synth_italic);
    }

    #[test]
    fn synthesizes_missing_styles() {
        // JetBrains Mono ships only Regular here.
        let r = resolve("JetBrains Mono", true, true);
        assert!(r.synth_bold && r.synth_italic);
    }

    #[test]
    fn hebrew_fonts_substitute_hebrew_faces() {
        for f in ["David", "Miriam", "Arial Hebrew", "Calibri Hebrew", "Noto Sans Hebrew"] {
            assert!(substitutes(f).contains(&"Noto Sans Hebrew"), "{f}");
            // David is a Windows font; only the Windows list offers it.
            if cfg!(target_os = "windows") {
                assert!(substitutes(f).contains(&"David") || f == "David", "{f}");
            }
        }
        // Generic names with "Hebrew" in them also get Hebrew substitutions
        assert!(substitutes("SomeFont Hebrew").contains(&"Noto Sans Hebrew"));
    }

    #[test]
    fn khmer_fonts_substitute_khmer_faces() {
        for f in ["Khmer OS Bokor", "Khmer OS Battambang", "Khmer Mool1", "Khmer OS Muol Light"] {
            assert!(substitutes(f).iter().any(|s| s.contains("Khmer")), "{f}: {:?}", substitutes(f));
        }
    }

    #[test]
    fn thai_fonts_substitute_thai_faces() {
        // Every Thai document family maps to a list of Thai faces, never to the Latin lists.
        for f in [
            "TH Sarabun New",
            "TH SarabunPSK",
            "Angsana New",
            "AngsanaUPC",
            "Cordia New",
            "CordiaUPC",
            "Browallia New",
            "BrowalliaUPC",
            "JasmineUPC",
            "KodchiangUPC",
            "LilyUPC",
            "DilleniaUPC",
            "EucrosiaUPC",
            "FreesiaUPC",
            "IrisUPC",
            "Leelawadee",
            "Leelawadee UI",
            "Noto Sans Thai",
            "TH Niramit AS",
        ] {
            assert!(is_thai_family(f), "{f}");
            let list = substitutes(f);
            assert!(list.iter().any(|s| s.contains("Thai")), "{f}: {list:?}");
            assert!(!list.contains(&"Carlito") && !list.contains(&"Caladea"), "{f}: {list:?}");
        }
        // Serif Thai families prefer a serif Thai face; the sans families a sans one.
        let pos = |family: &str, name: &str| substitutes(family).iter().position(|s| *s == name).unwrap_or(usize::MAX);
        assert!(pos("Angsana New", "Noto Serif Thai") < pos("Angsana New", "Noto Sans Thai"));
        assert!(pos("TH Sarabun New", "Noto Sans Thai") < pos("TH Sarabun New", "Noto Serif Thai"));
        // Latin and other script families are unchanged.
        assert!(!is_thai_family("Arial") && !is_thai_family("Tahoma") && !is_thai_family("Calibri") && !is_thai_family("David"));
        assert_eq!(substitutes("Calibri").first(), Some(&"Carlito"));
    }

    #[test]
    fn a_missing_thai_font_resolves_to_an_installed_thai_face() {
        // The issue's run (#1549): Thai letters and digits, Latin letters, digits and the baht
        // sign in one run of a Thai font that isn't installed.
        const RUN: &str = "[D] น้ำ ค่ำ ปั้น ฝั่ง ปู่ ๑๒๓๔๕๖๗๘๙๐ ฿1,250.50";
        let db = FontDb::global();
        for family in ["TH Sarabun New", "Angsana New", "Cordia New", "BrowalliaUPC"] {
            if db.has_family(family) {
                continue;
            }
            let r = resolve(family, false, false);
            let face = r.face.get();
            if !face.covers('\u{0E19}') {
                eprintln!("skipped {family}: no Thai face from its substitute list is installed");
                continue;
            }
            assert!(r.substituted, "{family}");
            let thai: Vec<char> = RUN.chars().filter(|c| ('\u{0E00}'..='\u{0E7F}').contains(c) && !face.covers(*c)).collect();
            assert!(thai.is_empty(), "{family} → {} lacks {thai:?}", face.family);
            // A listed Thai face with Latin letters and digits wins over one without them.
            let latin = |name: &str| {
                let f = db.face(name, "Regular");
                f.named(name) && f.covers('\u{0E19}') && f.covers('D') && f.covers('5')
            };
            if substitutes(family).iter().any(|s| db.has_family(s) && latin(s)) {
                let missing: Vec<char> = RUN.chars().filter(|c| !c.is_whitespace() && !face.covers(*c)).collect();
                assert!(missing.is_empty(), "{family} → {} lacks {missing:?}", face.family);
            }
        }
    }

    #[test]
    fn thonburi_stacks_thai_marks_through_its_apple_tables() {
        // Thonburi has `morx` but no GSUB: the shaper must still move a tone mark up over an upper
        // vowel (ปั่) instead of drawing it where it sits on a bare consonant (ป่).
        let db = FontDb::global();
        if !db.has_family("Thonburi") {
            eprintln!("skipped: Thonburi not installed");
            return;
        }
        let face = db.face("Thonburi", "Regular");
        let tone = |text: &str| crate::shape(&face, text, &[], |c| c).into_iter().last().map(|g| (g.gid, g.y_offset));
        let (bare, stacked) = (tone("\u{0E1B}\u{0E48}"), tone("\u{0E1B}\u{0E31}\u{0E48}"));
        assert!(bare.is_some() && stacked.is_some());
        assert_ne!(bare, stacked, "the tone mark over ั is raised or swapped");
    }

    #[test]
    fn fonts_with_a_clone_have_named_substitutes() {
        for f in [
            "Times New Roman",
            "Times New Roman Cyr",
            "Arial",
            "Arial Narrow",
            "Courier New",
            "Palatino Linotype",
            "Symbol",
            "Century",
            "Microsoft Sans Serif",
        ] {
            assert!(has_named_substitutes(f), "{f}");
        }
        for f in ["No Such Brand Font", "Cambria", "Georgia", "Brand Serif"] {
            assert!(!has_named_substitutes(f), "{f}");
        }
    }

    #[test]
    fn century_takes_c059_first() {
        let subs = substitutes("Century");
        assert_eq!(subs.first(), Some(&"C059"));
        let caladea = subs.iter().position(|s| *s == "Caladea");
        let clones = subs.iter().position(|s| *s == "New Century Schoolbook");
        assert!(caladea > clones, "{subs:?}");
        assert_eq!(substitutes("century schoolbook"), subs);
    }

    #[test]
    fn microsoft_sans_serif_takes_liberation_sans_first() {
        let subs = substitutes("Microsoft Sans Serif");
        assert_eq!(subs.first(), Some(&"Liberation Sans"));
        assert!(!subs.contains(&"Carlito"), "{subs:?}");
        assert_eq!(substitutes("ms sans serif"), subs);
    }

    #[test]
    fn arial_narrow_takes_narrow_substitutes_first() {
        let subs = substitutes("Arial Narrow");
        assert_eq!(subs.get(1), Some(&"Liberation Sans Narrow"));
        assert!(!subs.contains(&"Carlito"), "{subs:?}");
        assert_eq!(substitutes("arial narrow"), subs);
    }

    /// Semicolon-separated fallback lists resolve to the first installed family, or to a
    /// candidate's substitute when no listed family is installed.
    #[test]
    fn libreoffice_fallback_list_resolves_first_installed_family() {
        let db = FontDb::global();
        if db.has_family("Liberation Sans") || db.has_family("Arial") {
            let r = resolve("Liberation Sans;Arial", false, false);
            assert!(!r.substituted, "should not be substituted when a candidate is installed: got {:?}", r.face.family);
            if db.has_family("Liberation Sans") {
                assert_eq!(r.face.family, "Liberation Sans");
            } else {
                assert_eq!(r.face.family, "Arial");
            }
        }
        let bundled = resolve("NonExistentSansFace123;Source Serif 4", false, false);
        assert!(!bundled.substituted);
        assert_eq!(bundled.face.family, "Source Serif 4");

        // When none of the families is installed, fall back to a substitute for an entry in the list rather than generic Carlito:
        if !db.has_family("NonExistentSansFace123") && !db.has_family("Times New Roman") {
            let r = resolve("NonExistentSansFace123;Times New Roman", false, false);
            assert!(r.substituted);
            assert_ne!(r.face.family, "Carlito", "should not fall back to generic Carlito when an entry is Times New Roman");
            assert!(r.face.family.contains("Serif") || r.face.family == "Times" || r.face.family == "Tinos", "got {:?}", r.face.family);
        }
        let r = resolve("NonExistentSansFace123;NonExistentSerifFace123", false, false);
        assert!(r.substituted);
        assert_ne!(r.face.family, "Carlito", "should not fall back to generic Carlito when an entry is a serif family");
        assert!(
            r.face.family.contains("Serif")
                || r.face.family == "Times"
                || r.face.family == "Times New Roman"
                || r.face.family == "Tinos"
                || r.face.family == "Caladea"
                || r.face.family == "Georgia",
            "got {:?}",
            r.face.family
        );

        // Metric compatibility of fallback list:
        assert!(metric_compatible("Open Sans;Arial", "Liberation Sans"));
        assert!(metric_compatible("Liberation Sans;Arial", "Helvetica"));
        assert!(!metric_compatible("Open Sans;Arial", "Carlito"));
    }

    /// In a mixed Thai-and-Latin fallback list where neither listed family is installed, an
    /// earlier Thai substitute takes precedence over a later candidate's metric twin.
    #[test]
    fn thai_fallback_list_prefers_thai_substitute_before_later_metric_twin() {
        let db = FontDb::global();
        let Some(thai_sub) = substitute(db, "NonExistentThaiUPC") else {
            eprintln!("skipped: no Thai substitute installed");
            return;
        };
        if !db.has_family("Cordia New") && !db.has_family("Arial") && metric_twins("Arial").any(|t| db.has_family(t)) {
            let r = resolve("Cordia New;Arial", false, false);
            assert!(r.substituted);
            assert!(substitutes("Cordia New").contains(&r.face.family.as_str()), "got {:?}", r.face.family);
        }
        if let Some(latin) =
            METRIC_TWINS.iter().flat_map(|g| g.iter().copied()).find(|&f| !db.has_family(f) && metric_twins(f).any(|t| db.has_family(t)))
        {
            let list = format!("NonExistentThaiUPC;{latin}");
            let r = resolve(&list, false, false);
            assert!(r.substituted);
            assert_eq!(r.face.family, thai_sub);
        }
    }

    /// Fallback lists longer than `FAMILY_KEY_MAX` still select a later installed family and
    /// cache their resolution without growing `resolve_cache` string keys.
    #[test]
    fn long_fallback_list_selects_later_installed_family_and_caches() {
        let mut candidates: Vec<String> = (0..120).map(|i| format!("MissingCandidateFont{i}")).collect();
        candidates.push("Source Serif 4".to_string());
        let list = candidates.join(";");
        assert!(list.len() > FAMILY_KEY_MAX);

        let r1 = resolve(&list, false, false);
        let r2 = resolve(&list, false, false);
        assert!(!r1.substituted);
        assert_eq!(r1.face.family, "Source Serif 4");
        assert_eq!(r1.face.id(), r2.face.id());
        assert!(!resolve_cache().lock().unwrap_or_else(|e| e.into_inner()).1.keys().any(|k| k.0.len() > FAMILY_KEY_MAX));
    }

    /// The name-based predicates look at every candidate, and their answer is cached per list: a
    /// later Thai or named fallback still counts, and a list is classified once rather than for
    /// every piece of text shaped in it.
    #[test]
    fn list_classification_reads_every_candidate_and_is_cached() {
        // The Thai family is last, past any shorter bound, and still classifies the list.
        let mut candidates: Vec<String> = (0..200).map(|i| format!("MissingCandidateFont{i}")).collect();
        candidates.push("TH Sarabun New".to_string());
        let thai_list = candidates.join(";");
        assert!(is_thai_family(&thai_list), "a later Thai family still counts");

        // A named substitute at the end classifies too (Times New Roman has Liberation Serif).
        candidates.pop();
        candidates.push("Times New Roman".to_string());
        let named_list = candidates.join(";");
        assert!(has_named_substitutes(&named_list), "a later named fallback still counts");

        // Both are memoised by list text: a second ask answers without rescanning, and a list
        // whose text differs only after the key's hash is not mistaken for the cached one.
        assert!(is_thai_family(&thai_list) && !is_thai_family(&named_list));
        let cached = fallback_list_cache().lock().unwrap_or_else(|e| e.into_inner()).classes.len();
        assert!(cached >= 2, "{cached} lists classified");
    }
}
