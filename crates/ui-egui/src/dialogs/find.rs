use super::{Serialize, Ui, Value, WordApp, json};
use wordcraft_doc::StoryRef;
use wordcraft_doc::para::InlineObject;
use wordcraft_engine::FindFormat;

/// The Find and Replace dialog's search options beyond the first row (#543).
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindExtra {
    pub wildcards: bool,
    pub prefix: bool,
    pub suffix: bool,
    pub ignore_punct: bool,
    pub ignore_space: bool,
    /// The search options are shown.
    pub more: bool,
    /// Special ▾ and Format ▾ go to Replace with (it last had the focus), not Find what.
    pub to_replace: bool,
    /// Find what's formatting (Format ▾); `None` = No Formatting.
    pub format: Option<FindFormat>,
    /// Replace with's formatting.
    pub replace_format: Option<FindFormat>,
}

/// The tabs of the Find and Replace dialog (#712).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FindTab {
    #[default]
    Find,
    Replace,
    GoTo,
}

/// What Go To moves to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GoToWhat {
    #[default]
    Page,
    Section,
    Bookmark,
    Comment,
    Footnote,
    Endnote,
    Field,
    Table,
    Graphic,
    Equation,
    Object,
    Heading,
}

impl GoToWhat {
    /// "Go to what", in Word's order (no Line yet).
    pub const ALL: [GoToWhat; 12] = [
        GoToWhat::Page,
        GoToWhat::Section,
        GoToWhat::Bookmark,
        GoToWhat::Comment,
        GoToWhat::Footnote,
        GoToWhat::Endnote,
        GoToWhat::Field,
        GoToWhat::Table,
        GoToWhat::Graphic,
        GoToWhat::Equation,
        GoToWhat::Object,
        GoToWhat::Heading,
    ];

    /// `edit.goto`'s `what`.
    pub fn name(self) -> &'static str {
        match self {
            GoToWhat::Page => "page",
            GoToWhat::Section => "section",
            GoToWhat::Bookmark => "bookmark",
            GoToWhat::Comment => "comment",
            GoToWhat::Footnote => "footnote",
            GoToWhat::Endnote => "endnote",
            GoToWhat::Field => "field",
            GoToWhat::Table => "table",
            GoToWhat::Graphic => "graphic",
            GoToWhat::Equation => "equation",
            GoToWhat::Object => "object",
            GoToWhat::Heading => "heading",
        }
    }

    fn label(self) -> &'static str {
        match self {
            GoToWhat::Page => tl!("Page"),
            GoToWhat::Section => tl!("Section"),
            GoToWhat::Bookmark => tl!("Bookmark"),
            GoToWhat::Comment => tl!("Comment"),
            GoToWhat::Footnote => tl!("Footnote"),
            GoToWhat::Endnote => tl!("Endnote"),
            GoToWhat::Field => tl!("Field"),
            GoToWhat::Table => tl!("Table"),
            GoToWhat::Graphic => tl!("Graphic"),
            GoToWhat::Equation => tl!("Equation"),
            GoToWhat::Object => tl!("Object"),
            GoToWhat::Heading => tl!("Heading"),
        }
    }

    /// The number box's label.
    fn prompt(self) -> &'static str {
        match self {
            GoToWhat::Page => tl!("Enter page number:"),
            GoToWhat::Section => tl!("Enter section number:"),
            GoToWhat::Bookmark => tl!("Enter bookmark name:"),
            GoToWhat::Comment => tl!("Enter reviewer's name:"),
            GoToWhat::Footnote => tl!("Enter footnote number:"),
            GoToWhat::Endnote => tl!("Enter endnote number:"),
            GoToWhat::Field => tl!("Enter field name:"),
            GoToWhat::Table => tl!("Enter table number:"),
            GoToWhat::Graphic => tl!("Enter graphic number:"),
            GoToWhat::Equation => tl!("Enter equation number:"),
            GoToWhat::Object => tl!("Enter object number:"),
            GoToWhat::Heading => tl!("Enter heading number:"),
        }
    }

    /// What a bad number box says.
    fn bad_number(self) -> &'static str {
        match self {
            GoToWhat::Page => tl!("Type a page number, or +2 / -1."),
            GoToWhat::Section => tl!("Type a section number, or +2 / -1."),
            GoToWhat::Footnote => tl!("Type a footnote number, or +2 / -1."),
            GoToWhat::Endnote => tl!("Type an endnote number, or +2 / -1."),
            GoToWhat::Table => tl!("Type a table number, or +2 / -1."),
            GoToWhat::Graphic => tl!("Type a graphic number, or +2 / -1."),
            GoToWhat::Equation => tl!("Type an equation number, or +2 / -1."),
            GoToWhat::Object => tl!("Type an object number, or +2 / -1."),
            GoToWhat::Heading => tl!("Type a heading number, or +2 / -1."),
            GoToWhat::Bookmark | GoToWhat::Comment | GoToWhat::Field => "",
        }
    }

    /// Picked from a list (a name), not typed as a number.
    fn named(self) -> bool {
        matches!(self, GoToWhat::Bookmark | GoToWhat::Comment | GoToWhat::Field)
    }
}

/// The Special ▾ menu: (name, code) for Find what, without and with wildcards.
const SPECIAL_FIND: &[(&str, &str)] = &[
    ("Paragraph Mark", "^p"),
    ("Tab Character", "^t"),
    ("Any Character", "^?"),
    ("Any Digit", "^#"),
    ("Any Letter", "^$"),
    ("Caret Character", "^^"),
    ("Section Character", "^%"),
    ("Paragraph Character", "^v"),
    ("Column Break", "^n"),
    ("Em Dash", "^+"),
    ("En Dash", "^="),
    ("Graphic", "^g"),
    ("Manual Line Break", "^l"),
    ("Manual Page Break", "^m"),
    ("Nonbreaking Hyphen", "^~"),
    ("Nonbreaking Space", "^s"),
    ("Optional Hyphen", "^-"),
    ("Section Break", "^b"),
    ("White Space", "^w"),
];

const SPECIAL_FIND_WILD: &[(&str, &str)] = &[
    ("Any Character", "?"),
    ("Character in Range", "[-]"),
    ("Beginning of Word", "<"),
    ("End of Word", ">"),
    ("Expression", "()"),
    ("Not", "[!]"),
    ("Num Occurrences", "{,}"),
    ("Previous 1 or More", "@"),
    ("0 or More Characters", "*"),
    ("Paragraph Mark", "^13"),
    ("Tab Character", "^t"),
    ("Caret Character", "^^"),
    ("Section Character", "^%"),
    ("Paragraph Character", "^v"),
    ("Column Break", "^n"),
    ("Em Dash", "^+"),
    ("En Dash", "^="),
    ("Graphic", "^g"),
    ("Manual Line Break", "^l"),
    ("Manual Page Break", "^m"),
    ("Nonbreaking Hyphen", "^~"),
    ("Nonbreaking Space", "^s"),
    ("Optional Hyphen", "^-"),
];

/// Replace with; with wildcards, "Find What Expression" (`\1`) comes first.
const SPECIAL_REPLACE: &[(&str, &str)] = &[
    ("Paragraph Mark", "^p"),
    ("Tab Character", "^t"),
    ("Caret Character", "^^"),
    ("Section Character", "^%"),
    ("Paragraph Character", "^v"),
    ("Clipboard Contents", "^c"),
    ("Column Break", "^n"),
    ("Em Dash", "^+"),
    ("En Dash", "^="),
    ("Find What Text", "^&"),
    ("Manual Line Break", "^l"),
    ("Manual Page Break", "^m"),
    ("Nonbreaking Hyphen", "^~"),
    ("Nonbreaking Space", "^s"),
    ("Optional Hyphen", "^-"),
];

/// The Special ▾ menu button; returns the code picked.
pub(super) fn special_menu(ui: &mut Ui, extra: &FindExtra) -> Option<&'static str> {
    let mut picked = None;
    ui.menu_button(format!("{} ▾", tl!("Special")), |ui| {
        let wild_replace: &[(&str, &str)] = &[("Find What Expression", "\\1")];
        let lists: [&[(&str, &str)]; 2] = match (extra.to_replace, extra.wildcards) {
            (false, false) => [SPECIAL_FIND, &[]],
            (false, true) => [SPECIAL_FIND_WILD, &[]],
            (true, false) => [SPECIAL_REPLACE, &[]],
            (true, true) => [wild_replace, SPECIAL_REPLACE],
        };
        for (name, code) in lists.into_iter().flatten() {
            if ui.button(format!("{}   {code}", tl!(name))).clicked() {
                picked = Some(*code);
                ui.close();
            }
        }
    });
    picked
}

/// Highlight colours Format ▾ › Highlight offers: (value, label).
const FORMAT_HIGHLIGHTS: &[(&str, &str)] = &[
    ("any", "Highlight"),
    ("none", "Not Highlight"),
    ("yellow", "Yellow"),
    ("brightGreen", "Bright Green"),
    ("turquoise", "Turquoise"),
    ("pink", "Pink"),
    ("red", "Red"),
    ("gray25", "Gray-25%"),
];

/// One on/off condition: either way or not at all.
fn tri_state(ui: &mut Ui, label: &str, value: &mut Option<bool>) {
    ui.horizontal(|ui| {
        ui.label(tl!(label));
        for (v, l) in [(None, "Any"), (Some(true), "Yes"), (Some(false), "No")] {
            ui.selectable_value(value, v, tl!(l));
        }
    });
}

/// Format ▾ (Font, Highlight, Style) and No Formatting, for the box that last had the focus, as
/// in Word's More › Format. A compact picker: the conditions are kept in `extra` and sent with
/// every search.
pub(super) fn format_menu(app: &WordApp, ui: &mut Ui, extra: &mut FindExtra) {
    let styles: Vec<(String, String)> = app.session.doc.styles.gallery().iter().map(|s| (s.name.clone(), crate::i18n::style_name(s))).collect();
    let target = if extra.to_replace { &mut extra.replace_format } else { &mut extra.format };
    ui.menu_button(format!("{} ▾", tl!("Format")), |ui| {
        ui.menu_button(tl!("Font"), |ui| {
            let f = target.get_or_insert_with(FindFormat::default);
            tri_state(ui, "Bold", &mut f.bold);
            tri_state(ui, "Italic", &mut f.italic);
            tri_state(ui, "Underline", &mut f.underline);
            ui.horizontal(|ui| {
                let label = ui.label(tl!("Font:"));
                let mut name = f.font.clone().unwrap_or_default();
                if ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0)).labelled_by(label.id).changed() {
                    f.font = (!name.trim().is_empty()).then_some(name);
                }
            });
            ui.horizontal(|ui| {
                let label = ui.label(tl!("Size:"));
                let mut size = f.size.map(|s| s.to_string()).unwrap_or_default();
                if ui.add(egui::TextEdit::singleline(&mut size).desired_width(60.0)).labelled_by(label.id).changed() {
                    f.size = size.trim().parse::<f32>().ok().filter(|s| s.is_finite());
                }
            });
            ui.horizontal(|ui| {
                let label = ui.label(tl!("Font color:"));
                let mut color = f.color.clone().unwrap_or_default();
                if ui.add(egui::TextEdit::singleline(&mut color).desired_width(80.0).hint_text("RRGGBB")).labelled_by(label.id).changed() {
                    f.color = (!color.trim().is_empty()).then(|| color.trim().to_string());
                }
            });
        });
        ui.menu_button(tl!("Highlight"), |ui| {
            for (value, label) in FORMAT_HIGHLIGHTS {
                let on = target.as_ref().and_then(|f| f.highlight.as_deref()) == Some(*value);
                if ui.selectable_label(on, tl!(label)).clicked() {
                    target.get_or_insert_with(FindFormat::default).highlight = (!on).then(|| value.to_string());
                    ui.close();
                }
            }
        });
        ui.menu_button(tl!("Style"), |ui| {
            for (name, shown) in &styles {
                let on = target.as_ref().and_then(|f| f.style.as_deref()) == Some(name.as_str());
                if ui.selectable_label(on, shown.as_str()).clicked() {
                    target.get_or_insert_with(FindFormat::default).style = (!on).then(|| name.clone());
                    ui.close();
                }
            }
        });
    });
    if ui.add_enabled(target.is_some(), egui::Button::new(tl!("No Formatting"))).clicked() {
        *target = None;
    }
    if target.as_ref().is_some_and(FindFormat::is_empty) {
        *target = None;
    }
}

/// The grey line under Find what or Replace with: "Format: Font: Bold, Not Italic".
pub(super) fn format_line(app: &WordApp, f: &FindFormat) -> String {
    let mut font: Vec<String> = Vec::new();
    if let Some(n) = &f.font {
        font.push(n.clone());
    }
    if let Some(sz) = f.size {
        font.push(crate::i18n::fmt(tl!("{size} pt"), &[("size", &sz.to_string())]));
    }
    for (on, yes, no) in [(f.bold, "Bold", "Not Bold"), (f.italic, "Italic", "Not Italic"), (f.underline, "Underline", "No underline")] {
        if let Some(b) = on {
            font.push(tl!(if b { yes } else { no }).to_string());
        }
    }
    if let Some(c) = &f.color {
        font.push(format!("{} {c}", tl!("Font color:")));
    }
    let mut parts = Vec::new();
    if !font.is_empty() {
        parts.push(format!("{} {}", tl!("Font:"), font.join(", ")));
    }
    if let Some(h) = &f.highlight {
        let label = FORMAT_HIGHLIGHTS.iter().find(|(v, _)| v == h).map_or("Highlight", |(_, l)| l);
        parts.push(tl!(label).to_string());
    }
    if let Some(st) = &f.style {
        let shown = app.session.doc.styles.find(st).map_or_else(|| st.clone(), crate::i18n::style_name);
        parts.push(format!("{} {shown}", tl!("Style:")));
    }
    format!("{} {}", tl!("Format:"), parts.join(", "))
}

/// What Search lists before anything is typed: common commands (#819).
pub(super) const SEARCH_SUGGESTIONS: [&str; 3] = ["review.spelling", "review.wordCount", "file.print"];

/// Search's "Find … in the document": the Navigation pane opens on its results for `text`.
pub(super) fn find_in_navigation_pane(app: &mut WordApp, ctx: &egui::Context, text: &str) {
    let qid = app.window_id("nav_query");
    ctx.data_mut(|d| d.insert_temp(qid, text.to_string()));
    app.ui.nav_tab = "results".into();
    let _ = app.run("view.navigationPane", json!({"value": true}));
    let _ = app.run("edit.find", json!({"text": text}));
}

/// Whether `e` is the text of the shortcut that opened a box (see `Dialog::Commands::swallow`):
/// a plain key press first means the user is typing, and one swallowed text is all there is.
pub(crate) fn swallow_shortcut_text(swallow: &mut bool, e: &egui::Event) -> bool {
    match e {
        egui::Event::Key { pressed: true, modifiers, .. } if !modifiers.alt => {
            *swallow = false;
            false
        }
        egui::Event::Text(_) if *swallow => {
            *swallow = false;
            true
        }
        _ => false,
    }
}

/// Search's filter: every word of the query is in the command's label, location or id, in English
/// or in the interface language, so "insert table" finds Insert › Tables › Table (#819).
pub(super) fn command_matches(c: &wordcraft_engine::CommandSpec, query: &str) -> bool {
    if query.trim().is_empty() {
        return false;
    }
    let hay = format!(
        "{} {} {} {} {}",
        c.label.to_lowercase(),
        c.location.to_lowercase(),
        c.id.to_lowercase(),
        crate::i18n::lowercase(tl!(c.label)),
        crate::i18n::lowercase(&crate::i18n::location(c.location))
    );
    query.split_whitespace().all(|w| hay.contains(&w.to_lowercase()) || hay.contains(&crate::i18n::lowercase(w)))
}

/// The Find and Replace dialog's Go To tab (#712): Word's "Go to what" kinds. A number (or `+2` /
/// `-1` from the selection; empty, Next and Previous step one), a bookmark, or a comment's
/// reviewer or a field's type to step through. Returns true to close.
pub(super) fn goto_body(app: &mut WordApp, ui: &mut Ui, what: &mut GoToWhat, target: &mut String, message: &mut String) -> bool {
    // The names to pick from, only while they are listed (finding them walks the document).
    let names: Vec<String> = match what {
        GoToWhat::Bookmark => app.session.doc.bookmarks().into_iter().map(|(n, _)| n).filter(|n| !n.starts_with('_')).collect(),
        GoToWhat::Comment => goto_names(&app.session.doc, |o, doc| match o {
            InlineObject::CommentStart { id } => doc.comments.get(id).map(|c| c.author.clone()),
            _ => None,
        }),
        GoToWhat::Field => goto_names(&app.session.doc, |o, _| match o {
            InlineObject::Field { instr, .. } | InlineObject::FieldStart { instr, .. } => instr.split_whitespace().next().map(str::to_uppercase),
            _ => None,
        }),
        _ => Vec::new(),
    };
    crate::rtl::horizontal_top(ui, |ui| {
        crate::rtl::vertical(ui, |ui| {
            ui.label(tl!("Go to what:"));
            for w in GoToWhat::ALL {
                if ui.selectable_label(*what == w, w.label()).clicked() && *what != w {
                    *what = w;
                    target.clear();
                    message.clear();
                }
            }
        });
        ui.add_space(16.0);
        crate::rtl::vertical(ui, |ui| {
            let label = ui.label(what.prompt());
            if !what.named() {
                let r = ui.add(egui::TextEdit::singleline(target).desired_width(220.0)).labelled_by(label.id);
                if !r.has_focus() {
                    r.request_focus();
                }
                let hint = if *what == GoToWhat::Page {
                    tl!("Type +2 or -1 to move from the page you are on.")
                } else {
                    tl!("Type +2 or -1 to move from where you are.")
                };
                ui.label(egui::RichText::new(hint).weak());
                return;
            }
            let any = match what {
                GoToWhat::Comment => Some(tl!("Any reviewer")),
                GoToWhat::Field => Some(tl!("Any field")),
                _ => None,
            };
            let shown = if target.is_empty() { any.unwrap_or("") } else { target.as_str() };
            craft_ui::ComboBox::from_id_salt("goto_bookmark")
                .width(220.0)
                .selected_text(shown.to_string())
                .show_ui(ui, |ui| {
                    if let Some(any) = any {
                        ui.selectable_value(target, String::new(), any);
                    }
                    for b in &names {
                        ui.selectable_value(target, b.clone(), b.as_str());
                    }
                })
                .response
                .labelled_by(label.id);
            if names.is_empty() {
                let none = match what {
                    GoToWhat::Comment => tl!("This document has no comments."),
                    GoToWhat::Field => tl!("This document has no fields."),
                    _ => tl!("This document has no bookmarks."),
                };
                ui.label(egui::RichText::new(none).weak());
            }
        });
    });
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).weak());
    }
    let mut close = false;
    let mut go: Option<Value> = None;
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        // Comments and fields step through Next / Previous; numbers do while their box is empty.
        let step = matches!(what, GoToWhat::Comment | GoToWhat::Field) || (!what.named() && target.trim().is_empty());
        let named_step = |by: i64| json!({"what": what.name(), "by": by, "name": target.clone()});
        if step && ui.button(tl!("Previous")).clicked() {
            go = Some(if what.named() { named_step(-1) } else { json!({"what": what.name(), "by": -1}) });
        }
        let label = if step { tl!("Next") } else { tl!("Go To") };
        if ui.button(label).clicked() || enter {
            go = match *what {
                GoToWhat::Bookmark if !target.is_empty() => Some(json!({"bookmark": target.clone()})),
                GoToWhat::Bookmark => None,
                GoToWhat::Comment | GoToWhat::Field => Some(named_step(1)),
                w => match goto_number(w, target) {
                    Some(p) => Some(p),
                    None => {
                        *message = w.bad_number().to_string();
                        None
                    }
                },
            };
        }
        if ui.button(tl!("Close")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
    });
    if let Some(params) = go {
        match app.run("edit.goto", params) {
            Ok(_) => message.clear(),
            Err(e) => *message = e,
        }
    }
    close
}

/// The distinct names `name_of` gives the body's inline objects, in document order.
fn goto_names(doc: &wordcraft_doc::Document, name_of: impl Fn(&InlineObject, &wordcraft_doc::Document) -> Option<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(para) = doc.para(StoryRef::Body, &path) else { continue };
        for name in para.objects.iter().filter_map(|o| name_of(o, doc)) {
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// Go To's number box as `edit.goto` params for `what`: empty steps one on, `+n` / `-n` move from
/// the selection, a number is that one. `None` for anything else.
pub(super) fn goto_number(what: GoToWhat, text: &str) -> Option<Value> {
    let w = what.name();
    let t = text.trim().replace('\u{2212}', "-");
    if t.is_empty() {
        return Some(json!({"what": w, "by": 1}));
    }
    if let Some(rest) = t.strip_prefix('+') {
        return rest.parse::<i64>().ok().filter(|n| *n >= 0).map(|n| json!({"what": w, "by": n}));
    }
    if t.starts_with('-') {
        return t.parse::<i64>().ok().map(|n| json!({"what": w, "by": n}));
    }
    t.parse::<u64>().ok().filter(|n| *n >= 1).map(|n| json!({"what": w, "n": n}))
}
