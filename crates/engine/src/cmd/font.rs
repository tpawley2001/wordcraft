//! Fonts the user adds without rebuilding (#582): `font.add`, `font.list`, `font.remove`; and
//! the fonts the document's text asks for that aren't installed, with the fonts drawn instead
//! (`font.missing`), which the user may show in fonts of their choice or replace for good (File ›
//! Options › Advanced › Font Substitution: `font.substitution`, `font.setSubstitute`,
//! `font.convertPermanently`).
//!
//! An added font joins the process-wide font database on top of the bundled and system fonts
//! (never replacing a face already there), so it shows in the font lists, lays out, renders and
//! embeds in PDFs like any other. Where it is kept between runs is [`FontDb::load_user_fonts`]'s
//! folder (desktop); without one it lasts for the session (the web shell stores it itself).

use std::sync::Arc;

use serde_json::{Value, json};
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Block, Blocks};
use wordcraft_fonts::{FontDb, UserFont};
use wordcraft_layout::{DocLayout, FontSubstitutes};

use crate::cmd::insert::base64_decode;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("font.add", "Add Font…", "Home › Font", add)
            .params(r#"{"path"?: string, "data"?: base64, "name"?: string (file name for "data")} — a TTF, OTF, TTC, WOFF or WOFF2 file; none: pick a file"#)
            .pure(),
        CommandSpec::new("font.list", "Added Fonts", "Home › Font", list).params(r#"{"all"?: bool (also every available family)}"#).pure(),
        CommandSpec::new("font.remove", "Remove Added Font", "Home › Font", remove).params(r#"{"family": string}"#).pure(),
        CommandSpec::new("font.loadSystem", "Load System Installed Fonts…", "Home › Font", load_system).pure(),
        CommandSpec::new("font.missing", "Missing Fonts", "Status Bar", missing)
            .params(r#"{} → {"missing": [{"font": string, "shownAs": [string] (the families drawn instead), "metricCompatible": bool (its substitute measures like the font)}]} — the text the current view shows"#)
            .pure(),
        CommandSpec::new("font.substitution", "Font Substitution…", "File › Options › Advanced", substitution)
            .params(r#"{} → {"fonts": [{"font": string, "shownAs": [string], "substitute": string|null (the font picked to show it in; null: Default)}]} — the fonts the document shows that aren't installed"#)
            .pure(),
        CommandSpec::new("font.setSubstitute", "Substituted Font", "File › Options › Advanced", set_substitute)
            .params(r#"{"font": string (a font that isn't installed), "substitute": string|null (an installed family; null: Default)} — what the font is shown and printed in on this computer; the document keeps naming it"#)
            .pure(),
        CommandSpec::new("font.convertPermanently", "Convert Permanently…", "File › Options › Advanced", convert_permanently)
            .params(r#"{"font": string (a font that isn't installed), "to"?: string (an installed family; default: its substitute)} — replaces the font in the document"#),
    ]
}

fn load_system(s: &mut Session, _v: &Value) -> CmdResult {
    s.push_ui_request(json!({"open": "loadSystemFonts"}));
    Ok(json!({"pending": true}))
}

fn font_json(f: &UserFont) -> Value {
    json!({
        "file": f.file,
        "families": f.families(),
        "faces": f.faces.iter().map(|(family, style)| json!({"family": family, "style": style})).collect::<Vec<_>>(),
        "path": f.path.as_ref().map(|p| p.to_string_lossy().to_string()),
    })
}

/// Read a font file named by the user, refusing one larger than the cap before reading it.
#[cfg(not(target_arch = "wasm32"))]
fn read_font_file(path: &str) -> Result<Vec<u8>, CmdError> {
    // Through `io`, which keeps automation inside its read root (#769).
    crate::io::read_file_capped(path, wordcraft_fonts::MAX_FONT_FILE_BYTES as u64, "font file").map_err(CmdError::Failed)
}

/// Adding or removing a font writes to the user font folder: automation confined to a write root
/// (#769) may do that only when the folder is inside it.
fn check_font_store(dir: Option<&std::path::Path>) -> Result<(), CmdError> {
    match dir {
        Some(dir) => crate::file_access::check_write(&dir.to_string_lossy()).map_err(CmdError::Failed),
        None => Ok(()),
    }
}

fn add(s: &mut Session, v: &Value) -> CmdResult {
    if p::str(v, "url").is_some() {
        return Err(CmdError::Failed(
            "adding a font from a link isn't available yet: download the font file and add it with {\"path\"} or {\"data\"}".into(),
        ));
    }
    let (name, bytes) = if let Some(d) = p::str(v, "data") {
        // Base64 is a third larger than the file: refuse before decoding an oversized one.
        if d.len() / 4 * 3 > wordcraft_fonts::MAX_FONT_FILE_BYTES + 3 {
            return Err(CmdError::Failed(format!("the font file is larger than {} MB", wordcraft_fonts::MAX_FONT_FILE_BYTES >> 20)));
        }
        let bytes = base64_decode(d).ok_or_else(|| CmdError::Params("bad base64 data".into()))?;
        (p::str(v, "name").unwrap_or("font").to_string(), bytes)
    } else if let Some(path) = p::str(v, "path") {
        #[cfg(not(target_arch = "wasm32"))]
        {
            (path.to_string(), read_font_file(path)?)
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = path;
            return Err(CmdError::Failed("reading files by path isn't available on the web".into()));
        }
    } else {
        // From the ribbon: the UI shows a file picker and runs this again with the file.
        s.push_ui_request(json!({"open": "addFont"}));
        return Ok(json!({"pending": true}));
    };
    check_font_store(FontDb::global().user_font_dir().as_deref())?;
    let r = FontDb::global().install_font(&name, &bytes).map_err(|e| CmdError::Failed(e.to_string()))?;
    // Text already set in the family (shown in a fallback until now) takes the new font.
    s.relayout();
    let families = r.font.families().join(", ");
    s.status = match &r.warning {
        Some(w) => format!("Added {families} ({w})"),
        None => format!("Added {families}"),
    };
    let mut out = font_json(&r.font);
    if let Some(w) = r.warning {
        out["warning"] = json!(w);
    }
    Ok(out)
}

fn list(_: &mut Session, v: &Value) -> CmdResult {
    let db = FontDb::global();
    let mut out = json!({
        "fonts": db.user_fonts().iter().map(font_json).collect::<Vec<_>>(),
        "folder": db.user_font_dir().map(|p| p.to_string_lossy().to_string()),
    });
    if p::bool(v, "all") == Some(true) {
        out["families"] = json!(db.families());
    }
    Ok(out)
}

fn remove(s: &mut Session, v: &Value) -> CmdResult {
    let family = p::str(v, "family").filter(|f| !f.trim().is_empty()).ok_or_else(|| CmdError::Params("family is required".into()))?;
    check_font_store(FontDb::global().user_font_dir().as_deref())?;
    let gone = FontDb::global().remove_user_font(family.trim()).map_err(|e| CmdError::Failed(e.to_string()))?;
    s.relayout();
    s.status = format!("Removed {}", family.trim());
    Ok(json!({"removed": gone.iter().map(font_json).collect::<Vec<_>>()}))
}

/// A font the document's text asks for that isn't installed.
#[derive(Clone, Debug, PartialEq)]
pub struct MissingFont {
    /// The name the document asks for.
    pub font: String,
    /// The families its text is drawn in instead: the substitute, and fallback faces for
    /// characters the substitute lacks.
    pub shown_as: Vec<String>,
    /// The font's substitute measures like it (Carlito for Calibri…): line breaks don't move.
    /// Fallback faces for characters the substitute lacks don't count: the font itself might
    /// lack them too (an emoji in Calibri text).
    pub metric_compatible: bool,
}

/// The family text set in `font` is drawn in when `font` isn't installed (None when it is), as
/// layout picks it for `doc`.
pub fn shown_as(doc: &Document, font: &str) -> Option<String> {
    shown_as_with(doc, &FontSubstitutes::default(), font)
}

/// [`shown_as`] with the fonts the user picked to show missing fonts in (`font.setSubstitute`).
pub fn shown_as_with(doc: &Document, subs: &FontSubstitutes, font: &str) -> Option<String> {
    wordcraft_layout::para::substitute_face_with(doc, subs, font).map(|f| f.get().family.clone())
}

/// The family Chinese, Japanese and Korean text is drawn in when its East Asian font `font_ea`
/// isn't installed (None when it is), the run's font being `font`, as layout picks it for `doc`.
pub fn east_asian_shown_as(doc: &Document, font_ea: &str, font: &str) -> Option<String> {
    east_asian_shown_as_with(doc, &FontSubstitutes::default(), font_ea, font)
}

/// [`east_asian_shown_as`] with the fonts the user picked to show missing fonts in.
pub fn east_asian_shown_as_with(doc: &Document, subs: &FontSubstitutes, font_ea: &str, font: &str) -> Option<String> {
    wordcraft_layout::para::east_asian_substitute_face_with(doc, subs, font_ea, font).map(|f| f.get().family.clone())
}

/// Whether text in `font` drawn in `substitute` is worth a warning: the substitute measures
/// differently, so line breaks can move, and the font isn't a symbol font (drawn as its
/// Unicode equivalents on purpose, which layout leaves out of [`missing_fonts`] too). Decides
/// [`MissingFont::metric_compatible`], which the font box and the status bar both read.
pub fn warns(font: &str, substitute: &str) -> bool {
    !wordcraft_doc::symbol_fonts::is_symbol_font(font) && !wordcraft_fonts::word::metric_compatible(font, substitute)
}

/// The fonts `doc`'s laid-out text asks for that aren't installed, from what layout drew
/// ([`DocLayout::substituted_fonts`]): only text the current view shows counts (no headers or
/// footers in Web, Draft or Read view), in every story on its pages, list labels and field
/// results included. Names differing only in case are one font.
pub fn missing_fonts(doc: &Document, layout: &DocLayout) -> Vec<MissingFont> {
    missing_fonts_with(doc, &FontSubstitutes::default(), layout)
}

/// [`missing_fonts`] of a layout made with the fonts the user picked to show missing fonts in
/// (`subs`): a font shown in its choice reports that family.
pub fn missing_fonts_with(doc: &Document, subs: &FontSubstitutes, layout: &DocLayout) -> Vec<MissingFont> {
    // By key: in name order, case variants merged.
    let mut out: std::collections::BTreeMap<String, MissingFont> = std::collections::BTreeMap::new();
    for (recorded, face) in layout.substituted_fonts() {
        let family = face.get().family.clone();
        let font = shown_name(&recorded);
        let m = out.entry(font_key(font)).or_insert_with(|| MissingFont {
            font: font.to_string(),
            shown_as: Vec::new(),
            // Judged by the name layout looked up, spaces and all: "Arial " isn't Arial to it, so
            // its text is in a substitute even where Arial is installed.
            metric_compatible: shown_as_with(doc, subs, &recorded).is_some_and(|s| !warns(font, &s)),
        });
        if !m.shown_as.contains(&family) {
            m.shown_as.push(family);
        }
    }
    out.into_values().collect()
}

/// A font name as [`missing_fonts`] lists it: as layout records it (at most 256 characters),
/// trimmed.
fn shown_name(font: &str) -> &str {
    wordcraft_layout::para::reported_font_name(font).trim()
}

/// The key a font name is listed under in [`missing_fonts`]: two names with the same key are
/// one font. Look a document's font up in the list through this.
pub fn font_key(font: &str) -> String {
    FontSubstitutes::key(font)
}

/// The status line's form of a font name: a hostile file's names are long.
fn short_name(font: &str) -> String {
    match font.char_indices().nth(40) {
        Some((i, _)) => format!("{}…", font.get(..i).unwrap_or(font)),
        None => font.to_string(),
    }
}

fn missing(s: &mut Session, _: &Value) -> CmdResult {
    let layout = s.layout();
    let found = missing_fonts_with(&s.doc, &s.prefs.font_substitutes, &layout);
    // From command search there's no dialog: the status line says what was found.
    // A hostile file can name thousands of fonts: name the first few.
    const NAMED: usize = 5;
    let changing: Vec<&str> = found.iter().filter(|f| !f.metric_compatible).map(|f| f.font.as_str()).collect();
    let named = changing.iter().take(NAMED).map(|f| short_name(f)).collect::<Vec<_>>().join(", ");
    s.status = match (found.is_empty(), changing.len()) {
        (true, _) => "Every font the document shows is installed".into(),
        (false, 0) => "Fonts that aren't installed are shown in fonts that measure the same: line breaks don't move".into(),
        (_, n) if n > NAMED => format!("Not installed: {named} and {} more", n - NAMED),
        _ => format!("Not installed: {named}"),
    };
    let fonts: Vec<Value> = found.iter().map(|f| json!({"font": f.font, "shownAs": f.shown_as, "metricCompatible": f.metric_compatible})).collect();
    Ok(json!({"missing": fonts}))
}

/// Longest font name `font.setSubstitute` and `font.convertPermanently` take.
const MAX_FONT_NAME: usize = 256;

/// The font name param `key`, trimmed: required unless `optional`, not blank, at most
/// [`MAX_FONT_NAME`] characters.
fn font_param<'v>(v: &'v Value, key: &str) -> Result<Option<&'v str>, CmdError> {
    let Some(name) = v.get(key).filter(|x| !x.is_null()) else { return Ok(None) };
    let name = name.as_str().map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| CmdError::Params(format!("{key} must be a font name")))?;
    if name.chars().nth(MAX_FONT_NAME).is_some() {
        return Err(CmdError::Params(format!("{key} is longer than {MAX_FONT_NAME} characters")));
    }
    Ok(Some(name))
}

/// The installed family `name` names, as the font list spells it.
fn installed_family(name: &str) -> Result<String, CmdError> {
    let db = FontDb::global();
    if !db.has_family(name) {
        return Err(CmdError::Failed(format!("{} isn't installed", short_name(name))));
    }
    Ok(db.families().into_iter().find(|f| f.eq_ignore_ascii_case(name)).unwrap_or_else(|| name.to_string()))
}

/// `font` isn't installed (text set in it is drawn in another face), else an error.
fn check_missing(doc: &Document, font: &str) -> Result<(), CmdError> {
    match shown_as(doc, font) {
        Some(_) => Ok(()),
        None => Err(CmdError::Failed(format!("{} is installed: only fonts that aren't are substituted", short_name(font)))),
    }
}

fn substitution(s: &mut Session, _: &Value) -> CmdResult {
    let layout = s.layout();
    let subs = &s.prefs.font_substitutes;
    let fonts: Vec<Value> = missing_fonts_with(&s.doc, subs, &layout)
        .iter()
        // `default`: the face Default (no choice) draws the font in, for showing it after a
        // choice is taken back.
        .map(|f| json!({"font": f.font, "shownAs": f.shown_as, "substitute": subs.chosen(&f.font), "default": shown_as(&s.doc, &f.font)}))
        .collect();
    Ok(json!({"fonts": fonts}))
}

fn set_substitute(s: &mut Session, v: &Value) -> CmdResult {
    let font = font_param(v, "font")?.ok_or_else(|| CmdError::Params("font is required".into()))?;
    let substitute = font_param(v, "substitute")?.map(installed_family).transpose()?;
    if substitute.is_some() {
        check_missing(&s.doc, font)?;
    }
    let subs = s.prefs.font_substitutes.with(font, substitute.as_deref());
    s.set_font_substitutes(subs);
    s.status = match &substitute {
        Some(f) => format!("{} is shown in {}", short_name(font), short_name(f)),
        None => format!("{} is shown in its default substitute", short_name(font)),
    };
    Ok(json!({"font": font, "substitute": substitute}))
}

fn convert_permanently(s: &mut Session, v: &Value) -> CmdResult {
    let font = font_param(v, "font")?.ok_or_else(|| CmdError::Params("font is required".into()))?;
    check_missing(&s.doc, font)?;
    let to = match font_param(v, "to")? {
        Some(to) => to.to_string(),
        None => shown_as_with(&s.doc, &s.prefs.font_substitutes, font).ok_or_else(|| CmdError::Failed("no substitute for the font".into()))?,
    };
    let to = installed_family(&to)?;
    let key = font_key(font);
    s.checkpoint("Convert Font");
    let doc = &mut s.doc;
    let mut changed = convert_blocks(&mut doc.body, &key, &to, 0);
    for part in doc.parts.values_mut() {
        changed += convert_blocks(&mut part.blocks, &key, &to, 0);
    }
    let styles = Arc::make_mut(&mut doc.styles);
    changed += convert_props(&mut styles.default_chr, &key, &to);
    for st in &mut styles.styles {
        changed += convert_props(&mut st.chr, &key, &to);
        // A table style's conditional formatting: header row, bands, first/last column, total
        // row and the other regions.
        if let Some(t) = &mut st.table {
            for c in [&mut t.header_chr, &mut t.band_chr, &mut t.first_col_chr, &mut t.total_chr, &mut t.last_col_chr, &mut t.col_band_chr] {
                changed += convert_props(c, &key, &to);
            }
            for part in t.regions.values_mut() {
                changed += convert_props(&mut part.chr, &key, &to);
            }
        }
    }
    let numbering = Arc::make_mut(&mut doc.numbering);
    for level in numbering.abstracts.iter_mut().flat_map(|a| a.levels.iter_mut()) {
        changed += convert_props(&mut level.chr, &key, &to);
    }
    for (_, level) in numbering.nums.iter_mut().flat_map(|n| n.level_overrides.iter_mut()) {
        changed += convert_props(&mut level.chr, &key, &to);
    }
    if changed == 0 {
        return Err(CmdError::Failed(format!("the document doesn't use {}", short_name(font))));
    }
    // Style and list changes reach paragraphs whose own text didn't change.
    s.dirty = true;
    s.relayout();
    s.status = format!("Replaced {} with {}", short_name(font), short_name(&to));
    Ok(json!({"font": font, "to": to, "changed": changed}))
}

/// Deepest table nesting [`convert_blocks`] follows.
const MAX_DEPTH: usize = 64;

/// Set the fonts keyed `key` in `c` (Latin, East Asian and complex script) to `to`; how many.
fn convert_props(c: &mut CharProps, key: &str, to: &str) -> usize {
    let mut n = 0;
    for f in [&mut c.font, &mut c.font_ea, &mut c.font_cs] {
        if f.as_deref().is_some_and(|f| font_key(f) == key) {
            *f = Some(to.to_string());
            n += 1;
        }
    }
    n
}

/// Whether a paragraph or table cell in `b` names the font keyed `key`.
fn block_uses(b: &Block, key: &str, depth: usize) -> bool {
    let uses = |c: &CharProps| [&c.font, &c.font_ea, &c.font_cs].into_iter().flatten().any(|f| font_key(f) == key);
    match b {
        Block::Para(p) => uses(&p.mark) || p.runs.iter().any(|r| uses(&r.props)),
        Block::Table(t) if depth < MAX_DEPTH => {
            t.rows.iter().flat_map(|r| r.cells.iter()).flat_map(|c| c.blocks.iter()).any(|b| block_uses(b, key, depth + 1))
        }
        Block::Table(_) => false,
    }
}

/// [`convert_props`] in every paragraph of `blocks`, tables included; copies only the blocks
/// that change.
fn convert_blocks(blocks: &mut Blocks, key: &str, to: &str, depth: usize) -> usize {
    let mut n = 0;
    for b in blocks.iter_mut().filter(|b| block_uses(b, key, depth)) {
        match Arc::make_mut(b) {
            Block::Para(p) => {
                n += convert_props(&mut p.mark, key, to);
                for r in &mut p.runs {
                    n += convert_props(&mut r.props, key, to);
                }
                p.touch();
            }
            Block::Table(t) => {
                for cell in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                    n += convert_blocks(&mut cell.blocks, key, to, depth + 1);
                }
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    const MISSING: &str = "Wordcraft Nowhere Face";
    const BUNDLED: &str = "JetBrains Mono";

    /// "Missing" set in [`MISSING`] (the rest in [`BUNDLED`]), nothing to undo, saved.
    fn session() -> Session {
        let mut s = Session::new(Document::new());
        s.run("document.setText", &json!({"text": "Missing\nBundled"})).unwrap();
        s.run("select.all", &json!({})).unwrap();
        s.run("format.font", &json!({"name": BUNDLED})).unwrap();
        s.run("select.text", &json!({"text": "Missing"})).unwrap();
        s.run("format.font", &json!({"name": MISSING})).unwrap();
        s
    }

    fn uses(c: &CharProps, font: &str) -> bool {
        [&c.font, &c.font_ea, &c.font_cs].into_iter().flatten().any(|f| f == font)
    }

    #[test]
    fn a_display_substitute_is_no_edit() {
        let mut s = session();
        s.dirty = false;
        let undo = s.undo_depth();
        s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": BUNDLED})).unwrap();
        assert!(!s.dirty, "the document didn't change");
        assert_eq!(s.undo_depth(), undo, "no undo step");
        // The document still names the missing font.
        let Some(Block::Para(p)) = s.doc.body.first().map(|b| &**b) else { panic!("no paragraph") };
        assert!(p.runs.iter().any(|r| r.props.font.as_deref() == Some(MISSING)));
    }

    #[test]
    fn text_shown_in_a_substitute_is_measured_in_it() {
        // The width of the first paragraph, "Missing".
        let width = |s: &mut Session| {
            let l = s.layout();
            let para = l.pages.first().and_then(|p| {
                p.items.iter().find_map(|it| match it {
                    wordcraft_layout::Placed::Lines { para, .. } => Some(para.clone()),
                    _ => None,
                })
            });
            para.map_or(0.0, |p| p.clusters.iter().map(|c| c.adv).sum::<f32>())
        };
        let mut s = session();
        let default = width(&mut s);
        s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": BUNDLED})).unwrap();
        let chosen = width(&mut s);
        assert!(chosen > 0.0 && default > 0.0);
        assert_ne!(chosen, default, "measured in the chosen font, not the default substitute");
    }

    #[test]
    fn convert_permanently_reaches_styles_lists_and_headers() {
        let mut s = session();
        s.run("para.bullets", &json!({})).unwrap();
        let header = s.run("insert.header", &json!({"text": "Head"})).unwrap()["story"].as_u64().unwrap() as u32;
        if let Some(part) = s.doc.parts.get_mut(&header)
            && let Some(Block::Para(p)) = part.blocks.first_mut().map(Arc::make_mut)
        {
            for r in &mut p.runs {
                r.props.font_ea = Some(MISSING.to_lowercase());
            }
        }
        let styles = Arc::make_mut(&mut s.doc.styles);
        if let Some(st) = styles.styles.first_mut() {
            st.chr.font_cs = Some(MISSING.into());
        }
        let numbering = Arc::make_mut(&mut s.doc.numbering);
        for l in numbering.abstracts.iter_mut().flat_map(|a| a.levels.iter_mut()) {
            l.chr.font = Some(MISSING.into());
        }
        s.run("font.convertPermanently", &json!({"font": MISSING, "to": BUNDLED})).unwrap();
        let doc = &s.doc;
        assert!(!doc.styles.styles.iter().any(|st| uses(&st.chr, MISSING)));
        assert!(doc.styles.styles.first().is_some_and(|st| st.chr.font_cs.as_deref() == Some(BUNDLED)));
        assert!(doc.numbering.abstracts.iter().flat_map(|a| a.levels.iter()).all(|l| l.chr.font.as_deref() == Some(BUNDLED)));
        let Some(Block::Para(p)) = doc.parts.get(&header).and_then(|p| p.blocks.first()).map(|b| &**b) else { panic!("no header") };
        assert!(p.runs.iter().all(|r| r.props.font_ea.as_deref() == Some(BUNDLED)), "{:?}", p.runs);
        s.run("edit.undo", &json!({})).unwrap();
        assert!(s.doc.numbering.abstracts.iter().flat_map(|a| a.levels.iter()).all(|l| l.chr.font.as_deref() == Some(MISSING)), "one undo step");
    }

    #[test]
    fn convert_permanently_reaches_table_style_regions() {
        let mut s = session();
        let styles = Arc::make_mut(&mut s.doc.styles);
        let Some(st) = styles.styles.iter_mut().find(|st| st.kind == wordcraft_doc::styles::StyleKind::Table) else { panic!("no table style") };
        let id = st.id.clone();
        let t = st.table.get_or_insert_with(Default::default);
        t.header_chr.font = Some(MISSING.into());
        t.band_chr.font_ea = Some(MISSING.into());
        t.first_col_chr.font_cs = Some(MISSING.into());
        t.last_col_chr.font = Some(MISSING.into());
        t.total_chr.font = Some(MISSING.into());
        t.col_band_chr.font = Some(MISSING.into());
        t.regions.entry(wordcraft_doc::styles::TablePart::NeCell).or_default().chr.font = Some(MISSING.into());
        s.run("font.convertPermanently", &json!({"font": MISSING, "to": BUNDLED})).unwrap();
        let Some(t) = s.doc.styles.styles.iter().find(|st| st.id == id).and_then(|st| st.table.as_ref()) else { panic!("style gone") };
        for c in [&t.header_chr, &t.band_chr, &t.first_col_chr, &t.last_col_chr, &t.total_chr, &t.col_band_chr] {
            assert!(!uses(c, MISSING) && uses(c, BUNDLED), "{c:?}");
        }
        assert!(t.regions.values().all(|p| !uses(&p.chr, MISSING)));
        s.run("edit.undo", &json!({})).unwrap();
        let t = s.doc.styles.styles.iter().find(|st| st.id == id).and_then(|st| st.table.as_ref());
        assert!(t.is_some_and(|t| uses(&t.header_chr, MISSING)), "one undo step");
    }

    #[test]
    fn a_missing_east_asian_font_is_drawn_in_its_chosen_substitute() {
        const MINCHO: &str = "MS Mincho";
        if !wordcraft_fonts::word::resolve(MINCHO, false, false).substituted {
            return; // installed here: nothing is substituted
        }
        let mut s = Session::new(Document::new());
        s.run("document.setText", &json!({"text": "Hello 日本語の文章"})).unwrap();
        s.run("select.all", &json!({})).unwrap();
        s.run("format.font", &json!({"name": BUNDLED, "eastAsia": MINCHO})).unwrap();
        let shown = |s: &mut Session| -> Vec<String> {
            let v = s.run("font.substitution", &json!({})).unwrap();
            let row = v["fonts"].as_array().and_then(|f| f.iter().find(|f| f["font"] == MINCHO).cloned()).unwrap_or_default();
            row["shownAs"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
        };
        let default = shown(&mut s);
        assert!(!default.is_empty(), "MS Mincho is listed as missing");
        // Another installed face with the ideographs.
        let db = FontDb::global();
        let Some(other) = db.families().into_iter().filter(|f| !default.iter().any(|d| d.eq_ignore_ascii_case(f))).find(|f| {
            let r = wordcraft_fonts::word::resolve(f, false, false);
            !r.substituted && "日本語文章".chars().all(|c| r.face.covers(c))
        }) else {
            return; // only one CJK face here
        };
        s.run("font.setSubstitute", &json!({"font": MINCHO, "substitute": other})).unwrap();
        assert_eq!(shown(&mut s), vec![other.clone()], "the CJK text is laid out in the chosen face");
        s.run("font.setSubstitute", &json!({"font": MINCHO, "substitute": null})).unwrap();
        assert_eq!(shown(&mut s), default);
    }

    #[test]
    fn a_pdf_copy_draws_missing_fonts_in_their_chosen_substitutes() {
        let mut s = session();
        let pdf = |s: &Session| crate::io::save_bytes_opts("copy.pdf", &s.doc, &crate::cmd::file::export_options(s, &json!({}))).unwrap();
        let default = pdf(&s);
        assert_eq!(pdf(&s), default, "the same copy each time");
        s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": BUNDLED})).unwrap();
        let chosen = pdf(&s);
        assert_ne!(chosen, default, "laid out with the chosen font, as printing is");
        // Laid out as the session lays it out for printing.
        s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": null})).unwrap();
        assert_eq!(pdf(&s), default);
    }

    #[test]
    fn hostile_names_are_refused() {
        let mut s = session();
        let long = "x".repeat(MAX_FONT_NAME + 1);
        assert!(s.run("font.setSubstitute", &json!({"font": long, "substitute": BUNDLED})).is_err());
        assert!(s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": long})).is_err());
        assert!(s.run("font.convertPermanently", &json!({"font": long})).is_err());
        assert!(s.run("font.convertPermanently", &json!({"font": MISSING, "to": long})).is_err());
        assert!(s.run("font.setSubstitute", &json!({"font": 7, "substitute": BUNDLED})).is_err());
        assert!(s.prefs.font_substitutes.is_empty());
    }

    #[test]
    fn choices_are_kept_with_the_preferences() {
        let mut s = session();
        s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": BUNDLED})).unwrap();
        let saved = serde_json::to_value(&s.prefs).unwrap();
        let back: crate::Prefs = serde_json::from_value(saved).unwrap();
        assert_eq!(back.font_substitutes.get(MISSING), Some(BUNDLED));
        // Junk in the saved value reads as no choice, keeping the other preferences.
        let junk: crate::Prefs = serde_json::from_value(json!({"fontSubstitutes": [[1, 2], "x", ["a"]], "overtype": true})).unwrap();
        assert!(junk.font_substitutes.is_empty() && junk.overtype);
        let junk: crate::Prefs = serde_json::from_value(json!({"fontSubstitutes": 7})).unwrap();
        assert!(junk.font_substitutes.is_empty());
    }

    #[test]
    fn at_most_so_many_choices_are_kept_oldest_dropped() {
        let mut s = session();
        for i in 0..FontSubstitutes::MAX + 5 {
            s.run("font.setSubstitute", &json!({"font": format!("Wordcraft Gone {i}"), "substitute": BUNDLED})).unwrap();
        }
        let subs = &s.prefs.font_substitutes;
        assert_eq!(subs.pairs().len(), FontSubstitutes::MAX);
        assert!(subs.get("Wordcraft Gone 0").is_none());
        assert_eq!(subs.get(&format!("WORDCRAFT GONE {}", FontSubstitutes::MAX + 4)), Some(BUNDLED));
    }

    #[test]
    fn confined_automation_changes_only_a_font_store_inside_its_write_root() {
        let root = std::env::temp_dir().join(format!("wordcraft-font-store-{}", std::process::id()));
        let outside = std::env::temp_dir().join(format!("wordcraft-font-store-out-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let write = crate::file_access::AutomationRoots::new(None, Some(&root)).unwrap();
        let read_only = crate::file_access::AutomationRoots::new(Some(&root), None).unwrap();
        crate::file_access::confine(write.as_ref(), || {
            assert!(check_font_store(None).is_ok(), "no folder: the font lasts for the session");
            assert!(check_font_store(Some(&root.join("fonts"))).is_ok());
            assert!(check_font_store(Some(&outside)).is_err());
        });
        crate::file_access::confine(read_only.as_ref(), || assert!(check_font_store(Some(&root)).is_err()));
        assert!(check_font_store(Some(&outside)).is_ok(), "unconfined: the person at the keyboard");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
