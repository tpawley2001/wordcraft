//! File › Options (#485): the settings agents can read and change without the window — user
//! name and initials, typing (replace selection, overtype), measurement units, formatting marks
//! and proofing. The Options page in the app sets them through the same command.

use serde_json::{Value, json};

use crate::{CmdError, CmdResult, CommandSpec, Prefs, Session, p};

/// Options panes, in the order the page lists them (the `pane` param of `file.options`).
pub const PANES: [&str; 8] = ["general", "display", "proofing", "save", "language", "accessibility", "advanced", "agents"];

/// Longest initials kept, in characters.
pub const MAX_INITIALS_CHARS: usize = 9;
/// Longest folder path (dictionaries, user templates) taken, in characters.
const MAX_FOLDER_CHARS: usize = 4096;
/// Longest word the custom dictionary takes, in characters.
pub const MAX_WORD_CHARS: usize = 64;
/// Most words one call may add or remove.
const MAX_WORDS_PER_CALL: usize = 1000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("file.options", "Options", "File", options)
            .params(
                r#"{"pane"?: "general|display|proofing|save|language|accessibility|advanced|agents", "author"?: string, "initials"?: string, "units"?: "inches|centimeters|millimeters|points|picas|auto" (auto: the region's), "region"?: string (a locale tag like pl-PL, saved: the paper File › New starts on, the default units and the decimal separator; auto: the system's), "typingReplacesSelection"?: bool, "insertKeyOvertype"?: bool, "insertKeyPaste"?: bool, "overtype"?: bool, "autoWordSelection"?: bool (a mouse drag into another word selects whole words), "smartParagraphSelection"?: bool (a drag over a whole paragraph takes its mark), "smartCursoring"?: bool (after scrolling the caret out of view, an arrow key puts it on the page in view), "marks"?: {"tabs"?: bool, "spaces"?: bool, "paragraphs"?: bool, "hidden"?: bool}, "checkSpelling"?: bool, "markGrammar"?: bool, "printProperties"?: bool, "printBackground"?: bool (print the page color and picture), "printDrawings"?: bool (print shapes, text boxes, Decorative Text and drawing canvases), "printHiddenText"?: bool, "printFieldCodes"?: bool, "printFormsData"?: bool (this document: print only the data from its form fields), "printReverse"?: bool, "printUpdateLinks"?: bool (update links to other files before printing), "printScaleA4Letter"?: bool (default true; scale between A4 and Letter printer paper), "printDuplexFront"?: bool, "printDuplexBack"?: bool, "ignoreUppercase"?: bool, "ignoreNumbers"?: bool, "ignoreInternet"?: bool, "alwaysCreateBackup"?: bool, "confirmConversions"?: bool (ask for a plain-text file's encoding before opening it, as Word's File Conversion on open), "dictionariesFolder"?: string (a folder of Hunspell .aff/.dic files searched first, saved; auto: the system locations only), "userTemplatesFolder"?: string (the folder File › New lists personal templates from, saved; auto: the app's own Templates folder; not a network folder), "addWords"?: [string], "removeWords"?: [string]} — with any setting it changes them and opens nothing; without, it opens File › Options (at `pane`). Returns the settings."#,
            )
            .pure(),
        CommandSpec::new("text.overtype", "Overtype", "File › Options › Advanced", |s, v| {
            s.prefs.overtype = p::bool(v, "value").unwrap_or(!s.prefs.overtype);
            Ok(json!({"value": s.prefs.overtype}))
        })
        .params(r#"{"value"?: bool (default: toggle)}"#)
        .pure(),
        CommandSpec::new("view.styleAreaWidth", "Style area pane width", "File › Options › Advanced", style_area_width)
            .params(r#"{"value"?: number (points, 0 to 1584; 0: no style area)} — the column of paragraph style names Draft and Outline views show at the left; without `value` it only reports. Returns {"styleAreaWidth"}."#)
            .pure(),
    ]
}

/// Style area pane width in Draft and Outline views: a user preference in points.
fn style_area_width(s: &mut Session, v: &Value) -> CmdResult {
    match v.get("value") {
        None | Some(Value::Null) => {}
        Some(x) => {
            let w = x
                .as_f64()
                .filter(|w| w.is_finite() && (0.0..=f64::from(crate::prefs::MAX_STYLE_AREA_WIDTH)).contains(w))
                .ok_or_else(|| CmdError::Params(format!("`value` must be a width in points from 0 to {}", crate::prefs::MAX_STYLE_AREA_WIDTH)))?;
            s.prefs.style_area_width = w as f32;
        }
    }
    Ok(json!({"styleAreaWidth": s.prefs.style_area_width}))
}

/// The on/off settings by their `file.options` name.
fn flags() -> [(&'static str, fn(&mut Prefs) -> &mut bool); 25] {
    [
        ("typingReplacesSelection", |p| &mut p.typing_replaces_selection),
        ("insertKeyPaste", |p| &mut p.insert_key_paste),
        ("insertKeyOvertype", |p| &mut p.insert_key_overtype),
        ("autoWordSelection", |p| &mut p.auto_word_selection),
        ("smartParagraphSelection", |p| &mut p.smart_paragraph_selection),
        ("smartCursoring", |p| &mut p.smart_cursoring),
        ("overtype", |p| &mut p.overtype),
        ("markGrammar", |p| &mut p.mark_grammar),
        ("printProperties", |p| &mut p.print_properties),
        ("printBackground", |p| &mut p.print_background),
        ("printDrawings", |p| &mut p.print_drawings),
        ("printHiddenText", |p| &mut p.print_hidden_text),
        ("printFieldCodes", |p| &mut p.print_field_codes),
        ("printReverse", |p| &mut p.print_reverse),
        ("printScaleA4Letter", |p| &mut p.print_scale_a4_letter),
        ("printUpdateLinks", |p| &mut p.print_update_links),
        ("printDuplexFront", |p| &mut p.print_duplex_front),
        ("printDuplexBack", |p| &mut p.print_duplex_back),
        ("ignoreUppercase", |p| &mut p.ignore_uppercase),
        ("ignoreNumbers", |p| &mut p.ignore_numbers),
        ("ignoreInternet", |p| &mut p.ignore_internet),
        ("alwaysCreateBackup", |p| &mut p.always_create_backup),
        ("smartCutPaste", |p| &mut p.smart_cut_paste),
        ("pasteAdjustWordSpacing", |p| &mut p.paste_adjust_word_spacing),
        ("confirmConversions", |p| &mut p.confirm_conversions),
    ]
}

fn mark_flags() -> [(&'static str, fn(&mut wordcraft_layout::display::FormattingMarks) -> &mut bool); 4] {
    [("tabs", |m| &mut m.tabs), ("spaces", |m| &mut m.spaces), ("paragraphs", |m| &mut m.paragraphs), ("hidden", |m| &mut m.hidden)]
}

fn bool_param(v: &Value, k: &str) -> Result<Option<bool>, CmdError> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(CmdError::Params(format!("`{k}` must be true or false"))),
    }
}

/// Words for the custom dictionary: trimmed, single words, at most [`MAX_WORD_CHARS`] long.
fn words_param(v: &Value, k: &str) -> Result<Vec<String>, CmdError> {
    let Some(x) = v.get(k).filter(|x| !x.is_null()) else { return Ok(Vec::new()) };
    let bad = || CmdError::Params(format!("`{k}` must be a list of words"));
    let list = x.as_array().ok_or_else(bad)?;
    if list.len() > MAX_WORDS_PER_CALL {
        return Err(CmdError::Params(format!("`{k}` takes at most {MAX_WORDS_PER_CALL} words")));
    }
    let mut out = Vec::new();
    for w in list {
        let w = w.as_str().ok_or_else(bad)?.trim();
        if w.is_empty() || w.chars().count() > MAX_WORD_CHARS || w.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(CmdError::Params(format!("`{k}`: {w:?} isn't a single word of at most {MAX_WORD_CHARS} characters")));
        }
        out.push(w.to_string());
    }
    Ok(out)
}

fn options(s: &mut Session, v: &Value) -> CmdResult {
    let pane = match p::str(v, "pane") {
        Some(name) if PANES.contains(&name) => name,
        Some(name) => return Err(CmdError::Params(format!("unknown pane `{name}`; one of {}", PANES.join(", ")))),
        None => "general",
    };
    // Read every setting first, so a bad one changes nothing.
    let author = match v.get("author") {
        None | Some(Value::Null) => None,
        Some(Value::String(n)) => Some(n.clone()),
        Some(_) => return Err(CmdError::Params("`author` must be a string".into())),
    };
    let initials = match v.get("initials") {
        None | Some(Value::Null) => None,
        Some(Value::String(i)) => Some(i.trim().chars().filter(|c| !c.is_control()).take(MAX_INITIALS_CHARS).collect::<String>()),
        Some(_) => return Err(CmdError::Params("`initials` must be a string".into())),
    };
    let units = match v.get("units") {
        None | Some(Value::Null) => None,
        // `auto` forgets the choice: lengths follow the region again (#838).
        Some(Value::String(a)) if a == "auto" => Some(None),
        Some(u) => Some(Some(
            serde_json::from_value::<wordcraft_geom::Unit>(u.clone())
                .map_err(|_| CmdError::Params("`units` is one of inches, centimeters, millimeters, points, picas, auto".into()))?,
        )),
    };
    let region = match v.get("region") {
        None | Some(Value::Null) => None,
        // `auto` (or an empty tag) forgets the choice: the system's region again.
        Some(Value::String(t)) if t.trim().is_empty() || t.trim().eq_ignore_ascii_case("auto") => Some(None),
        // Kept as the tag (normalised, short), so it is saved and read back as chosen.
        Some(Value::String(t)) => {
            let r = wordcraft_geom::Region::from_tag(t);
            Some(Some(if r.territory.is_empty() { r.language } else { format!("{}-{}", r.language, r.territory) }))
        }
        Some(_) => return Err(CmdError::Params("`region` must be a locale tag like pl-PL, or auto".into())),
    };
    let folder = match v.get("dictionariesFolder") {
        None | Some(Value::Null) => None,
        Some(Value::String(f)) => Some(folder_setting("dictionariesFolder", f, false)?),
        Some(_) => return Err(CmdError::Params("`dictionariesFolder` must be a folder path, or auto".into())),
    };
    let templates_folder = match v.get("userTemplatesFolder") {
        None | Some(Value::Null) => None,
        // Never a folder on another machine (#2852): listing it would make Windows authenticate there.
        Some(Value::String(f)) => Some(folder_setting("userTemplatesFolder", f, true)?),
        Some(_) => return Err(CmdError::Params("`userTemplatesFolder` must be a folder path, or auto".into())),
    };
    let mut flag_values = Vec::new();
    for (k, f) in flags() {
        if let Some(b) = bool_param(v, k)? {
            flag_values.push((f, b));
        }
    }
    let mut mark_values = Vec::new();
    match v.get("marks") {
        None | Some(Value::Null) => {}
        Some(m @ Value::Object(_)) => {
            for (k, f) in mark_flags() {
                if let Some(b) = bool_param(m, k)? {
                    mark_values.push((f, b));
                }
            }
        }
        Some(_) => return Err(CmdError::Params("`marks` must be an object of true/false".into())),
    }
    let check_spelling = bool_param(v, "checkSpelling")?;
    // Per document (`w:printFormsData`), not a preference.
    let forms_data = bool_param(v, "printFormsData")?;
    let add = words_param(v, "addWords")?;
    let remove = words_param(v, "removeWords")?;

    let setting = author.is_some()
        || initials.is_some()
        || units.is_some()
        || region.is_some()
        || folder.is_some()
        || templates_folder.is_some()
        || !flag_values.is_empty()
        || !mark_values.is_empty()
        || check_spelling.is_some()
        || forms_data.is_some()
        || !add.is_empty()
        || !remove.is_empty();
    if let Some(name) = author {
        super::file::set_author(s, &json!({"name": name}))?;
    }
    if let Some(i) = initials {
        s.initials = i;
    }
    if let Some(u) = units {
        s.prefs.units = u;
    }
    if let Some(r) = region {
        s.prefs.region = r;
    }
    if let Some(f) = folder {
        use_dictionaries_folder(f.as_deref());
        s.prefs.dictionaries_folder = f;
    }
    if let Some(f) = templates_folder {
        s.prefs.user_templates_folder = f;
    }
    for (f, b) in flag_values {
        *f(&mut s.prefs) = b;
    }
    for (f, b) in mark_values {
        *f(&mut s.prefs.marks) = b;
    }
    if let Some(b) = forms_data
        && b != s.doc.settings.print_forms_data
    {
        s.doc.settings.print_forms_data = b;
        s.dirty = true;
    }
    if let Some(b) = check_spelling {
        s.view.proofing = b;
        s.prefs.check_spelling = b;
    }
    for w in &add {
        wordcraft_proof::add_word(w);
    }
    for w in &remove {
        wordcraft_proof::remove_word(w);
    }
    if !add.is_empty() || !remove.is_empty() {
        s.persist_user_dictionary(&remove);
    }
    if setting {
        // Marks, hidden text and proofing change what is laid out.
        s.relayout();
    } else {
        s.push_ui_request(json!({"open": "options", "pane": pane}));
    }
    let mut r = current(s);
    if let Some(o) = r.as_object_mut() {
        o.insert("opened".into(), json!(!setting));
    }
    Ok(r)
}

/// The settings `file.options` reads and sets.
pub fn current(s: &Session) -> Value {
    let pr = &s.prefs;
    let region = s.region();
    json!({
        "author": s.author,
        "initials": s.user_initials(),
        "units": s.units(),
        "unitsFromRegion": pr.units.is_none(),
        "region": {"language": region.language, "territory": region.territory, "paper": region.paper().0, "decimal": region.decimal().to_string()},
        "regionFromSystem": pr.region.is_none(),
        "typingReplacesSelection": pr.typing_replaces_selection,
        "insertKeyPaste": pr.insert_key_paste,
        "insertKeyOvertype": pr.insert_key_overtype,
        "autoWordSelection": pr.auto_word_selection,
        "smartParagraphSelection": pr.smart_paragraph_selection,
        "smartCursoring": pr.smart_cursoring,
        "overtype": pr.overtype,
        "marks": pr.marks,
        "checkSpelling": s.view.proofing,
        "markGrammar": pr.mark_grammar,
        "printProperties": pr.print_properties,
        "printBackground": pr.print_background,
        "printDrawings": pr.print_drawings,
        "printHiddenText": pr.print_hidden_text,
        "printFieldCodes": pr.print_field_codes,
        "printFormsData": s.doc.settings.print_forms_data,
        "printReverse": pr.print_reverse,
        "printScaleA4Letter": pr.print_scale_a4_letter,
        "printUpdateLinks": pr.print_update_links,
        "printDuplexFront": pr.print_duplex_front,
        "printDuplexBack": pr.print_duplex_back,
        "ignoreUppercase": pr.ignore_uppercase,
        "ignoreNumbers": pr.ignore_numbers,
        "ignoreInternet": pr.ignore_internet,
        "alwaysCreateBackup": pr.always_create_backup,
        "smartCutPaste": pr.smart_cut_paste,
        "pasteAdjustWordSpacing": pr.paste_adjust_word_spacing,
        "confirmConversions": pr.confirm_conversions,
        "dictionary": wordcraft_proof::user_dictionary(),
        "dictionariesFolder": pr.dictionaries_folder,
        "userTemplatesFolder": pr.user_templates_folder,
        "dictionaries": wordcraft_proof::installed_dictionaries()
            .into_iter()
            .map(|(lang, dic)| json!({"language": lang.tag(), "file": dic.map(|d| d.to_string_lossy().into_owned())}))
            .collect::<Vec<_>>(),
    })
}

/// A folder setting (`dictionariesFolder`, `userTemplatesFolder`): `auto` (or nothing) forgets
/// it; otherwise an existing folder, given as an absolute path or one starting with `~/`. With
/// `local_only`, a network folder is refused before anything looks at it (#2852): even checking
/// that it exists would make Windows authenticate to that host.
fn folder_setting(key: &str, f: &str, local_only: bool) -> Result<Option<String>, CmdError> {
    let f = f.trim();
    if f.is_empty() || f.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    if f.chars().count() > MAX_FOLDER_CHARS || f.chars().any(char::is_control) {
        return Err(CmdError::Params(format!("`{key}` isn't a folder path")));
    }
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).map(std::path::PathBuf::from);
    let path = match (f.strip_prefix("~/"), home) {
        (Some(rest), Some(h)) => h.join(rest),
        _ => std::path::PathBuf::from(f),
    };
    if local_only && (super::templates::is_network_path(f) || super::templates::is_network_path(&path.to_string_lossy())) {
        return Err(CmdError::Params(format!("`{key}` can't be a network folder")));
    }
    if !path.is_absolute() {
        return Err(CmdError::Params(format!("`{key}`: {f:?} isn't a full path")));
    }
    // Its files are read from now on, and on every start: only from inside the read root (#3737).
    crate::file_access::check_read(&path.to_string_lossy()).map_err(CmdError::Failed)?;
    if !path.is_dir() {
        return Err(CmdError::Failed(format!("no folder at {}", path.display())));
    }
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Search `folder` for Hunspell dictionaries before the system locations, from now on (the saved
/// setting, applied when the app starts and when it changes).
pub fn use_dictionaries_folder(folder: Option<&str>) {
    wordcraft_proof::set_dictionaries_folder(folder.map(std::path::PathBuf::from));
}
