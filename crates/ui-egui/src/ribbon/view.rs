use super::common::stack;
use super::{Ui, WordApp, big, big_toggle, group, json, menu_button, small, vec2};

/// Draws the View tab's Views, Immersive, Page Movement, Show, Zoom, Dark Mode, Window and Macros
/// and Templates groups.
pub(super) fn view(app: &mut WordApp, ui: &mut Ui) {
    let v = app.session.view.clone();
    let v_source = app.session.md_source.is_some();
    group(ui, "Views", None, app, |ui, app| {
        big(ui, app, "readMode", "Read\nMode", "view.readMode", json!({}), false);
        big(ui, app, "printLayout", "Print\nLayout", "view.printLayout", json!({}), false);
        big(ui, app, "webLayout", "Web\nLayout", "view.webLayout", json!({}), false);
        stack(ui, |ui| {
            small(ui, app, "outline", Some("Outline"), "Outline", crate::widgets::ButtonAction::new("view.outline", json!({}), false));
            small(ui, app, "draft", Some("Draft"), "Draft", crate::widgets::ButtonAction::new("view.draft", json!({}), false));
            let source = crate::widgets::ButtonAction::new("view.markdownSource", json!({}), v_source);
            small(ui, app, "markdownSource", Some("Markdown"), "Markdown Source", source);
        });
    });
    group(ui, "Immersive", None, app, |ui, app| {
        big(ui, app, "focus", "Focus", "view.focus", json!({}), false);
        big(ui, app, "immersive", "Immersive\nReader", "view.immersive", json!({}), false);
    });
    group(ui, "Page Movement", None, app, |ui, app| {
        big(ui, app, "vertical", "Vertical", "view.vertical", json!({}), false);
        big(ui, app, "sideToSide", "Side\nto Side", "view.sideToSide", json!({}), false);
    });
    group(ui, "Show", None, app, |ui, app| {
        // Three rows fill the group's height; a fourth check box would run into its label.
        stack(ui, |ui| {
            let mut r = v.ruler;
            if ui.checkbox(&mut r, tl!("Ruler")).changed() {
                let _ = app.run("view.ruler", json!({"value": r}));
            }
            let mut g = v.gridlines;
            if ui.checkbox(&mut g, tl!("Gridlines")).changed() {
                let _ = app.run("view.gridlines", json!({"value": g}));
            }
            let mut n = v.nav_pane;
            if ui.checkbox(&mut n, tl!("Navigation Pane")).changed() {
                let _ = app.run("view.navigationPane", json!({"value": n}));
            }
        });
        stack(ui, |ui| {
            let mut w = v.hide_white_space;
            if ui.checkbox(&mut w, tl!("Hide White Space")).changed() {
                let _ = app.run("view.hideWhiteSpace", json!({"value": w}));
            }
        });
    });
    group(ui, "Zoom", None, app, |ui, app| {
        big(ui, app, "zoom", "Zoom", "ui.dialog", json!({"name": "zoom"}), false);
        big(ui, app, "zoom100", "100%", "view.zoom100", json!({}), false);
        // Step the zoom up and down by 10% (issue #67), from whatever the page shows now.
        stack(ui, |ui| {
            small(ui, app, "zoomIn", Some("Zoom In"), "Zoom In", crate::widgets::ButtonAction::new("view.zoomIn", json!({}), false));
            small(ui, app, "zoomOut", Some("Zoom Out"), "Zoom Out", crate::widgets::ButtonAction::new("view.zoomOut", json!({}), false));
        });
        stack(ui, |ui| {
            small(ui, app, "onePage", Some("One Page"), "One Page", crate::widgets::ButtonAction::new("view.onePage", json!({}), v.fit == "onePage"));
            small(
                ui,
                app,
                "multiplePages",
                Some("Multiple Pages"),
                "Multiple Pages",
                crate::widgets::ButtonAction::new("view.multiplePages", json!({}), v.multi_page),
            );
            small(
                ui,
                app,
                "pageWidth",
                Some("Page Width"),
                "Page Width",
                crate::widgets::ButtonAction::new("view.pageWidth", json!({}), v.fit == "pageWidth"),
            );
        });
    });
    group(ui, "Dark Mode", None, app, |ui, app| {
        big(ui, app, "darkMode", "Switch\nModes", "view.darkMode", json!({}), false);
        menu_button(
            ui,
            app,
            crate::widgets::MenuButtonOptions::new("interfaceTheme", Some("Interface\nTheme"), "Interface Theme", true, ""),
            |ui, app| {
                for a in crate::theme::Appearance::ALL {
                    if ui.add(egui::Button::selectable(app.ui.theme == a, tl!(a.label())).min_size(vec2(200.0, 0.0))).clicked() {
                        let _ = app.run("view.interfaceTheme", json!({"value": a.code()}));
                        ui.close();
                    }
                }
            },
        );
    });
    group(ui, "Window", None, app, |ui, app| {
        big(ui, app, "newWindow", "New\nWindow", "view.newWindow", json!({}), false);
        big(ui, app, "arrangeAll", "Arrange\nAll", "view.arrangeAll", json!({}), false);
        // While split the button is pressed and reads Remove Split, as Word's does (#3254).
        if v.split.is_some() {
            big_toggle(ui, app, "split", "Remove\nSplit", "view.split", true);
        } else {
            big(ui, app, "split", "Split", "view.split", json!({}), false);
        }
        // Across the app's windows (#322; `windows`).
        let w = app.session.windows.clone();
        stack(ui, |ui| {
            if w.side_by_side.is_none() && w.others.len() > 1 {
                menu_button(
                    ui,
                    app,
                    crate::widgets::MenuButtonOptions::new("sideBySide", Some("View Side by Side"), "View Side by Side", false, "view.sideBySide"),
                    |ui, app| window_list(ui, app, &w.others, "view.sideBySide"),
                );
            } else {
                let action = crate::widgets::ButtonAction::new("view.sideBySide", json!({}), w.side_by_side.is_some());
                small(ui, app, "sideBySide", Some("View Side by Side"), "View Side by Side", action);
            }
            let action = crate::widgets::ButtonAction::new("view.syncScroll", json!({}), w.sync_scroll);
            small(ui, app, "syncScroll", Some("Synchronous Scrolling"), "Synchronous Scrolling", action);
            let action = crate::widgets::ButtonAction::new("view.resetWindowPosition", json!({}), false);
            small(ui, app, "resetWindowPosition", Some("Reset Window Position"), "Reset Window Position", action);
        });
        menu_button(
            ui,
            app,
            crate::widgets::MenuButtonOptions::new("switchWindows", Some("Switch\nWindows"), "Switch Windows", true, "view.switchWindows"),
            crate::documents::switch_windows_menu,
        );
        // The Full Screen button carries its name for assistive technology (and for the
        // regression test that clicks it): big buttons are otherwise drawn unlabeled. Its
        // enabled state is the command's own, as every other ribbon control reports: the
        // browser has no full screen to ask for, and says so (#662).
        let on = crate::widgets::enabled(app, "view.fullScreen");
        let resp = big_toggle(ui, app, "fullScreen", "Full\nScreen", "view.fullScreen", v.full_screen);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, on, v.full_screen, "Full Screen"));
    });
    group(ui, "Macros", None, app, |ui, app| {
        big(ui, app, "macros", "Macros", "tools.macros", json!({}), false);
    });
    group(ui, "Templates", None, app, |ui, app| {
        big(ui, app, "documentTemplate", "Templates", "tools.templates", json!({}), false);
    });
}

/// The Outlining tab (Outline view): outline levels, moving, expanding and collapsing, what is
/// shown, and closing the view.
pub(super) fn outlining(app: &mut WordApp, ui: &mut Ui) {
    use wordcraft_engine::layout::outline::{BODY, para_level};
    let o = app.session.view.outline.clone();
    let level = app.session.doc.para_at(&app.session.sel.focus).map(|p| para_level(&app.session.doc, p)).unwrap_or(BODY);
    let name = |l: u8| if l >= BODY { tl!("Body Text").to_string() } else { crate::i18n::fmt(tl!("Level {n}"), &[("n", &l.to_string())]) };
    group(ui, "Outline Tools", None, app, |ui, app| {
        stack(ui, |ui| {
            crate::widgets::row(ui, |ui| {
                small(
                    ui,
                    app,
                    "outlineTop",
                    None,
                    "Promote to Heading 1",
                    crate::widgets::ButtonAction::new("outline.promoteToHeading1", json!({}), false),
                );
                small(ui, app, "outlinePromote", None, "Promote", crate::widgets::ButtonAction::new("outline.promote", json!({}), false));
                let levels: Vec<String> = (1..=BODY).map(name).collect();
                if let Some(v) = crate::widgets::combo_hint(ui, "outlineLevel", 100.0, &name(level), &levels, None, tl!("Outline Level"))
                    && let Some(i) = levels.iter().position(|x| *x == v)
                {
                    let l = if i + 1 >= usize::from(BODY) { json!("body") } else { json!(i + 1) };
                    let _ = app.run("outline.level", json!({ "level": l }));
                }
                small(ui, app, "outlineDemote", None, "Demote", crate::widgets::ButtonAction::new("outline.demote", json!({}), false));
                small(
                    ui,
                    app,
                    "outlineBody",
                    None,
                    "Demote to Body Text",
                    crate::widgets::ButtonAction::new("outline.demoteToBody", json!({}), false),
                );
            });
            ui.add_space(3.0);
            crate::widgets::row(ui, |ui| {
                small(ui, app, "outlineUp", None, "Move Up", crate::widgets::ButtonAction::new("outline.moveUp", json!({}), false));
                small(ui, app, "outlineDown", None, "Move Down", crate::widgets::ButtonAction::new("outline.moveDown", json!({}), false));
                small(ui, app, "outlineExpand", None, "Expand", crate::widgets::ButtonAction::new("outline.expand", json!({}), false));
                small(ui, app, "outlineCollapse", None, "Collapse", crate::widgets::ButtonAction::new("outline.collapse", json!({}), false));
                ui.add_space(6.0);
                ui.label(egui::RichText::new(tl!("Show Level")).size(11.5));
                let mut shown: Vec<String> = vec![tl!("All Levels").to_string()];
                shown.extend((1..BODY).map(name));
                let current = shown.get(usize::from(o.show_level)).cloned().unwrap_or_default();
                if let Some(v) = crate::widgets::combo_hint(ui, "outlineShowLevel", 90.0, &current, &shown, None, tl!("Show Level"))
                    && let Some(i) = shown.iter().position(|x| *x == v)
                {
                    let l = if i == 0 { json!("all") } else { json!(i) };
                    let _ = app.run("outline.showLevel", json!({ "level": l }));
                }
            });
            ui.add_space(3.0);
            crate::widgets::row(ui, |ui| {
                let mut f = !o.hide_formatting;
                if ui.checkbox(&mut f, tl!("Show Text Formatting")).changed() {
                    let _ = app.run("outline.showFormatting", json!({ "value": f }));
                }
                let mut first = o.first_line_only;
                if ui.checkbox(&mut first, tl!("Show First Line Only")).changed() {
                    let _ = app.run("outline.firstLineOnly", json!({ "value": first }));
                }
            });
        });
    });
    group(ui, "Close", None, app, |ui, app| {
        big(ui, app, "closeOutline", "Close Outline\nView", "outline.close", json!({}), false);
    });
}

/// The other WordCraft windows by title; picking one runs `id` with it.
fn window_list(ui: &mut Ui, app: &mut WordApp, others: &[wordcraft_engine::cmd::view::OtherWindow], id: &str) {
    if others.is_empty() {
        ui.add_enabled(false, egui::Label::new(tl!("No other WordCraft windows are open.")));
    }
    for o in others {
        if ui.add(egui::Button::new(o.title.as_str()).min_size(vec2(200.0, 0.0))).clicked() {
            let _ = app.run(id, json!({"window": o.id}));
            ui.close();
        }
    }
}

/// Immersive Reader's Page Color grid: four rows of six of our own soft reading tints.
pub(crate) const IMMERSIVE_COLORS: [[(u8, u8, u8); 6]; 4] = [
    [(0xF8, 0xF1, 0xE3), (0xF6, 0xE7, 0xD3), (0xF4, 0xEC, 0xC8), (0xE6, 0xF0, 0xD2), (0xD9, 0xEE, 0xE1), (0xDA, 0xEC, 0xF3)],
    [(0xE3, 0xE5, 0xF6), (0xEF, 0xE1, 0xF2), (0xF6, 0xDF, 0xE6), (0xF0, 0xF0, 0xF0), (0xE4, 0xE4, 0xE0), (0xFF, 0xF8, 0xDC)],
    [(0xE8, 0xD5, 0xB5), (0xD9, 0xC4, 0x9E), (0xC9, 0xDB, 0xB0), (0xB3, 0xD6, 0xC6), (0xB4, 0xCF, 0xE0), (0xC6, 0xC3, 0xE6)],
    [(0x3A, 0x34, 0x2C), (0x2E, 0x3B, 0x33), (0x27, 0x34, 0x40), (0x33, 0x2C, 0x40), (0x40, 0x2C, 0x33), (0x24, 0x24, 0x24)],
];

/// The Immersive Reader tab (View › Immersive Reader): Column Width, Page Color, Line Focus, Text
/// Spacing and Syllables, Read Aloud, and Close Immersive Reader. Screen settings only.
pub(super) fn immersive_reader(app: &mut WordApp, ui: &mut Ui) {
    use wordcraft_engine::cmd::view::ImmersiveColumnWidth;
    let v = app.session.view.clone();
    group(ui, "Immersive Reader", None, app, |ui, app| {
        menu_button(
            ui,
            app,
            crate::widgets::MenuButtonOptions::new("columns", Some("Column\nWidth"), "Column Width", true, "view.immersiveColumnWidth"),
            |ui, app| {
                for w in ImmersiveColumnWidth::ALL {
                    if ui.selectable_label(v.immersive_column_width == w, tl!(w.label())).clicked() {
                        let _ = app.run("view.immersiveColumnWidth", json!({"value": w.name()}));
                        ui.close();
                    }
                }
            },
        );
        menu_button(
            ui,
            app,
            crate::widgets::MenuButtonOptions::new("pageColor", Some("Page\nColor"), "Page Color", true, "view.immersivePageColor"),
            |ui, app| {
                for (value, name) in [("none", tl!("None")), ("sepia", tl!("Sepia")), ("inverse", tl!("Inverse"))] {
                    if ui.selectable_label(v.immersive_page_color == value, name).clicked() {
                        let _ = app.run("view.immersivePageColor", json!({"value": value}));
                        ui.close();
                    }
                }
                ui.separator();
                for row in IMMERSIVE_COLORS {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                        for (r, g, b) in row {
                            let hex = format!("#{r:02X}{g:02X}{b:02X}");
                            let chosen = v.immersive_page_color == hex;
                            let stroke = if chosen {
                                egui::Stroke::new(2.0, ui.visuals().selection.stroke.color)
                            } else {
                                egui::Stroke::new(1.0, egui::Color32::GRAY)
                            };
                            let swatch = egui::Button::new("").fill(egui::Color32::from_rgb(r, g, b)).stroke(stroke).min_size(vec2(18.0, 18.0));
                            if ui.add(swatch).on_hover_text(&hex).clicked() {
                                let _ = app.run("view.immersivePageColor", json!({"value": hex}));
                                ui.close();
                            }
                        }
                    });
                }
            },
        );
        menu_button(
            ui,
            app,
            crate::widgets::MenuButtonOptions::new("focus", Some("Line\nFocus"), "Line Focus", true, "view.lineFocus"),
            |ui, app| {
                for (n, name) in [(0u8, tl!("None")), (1, tl!("One Line")), (3, tl!("Three Lines")), (5, tl!("Five Lines"))] {
                    if ui.selectable_label(v.line_focus == n, name).clicked() {
                        let _ = app.run("view.lineFocus", json!({"value": n}));
                        ui.close();
                    }
                }
            },
        );
        big_toggle(ui, app, "lineSpacing", "Text\nSpacing", "view.textSpacing", v.text_spacing);
        big_toggle(ui, app, "hyphenation", "Syllables", "view.syllables", v.syllables);
        big(ui, app, "readAloud", "Read\nAloud", "review.readAloud", json!({}), false);
    });
    group(ui, "Close", None, app, |ui, app| {
        big(ui, app, "close", "Close Immersive\nReader", "view.closeImmersive", json!({}), false);
    });
}
