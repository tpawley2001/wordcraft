//! File Conversion: saving as Plain Text first asks for the text encoding, the line ends and
//! whether a character the encoding lacks may become its closest match, as Word does. The
//! preview shows in red what won't save correctly. The choice is remembered for the session
//! (not on disk), and a later Save of the same .txt file uses it without asking again.

use serde::Serialize;
use serde_json::{Value, json};
use wordcraft_engine::txt::{LineEnd, TxtEncoding, TxtSaveOptions};

use crate::WordApp;
use crate::dialogs::Dialog;

/// The parameters that carry the choice on `file.save` / `file.saveAs`.
const KEYS: [&str; 3] = ["encoding", "lineEnd", "allowSubstitution"];

/// Most characters the preview shows.
const PREVIEW_CHARS: usize = 4000;

/// Other encoding's list: the WHATWG label and its name.
pub const OTHER_ENCODINGS: [(&str, &str); 17] = [
    ("utf-16le", "Unicode"),
    ("utf-16be", "Unicode (Big-Endian)"),
    ("utf-8", "Unicode (UTF-8)"),
    ("windows-1252", "Western European (windows-1252)"),
    ("windows-1250", "Central European (windows-1250)"),
    ("windows-1251", "Cyrillic (windows-1251)"),
    ("windows-1253", "Greek (windows-1253)"),
    ("windows-1254", "Turkish (windows-1254)"),
    ("windows-1255", "Hebrew (windows-1255)"),
    ("windows-1256", "Arabic (windows-1256)"),
    ("windows-1257", "Baltic (windows-1257)"),
    ("windows-1258", "Vietnamese (windows-1258)"),
    ("windows-874", "Thai (windows-874)"),
    ("shift_jis", "Japanese (Shift_JIS)"),
    ("gbk", "Simplified Chinese (GBK)"),
    ("big5", "Traditional Chinese (Big5)"),
    ("euc-kr", "Korean (EUC-KR)"),
];

/// End lines with: the parameter value and its name.
pub const LINE_ENDS: [(&str, &str); 4] = [("crlf", "CR/LF"), ("cr", "CR only"), ("lf", "LF only"), ("lfcr", "LF/CR")];

/// What the session remembers of the choice.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    /// The last choice made in the dialog.
    pub options: Option<TxtSaveOptions>,
    /// Plain-text files saved with a choice this session: Save writes them without asking.
    pub saved: Vec<std::path::PathBuf>,
}

/// The dialog's state.
#[derive(Clone, Debug, Serialize)]
pub struct FileConversionForm {
    /// The save it was asked for, run with the choice on OK.
    pub then: String,
    #[serde(skip)]
    pub params: Value,
    #[serde(skip)]
    pub document: u64,
    /// `windows`, `msdos` or `other`.
    pub kind: String,
    /// With `other`: a label from [`OTHER_ENCODINGS`].
    pub other: String,
    /// A value from [`LINE_ENDS`].
    pub line_end: String,
    pub allow_substitution: bool,
    /// The text the save writes (its beginning).
    #[serde(skip)]
    pub preview: String,
}

/// The dialog's fields for `o`: the encoding kind, the Other encoding label, the line end.
fn fields(o: &TxtSaveOptions) -> (&'static str, String, &'static str) {
    let (kind, other) = match &o.encoding {
        TxtEncoding::Windows => ("windows", "utf-16le".to_string()),
        TxtEncoding::MsDos => ("msdos", "utf-16le".to_string()),
        TxtEncoding::Utf8 => ("other", "utf-8".to_string()),
        TxtEncoding::Utf16Le => ("other", "utf-16le".to_string()),
        TxtEncoding::Utf16Be => ("other", "utf-16be".to_string()),
        TxtEncoding::Label(l) => ("other", l.clone()),
    };
    let line_end = match o.line_end {
        LineEnd::CrLf => "crlf",
        LineEnd::Cr => "cr",
        LineEnd::Lf => "lf",
        LineEnd::LfCr => "lfcr",
    };
    (kind, other, line_end)
}

/// `o` as `file.save` parameters.
fn params_of(o: &TxtSaveOptions) -> Value {
    let (kind, other, line_end) = fields(o);
    let encoding = if kind == "other" { other } else { kind.to_string() };
    json!({"encoding": encoding, "lineEnd": line_end, "allowSubstitution": o.allow_substitution})
}

impl FileConversionForm {
    fn new(app: &WordApp, then: &str, params: Value) -> FileConversionForm {
        let o = app.txt_save.options.clone().unwrap_or_default();
        let (kind, other, line_end) = fields(&o);
        let preview: String = wordcraft_engine::txt::export_text(&app.session.doc).chars().take(PREVIEW_CHARS).collect();
        FileConversionForm {
            then: then.to_string(),
            params,
            document: app.session.document_id(),
            kind: kind.to_string(),
            other,
            line_end: line_end.to_string(),
            allow_substitution: o.allow_substitution,
            preview,
        }
    }

    /// The encoding parameter for the choice.
    fn encoding(&self) -> &str {
        match self.kind.as_str() {
            "msdos" => "msdos",
            "other" => self.other.as_str(),
            _ => "windows",
        }
    }

    /// The choice as `file.save` parameters.
    fn choice(&self) -> Value {
        json!({"encoding": self.encoding(), "lineEnd": self.line_end, "allowSubstitution": self.allow_substitution})
    }

    fn options(&self) -> TxtSaveOptions {
        wordcraft_engine::cmd::file::txt_options(&self.choice()).unwrap_or_default()
    }
}

/// The plain-text file a user's save writes, if it is one.
fn txt_target(app: &WordApp, id: &str, params: &Value) -> Option<std::path::PathBuf> {
    if !matches!(id, "file.save" | "file.saveAs") {
        return None;
    }
    let path = params.get("path").and_then(Value::as_str).map(std::path::PathBuf::from).or_else(|| app.session.path.clone())?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "txt" | "text").then_some(path)
}

/// Before a user's save of a plain-text file: a Save of a file already saved with a choice this
/// session gets that choice; any other opens File Conversion (`Some`, the pending result), and
/// the save runs once it is answered (`ui.fileConversion`). A save that already says how
/// (`encoding`, `lineEnd`, `allowSubstitution`) goes ahead as it is.
pub fn before_save(app: &mut WordApp, id: &str, params: &mut Value) -> Option<Result<Value, String>> {
    if KEYS.iter().any(|k| params.get(k).is_some_and(|v| !v.is_null())) {
        return None;
    }
    let path = txt_target(app, id, params)?;
    let own_file = app.session.path.as_deref() == Some(path.as_path());
    if id == "file.save"
        && own_file
        && app.txt_save.saved.contains(&path)
        && let Some(o) = &app.txt_save.options
        && let (Some(obj), Some(choice)) = (params.as_object_mut(), params_of(o).as_object())
    {
        obj.extend(choice.clone());
        return None;
    }
    let form = FileConversionForm::new(app, id, params.clone());
    app.dialog = Some(Dialog::FileConversion { form: Box::new(form) });
    Some(Ok(json!({"pending": "fileConversion"})))
}

/// `ui.fileConversion`: answer the open File Conversion dialog. `answer` is `ok` (save with
/// the choice, which `encoding`, `lineEnd` and `allowSubstitution` may change first) or
/// `cancel` (nothing is saved).
pub fn answer(app: &mut WordApp, p: &Value) -> Result<Value, String> {
    let ok = match p.get("answer").and_then(Value::as_str) {
        Some("ok") => true,
        Some("cancel") => false,
        _ => return Err("`answer` must be ok or cancel".into()),
    };
    let Some(Dialog::FileConversion { mut form }) = app.dialog.take() else {
        return Err("no File Conversion dialog is open".into());
    };
    if !ok {
        return Ok(json!({"saved": false, "cancelled": true}));
    }
    if form.document != app.session.document_id() {
        return Err("the document for this save is no longer the one shown".into());
    }
    if let Some(e) = p.get("encoding").and_then(Value::as_str) {
        match e {
            "windows" | "msdos" => form.kind = e.to_string(),
            other => {
                form.kind = "other".into();
                form.other = other.to_string();
            }
        }
    }
    if let Some(l) = p.get("lineEnd").and_then(Value::as_str) {
        let Some((v, _)) = LINE_ENDS.iter().find(|(v, _)| *v == l) else {
            return Err("`lineEnd` must be crlf, cr, lf or lfcr".into());
        };
        form.line_end = (*v).to_string();
    }
    if let Some(a) = p.get("allowSubstitution").and_then(Value::as_bool) {
        form.allow_substitution = a;
    }
    let mut params = form.params.clone();
    if !params.is_object() {
        params = json!({});
    }
    if let (Some(obj), Some(choice)) = (params.as_object_mut(), form.choice().as_object()) {
        obj.extend(choice.clone());
    }
    let path = txt_target(app, &form.then, &params);
    app.txt_save.options = Some(form.options());
    let r = app.run(&form.then, params);
    if r.is_ok()
        && let Some(path) = path
        && !app.txt_save.saved.contains(&path)
    {
        app.txt_save.saved.push(path);
    }
    r
}

/// The dialog's body; true to close.
pub fn dialog(app: &mut WordApp, ui: &mut egui::Ui, form: &mut FileConversionForm) -> bool {
    ui.label(tl!("Select the encoding that makes your document readable."));
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tl!("Text encoding:")).strong());
    ui.radio_value(&mut form.kind, "windows".to_string(), tl!("Windows (Default)"));
    ui.radio_value(&mut form.kind, "msdos".to_string(), tl!("MS-DOS"));
    ui.horizontal(|ui| {
        ui.radio_value(&mut form.kind, "other".to_string(), tl!("Other encoding:"));
        ui.add_enabled_ui(form.kind == "other", |ui| {
            let shown = OTHER_ENCODINGS.iter().find(|(v, _)| *v == form.other).map_or(form.other.as_str(), |(_, l)| tl!(l));
            craft_ui::ComboBox::from_id_salt("file_conversion_encoding").width(240.0).height(300.0).selected_text(shown).show_ui(ui, |ui| {
                for (v, l) in OTHER_ENCODINGS {
                    ui.selectable_value(&mut form.other, v.to_string(), tl!(l));
                }
            });
        });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let label = ui.label(tl!("End lines with:"));
        let shown = LINE_ENDS.iter().find(|(v, _)| *v == form.line_end).map_or(form.line_end.as_str(), |(_, l)| tl!(l));
        let combo = craft_ui::ComboBox::from_id_salt("file_conversion_line_end").selected_text(shown).show_ui(ui, |ui| {
            for (v, l) in LINE_ENDS {
                ui.selectable_value(&mut form.line_end, v.to_string(), tl!(l));
            }
        });
        combo.response.labelled_by(label.id);
    });
    ui.checkbox(&mut form.allow_substitution, tl!("Allow character substitution"));
    ui.add_space(4.0);
    ui.label(tl!("Preview:"));
    let options = form.options();
    let bad = wordcraft_engine::txt::unencodable(&form.preview, &options);
    let tokens = crate::theme::Tokens::get(ui.ctx());
    let normal = egui::TextFormat { font_id: egui::FontId::monospace(12.0), color: ui.visuals().text_color(), ..Default::default() };
    let red = egui::TextFormat { color: tokens.red, ..normal.clone() };
    let mut job = egui::text::LayoutJob::default();
    let mut bad_at = bad.iter().peekable();
    let mut one = [0u8; 4];
    for (i, c) in form.preview.chars().enumerate() {
        let is_bad = bad_at.peek().is_some_and(|b| **b == i);
        if is_bad {
            bad_at.next();
        }
        job.append(c.encode_utf8(&mut one), 0.0, if is_bad { red.clone() } else { normal.clone() });
    }
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(420.0);
        egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
            ui.label(job);
        });
    });
    if !bad.is_empty() {
        ui.label(egui::RichText::new(tl!("Text marked in red won't save correctly in the chosen encoding.")).color(tokens.red));
    }
    let (ok, cancel) = crate::dialogs::buttons(ui, tl!("OK"));
    if !(ok || cancel) {
        return false;
    }
    // `ui.fileConversion` reads the dialog from `app.dialog`, which `show` has taken out.
    app.dialog = Some(Dialog::FileConversion { form: Box::new(form.clone()) });
    if let Err(e) = app.run("ui.fileConversion", json!({"answer": if ok { "ok" } else { "cancel" }})) {
        app.status(e);
    }
    true
}
