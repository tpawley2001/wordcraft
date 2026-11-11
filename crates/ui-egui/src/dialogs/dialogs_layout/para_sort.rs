//! Home › Paragraph › Sort Text.
use super::table::{SortKeyRow, sort_type_label};
use super::{Ui, Value, WordApp, buttons, heading, json, run};
use serde::Serialize;

/// A zero choice reads the whole paragraph; subsequent choices read fields.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaSortForm {
    pub keys: [SortKeyRow; 3],
    pub header: bool,
    pub separator: String,
    pub other: String,
    pub paragraphs: Vec<String>,
    pub lang: Option<String>,
    pub case_sensitive: bool,
}

impl ParaSortForm {
    pub fn read(app: &WordApp) -> Option<Self> {
        let s = &app.session;
        let (a, b) = s.sel.ordered();
        if a.story != b.story || a.path.parent() != b.path.parent() {
            return None;
        }
        let blocks = s.doc.container(a.story, &a.path)?.get(a.path.last()..=b.path.last())?;
        let paragraphs = blocks.iter().map(|b| b.as_para().map(|p| p.plain_text())).collect::<Option<Vec<_>>>()?;
        let none = SortKeyRow { column: None, kind: "text", descending: false };
        let mut f = Self {
            keys: [none.clone(), none.clone(), none],
            header: false,
            separator: "tab".into(),
            other: ";".into(),
            paragraphs,
            lang: s
                .doc
                .para(a.story, &a.path)
                .and_then(|p| wordcraft_engine::cmd::para_lang(&s.doc, p))
                .or_else(|| wordcraft_engine::cmd::default_lang(s)),
            case_sensitive: false,
        };
        f.pick(0, Some(0));
        Some(f)
    }

    fn separator_char(&self) -> char {
        match self.separator.as_str() {
            "tab" => '\t',
            "comma" => ',',
            _ => self.other.chars().next().unwrap_or(';'),
        }
    }

    pub fn fields(&self) -> usize {
        self.paragraphs.iter().map(|p| p.split(self.separator_char()).count()).max().unwrap_or(1)
    }

    pub fn pick(&mut self, row: usize, choice: Option<usize>) {
        let cells: Vec<_> = self
            .paragraphs
            .iter()
            .skip(usize::from(self.header))
            .map(|p| choice.and_then(|c| c.checked_sub(1)).map_or(p.as_str(), |n| p.split(self.separator_char()).nth(n).unwrap_or("")))
            .collect();
        let kind = wordcraft_engine::cmd::table::sort_type(cells, self.lang.as_deref());
        if let Some(k) = self.keys.get_mut(row) {
            k.column = choice;
            k.kind = kind;
        }
    }

    fn label(choice: Option<usize>) -> String {
        match choice {
            None => tl!("(none)").into(),
            Some(0) => tl!("Paragraphs").into(),
            Some(n) => crate::i18n::fmt(tl!("Field {n}"), &[("n", &n.to_string())]),
        }
    }

    pub fn params(&self) -> Value {
        let keys: Vec<_> = self
            .keys
            .iter()
            .filter_map(|k| {
                let c = k.column?;
                let mut v = json!({"type": k.kind, "descending": k.descending});
                if let Some(field) = c.checked_sub(1) {
                    v["field"] = json!(field);
                }
                Some(v)
            })
            .collect();
        let separator = if self.separator == "other" { &self.other } else { &self.separator };
        json!({"keys": keys, "header": self.header, "separator": separator, "caseSensitive": self.case_sensitive})
    }
}

pub(super) fn para_sort_ui(app: &mut WordApp, ui: &mut Ui, f: &mut ParaSortForm) -> bool {
    for row in 0..f.keys.len() {
        heading(ui, if row == 0 { tl!("Sort by") } else { tl!("Then by") });
        let Some(key) = f.keys.get(row).cloned() else { continue };
        let mut choice = key.column;
        ui.horizontal(|ui| {
            craft_ui::ComboBox::from_id_salt(("para_sort_field", row)).selected_text(ParaSortForm::label(choice)).width(180.0).show_ui(ui, |ui| {
                if row > 0 {
                    ui.selectable_value(&mut choice, None, tl!("(none)"));
                }
                for c in 0..=f.fields() {
                    ui.selectable_value(&mut choice, Some(c), ParaSortForm::label(Some(c)));
                }
            });
            if choice != key.column {
                f.pick(row, choice);
            }
            let Some(k) = f.keys.get_mut(row) else { return };
            ui.add_enabled_ui(k.column.is_some(), |ui| {
                ui.label(tl!("Type:"));
                craft_ui::ComboBox::from_id_salt(("para_sort_type", row)).selected_text(sort_type_label(k.kind)).width(90.0).show_ui(ui, |ui| {
                    for kind in ["text", "number", "date"] {
                        ui.selectable_value(&mut k.kind, kind, sort_type_label(kind));
                    }
                });
                ui.vertical(|ui| {
                    ui.radio_value(&mut k.descending, false, tl!("Ascending"));
                    ui.radio_value(&mut k.descending, true, tl!("Descending"));
                });
            });
        });
    }
    heading(ui, tl!("My list has"));
    ui.horizontal(|ui| {
        ui.radio_value(&mut f.header, true, tl!("Header row"));
        ui.radio_value(&mut f.header, false, tl!("No header row"));
    });
    let before = (f.separator.clone(), f.other.clone());
    ui.collapsing(tl!("Options…"), |ui| {
        ui.label(tl!("Separate fields at:"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut f.separator, "tab".into(), tl!("Tabs"));
            ui.radio_value(&mut f.separator, "comma".into(), tl!("Commas"));
            ui.radio_value(&mut f.separator, "other".into(), tl!("Other"));
            ui.add_enabled(f.separator == "other", egui::TextEdit::singleline(&mut f.other).char_limit(1).desired_width(40.0));
        });
        ui.checkbox(&mut f.case_sensitive, tl!("Case sensitive"));
    });
    if before != (f.separator.clone(), f.other.clone()) {
        for row in 0..f.keys.len() {
            let choice = f.keys.get(row).and_then(|k| k.column).map(|c| c.min(f.fields()));
            f.pick(row, choice);
        }
    }
    let ready = f.separator != "other" || f.other.chars().count() == 1;
    let (ok, cancel) = buttons(ui, "OK", ready);
    if ok {
        return run(app, "para.sort", f.params());
    }
    cancel
}
