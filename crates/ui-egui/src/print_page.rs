//! File › Print (#641): the print settings and a preview of the pages.
//!
//! Page range and parity shape the PDF. Native printer settings and copy collation go to
//! the shared spooler; browser printing and saved print PDFs retain copies in the PDF.

use std::sync::Arc;

use egui::{Rect, Sense, Stroke, TextureHandle, Ui, pos2, vec2};
use serde_json::{Value, json};
use wordcraft_layout::DocLayout;

use crate::theme::{Tokens, medium, regular};
use crate::{WordApp, icons};

/// The Print page's settings. Not saved with the document or the UI state.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintSettings {
    /// `all`, `current`, `selection` or `custom`, or a Document Info report (`properties`,
    /// `markup`, `styles`, `autotext`, `keys`) printed instead of the document.
    pub range: String,
    /// The custom range text, like `1-3, 5`.
    pub pages: String,
    /// `all`, `odd` or `even`.
    pub parity: String,
    pub copies: u32,
    pub collate: bool,
    /// The physical sheet side shown in the planned print job (0-based).
    pub preview: usize,
    /// `auto`, `portrait` or `landscape`.
    pub orientation: String,
    /// The preview's zoom (1.0 is the page's real size), or `None` to fit the whole page.
    pub zoom: Option<f32>,
    /// Scale to paper size (#2108): `none` prints the document's page sizes, a Layout › Size
    /// name scales every page onto that paper. The document itself is unchanged.
    pub paper: String,
    /// Physical grid size or booklet binding, independent of document page setup.
    pub pages_per_sheet: String,
    /// `one` (Print One Sided) or `manual` (Manually Print on Both Sides: the fronts, then the
    /// backs once the user has turned the stack over).
    pub sides: String,
    /// The back pass of a manual duplex job, waiting for the user to turn the printed pages over.
    pub back_pass: Option<Value>,
}

impl Default for PrintSettings {
    /// Start with one page per sheet and the document's paper size.
    fn default() -> Self {
        PrintSettings {
            range: "all".into(),
            pages: String::new(),
            parity: "all".into(),
            copies: 1,
            collate: true,
            preview: 0,
            zoom: None,
            orientation: "auto".into(),
            paper: NO_SCALING.into(),
            pages_per_sheet: "1".into(),
            sides: "one".into(),
            back_pass: None,
        }
    }
}

impl PrintSettings {
    /// The params of `ui.print` and `file.printPdf` for these settings.
    pub fn params(&self) -> Value {
        let mut v = json!({
            "range": if self.what().is_some() { "all" } else { self.range.as_str() },
            "parity": self.parity,
            "copies": self.copies,
            "collate": self.collate,
            "orientation": self.orientation,
            "pagesPerSheet": self.pages_per_sheet.parse::<u32>().map_or_else(|_| json!(self.pages_per_sheet), |n| json!(n)),
        });
        if self.range == "custom" {
            v["pages"] = json!(self.pages);
        }
        if self.paper != NO_SCALING {
            v["paper"] = json!(self.paper);
        }
        if let Some(what) = self.what() {
            v["what"] = json!(what);
        }
        v
    }

    /// The Document Info report chosen instead of the document, if any.
    pub fn what(&self) -> Option<&str> {
        DOCUMENT_INFO.iter().any(|(id, _)| *id == self.range).then_some(self.range.as_str())
    }

    /// The params of the first job Print submits: with manual duplex, the fronts of the sheets.
    pub fn first_job(&self) -> Value {
        let mut v = self.params();
        if self.sides == MANUAL_DUPLEX {
            v["duplexPass"] = json!("front");
        }
        v
    }

    /// The paper pages are scaled to, as (width, height) in points, or `None` for their own size.
    pub fn paper_size(&self) -> Option<(f32, f32)> {
        wordcraft_geom::paper_by_name(&self.paper).map(|(_, w, h)| (w, h))
    }
}

/// `selectedText` is Word's Print Selection (only the selected text, without headers and
/// footers); `selection` prints the whole pages the selection touches.
const RANGES: [(&str, &str); 5] =
    [("all", "All pages"), ("selectedText", "Selection"), ("current", "Current page"), ("selection", "Selected pages"), ("custom", "Custom range")];

/// Something is selected to print: text, or whole table cells. Print Selection is greyed out
/// without it, as in Word.
fn has_selection(s: &wordcraft_engine::Session) -> bool {
    !s.sel.is_collapsed() || s.cell_range().is_some()
}
/// File › Print › Document Info: reports printed instead of the document, listed after the
/// page ranges as in Word.
const DOCUMENT_INFO: [(&str, &str); 5] = [
    ("properties", "Document Properties"),
    ("markup", "List of Markup"),
    ("styles", "Styles"),
    ("autotext", "AutoText Entries"),
    ("keys", "Key Assignments"),
];
const PARITIES: [(&str, &str); 3] = [("all", "All pages in the range"), ("odd", "Odd pages only"), ("even", "Even pages only")];
/// The Scale to paper size choice that keeps each page's own size.
const NO_SCALING: &str = "none";
const SIDES: [(&str, &str); 2] = [("one", "Print One Sided"), (MANUAL_DUPLEX, "Manually Print on Both Sides")];
/// The sides choice that prints the fronts, then the backs.
const MANUAL_DUPLEX: &str = "manual";
const ORIENTATIONS: [(&str, &str); 3] = [("auto", "Auto orientation"), ("portrait", "Portrait"), ("landscape", "Landscape")];

/// A choice list over `(id, label)` pairs; true when the choice changed.
fn choice(ui: &mut Ui, salt: &str, current: &mut String, items: &[(&str, &str)], label: &egui::Response) -> bool {
    let shown = items.iter().find(|(id, _)| *id == current.as_str()).map_or("", |(_, l)| *l);
    let mut changed = false;
    let combo = craft_ui::ComboBox::from_id_salt(salt).width(260.0).selected_text(tl!(shown)).show_ui(ui, |ui| {
        for (id, item) in items {
            if ui.selectable_label(current == id, tl!(item)).clicked() && current != id {
                *current = (*id).to_string();
                changed = true;
            }
        }
    });
    combo.response.labelled_by(label.id);
    changed
}

/// The Pages choice: the page ranges, then the Document Info reports under their heading.
fn range_choice(ui: &mut Ui, current: &mut String, label: &egui::Response, has_selection: bool) -> bool {
    let shown = RANGES.iter().chain(DOCUMENT_INFO.iter()).find(|(id, _)| *id == current.as_str()).map_or("", |(_, l)| *l);
    let mut changed = false;
    let combo = craft_ui::ComboBox::from_id_salt("print_range").width(260.0).selected_text(tl!(shown)).show_ui(ui, |ui| {
        for (i, (id, item)) in RANGES.iter().chain(DOCUMENT_INFO.iter()).enumerate() {
            if i == RANGES.len() {
                ui.separator();
                ui.label(egui::RichText::new(tl!("Document Info")).strong());
            }
            let enabled = *id != "selectedText" || has_selection;
            if ui.add_enabled_ui(enabled, |ui| ui.selectable_label(current == id, tl!(item))).inner.clicked() && current != id {
                *current = (*id).to_string();
                changed = true;
            }
        }
    });
    combo.response.labelled_by(label.id);
    changed
}

/// A choice list named without a visible label beside it (its heading names a group of choices).
fn choice_named(ui: &mut Ui, salt: &str, current: &mut String, items: &[(&str, &str)], name: &str) -> bool {
    let shown = items.iter().find(|(id, _)| *id == current.as_str()).map_or("", |(_, l)| *l);
    let mut changed = false;
    let combo = craft_ui::ComboBox::from_id_salt(salt).width(260.0).selected_text(tl!(shown)).show_ui(ui, |ui| {
        for (id, item) in items {
            if ui.selectable_label(current == id, tl!(item)).clicked() && current != id {
                *current = (*id).to_string();
                changed = true;
            }
        }
    });
    combo.response.accessible_name(name);
    changed
}

/// What the preview shows: the output layout, or a Document Info report (its own document and
/// layout, or why there is nothing to print).
#[derive(Clone)]
struct Previewed {
    lay: Arc<DocLayout>,
    /// The report (or, for Print Selection, the selection) printed instead of the document;
    /// `None` for the document itself.
    report: Option<Arc<wordcraft_doc::Document>>,
    error: Option<String>,
}

/// The output layout, kept while the document, Print Markup setting and Document Info choice
/// are unchanged.
fn layout_of(app: &WordApp, ctx: &egui::Context) -> Previewed {
    let id = egui::Id::unique("print_page_layout");
    let rev = app.session.rev();
    let markup = app.session.prefs.print_markup;
    let what = preview_key(app);
    if let Some((r, m, w, l)) = ctx.data(|d| d.get_temp::<(u64, bool, Option<String>, Previewed)>(id))
        && r == rev
        && m == markup
        && w == what
    {
        return l;
    }
    let l = match app.ui.print.what() {
        None if app.ui.print.range == "selectedText" => match app.session.print_selection(&app.ui.print.params()) {
            Ok(Some((doc, lay))) => Previewed { lay, report: Some(Arc::new(doc)), error: None },
            Ok(None) => Previewed { lay: app.session.export_layout_with(markup), report: None, error: None },
            Err(e) => Previewed { lay: app.session.export_layout_with(markup), report: None, error: Some(e.to_string()) },
        },
        None => Previewed { lay: app.session.export_layout_with(markup), report: None, error: None },
        Some(w) => match app.session.print_report(&json!({"what": w})) {
            Ok(Some((doc, lay))) => Previewed { lay, report: Some(Arc::new(doc)), error: None },
            Ok(None) => Previewed { lay: app.session.export_layout_with(markup), report: None, error: None },
            Err(e) => Previewed { lay: app.session.export_layout_with(markup), report: None, error: Some(e.to_string()) },
        },
    };
    ctx.data_mut(|d| d.insert_temp(id, (rev, markup, what, l.clone())));
    l
}

/// What the preview shows besides the document itself, for its caches: the Document Info report,
/// or Print Selection with the selection it prints.
fn preview_key(app: &WordApp) -> Option<String> {
    match app.ui.print.what() {
        Some(w) => Some(w.to_string()),
        None if app.ui.print.range == "selectedText" => Some(format!("selectedText {:?} {:?}", app.session.sel, app.session.cell_range())),
        None => None,
    }
}

/// A bounded per-cell texture cache, refreshed when the page, scale or Print Markup changes.
fn page_texture(app: &WordApp, ctx: &egui::Context, shown: &Previewed, page: usize, cache_slot: usize, width: f32) -> Option<TextureHandle> {
    let lay = &shown.lay;
    let id = egui::Id::unique(("print_page_texture", cache_slot));
    let what = preview_key(app);
    // File › Options › Display: the preview shows the background and drawings only if they print.
    let (background, drawings) = (app.session.prefs.print_background, app.session.prefs.print_drawings);
    let key = (
        app.session.rev(),
        app.session.prefs.print_markup,
        what,
        page,
        width as u32,
        (ctx.pixels_per_point() * 100.0) as u32,
        (background, drawings),
    );
    if let Some((k, t)) = ctx.data(|d| d.get_temp::<((u64, bool, Option<String>, usize, u32, u32, (bool, bool)), TextureHandle)>(id))
        && k == key
    {
        return Some(t);
    }
    let pg = lay.pages.get(page)?;
    // Zoomed in, the texture stays within what the GPU takes and is stretched beyond that.
    let max_side = ctx.input(|i| i.max_texture_side) as f32 / pg.w.max(pg.h).max(1.0);
    let scale = (width / pg.w.max(1.0) * ctx.pixels_per_point()).clamp(0.05, 4.0).min(max_side);
    let doc = shown.report.as_deref().unwrap_or(&app.session.doc);
    let base = wordcraft_render::RenderOptions::default();
    let opts = wordcraft_render::RenderOptions {
        display: wordcraft_layout::display::DisplayOptions { hide_drawings: !drawings, ..base.display.clone() },
        // Plain paper stands in for the page colour and page picture.
        read_paper: (!background).then_some(base.paper),
        ..base
    };
    let img = wordcraft_render::render_page(doc, pg, scale, &opts);
    let color = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.to_straight());
    let tex = ctx.load_texture("print_page_preview", color, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, (key, tex.clone())));
    Some(tex)
}

/// Return pressed with nothing else to take it: no field or list open and no control focused (a
/// focused button takes Return itself). Consumes it when so.
fn return_prints(ctx: &egui::Context) -> bool {
    !ctx.text_edit_focused()
        && !egui::Popup::is_any_open(ctx)
        && ctx.memory(|m| m.focused().is_none())
        && ctx.input_mut(|i| {
            // A fresh press only: `consume_key` also takes key repeats, so holding Return would
            // send a job per repeat once the first is handed off.
            let at = i.events.iter().position(
                |e| matches!(e, egui::Event::Key { key: egui::Key::Enter, pressed: true, repeat: false, modifiers, .. } if modifiers.is_none()),
            );
            at.map(|k| i.events.remove(k)).is_some()
        })
}

/// The Print page: settings on the left, the preview on the right.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    // Print Selection with nothing selected falls back to all pages, as Word greys it out.
    if app.ui.print.range == "selectedText" && !has_selection(&app.session) {
        app.ui.print.range = "all".into();
    }
    let shown = layout_of(app, &ctx);
    let lay = shown.lay.clone();
    let total = lay.pages.len();
    // A report prints from its own first page.
    let current = if shown.report.is_some() { (0, 0) } else { app.session.current_pages(&lay) };
    let ids = wordcraft_engine::print::page_ids(&lay);
    app.ui.print.preview = app.ui.print.preview.min(total.saturating_sub(1));

    back_pass(app, &ctx);
    let settings = |app: &mut WordApp, ui: &mut Ui| {
        {
            ui.set_width(SETTINGS_WIDTH);
            // The settings scroll on their own: the preview beside them keeps the whole height (#1156).
            // egui draws the bar at the right edge, over the right-aligned controls of a mirrored
            // column: a solid bar there takes its own gutter instead of floating over them.
            if crate::rtl::flows(ui) {
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            }
            egui::ScrollArea::vertical().id_salt("print_settings").show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let native = app.services.print_native.is_some();
                let hook = native || app.services.print.is_some();
                // #2958: as in Word, the page's action leads the settings, in the accent the app
                // gives a dialog's primary button: the first thing seen, and plainly a button
                // that prints now rather than one more setting in the list below.
                let ready = !native || app.printing.ready();
                // Return prints too, as in Word, whose ⌘P opens on Print as the default button.
                if hook
                    && (ui.add_enabled(ready, crate::widgets::primary_button(ui, tl!("Print")).min_size(vec2(320.0, 34.0))).clicked()
                        || (ready && app.dialog.is_none() && !app.file_dialog_open() && return_prints(ui.ctx())))
                {
                    print(app);
                }
                if native {
                    if let Some(discover) = &app.services.printers {
                        crate::printing::show(&mut app.printing, ui, discover, |label| crate::i18n::t(label).to_owned());
                    }
                    // A job can wait long on a sleeping printer's driver; Cancel frees the page.
                    if app.printing.busy() && app.services.cancel_print.is_some() && ui.button(tl!("Cancel printing")).clicked() {
                        let _ = app.run("ui.printCancel", serde_json::json!({}));
                    }
                    app.ui.print.copies = app.printing.settings.copies;
                } else if hook {
                    ui.label(tl!("Send the document straight to the system print dialog, or save a copy in another format below."));
                } else {
                    ui.label(tl!("Save a copy in another format. Printing goes through a PDF you can print from any viewer."));
                }
                if ui
                    .add(egui::Button::new(egui::RichText::new(tl!("Save these pages as PDF…")).font(medium(13.5))).min_size(vec2(320.0, 34.0)))
                    .clicked()
                {
                    app.print_pdf_dialog();
                }
                ui.add_space(6.0);

                if native {
                    if app.ui.print.copies > 1 {
                        ui.add_enabled_ui(!app.printing.busy(), |ui| ui.checkbox(&mut app.ui.print.collate, tl!("Collate copies")));
                    }
                } else {
                    let copies_label = ui.label(egui::RichText::new(tl!("Copies")).font(medium(13.0)));
                    ui.horizontal(|ui| {
                        let mut n = app.ui.print.copies;
                        ui.add(egui::DragValue::new(&mut n).range(1..=wordcraft_engine::print::MAX_COPIES).speed(0.2)).labelled_by(copies_label.id);
                        app.ui.print.copies = n.clamp(1, wordcraft_engine::print::MAX_COPIES);
                        if app.ui.print.copies > 1 {
                            ui.checkbox(&mut app.ui.print.collate, tl!("Collate copies"));
                        }
                    });
                }

                let pages_label = ui.label(egui::RichText::new(tl!("Pages")).font(medium(13.0)));
                if range_choice(ui, &mut app.ui.print.range, &pages_label, has_selection(&app.session)) {
                    // Show the first page that will print.
                    app.ui.print.preview = 0;
                }
                if app.ui.print.range == "custom"
                    && ui
                        .add(
                            egui::TextEdit::singleline(&mut app.ui.print.pages)
                                .hint_text(tl!("Pages like 1-3, 5 or p1s2-p3s2"))
                                .desired_width(260.0)
                                .char_limit(wordcraft_engine::print::MAX_SPEC_CHARS),
                        )
                        .labelled_by(pages_label.id)
                        .changed()
                {
                    app.ui.print.preview = 0;
                }
                if choice_named(ui, "print_parity", &mut app.ui.print.parity, &PARITIES, tl!("Pages in the range")) {
                    app.ui.print.preview = 0;
                }
                let sides_label = ui.label(egui::RichText::new(tl!("Sides")).font(medium(13.0)));
                choice(ui, "print_sides", &mut app.ui.print.sides, &SIDES, &sides_label);
                let orientation_label = ui.label(egui::RichText::new(tl!("Orientation")).font(medium(13.0)));
                if app.ui.print.pages_per_sheet == "2" || app.ui.print.pages_per_sheet.starts_with("booklet") {
                    ui.label(tl!("Landscape"));
                } else {
                    choice(ui, "print_orientation", &mut app.ui.print.orientation, &ORIENTATIONS, &orientation_label);
                }
                let pages_per_sheet_label = ui.label(egui::RichText::new(tl!("Pages per sheet")).font(medium(13.0)));
                choice(
                    ui,
                    "print_pages_per_sheet",
                    &mut app.ui.print.pages_per_sheet,
                    &[
                        ("1", "1"),
                        ("2", "2"),
                        ("4", "4"),
                        ("6", "6"),
                        ("9", "9"),
                        ("16", "16"),
                        ("booklet", "Booklet (left binding)"),
                        ("bookletReverse", "Booklet (right binding)"),
                    ],
                    &pages_per_sheet_label,
                );
                if app.ui.print.pages_per_sheet.starts_with("booklet") {
                    ui.label(tl!("Booklets: duplex on the short edge. Blank pages are added at the end."));
                }
                // Word: Pages per sheet › Scale to paper size. Only the printout changes.
                let paper_label = ui.label(egui::RichText::new(tl!("Scale to paper size")).font(medium(13.0)));
                let papers: Vec<(&str, &str)> =
                    std::iter::once((NO_SCALING, "No Scaling")).chain(wordcraft_geom::PAPER_SIZES.iter().map(|(n, _, _)| (*n, *n))).collect();
                choice(ui, "print_paper", &mut app.ui.print.paper, &papers, &paper_label);

                // Manual duplex refuses copies and N-up the engine can't split into sides: say so here.
                match shown
                    .error
                    .clone()
                    .map_or_else(|| wordcraft_engine::print::plan_in(&app.ui.print.first_job(), &ids, current).map_err(|e| e.to_string()), Err)
                {
                    Ok(plan) => {
                        let msg = crate::i18n::fmt(
                            tl!("{count} of {total} pages will print"),
                            &[("count", &plan.chosen.to_string()), ("total", &total.to_string())],
                        );
                        ui.label(egui::RichText::new(msg).color(t.text_dim));
                    }
                    Err(e) => {
                        ui.label(egui::RichText::new(e.to_string()).color(t.red));
                    }
                }
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                if hook {
                    ui.label(tl!("Or save a copy in another format:"));
                }
                export_formats(app, ui);
            });
        }
    };
    // A right-to-left interface splits the room by hand, settings on the side reading starts from:
    // a `horizontal_top` row there lays the preview over the settings.
    if crate::rtl::flows(ui) {
        let room = ui.available_rect_before_wrap();
        let split = (room.max.x - SETTINGS_WIDTH).max(room.min.x);
        let settings_rect = Rect::from_min_max(pos2(split, room.min.y), room.max);
        let preview_rect = Rect::from_min_max(room.min, pos2((split - SETTINGS_GAP).max(room.min.x), room.max.y));
        let layout = egui::Layout::top_down(egui::Align::Max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(settings_rect).layout(layout), |ui| settings(app, ui));
        // A window too narrow for a usable preview shows the settings alone.
        if preview_rect.width() >= MIN_PREVIEW_WIDTH {
            ui.scope_builder(egui::UiBuilder::new().max_rect(preview_rect).layout(layout), |ui| preview(app, ui, &shown));
        }
        ui.allocate_rect(room, Sense::hover());
        return;
    }
    crate::rtl::horizontal_top(ui, |ui| {
        crate::rtl::vertical(ui, |ui| settings(app, ui));
        ui.add_space(SETTINGS_GAP);
        crate::rtl::vertical(ui, |ui| preview(app, ui, &shown));
    });
}

/// Print: submit the job. With Manually Print on Both Sides that is the fronts of the sheets,
/// and when the chosen pages fill a back, the backs wait for the turn-over message's Continue.
pub fn print(app: &mut WordApp) {
    let params = app.ui.print.first_job();
    let manual = params.get("duplexPass").is_some();
    match app.run("ui.print", params.clone()) {
        Ok(r) if manual && r.get("chosen").and_then(Value::as_u64).is_some_and(|n| n >= 2) => {
            let mut back = params;
            back["duplexPass"] = json!("back");
            app.ui.print.back_pass = Some(back);
        }
        _ => {}
    }
}

/// Continue (true) prints the waiting back pass of a manual duplex job; Cancel (false) drops it.
pub fn finish_back_pass(app: &mut WordApp, go: bool) {
    let Some(back) = app.ui.print.back_pass.take() else { return };
    if go {
        let _ = app.run("ui.print", back);
    }
}

/// Manual duplex's message between the passes: turn the printed pages over and put them back.
/// Continue waits for a native printer to take the front pass first.
fn back_pass(app: &mut WordApp, ctx: &egui::Context) {
    if app.ui.print.back_pass.is_none() {
        return;
    }
    let ready = app.services.print_native.is_none() || app.printing.ready();
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("wordcraft.manualDuplex")).show(ctx, |ui| {
        ui.set_max_width(380.0);
        ui.heading(tl!("Manually Print on Both Sides"));
        ui.label(tl!("When the first side has printed, turn the printed pages over and put them back in the paper tray, then select Continue."));
        ui.add_space(8.0);
        ui.with_layout(crate::rtl::row_layout(ui, egui::Align::Center), |ui| {
            if ui.add_enabled(ready, crate::widgets::primary_button(ui, tl!("Continue"))).clicked() {
                answer = Some(true);
            }
            if ui.button(tl!("Cancel")).clicked() {
                answer = Some(false);
            }
        });
    });
    if modal.should_close() {
        answer = answer.or(Some(false));
    }
    if let Some(go) = answer {
        finish_back_pass(app, go);
    }
}

/// The settings column's width, and the space between it and the preview.
const SETTINGS_WIDTH: f32 = 380.0;
const SETTINGS_GAP: f32 = 24.0;
/// The narrowest preview pane worth drawing when the layout is mirrored.
const MIN_PREVIEW_WIDTH: f32 = 200.0;

/// Room around the page inside the preview pane.
const PREVIEW_MARGIN: f32 = 10.0;
/// The preview's bottom bar: the page indicator on the left, the zoom on the right.
const PREVIEW_BAR: f32 = 34.0;

/// The next preview zoom one step (10%) in or out, from 10% to 500% as in the status bar.
fn zoom_step(z: f32, up: bool) -> f32 {
    let z = (z * 10.0).round() / 10.0;
    (if up { z + 0.1 } else { z - 0.1 }).clamp(0.1, 5.0)
}

/// The preview (#1156): the page fills the pane, a bar below pages through the document and
/// zooms; zoomed in, the page scrolls both ways.
fn preview(app: &mut WordApp, ui: &mut Ui, shown: &Previewed) {
    if let Some(error) = &shown.error {
        ui.label(error.as_str());
        return;
    }
    let lay = &*shown.lay;
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let sheets = match wordcraft_engine::print::sheets(
        &app.ui.print.params(),
        lay,
        if shown.report.is_some() { (0, 0) } else { app.session.current_pages(lay) },
    ) {
        Ok(sheets) => sheets,
        Err(error) => {
            ui.label(error.to_string());
            return;
        }
    };
    let total = sheets.len();
    let page = app.ui.print.preview.min(total.saturating_sub(1));
    app.ui.print.preview = page;
    let Some(sheet) = sheets.get(page) else { return };
    let (pw, ph) = (sheet.width, sheet.height);
    let pane = vec2(ui.available_width().max(120.0), (ui.available_height() - PREVIEW_BAR).max(120.0));
    // Screen points per page point.
    let fit = ((pane.x - 2.0 * PREVIEW_MARGIN) / pw).min((pane.y - 2.0 * PREVIEW_MARGIN) / ph).max(0.01);
    let scale = app.ui.print.zoom.map_or(fit, |z| z * crate::canvas::PX_PER_PT);
    let size = vec2(pw * scale, ph * scale);
    // Navigate the imposed sheet views rather than the logical document pages.
    let label = crate::i18n::fmt(tl!("Sheet {page} of {pages}"), &[("page", &(page + 1).to_string()), ("pages", &total.to_string())]);
    egui::ScrollArea::both().id_salt("print_preview").auto_shrink(false).max_width(pane.x).max_height(pane.y).show(ui, |ui| {
        let room = vec2((size.x + 2.0 * PREVIEW_MARGIN).max(ui.available_width()), (size.y + 2.0 * PREVIEW_MARGIN).max(ui.available_height()));
        let (area, _) = ui.allocate_exact_size(room, Sense::hover());
        let r = Rect::from_center_size(area.center(), size);
        let resp = ui.interact(r, ui.scope_id().with("print_preview_page"), Sense::hover());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, &label));
        ui.painter().rect_filled(r.translate(vec2(2.0, 3.0)), 0.0, t.page_shadow);
        ui.painter().rect(r, 0.0, egui::Color32::WHITE, Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
        for (cache_slot, slot) in sheet.slots.iter().enumerate() {
            let Some(pg) = lay.pages.get(slot.page) else { continue };
            let k = (slot.width / pg.w).min(slot.height / pg.h);
            let offset = vec2(slot.x + (slot.width - pg.w * k) / 2.0, slot.y + (slot.height - pg.h * k) / 2.0);
            let on_sheet = Rect::from_min_size(r.min + offset * scale, vec2(pg.w, pg.h) * k * scale);
            if let Some(tex) = page_texture(app, &ctx, shown, slot.page, cache_slot, on_sheet.width()) {
                ui.painter().image(tex.id(), on_sheet, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
            }
        }
    });
    let rtl = crate::rtl::flows(ui);
    ui.horizontal(|ui| {
        ui.set_height(PREVIEW_BAR);
        let nav = |ui: &mut Ui, icon: &str, tip: &str, enabled: bool| {
            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tip));
            let col = if enabled { t.icon } else { t.text_disabled };
            if enabled && resp.hovered() {
                ui.painter().rect_filled(r, 3.0, t.hover);
            }
            icons::paint(ui.painter(), r.shrink(4.0), icon, col, col);
            enabled && resp.on_hover_text(tip).clicked()
        };
        if nav(ui, if rtl { "chevronRight" } else { "chevronLeft" }, tl!("Previous sheet"), page > 0) {
            app.ui.print.preview = page - 1;
        }
        ui.label(egui::RichText::new(&label).font(regular(13.0)));
        if nav(ui, if rtl { "chevronLeft" } else { "chevronRight" }, tl!("Next sheet"), page + 1 < total) {
            app.ui.print.preview = page + 1;
        }
        ui.with_layout(crate::rtl::far_side(ui, egui::Align::Center), |ui| {
            let z = scale / crate::canvas::PX_PER_PT;
            if nav(ui, "onePage", tl!("One Page"), true) {
                app.ui.print.zoom = None;
            }
            ui.label(egui::RichText::new(format!("{}%", (z * 100.0).round())).font(regular(13.0)));
            // The slider grows to the right in every language, as in the status bar: minus at its
            // left end, plus at its right. This cluster fills from the far end inwards.
            let (inner, outer) = (("minus", "Zoom Out", false), ("plus", "Zoom In", true));
            let (near, far) = if rtl { (inner, outer) } else { (outer, inner) };
            let enabled = |up: bool| if up { z < 5.0 } else { z > 0.1 };
            if nav(ui, near.0, tl!(near.1), enabled(near.2)) {
                app.ui.print.zoom = Some(zoom_step(z, near.2));
            }
            // A narrow window shortens the slider, then drops it, before it covers the page indicator.
            let room = ui.available_width() - 26.0 - 2.0 * ui.spacing().item_spacing.x - 12.0;
            if room >= 40.0 {
                let mut v = z;
                ui.spacing_mut().slider_width = room.min(110.0);
                let response = ui.add(egui::Slider::new(&mut v, 0.1..=5.0).logarithmic(true).show_value(false)).on_hover_text(tl!("Zoom"));
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, ui.is_enabled(), tl!("Zoom")));
                if response.changed() {
                    app.ui.print.zoom = Some(v.clamp(0.1, 5.0));
                }
            }
            if nav(ui, far.0, tl!(far.1), enabled(far.2)) {
                app.ui.print.zoom = Some(zoom_step(z, far.2));
            }
        });
    });
}

/// The export "format" of a Word 2003 XML Document: an `.xml` file saved with `format: "wordml2003"`.
pub(crate) const EXPORT_WORD_2003: &str = "wordml2003";

/// The "save a copy" formats (File › Export, and below the settings on the Print page).
pub fn export_formats(app: &mut WordApp, ui: &mut Ui) {
    for (label, ext) in [
        ("PDF document (*.pdf)", "pdf"),
        ("Word document (*.docx)", "docx"),
        ("Word XML Document (*.xml)", "xml"),
        ("Word 2003 XML Document (*.xml)", EXPORT_WORD_2003),
        ("OpenDocument Text (*.odt)", "odt"),
        ("OpenDocument Text Template (*.ott)", "ott"),
        ("Flat OpenDocument Text (*.fodt)", "fodt"),
        ("Rich Text Format (*.rtf)", "rtf"),
        ("Web page (*.html)", "html"),
        ("Markdown (*.md)", "md"),
        ("LaTeX (*.tex)", "tex"),
        ("Plain text (*.txt)", "txt"),
        ("Page image (*.png)", "png"),
    ] {
        if ui.add(egui::Button::new(egui::RichText::new(tl!(label)).font(medium(13.5))).min_size(vec2(320.0, 34.0))).clicked() {
            app.export_dialog(ext);
        }
        ui.add_space(4.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use wordcraft_engine::Session;

    /// A Document Info choice prints its report: the job says `what`, the preview lays out the
    /// report, and an empty report says why instead of showing pages.
    #[test]
    fn document_info_choices_send_what_and_preview_the_report() {
        let mut settings = PrintSettings::default();
        assert!(settings.params().get("what").is_none(), "the document prints by default");
        for (id, _) in DOCUMENT_INFO {
            settings.range = id.into();
            let v = settings.params();
            assert_eq!((v["what"].as_str(), v["range"].as_str()), (Some(id), Some("all")));
        }
        let mut app = WordApp::new(Session::new(wordcraft_doc::Document::from_text("Synthetic report text")), Services::default());
        let ctx = egui::Context::default();
        app.ui.print.range = "properties".into();
        let shown = layout_of(&app, &ctx);
        assert!(shown.report.is_some() && shown.error.is_none() && !shown.lay.pages.is_empty());
        assert!(page_texture(&app, &ctx, &shown, 0, 0, 200.0).is_some());
        app.ui.print.range = "markup".into();
        let shown = layout_of(&app, &ctx);
        assert!(shown.report.is_none() && shown.error.as_deref().is_some_and(|e| e.contains("nothing to print")));
        app.ui.print.range = "all".into();
        assert!(layout_of(&app, &ctx).report.is_none());
    }

    /// Print Markup changes layout and pixels without changing the document revision.
    #[test]
    fn preview_refreshes_layout_and_texture_when_print_markup_toggles() {
        let mut session = Session::new(wordcraft_doc::Document::from_text("Synthetic print preview"));
        session.run("select.text", &json!({"text": "preview"})).unwrap();
        session.run("review.newComment", &json!({"text": "Synthetic printed comment"})).unwrap();
        session.prefs.markup.comments = true;
        let mut app = WordApp::new(session, Services::default());
        let ctx = egui::Context::default();
        let rev = app.session.rev();
        let dirty = app.session.dirty;
        let mut previous = None;
        for markup in [false, true, false] {
            app.session.run("file.printMarkup", &json!({"value": markup})).unwrap();
            let shown = layout_of(&app, &ctx);
            let layout = shown.lay.clone();
            assert_eq!(layout.pages[0].markup > 0.0, markup);
            assert!(Arc::ptr_eq(&layout, &layout_of(&app, &ctx).lay), "unchanged layout is reused");
            // Keep width fixed: an accidental scale change must not mask a stale texture.
            let texture = page_texture(&app, &ctx, &shown, 0, 0, 300.0).unwrap();
            assert_eq!(texture.id(), page_texture(&app, &ctx, &shown, 0, 0, 300.0).unwrap().id());
            if let Some((old_layout, old_texture)) = previous.take() {
                assert!(!Arc::ptr_eq(&layout, &old_layout));
                assert_ne!(texture.id(), old_texture);
            }
            previous = Some((layout, texture.id()));
            assert_eq!(app.session.rev(), rev);
            assert_eq!(app.session.dirty, dirty);
        }
    }
}
