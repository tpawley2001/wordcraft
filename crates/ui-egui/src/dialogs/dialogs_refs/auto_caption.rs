use super::common::{NUMBER_FORMATS, SEPARATORS, choice};
use super::shell::{Next, go};
use super::{CaptionForm, RefsDialog, Serialize, Ui, Value, WordApp, buttons, json, vec2};

/// One kind of object in the AutoCaption list.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoCaptionRow {
    /// The object type as the document names it (`Microsoft Word Table`).
    pub name: String,
    /// Checked: inserting one adds a caption.
    pub on: bool,
    pub label: String,
    /// `below` or `above`.
    pub position: String,
    /// `\*` format name, as in the Caption dialog's Numbering.
    pub format: String,
    pub chapter: bool,
    pub chapter_level: u8,
    pub separator: String,
}

/// The AutoCaption dialog (Insert Caption › AutoCaption…): which objects get a caption as they
/// are inserted, and with what label, position and numbering.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoCaptionForm {
    pub objects: Vec<AutoCaptionRow>,
    /// The row the options below the list apply to.
    pub selected: usize,
    pub labels: Vec<String>,
    /// New Label's text while it is being typed.
    pub new_label: Option<String>,
    /// Numbering's options are shown.
    pub numbering: bool,
    /// The Caption dialog to go back to.
    #[serde(skip)]
    pub caption: Box<CaptionForm>,
}

/// The format names `references.autoCaption` lists, as the Caption dialog's `\*` names.
fn seq_name(ooxml: &str) -> &'static str {
    match ooxml {
        "lowerLetter" => "alphabetic",
        "upperLetter" => "ALPHABETIC",
        "lowerRoman" => "roman",
        "upperRoman" => "ROMAN",
        _ => "ARABIC",
    }
}

impl AutoCaptionForm {
    pub fn read(app: &mut WordApp, caption: CaptionForm) -> AutoCaptionForm {
        let v = app.session.run("references.autoCaption", &json!({})).unwrap_or(Value::Null);
        let objects = v["objects"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|o| {
                        let label = o["label"].as_str();
                        Some(AutoCaptionRow {
                            name: o["name"].as_str()?.to_string(),
                            on: label.is_some(),
                            label: label.unwrap_or("Figure").to_string(),
                            position: o["position"].as_str().unwrap_or("below").to_string(),
                            format: seq_name(o["format"].as_str().unwrap_or("decimal")).to_string(),
                            chapter: o["chapter"].as_u64().is_some_and(|c| c > 0),
                            chapter_level: o["chapter"].as_u64().filter(|c| (1..=9).contains(c)).unwrap_or(1) as u8,
                            separator: o["separator"].as_str().unwrap_or("-").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut labels = caption.labels.clone();
        for l in v["labels"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !labels.iter().any(|x| x == l) {
                labels.push(l.to_string());
            }
        }
        AutoCaptionForm { objects, selected: 0, labels, new_label: None, numbering: false, caption: Box::new(caption) }
    }

    /// `references.autoCaption` parameters for a row.
    pub fn params(row: &AutoCaptionRow) -> Value {
        if !row.on {
            return json!({"set": {"object": row.name, "label": null}});
        }
        json!({"set": {
            "object": row.name,
            "label": row.label,
            "position": row.position,
            "format": row.format,
            "chapter": if row.chapter { row.chapter_level } else { 0 },
            "separator": row.separator,
        }})
    }

    /// Turn AutoCaption on or off for every listed object, as the form says.
    pub fn apply(&self, app: &mut WordApp) -> Result<(), String> {
        for row in &self.objects {
            app.run("references.autoCaption", Self::params(row))?;
        }
        Ok(())
    }
}

/// The name an object type is shown by.
fn object_name(name: &str) -> String {
    match name {
        wordcraft_engine::cmd::references::AUTO_TABLE => tl!("Table").to_string(),
        wordcraft_engine::cmd::references::AUTO_EQUATION => tl!("Equation").to_string(),
        n => n.to_string(),
    }
}

/// Draws the AutoCaption dialog: a checklist of object types, and for the selected one its label,
/// position, New Label… and Numbering…; OK turns AutoCaption on or off for each.
pub(super) fn auto_caption_ui(app: &mut WordApp, ui: &mut Ui, f: &mut AutoCaptionForm) -> Next {
    ui.label(tl!("Add caption when inserting:"));
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(300.0);
        for (i, row) in f.objects.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut row.on, "");
                if ui.selectable_label(f.selected == i, object_name(&row.name)).clicked() {
                    f.selected = i;
                }
            });
        }
    });
    ui.add_space(6.0);
    let labels = f.labels.clone();
    let mut new_label = false;
    let mut numbering = false;
    if let Some(row) = f.objects.get_mut(f.selected) {
        ui.add_enabled_ui(row.on, |ui| {
            crate::rtl::Table::new("autocap_opts").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
                t.row(|r| {
                    let label_label = r.cell(|ui| ui.label(tl!("Use label:")));
                    r.cell(|ui| {
                        let opts: Vec<(String, &str)> = labels.iter().map(|l| (l.clone(), l.as_str())).collect();
                        choice(ui, "autocap_label", &mut row.label, &opts, &label_label)
                    });
                });
                t.row(|r| {
                    let position_label = r.cell(|ui| ui.label(tl!("Position:")));
                    r.cell(|ui| {
                        choice(
                            ui,
                            "autocap_pos",
                            &mut row.position,
                            &[("below".to_string(), "Below item"), ("above".to_string(), "Above item")],
                            &position_label,
                        )
                    });
                });
            });
            ui.horizontal(|ui| {
                new_label = ui.button(tl!("New Label…")).clicked();
                numbering = ui.button(tl!("Numbering…")).clicked();
            });
            if f.numbering {
                crate::rtl::Table::new("autocap_num").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
                    t.row(|r| {
                        let format_label = r.cell(|ui| ui.label(tl!("Format:")));
                        r.cell(|ui| {
                            let opts: Vec<(String, &str)> = NUMBER_FORMATS.iter().map(|(v, l)| (v.to_string(), *l)).collect();
                            choice(ui, "autocap_fmt", &mut row.format, &opts, &format_label)
                        });
                    });
                });
                ui.checkbox(&mut row.chapter, tl!("Add the chapter number"));
                ui.add_enabled_ui(row.chapter, |ui| {
                    crate::rtl::Table::new("autocap_chap").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
                        t.row(|r| {
                            let chapter_label = r.cell(|ui| ui.label(tl!("Chapters begin at:")));
                            r.cell(|ui| {
                                let names: Vec<String> = (1..=9u8).map(|l| crate::i18n::fmt(tl!("Heading {n}"), &[("n", &l.to_string())])).collect();
                                let opts: Vec<(u8, &str)> = (1..=9u8).zip(names.iter().map(String::as_str)).collect();
                                choice(ui, "autocap_level", &mut row.chapter_level, &opts, &chapter_label)
                            });
                        });
                        t.row(|r| {
                            let separator_label = r.cell(|ui| ui.label(tl!("Separator:")));
                            r.cell(|ui| {
                                let opts: Vec<(String, &str)> = SEPARATORS.iter().map(|(v, l)| (v.to_string(), *l)).collect();
                                choice(ui, "autocap_sep", &mut row.separator, &opts, &separator_label)
                            });
                        });
                    });
                });
            }
        });
    }
    if new_label {
        f.new_label = Some(String::new());
    }
    if numbering {
        f.numbering = !f.numbering;
    }
    if let Some(name) = f.new_label.as_mut() {
        ui.add_space(4.0);
        let label = ui.label(tl!("Label:"));
        let mut add = false;
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(name).desired_width(200.0)).labelled_by(label.id);
            add = ui.button(tl!("Add")).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        });
        if add {
            let typed = name.clone();
            f.new_label = None;
            match app.run("references.captionLabel", json!({"add": typed})) {
                Ok(v) => {
                    for l in v["labels"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                        if !f.labels.iter().any(|x| x == l) {
                            f.labels.push(l.to_string());
                            f.caption.labels.push(l.to_string());
                            if let Some(row) = f.objects.get_mut(f.selected) {
                                row.label = l.to_string();
                            }
                        }
                    }
                }
                Err(e) => app.status(e),
            }
        }
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok && let Err(e) = f.apply(app) {
        app.status(e);
    }
    if ok || cancel { go(RefsDialog::Caption((*f.caption).clone())) } else { Next::Stay }
}
