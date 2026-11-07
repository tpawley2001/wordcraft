//! Insert › Quick Parts › Building Blocks Organizer… and Create New Building Block (Save
//! Selection to Quick Part Gallery…, Alt+F3's AutoText gallery). Both end in
//! `insert.buildingBlocks`, so scripts get the same result without them.

use serde::Serialize;
use serde_json::json;

use super::chrome::buttons;
use super::{Dialog, Tokens, Ui, WordApp, vec2};

/// The galleries offered, with the names Word shows for them.
pub const GALLERIES: &[(&str, &str)] = &[
    ("docParts", "Quick Parts"),
    ("autoTxt", "AutoText"),
    ("bib", "Bibliographies"),
    ("coverPg", "Cover Pages"),
    ("eq", "Equations"),
    ("ftrs", "Footers"),
    ("hdrs", "Headers"),
    ("pgNum", "Page Numbers"),
    ("tbls", "Tables"),
    ("txtBox", "Text Boxes"),
    ("watermarks", "Watermarks"),
];

/// Insert options (`w:behavior`).
const BEHAVIORS: &[(&str, &str)] =
    &[("content", "Insert content only"), ("p", "Insert content in its own paragraph"), ("pg", "Insert content in its own page")];

/// A gallery's display name: Word's for the known ones, else the OOXML value.
pub fn gallery_label(g: &str) -> String {
    GALLERIES.iter().find(|(v, _)| *v == g).map(|(_, l)| tl!(l).to_string()).unwrap_or_else(|| g.to_string())
}

/// The organizer's column a click on its header sorts by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    Name,
    #[default]
    Gallery,
    Category,
    Template,
}

/// Create New Building Block / Modify Building Block.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingBlockForm {
    /// The entry edited (index into the document's glossary); `None` creates one.
    pub modify: Option<usize>,
    pub name: String,
    pub gallery: String,
    pub category: String,
    pub description: String,
    pub behavior: String,
    /// Opened from the organizer, which comes back when this closes.
    pub back: bool,
    /// Asking whether to redefine the entry with the same name and gallery.
    pub confirm: bool,
}

impl BuildingBlockForm {
    /// A new entry from the selection: its first line, at most 32 characters, as the name.
    pub fn create(app: &WordApp, gallery: &str) -> Self {
        let text = app.session.selected_text();
        let name: String = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(32).collect();
        BuildingBlockForm {
            modify: None,
            name: name.trim().to_string(),
            gallery: gallery.to_string(),
            category: "General".into(),
            description: String::new(),
            behavior: "content".into(),
            back: false,
            confirm: false,
        }
    }

    /// Entry `i`'s properties, to modify them.
    pub fn modify(app: &WordApp, i: usize) -> Option<Self> {
        let e = app.session.doc.glossary.get(i)?;
        Some(BuildingBlockForm {
            modify: Some(i),
            name: e.name.clone(),
            gallery: e.gallery.clone(),
            category: e.category.clone(),
            description: e.description.clone(),
            behavior: e.behaviors.first().cloned().unwrap_or_else(|| "content".into()),
            back: true,
            confirm: false,
        })
    }
}

/// One row of the organizer.
struct Row {
    index: usize,
    name: String,
    gallery: String,
    category: String,
}

fn rows(app: &WordApp, sort: SortBy) -> Vec<Row> {
    let mut rows: Vec<Row> = app
        .session
        .doc
        .glossary
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.types.iter().any(|t| t == "bbPlcHdr"))
        .map(|(index, e)| Row { index, name: e.name.clone(), gallery: gallery_label(&e.gallery), category: e.category.clone() })
        .collect();
    let key = |r: &Row| {
        let (n, g, c) = (r.name.to_lowercase(), r.gallery.to_lowercase(), r.category.to_lowercase());
        match sort {
            SortBy::Name => (n, g, c),
            SortBy::Category => (c, g, n),
            SortBy::Gallery | SortBy::Template => (g, c, n),
        }
    };
    rows.sort_by_cached_key(key);
    rows
}

/// Insert › Quick Parts › Building Blocks Organizer…. Returns true to close.
pub(super) fn organizer(app: &mut WordApp, ui: &mut Ui, selected: &mut Option<usize>, sort: &mut SortBy, confirm_delete: &mut bool) -> bool {
    let list = rows(app, *sort);
    if selected.is_none_or(|i| !list.iter().any(|r| r.index == i)) {
        *selected = list.first().map(|r| r.index);
    }
    let t = Tokens::get(ui.ctx());
    ui.label(tl!("Building blocks:"));
    egui::Frame::NONE.stroke(egui::Stroke::new(1.0, t.border)).inner_margin(4).show(ui, |ui| {
        ui.set_width(520.0);
        egui::ScrollArea::vertical().id_salt("building_blocks").max_height(200.0).auto_shrink([false, false]).show(ui, |ui| {
            crate::rtl::grid(ui, egui::Grid::new("building_blocks_table").num_columns(4).min_col_width(100.0).striped(true), |ui| {
                // A click on a header sorts by its column.
                for (col, label) in
                    [(SortBy::Name, "Name"), (SortBy::Gallery, "Gallery"), (SortBy::Category, "Category"), (SortBy::Template, "Template")]
                {
                    if ui.selectable_label(*sort == col, egui::RichText::new(tl!(label)).strong()).clicked() {
                        *sort = col;
                    }
                }
                ui.end_row();
                for r in &list {
                    let on = *selected == Some(r.index);
                    let mut clicked = false;
                    for text in [r.name.as_str(), r.gallery.as_str(), r.category.as_str(), tl!("This document")] {
                        clicked |= ui.selectable_label(on, text).clicked();
                    }
                    ui.end_row();
                    if clicked {
                        *selected = Some(r.index);
                    }
                }
            });
            if list.is_empty() {
                ui.label(egui::RichText::new(tl!("This document has no building blocks yet.")).small().color(t.text_dim));
            }
        });
    });
    // The selected entry's text and description.
    let entry = selected.and_then(|i| app.session.doc.glossary.get(i));
    let preview: String = entry
        .map(|e| e.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
        .chars()
        .take(400)
        .collect();
    let description = entry.map(|e| e.description.clone()).unwrap_or_default();
    ui.add_space(6.0);
    ui.label(tl!("Preview"));
    egui::Frame::NONE.stroke(egui::Stroke::new(1.0, t.border)).inner_margin(6).show(ui, |ui| {
        ui.set_width(512.0);
        ui.set_min_height(60.0);
        ui.label(preview);
    });
    if !description.is_empty() {
        ui.label(egui::RichText::new(description).small().color(t.text_dim));
    }
    ui.add_space(8.0);
    if *confirm_delete {
        ui.label(tl!("Are you sure you want to delete the selected building block?"));
        ui.horizontal(|ui| {
            if ui.button(tl!("Yes")).clicked() {
                if let Some(i) = *selected {
                    let _ = app.run("insert.buildingBlocks", json!({"delete": i}));
                }
                *selected = None;
                *confirm_delete = false;
            }
            if ui.button(tl!("No")).clicked() {
                *confirm_delete = false;
            }
        });
        return false;
    }
    let has = selected.is_some();
    let (mut edit, mut delete, mut insert, mut close) = (false, false, false, false);
    ui.horizontal(|ui| {
        edit = ui.add_enabled(has, egui::Button::new(tl!("Edit Properties…"))).clicked();
        delete = ui.add_enabled(has, egui::Button::new(tl!("Delete"))).clicked();
        insert = ui.add_enabled(has, egui::Button::new(tl!("Insert"))).clicked();
        close = ui.button(tl!("Close")).clicked();
    });
    close |= ui.input(|i| i.key_pressed(egui::Key::Escape));
    if delete {
        *confirm_delete = true;
    }
    if insert && let Some(i) = *selected {
        let _ = app.run("insert.buildingBlocks", json!({"insert": i}));
        return true;
    }
    if edit && let Some(form) = selected.and_then(|i| BuildingBlockForm::modify(app, i)) {
        app.dialog = Some(Dialog::NewBuildingBlock { form: Box::new(form) });
        return true;
    }
    close
}

/// Create New Building Block / Modify Building Block. Returns true to close.
pub(super) fn new_building_block(app: &mut WordApp, ui: &mut Ui, form: &mut BuildingBlockForm) -> bool {
    let mut categories: Vec<String> = app.session.doc.glossary.iter().map(|e| e.category.clone()).filter(|c| !c.is_empty()).collect();
    categories.push("General".into());
    categories.sort_by_key(|c| c.to_lowercase());
    categories.dedup();
    crate::rtl::grid(ui, egui::Grid::new("new_building_block").num_columns(2).spacing(vec2(10.0, 6.0)), |ui| {
        let l = ui.label(tl!("Name:"));
        ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(240.0)).labelled_by(l.id);
        ui.end_row();
        let l = ui.label(tl!("Gallery:"));
        let combo = craft_ui::ComboBox::from_id_salt("bb_gallery").selected_text(gallery_label(&form.gallery)).width(240.0).show_ui(ui, |ui| {
            for (v, label) in GALLERIES {
                ui.selectable_value(&mut form.gallery, v.to_string(), tl!(label));
            }
        });
        combo.response.labelled_by(l.id);
        ui.end_row();
        let l = ui.label(tl!("Category:"));
        let shown = if form.category == "General" { tl!("General").to_string() } else { form.category.clone() };
        let combo = craft_ui::ComboBox::from_id_salt("bb_category").selected_text(shown).width(240.0).show_ui(ui, |ui| {
            for c in &categories {
                let label = if c == "General" { tl!("General").to_string() } else { c.clone() };
                ui.selectable_value(&mut form.category, c.clone(), label);
            }
        });
        combo.response.labelled_by(l.id);
        ui.end_row();
        let l = ui.label(tl!("Description:"));
        ui.add(egui::TextEdit::singleline(&mut form.description).desired_width(240.0)).labelled_by(l.id);
        ui.end_row();
        let l = ui.label(tl!("Save in:"));
        let combo = craft_ui::ComboBox::from_id_salt("bb_save_in").selected_text(tl!("This document")).width(240.0).show_ui(ui, |ui| {
            let _ = ui.selectable_label(true, tl!("This document"));
        });
        combo.response.labelled_by(l.id);
        ui.end_row();
        let l = ui.label(tl!("Options:"));
        let current = BEHAVIORS.iter().find(|(v, _)| *v == form.behavior).map(|(_, l)| *l).unwrap_or("Insert content only");
        let combo = craft_ui::ComboBox::from_id_salt("bb_behavior").selected_text(tl!(current)).width(240.0).show_ui(ui, |ui| {
            for (v, label) in BEHAVIORS {
                ui.selectable_value(&mut form.behavior, v.to_string(), tl!(label));
            }
        });
        combo.response.labelled_by(l.id);
        ui.end_row();
    });
    if form.confirm {
        ui.add_space(8.0);
        ui.label(tl!("Do you want to redefine the building block entry?"));
        let (mut yes, mut no) = (false, false);
        ui.horizontal(|ui| {
            yes = ui.button(tl!("Yes")).clicked();
            no = ui.button(tl!("No")).clicked();
        });
        if no {
            form.confirm = false;
        }
        if !yes {
            return false;
        }
    }
    let (ok, cancel) = if form.confirm { (true, false) } else { buttons(ui, tl!("OK")) };
    // A new entry with the name of one in the same gallery redefines it: Word asks first.
    if ok && !form.confirm && form.modify.is_none() {
        let name = form.name.trim();
        if app.session.doc.glossary.iter().any(|e| e.name == name && e.gallery == form.gallery) {
            form.confirm = true;
            return false;
        }
    }
    let back = |app: &mut WordApp| {
        app.dialog = Some(Dialog::BuildingBlocks { selected: form.modify, sort: SortBy::default(), confirm_delete: false });
    };
    if ok && !form.name.trim().is_empty() {
        let props = json!({
            "name": form.name.trim(),
            "gallery": form.gallery,
            "category": form.category,
            "description": form.description,
            "behavior": form.behavior,
        });
        let params = match form.modify {
            Some(i) => {
                let mut p = props;
                if let Some(m) = p.as_object_mut() {
                    m.insert("modify".into(), json!(i));
                }
                p
            }
            None => json!({ "create": props }),
        };
        // The command redefines an entry with the same name and gallery, as Word does after asking.
        let _ = app.run("insert.buildingBlocks", params);
        if form.back {
            back(app);
        }
        return true;
    }
    if cancel && form.back {
        back(app);
    }
    cancel
}
