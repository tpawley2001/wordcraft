use super::bookmarks::{bookmark, enclose_preview, icons_body, macros};
use super::chrome::{buttons, color_field};
use super::find::{FindTab, SEARCH_SUGGESTIONS, command_matches, find_in_navigation_pane, goto_body, special_menu, swallow_shortcut_text};
use super::font::{
    UNDERLINE_STYLES, apply_font, font_box, font_sample_box, font_sample_lines, font_sample_props, sample_paper, scale_box, signed_by_box,
    underline_style,
};
use super::forms::{cell_margins_dialog, custom_properties, table_properties};
use super::mailings::{ADDRESS_PARTS, recipient_list, rule_params};
use super::page_setup::{
    page_layout_commands, page_paper_group, page_paper_source_group, page_section_start_combo, page_section_start_value, page_valign_combo,
    page_valign_value, run_as_one_step,
};
use super::paragraph::{OUTLINE_LEVELS, ParaNumber, apply_paragraph, line_spacing_boxes, outline_entry, para_check, para_number};
use super::styles::manage_styles;
use super::{Dialog, MeasureExt, Tokens, Ui, Value, WordApp, json, semibold, vec2};

/// Returns true to close.
pub(super) fn body(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    match d {
        Dialog::RenameDocument { .. } => rename_document_dialog(app, ui, d),
        Dialog::Font { .. } => font_dialog(app, ui, d),
        Dialog::Paragraph { .. } => paragraph_dialog(app, ui, d),
        Dialog::Tabs { form } => crate::dialogs_para::tabs(app, ui, form),
        Dialog::Borders { form } => crate::dialogs_para::borders(app, ui, form),
        Dialog::Find { .. } => find_dialog(app, ui, d),
        Dialog::Enclose { .. } => enclose_dialog(app, ui, d),
        Dialog::TableProperties { .. } => table_properties_dialog(app, ui, d),
        Dialog::CellMargins { .. } => cell_margins_arm(app, ui, d),
        Dialog::Break { kind } => crate::dialogs_insert::break_dialog(app, ui, kind),
        Dialog::InsertTable { .. } => insert_table_dialog(app, ui, d),
        Dialog::PageSetup { .. } => page_setup_dialog(app, ui, d),
        Dialog::Link { .. } => link_dialog(app, ui, d),
        Dialog::Bookmark { name, by_location, hidden, deleted } => bookmark(app, ui, name, by_location, hidden, deleted),
        Dialog::WordCount { .. } => word_count_dialog(app, ui, d),
        Dialog::Zoom { .. } => zoom_dialog(app, ui, d),
        Dialog::BrowseObject { objects, current, focus } => browse_object_dialog(app, ui, objects, current, focus),
        Dialog::Watermark { .. } => watermark_dialog(app, ui, d),
        Dialog::Phonetic { .. } => phonetic_dialog(app, ui, d),
        Dialog::NewStyle { .. } => new_style_dialog(app, ui, d),
        Dialog::ModifyStyle { .. } => modify_style_dialog(app, ui, d),
        Dialog::ManageStyles { alphabetical, selected } => manage_styles(app, ui, alphabetical, selected),
        Dialog::TableStyle { .. } => table_style_dialog(app, ui, d),
        Dialog::Insert(f) => crate::dialogs_insert::body(app, ui, f),
        Dialog::Refs(f) => crate::dialogs_refs::body(app, ui, f),
        Dialog::Commands { .. } => commands_dialog(app, ui, d),
        Dialog::Macros { name } => macros(app, ui, name),
        Dialog::BuildingBlocks { selected, sort, confirm_delete } => super::building_blocks::organizer(app, ui, selected, sort, confirm_delete),
        Dialog::NewBuildingBlock { form } => super::building_blocks::new_building_block(app, ui, form),
        Dialog::Icons { query, category, selected } => icons_body(app, ui, query, category, selected),
        Dialog::PasteSpecial { .. } => paste_special_dialog(app, ui, d),
        Dialog::About { .. } => about_dialog(ui, d, app.ui.pirate_speak),
        Dialog::DiscardRecovery { .. } => discard_recovery_dialog(app, ui, d),
        Dialog::SaveChanges { .. } => save_changes_dialog(app, ui, d),
        Dialog::RecipientList { fields, rows, message, filter } => recipient_list(app, ui, fields, rows, message, filter),
        Dialog::InsertMergeField { .. } => insert_merge_field_dialog(app, ui, d),
        Dialog::Password { .. } => password_dialog(app, ui, d),
        Dialog::EncryptPassword { .. } => encrypt_password_dialog(app, ui, d),
        Dialog::GeneralOptions { .. } => general_options_dialog(app, ui, d),
        Dialog::LockTracking { .. } => lock_tracking_dialog(app, ui, d),
        Dialog::ReadOnlyRecommended { .. } => read_only_recommended_dialog(app, ui, d),
        Dialog::DefineList { form } => crate::dialogs_lists::define_list(app, ui, form),
        Dialog::AdjustListIndents { form } => crate::dialogs_lists::adjust_list_indents(app, ui, form),
        Dialog::TrackOptions { form } => crate::dialogs_lists::track_options(app, ui, form),
        Dialog::Review(d) => crate::dialogs_review::body(app, ui, d),
        Dialog::CustomizeKeyboard { form, .. } => crate::dialogs_keyboard::customize_keyboard(app, ui, form),
        Dialog::CustomizeRibbon { form } => crate::dialogs_ribbon::customize_ribbon(app, ui, form),
        Dialog::FieldCode { .. } => field_code_dialog(app, ui, d),
        Dialog::Control { form } => crate::dialogs_controls::body(app, ui, form),
        Dialog::SelectTable { .. } => select_table_dialog(app, ui, d),
        Dialog::NoteOptions { form } => crate::dialogs_notes::note_options(app, ui, form),
        Dialog::Templates { form } => crate::dialogs_templates::templates(app, ui, form),
        Dialog::Organizer { form } => crate::dialogs_templates::organizer(app, ui, form),
        Dialog::StylePaneOptions { form } => super::style_pane_options::style_pane_options(app, ui, form),
        Dialog::CustomProperties(form) => custom_properties(app, ui, form),
        Dialog::Layout(d) => crate::dialogs_layout::body(app, ui, d),
        Dialog::FindRecipient { .. } => find_recipient_dialog(app, ui, d),
        Dialog::MergeRecords { .. } => merge_records_dialog(app, ui, d),
        Dialog::MergeRule { .. } => merge_rule_dialog(app, ui, d),
        Dialog::MatchFields { .. } => match_fields_dialog(ui, d),
        Dialog::CheckErrors { .. } => check_errors_dialog(ui, d),
        Dialog::ConvertDocument => convert_document_dialog(app, ui),
        Dialog::MarkAsFinal => mark_as_final_dialog(app, ui),
        Dialog::Compatibility { issues, version } => compatibility_dialog(ui, issues, version),
    }
}

fn rename_document_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::RenameDocument { name, message, focus } = d else {
        return false;
    };
    let label = ui.label(tl!("Document name"));
    let r = ui.add(egui::TextEdit::singleline(name).desired_width(320.0).char_limit(200)).labelled_by(label.id);
    if *focus {
        r.request_focus();
        *focus = false;
    }
    if !message.is_empty() {
        ui.colored_label(ui.visuals().error_fg_color, message.as_str());
    }
    let (ok, cancel) = buttons(ui, tl!("Rename"));
    if cancel {
        return true;
    }
    if ok && name.trim() == app.title_stem() {
        return true;
    }
    if ok {
        match app.run("file.rename", json!({"name": name})) {
            Ok(_) => return true,
            Err(e) => *message = e,
        }
    }
    false
}

fn font_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Font {
        font,
        font_ea,
        asian,
        size,
        bold,
        italic,
        underline,
        underline_color,
        strike,
        double_strike,
        sup,
        sub,
        small_caps,
        caps,
        hidden,
        color,
        spacing,
        scale,
        position,
        kern,
        kern_at,
        ..
    } = d
    else {
        return false;
    };
    let lined = underline_style(underline).is_some_and(|u| u != wordcraft_doc::props::Underline::None);
    let mut sample_props =
        font_sample_props(app, sample_paper(app), font, size, color, (*bold, *italic, lined, *strike), (*small_caps, *caps, *hidden), *spacing);
    font_sample_lines(&mut sample_props, underline, underline_color, *double_strike);
    // The sample is a few lines tall: a large raise or drop shows as the most it can.
    (sample_props.scale, sample_props.position) = (Some(scale.clamp(1.0, 600.0)), Some(position.clamp(-8.0, 8.0)));
    crate::rtl::Table::new("fontdlg").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
        // Word's two font boxes when East Asian text is in use: the Latin text font and
        // the Asian text font, each with its own list (type to find a font).
        let prev = app.previews.font_preview_fn();
        if *asian {
            t.row(|r| {
                r.cell(|ui| ui.label(tl!("Asian text font:")));
                r.cell(|ui| {
                    let fams = app.previews.east_asian_families();
                    let stored = app.previews.font_stored(font_ea);
                    let latin = app.previews.font_stored(font);
                    let warning = app.east_asian_font_box_warning(&stored, &latin);
                    if let Some(f) = font_box(ui, ("fontdlg_asian", font_ea.as_str()), &fams, &*prev, warning.as_deref()) {
                        if f == crate::widgets::ADD_FONT {
                            let _ = app.run("font.add", json!({}));
                        } else if f == crate::widgets::LOAD_SYSTEM_FONTS {
                            let _ = app.run("font.loadSystem", json!({}));
                        } else {
                            *font_ea = f;
                        }
                    }
                });
            });
        }
        t.row(|r| {
            r.cell(|ui| ui.label(if *asian { tl!("Latin text font:") } else { tl!("Font:") }));
            r.cell(|ui| {
                let fams = app.previews.families();
                let stored = app.previews.font_stored(font);
                let warning = app.font_dialog_warning(&stored);
                if let Some(f) = font_box(ui, ("fontdlg_font", font.as_str()), &fams, &*prev, warning.as_deref()) {
                    if f == crate::widgets::ADD_FONT {
                        let _ = app.run("font.add", json!({}));
                    } else if f == crate::widgets::LOAD_SYSTEM_FONTS {
                        let _ = app.run("font.loadSystem", json!({}));
                    } else {
                        *font = f;
                    }
                }
            });
        });
        t.row(|r| {
            let size_label = r.cell(|ui| ui.label(tl!("Size:")));
            r.cell(|ui| ui.text_edit_singleline(size).labelled_by(size_label.id));
        });
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Font color:")));
            r.cell(|ui| {
                let theme = app.session.doc.settings.theme_colors.clone();
                color_field(ui, &theme, color, "Automatic", "auto");
            });
        });
        t.row(|r| {
            let label = r.cell(|ui| ui.label(tl!("Underline style:")));
            r.cell(|ui| {
                let shown = UNDERLINE_STYLES.iter().find(|(v, _)| v == underline).map_or("", |(_, l)| tl!(l));
                let combo = craft_ui::ComboBox::from_id_salt("fontdlg_underline").width(160.0).height(400.0).selected_text(shown).show_ui(ui, |ui| {
                    for (v, l) in UNDERLINE_STYLES {
                        if ui.selectable_label(underline == v, tl!(l)).clicked() {
                            *underline = v.to_string();
                        }
                    }
                });
                combo.response.labelled_by(label.id);
            });
        });
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Underline color:")));
            r.cell(|ui| {
                // Nothing to colour while there is no underline.
                let theme = app.session.doc.settings.theme_colors.clone();
                ui.add_enabled_ui(underline != "none", |ui| color_field(ui, &theme, underline_color, "Automatic", "auto"));
            });
        });
        t.row(|r| {
            let label = r.cell(|ui| ui.label(tl!("Scale:")));
            r.cell(|ui| scale_box(ui, scale, &label));
        });
        t.row(|r| {
            let label = r.cell(|ui| ui.label(tl!("Spacing:")));
            r.cell(|ui| signed_by_box(ui, "fontdlg_spacing", spacing, (tl!("Expanded"), tl!("Condensed")), 1.0, &label));
        });
        t.row(|r| {
            let label = r.cell(|ui| ui.label(tl!("Position:")));
            r.cell(|ui| signed_by_box(ui, "fontdlg_position", position, (tl!("Raised"), tl!("Lowered")), 3.0, &label));
        });
        t.row(|r| {
            let label = tl!("Kerning for fonts:");
            r.cell(|ui| match kern {
                Some(on) => {
                    ui.checkbox(on, label);
                }
                None => {
                    if ui.add(egui::Checkbox::new(&mut false, label).indeterminate(true)).clicked() {
                        *kern = Some(true);
                    }
                }
            });
            r.cell(|ui| {
                ui.horizontal(|ui| {
                    let enabled = *kern == Some(true);
                    let field = egui::DragValue::new(kern_at).speed(0.5).range(1.0..=1638.0).max_decimals(1);
                    // The number comes first in the reading order, then "points and above"; a left-to-right
                    // row in a right-to-left interface runs the other way, so there the label is first.
                    if crate::rtl::on() && !crate::rtl::flows(ui) {
                        let points_label = ui.label(tl!("Points and above"));
                        ui.add_enabled(enabled, field).labelled_by(points_label.id);
                    } else {
                        let points = ui.add_enabled(enabled, field);
                        let points_label = ui.label(tl!("Points and above"));
                        points.labelled_by(points_label.id);
                    }
                });
            });
        });
    });
    ui.separator();
    ui.label(egui::RichText::new(tl!("Effects")).font(semibold(12.5)));
    crate::rtl::Table::new("fx").columns(2).show(ui, |t| {
        t.row(|r| {
            r.cell(|ui| ui.checkbox(bold, tl!("Bold")));
            r.cell(|ui| ui.checkbox(italic, tl!("Italic")));
        });
        t.row(|r| {
            // Strikethrough and Double strikethrough exclude each other: the one just ticked wins.
            let was_strike = *strike;
            r.cell(|ui| ui.checkbox(strike, tl!("Strikethrough")));
            r.cell(|ui| ui.checkbox(double_strike, tl!("Double strikethrough")));
            if *strike && *double_strike {
                if was_strike {
                    *strike = false;
                } else {
                    *double_strike = false;
                }
            }
        });
        t.row(|r| {
            // Superscript and Subscript exclude each other: the one just ticked wins.
            let was_sup = *sup;
            r.cell(|ui| ui.checkbox(sup, tl!("Superscript")));
            r.cell(|ui| ui.checkbox(sub, tl!("Subscript")));
            if *sup && *sub {
                if was_sup {
                    *sup = false;
                } else {
                    *sub = false;
                }
            }
        });
        t.row(|r| {
            r.cell(|ui| ui.checkbox(small_caps, tl!("Small caps")));
            r.cell(|ui| ui.checkbox(caps, tl!("All caps")));
        });
        t.row(|r| {
            r.cell(|ui| ui.checkbox(hidden, tl!("Hidden")));
        });
    });
    // Live preview of the chosen font/size/colour/effects.
    ui.separator();
    ui.label(egui::RichText::new(tl!("Preview")).small().weak());
    font_sample_box(app, ui, sample_paper(app), &sample_props, (*sup, *sub), *hidden);
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        apply_font(app, d);
    }
    ok || cancel
}

fn paragraph_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Paragraph {
        rtl,
        align,
        left,
        right,
        first,
        before,
        after,
        line_rule,
        line_at,
        keep_next,
        keep_lines,
        page_break,
        widow,
        outline,
        outline_fixed,
        tab,
        asian,
        opts,
        mixed,
        ..
    } = d
    else {
        return false;
    };
    let names = [tl!("Indents and Spacing").to_string(), tl!("Line and Page Breaks").to_string(), tl!("Asian Typography").to_string()];
    if let Some(k) = crate::widgets::dialog_tabs(ui, &names, *tab as usize) {
        *tab = k as u8;
    }
    ui.add_space(6.0);
    match *tab {
        0 => {
            ui.label(egui::RichText::new(tl!("General")).font(semibold(12.5)));
            ui.horizontal(|ui| {
                ui.label(tl!("Direction:"));
                // Neither while the selected paragraphs differ.
                let blank = mixed.contains("bidi");
                for (value, text) in [(true, tl!("Right-to-left")), (false, tl!("Left-to-right"))] {
                    if ui.radio(!blank && *rtl == value, text).clicked() {
                        *rtl = value;
                        mixed.remove("bidi");
                    }
                }
            });
            const ALIGNS: [(&str, &str); 4] = [("left", "Left"), ("center", "Center"), ("right", "Right"), ("justify", "Justify")];
            ui.horizontal(|ui| {
                let align_label = ui.label(tl!("Alignment:"));
                let shown =
                    if mixed.contains("align") { "" } else { ALIGNS.iter().find(|(v, _)| v == align).map_or(align.as_str(), |(_, l)| tl!(l)) };
                let combo = craft_ui::ComboBox::from_id_salt("para_align").selected_text(shown).show_ui(ui, |ui| {
                    for (v, l) in ALIGNS {
                        if ui.selectable_value(align, v.to_string(), tl!(l)).clicked() {
                            mixed.remove("align");
                        }
                    }
                });
                combo.response.labelled_by(align_label.id);
            });
            // Greyed for a built-in heading, whose level comes from its style.
            ui.add_enabled_ui(!*outline_fixed, |ui| {
                ui.horizontal(|ui| {
                    let outline_label = ui.label(tl!("Outline level:"));
                    let blank = mixed.contains("outlineLevel");
                    let shown = if blank { "" } else { OUTLINE_LEVELS.get(outline_entry(*outline)).map_or("", |l| tl!(l)) };
                    let combo = craft_ui::ComboBox::from_id_salt("para_outline").selected_text(shown).show_ui(ui, |ui| {
                        for (level, l) in std::iter::once(9u8).chain(0..9).zip(OUTLINE_LEVELS) {
                            if ui.selectable_label(!blank && *outline == level, tl!(l)).clicked() {
                                *outline = level;
                                mixed.remove("outlineLevel");
                            }
                        }
                    });
                    combo.response.labelled_by(outline_label.id);
                });
            });
            ui.label(egui::RichText::new(tl!("Indentation")).font(semibold(12.5)));
            // Mirrored indents are the inside and outside ones (they swap on even pages).
            let (l_label, r_label) = if opts[0] { ("Inside:", "Outside:") } else { ("Left:", "Right:") };
            crate::rtl::Table::new("ind").columns(4).show(ui, |t| {
                t.row(|r| {
                    let left_label = r.cell(|ui| ui.label(tl!(l_label)));
                    r.cell(|ui| para_number(ui, left, ParaNumber::Length, "left", mixed, &left_label));
                    let right_label = r.cell(|ui| ui.label(tl!(r_label)));
                    r.cell(|ui| para_number(ui, right, ParaNumber::Length, "right", mixed, &right_label));
                });
                t.row(|r| {
                    let first_label = r.cell(|ui| ui.label(tl!("First line:")));
                    r.cell(|ui| para_number(ui, first, ParaNumber::Length, "indentFirst", mixed, &first_label));
                });
            });
            para_check(ui, &mut opts[0], tl!("Mirror indents"), "mirrorIndents", mixed);
            para_check(ui, &mut opts[4], tl!("Automatically adjust right indent when document grid is defined"), "adjustRightIndent", mixed);
            ui.label(egui::RichText::new(tl!("Spacing")).font(semibold(12.5)));
            crate::rtl::Table::new("sp").columns(4).show(ui, |t| {
                t.row(|r| {
                    let before_label = r.cell(|ui| ui.label(tl!("Before:")));
                    r.cell(|ui| para_number(ui, before, ParaNumber::Points, "spaceBefore", mixed, &before_label));
                    let line_label = r.cell(|ui| ui.label(tl!("Line spacing:")));
                    r.cell(|ui| ui.horizontal(|ui| line_spacing_boxes(ui, line_rule, line_at, mixed, &line_label)));
                });
                t.row(|r| {
                    let after_label = r.cell(|ui| ui.label(tl!("After:")));
                    r.cell(|ui| para_number(ui, after, ParaNumber::Points, "spaceAfter", mixed, &after_label));
                });
            });
            para_check(ui, &mut opts[1], tl!("Don't add space between paragraphs of the same style"), "contextualSpacing", mixed);
            para_check(ui, &mut opts[5], tl!("Snap to grid when document grid is defined"), "snapToGrid", mixed);
        }
        1 => {
            ui.label(egui::RichText::new(tl!("Line and Page Breaks")).font(semibold(12.5)));
            para_check(ui, widow, tl!("Widow/Orphan control"), "widowControl", mixed);
            para_check(ui, keep_next, tl!("Keep with next"), "keepNext", mixed);
            para_check(ui, keep_lines, tl!("Keep lines together"), "keepLines", mixed);
            para_check(ui, page_break, tl!("Page break before"), "pageBreakBefore", mixed);
            ui.label(egui::RichText::new(tl!("Formatting exceptions")).font(semibold(12.5)));
            para_check(ui, &mut opts[2], tl!("Suppress line numbers"), "suppressLineNumbers", mixed);
            para_check(ui, &mut opts[3], tl!("Don't hyphenate"), "suppressHyphens", mixed);
        }
        _ => {
            let [kinsoku, word_wrap, overflow, top_line, de, dn] = asian;
            ui.label(egui::RichText::new(tl!("Line breaking")).font(semibold(12.5)));
            para_check(ui, kinsoku, tl!("Apply Asian line-breaking rules (kinsoku)"), "kinsoku", mixed);
            // The model's flag is "wrap whole words"; the box offers the opposite.
            let mut mid_word = !*word_wrap;
            para_check(ui, &mut mid_word, tl!("Allow Latin text to wrap in the middle of a word"), "wordWrap", mixed);
            *word_wrap = !mid_word;
            para_check(ui, overflow, tl!("Allow hanging punctuation"), "overflowPunct", mixed);
            ui.label(egui::RichText::new(tl!("Character Spacing")).font(semibold(12.5)));
            para_check(ui, top_line, tl!("Compress punctuation at the start of a line"), "topLinePunct", mixed);
            para_check(ui, de, tl!("Add space between Asian and Latin text"), "autoSpaceDE", mixed);
            para_check(ui, dn, tl!("Add space between Asian text and numbers"), "autoSpaceDN", mixed);
        }
    }
    ui.add_space(6.0);
    let tabs = ui.button(tl!("Tabs…")).clicked();
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok || tabs {
        apply_paragraph(app, d);
    }
    // Tabs… keeps what was set here and moves on to the Tabs dialog.
    if tabs {
        app.dialog = Dialog::open("tabs", app);
    }
    ok || cancel || tabs
}

fn find_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Find { query, replace, match_case, whole_word, regex, extra, tab, goto_what, goto_target, message, focus } = d else {
        return false;
    };
    const FIND_TABS: [(FindTab, &str); 3] = [(FindTab::Find, "Find"), (FindTab::Replace, "Replace"), (FindTab::GoTo, "Go To")];
    let names = FIND_TABS.map(|(_, l)| tl!(l).to_string());
    let current = FIND_TABS.iter().position(|(t, _)| t == tab).unwrap_or(0);
    if let Some((t, _)) = crate::widgets::dialog_tabs(ui, &names, current).and_then(|k| FIND_TABS.get(k)) {
        *tab = *t;
        message.clear();
    }
    ui.add_space(6.0);
    if *tab == FindTab::GoTo {
        return goto_body(app, ui, goto_what, goto_target, message);
    }
    let replace_mode = *tab == FindTab::Replace;
    ui.horizontal(|ui| {
        let find_label = ui.label(tl!("Find what:"));
        let r = ui.add(egui::TextEdit::singleline(query).id(egui::Id::unique("find_what")).desired_width(240.0)).labelled_by(find_label.id);
        if std::mem::take(focus) {
            r.request_focus();
        }
        if r.gained_focus() {
            extra.to_replace = false;
        }
    });
    if let Some(f) = &extra.format {
        ui.label(egui::RichText::new(super::find::format_line(app, f)).weak());
    }
    if replace_mode {
        ui.horizontal(|ui| {
            let replace_label = ui.label(tl!("Replace with:"));
            if ui.add(egui::TextEdit::singleline(replace).desired_width(228.0)).labelled_by(replace_label.id).gained_focus() {
                extra.to_replace = true;
            }
        });
        if let Some(f) = &extra.replace_format {
            ui.label(egui::RichText::new(super::find::format_line(app, f)).weak());
        }
    } else {
        extra.to_replace = false;
    }
    // Word greys out Match case and the word options with wildcards (they always match case).
    let plain = !extra.wildcards;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(plain, |ui| ui.checkbox(match_case, tl!("Match case")));
        ui.add_enabled_ui(plain, |ui| ui.checkbox(whole_word, tl!("Whole words")));
        ui.checkbox(&mut extra.wildcards, tl!("Use wildcards"));
    });
    let arrow = if extra.more { "▾" } else { "▸" };
    if ui.selectable_label(extra.more, format!("{arrow} {}", tl!("Search Options"))).clicked() {
        extra.more = !extra.more;
    }
    if extra.more {
        ui.horizontal(|ui| {
            ui.add_enabled_ui(plain, |ui| ui.checkbox(&mut extra.prefix, tl!("Match prefix")));
            ui.add_enabled_ui(plain, |ui| ui.checkbox(&mut extra.suffix, tl!("Match suffix")));
        });
        ui.horizontal(|ui| {
            ui.add_enabled_ui(plain, |ui| ui.checkbox(&mut extra.ignore_punct, tl!("Ignore punctuation characters")));
            ui.add_enabled_ui(plain, |ui| ui.checkbox(&mut extra.ignore_space, tl!("Ignore white-space characters")));
        });
        ui.horizontal(|ui| {
            ui.checkbox(regex, tl!("Regular expressions"));
            if let Some(code) = special_menu(ui, extra) {
                if extra.to_replace { replace.push_str(code) } else { query.push_str(code) }
            }
        });
        ui.horizontal(|ui| super::find::format_menu(app, ui, extra));
    }
    let opts = json!({
        "text": query,
        "with": replace,
        "matchCase": *match_case,
        "wholeWord": *whole_word,
        "regex": *regex,
        "wildcards": extra.wildcards,
        "matchPrefix": extra.prefix,
        "matchSuffix": extra.suffix,
        "ignorePunct": extra.ignore_punct,
        "ignoreSpace": extra.ignore_space,
        "format": extra.format,
        "replaceFormat": if replace_mode { json!(extra.replace_format) } else { json!(app.session.find.replace_format) },
    });
    if !replace_mode {
        ui.horizontal(|ui| {
            let lit = app.session.find.highlight;
            if ui.selectable_label(lit, tl!("Reading Highlight")).clicked() {
                let mut v = opts.clone();
                v["highlight"] = json!(!lit);
                match app.run("edit.advancedFind", v) {
                    Ok(r) if !lit => *message = crate::i18n::fmt(tl!("{count} items highlighted"), &[("count", &r["count"].to_string())]),
                    Ok(_) => message.clear(),
                    Err(e) => *message = e,
                }
            }
        });
    }
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).weak());
    }
    let mut close = false;
    ui.horizontal(|ui| {
        if replace_mode {
            if ui.button(tl!("Replace All")).clicked() {
                match app.run("edit.replaceAll", opts.clone()) {
                    Ok(r) => *message = format!("All done. We made {} replacements.", r["replaced"]),
                    Err(e) => *message = e,
                }
            }
            if ui.button(tl!("Replace")).clicked() {
                let _ = app.run("edit.replace", opts.clone());
            }
        }
        if ui.button(tl!("Find Next")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            // One step per click: `edit.find` would select a match itself, and the step after it skip one.
            match app.run("edit.findNext", opts.clone()) {
                Ok(r) => *message = format!("Match {} of {}", r["index"].as_u64().unwrap_or(0) + 1, r["count"]),
                Err(e) => *message = e,
            }
        }
        if ui.button(tl!("Close")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
    });
    close
}

fn enclose_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Enclose { style, text, shape } = d else {
        return false;
    };
    ui.label(egui::RichText::new(tl!("Style")).font(semibold(12.5)));
    ui.horizontal(|ui| {
        for (v, l) in [("none", "None"), ("shrink", "Shrink text"), ("enlarge", "Enlarge symbol")] {
            ui.radio_value(style, v.to_string(), tl!(l));
        }
    });
    ui.add_space(6.0);
    crate::rtl::horizontal_top(ui, |ui| {
        crate::rtl::vertical(ui, |ui| {
            ui.label(tl!("Text:"));
            ui.add(egui::TextEdit::singleline(text).desired_width(110.0).char_limit(2));
            ui.add_space(6.0);
            enclose_preview(ui, style, text, shape);
        });
        ui.add_space(12.0);
        crate::rtl::vertical(ui, |ui| {
            ui.label(tl!("Enclosure"));
            for (v, l) in [("circle", "Circle"), ("square", "Square"), ("triangle", "Triangle"), ("diamond", "Diamond")] {
                ui.add_enabled_ui(style != "none", |ui| ui.selectable_value(shape, v.to_string(), tl!(l)));
            }
        });
    });
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let params = if style == "none" { json!({"shape": null}) } else { json!({"shape": shape, "style": style, "text": text}) };
        let _ = app.run("format.enclose", params);
    }
    ok || cancel
}

fn table_properties_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::TableProperties { form, basis } = d else {
        return false;
    };
    table_properties(ui, form);
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let commands = form.commands(basis);
        if !commands.is_empty() {
            // Table settings and alt text: one undo step.
            let _ = super::page_setup::run_as_one_step(app, commands);
        }
    }
    ok || cancel
}

fn cell_margins_arm(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::CellMargins { form, basis } = d else {
        return false;
    };
    cell_margins_dialog(ui, form);
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let changes = form.changes(basis);
        if changes.as_object().is_some_and(|m| !m.is_empty()) {
            let _ = app.run("table.cellMargins", changes);
        }
    }
    ok || cancel
}

fn insert_table_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::InsertTable { rows, cols, autofit, col_width, remember, error } = d else {
        return false;
    };
    ui.label(egui::RichText::new(tl!("Table size")).font(semibold(12.5)));
    crate::rtl::Table::new("it").columns(2).show(ui, |t| {
        t.row(|r| {
            let label = r.cell(|ui| ui.label(tl!("Number of columns:")));
            r.cell(|ui| ui.add(egui::DragValue::new(cols).range(1..=63)).labelled_by(label.id));
        });
        t.row(|r| {
            let label = r.cell(|ui| ui.label(tl!("Number of rows:")));
            r.cell(|ui| ui.add(egui::DragValue::new(rows).range(1..=1000)).labelled_by(label.id));
        });
    });
    ui.label(egui::RichText::new(tl!("AutoFit behavior")).font(semibold(12.5)));
    ui.horizontal(|ui| {
        let radio = ui.radio_value(autofit, "fixed".to_string(), tl!("Fixed column width:"));
        ui.add_enabled(autofit == "fixed", egui::TextEdit::singleline(col_width).desired_width(72.0)).labelled_by(radio.id);
    });
    ui.radio_value(autofit, "contents".to_string(), tl!("AutoFit to contents"));
    ui.radio_value(autofit, "window".to_string(), tl!("AutoFit to window"));
    ui.checkbox(remember, tl!("Remember dimensions for new tables"));
    if let Some(e) = error.as_deref() {
        ui.colored_label(ui.visuals().error_fg_color, e);
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let mut v = json!({"rows": *rows, "cols": *cols, "autofit": autofit.as_str(), "remember": *remember});
        if autofit == "fixed" {
            match insert_table_width(col_width) {
                Some(w) => v["colWidth"] = json!(w),
                None => {
                    // Out of range: Word's message, and the dialog stays open.
                    let sep = crate::options::decimal();
                    let unit = crate::options::unit();
                    let (min, max) = (unit.format_with(0.0, sep), unit.format_with(22.0 * 72.0, sep));
                    *error = Some(crate::i18n::fmt(tl!("The measurement must be between {min} and {max}."), &[("min", &min), ("max", &max)]));
                    return false;
                }
            }
        }
        let _ = app.run("insert.table", v);
    }
    ok || cancel
}

/// Insert Table's Fixed column width in points: 0 for Auto, `None` when it isn't a measurement
/// from 0 to 22 in.
fn insert_table_width(text: &str) -> Option<f32> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("auto") || text == tl!("Auto") {
        return Some(0.0);
    }
    let unit = crate::options::unit();
    crate::options::parse_length(text, unit).map(|n| n as f32 * unit.pt_per_unit()).filter(|pt| (0.0..=22.0 * 72.0).contains(pt))
}

/// A margin's number box, in the unit the interface measures in.
fn margin(value: &mut f32) -> impl egui::Widget + '_ {
    move |ui: &mut Ui| {
        // DragValue's unfocused button extends to fit its value; keep it inside the field too.
        let (_, rect) = ui.allocate_space(egui::vec2(80.0, ui.spacing().interact_size.y));
        let mut field = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(crate::rtl::row_layout(ui, egui::Align::Center)));
        field.shrink_clip_rect(rect);
        field.add(
            egui::DragValue::new(value)
                .speed(3.6 / crate::options::unit().pt_per_unit())
                .measure(crate::options::unit())
                .max_decimals(2)
                .clip_text(true),
        )
    }
}

fn page_setup_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::PageSetup {
        top,
        bottom,
        left,
        right,
        landscape,
        mirror,
        rtl,
        grid,
        grid_was,
        paper,
        paper_was,
        paper_custom,
        message,
        valign,
        paper_src,
        paper_src_was,
        odd_even,
        first_page,
        gutter,
        header,
        footer,
        start,
    } = d
    else {
        return false;
    };
    ui.label(egui::RichText::new(tl!("Margins")).font(semibold(12.5)));
    crate::rtl::Table::new("ps").columns(4).show(ui, |t| {
        t.row(|r| {
            let top_label = r.cell(|ui| ui.label(tl!("Top:")));
            r.cell(|ui| ui.add(margin(top)).labelled_by(top_label.id));
            let bottom_label = r.cell(|ui| ui.label(tl!("Bottom:")));
            r.cell(|ui| ui.add(margin(bottom)).labelled_by(bottom_label.id));
        });
        t.row(|r| {
            let left_label = r.cell(|ui| ui.label(if *mirror { tl!("Inside:") } else { tl!("Left:") }));
            r.cell(|ui| ui.add(margin(left)).labelled_by(left_label.id));
            let right_label = r.cell(|ui| ui.label(if *mirror { tl!("Outside:") } else { tl!("Right:") }));
            r.cell(|ui| ui.add(margin(right)).labelled_by(right_label.id));
        });
        t.row(|r| {
            let gutter_label = r.cell(|ui| ui.label(tl!("Gutter:")));
            r.cell(|ui| ui.add(margin(gutter)).labelled_by(gutter_label.id));
        });
    });
    ui.label(egui::RichText::new(tl!("Orientation")).font(semibold(12.5)));
    ui.horizontal(|ui| {
        ui.radio_value(landscape, false, tl!("Portrait"));
        ui.radio_value(landscape, true, tl!("Landscape"));
    });
    ui.label(egui::RichText::new(tl!("Pages")).font(semibold(12.5)));
    ui.checkbox(mirror, tl!("Mirror margins"));
    page_paper_group(ui, paper, landscape, paper_custom);
    page_paper_source_group(ui, &mut paper_src.0, &mut paper_src.1);
    ui.label(egui::RichText::new(tl!("Document Grid")).font(semibold(12.5)));
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut grid.0, 0, tl!("No grid"));
        ui.radio_value(&mut grid.0, 1, tl!("Lines only"));
        ui.radio_value(&mut grid.0, 2, tl!("Lines and characters"));
        ui.radio_value(&mut grid.0, 3, tl!("Text snaps to characters"));
    });
    crate::rtl::Table::new("ps_grid").columns(4).show(ui, |t| {
        t.row(|r| {
            let chars_label = r.cell(|ui| ui.add_enabled(grid.0 >= 2, egui::Label::new(tl!("Characters per line:"))));
            r.cell(|ui| ui.add_enabled(grid.0 >= 2, egui::DragValue::new(&mut grid.2).range(1..=1000).clip_text(true)).labelled_by(chars_label.id));
            let lines_label = r.cell(|ui| ui.add_enabled(grid.0 >= 1, egui::Label::new(tl!("Lines per page:"))));
            r.cell(|ui| ui.add_enabled(grid.0 >= 1, egui::DragValue::new(&mut grid.1).range(1..=1000).clip_text(true)).labelled_by(lines_label.id));
        });
    });
    ui.horizontal(|ui| {
        ui.label(tl!("Section direction:"));
        ui.radio_value(rtl, false, tl!("Left-to-right"));
        ui.radio_value(rtl, true, tl!("Right-to-left"));
    });
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).small().color(ui.visuals().error_fg_color));
    }
    page_valign_combo(ui, valign);
    page_section_start_combo(ui, start);
    // Word's Page Setup › Layout: a title page without the page number (#1230).
    ui.label(egui::RichText::new(tl!("Headers and footers")).font(semibold(12.5)));
    ui.checkbox(odd_even, tl!("Different Odd & Even Pages"));
    ui.checkbox(first_page, tl!("Different First Page"));
    crate::rtl::Table::new("ps_hf").columns(4).show(ui, |t| {
        t.row(|r| {
            let header_label = r.cell(|ui| ui.label(tl!("Header:")));
            r.cell(|ui| ui.add(margin(header)).labelled_by(header_label.id));
            let footer_label = r.cell(|ui| ui.label(tl!("Footer:")));
            r.cell(|ui| ui.add(margin(footer)).labelled_by(footer_label.id));
        });
    });
    let (mut ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        // Everything OK changes is one undo step; a refused command applies nothing.
        let result = page_layout_commands(app, (*paper, *paper_was), *landscape, (*top, *bottom, *left, *right), (*gutter, *header, *footer))
            .map_err(|e| tl!(e).to_string())
            .and_then(|mut commands| {
                // Mirror margins go with the margins (#1181).
                if let Some((_, v)) = commands.iter_mut().rev().find(|(id, _)| *id == "layout.margins") {
                    v["mirror"] = json!(*mirror);
                }
                if *start != wordcraft_engine::cmd::page::sect(&app.session).start {
                    commands.push(("layout.sectionStart", json!({"value": page_section_start_value(*start)})));
                }
                if grid != grid_was {
                    let kind = ["none", "lines", "linesAndChars", "snapToChars"].get(grid.0 as usize).copied().unwrap_or("none");
                    let mut v = json!({"type": kind});
                    if grid.1 != grid_was.1 {
                        v["linesPerPage"] = json!(grid.1);
                    }
                    if grid.2 != grid_was.2 {
                        v["charsPerLine"] = json!(grid.2);
                    }
                    commands.push(("layout.documentGrid", v));
                }
                if *rtl != wordcraft_engine::cmd::page::sect(&app.session).rtl {
                    commands.push(("layout.sectionRtl", json!({"value": *rtl})));
                }
                if *valign != wordcraft_engine::cmd::page::sect(&app.session).valign {
                    commands.push(("layout.verticalAlign", json!({"value": page_valign_value(*valign)})));
                }
                if paper_src != paper_src_was {
                    let (first, other) = *paper_src;
                    commands.push(("layout.paperSource", json!({"first": first, "other": other})));
                }
                if *odd_even != app.session.doc.settings.even_odd_headers {
                    commands.push(("layout.differentOddEven", json!({"value": *odd_even})));
                }
                if *first_page != wordcraft_engine::cmd::page::sect(&app.session).title_page {
                    commands.push(("layout.differentFirstPage", json!({"value": *first_page})));
                }
                run_as_one_step(app, commands)
            });
        if let Err(e) = result {
            *message = e;
            ok = false;
        }
    }
    ok || cancel
}

fn link_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Link { url, text, kind, email, subject, targets, pick, focus, tip, tip_edit } = d else {
        return false;
    };
    // Enter and Escape belong to Set Hyperlink ScreenTip while it is open above this dialog.
    let tip_open = tip_edit.is_some();
    // Read document targets once, when a choice first needs them.
    ui.horizontal(|ui| {
        ui.label(tl!("Link to:"));
        ui.selectable_value(kind, super::LinkKind::Web, tl!("Existing File or Web Page"));
        ui.selectable_value(kind, super::LinkKind::Place, tl!("Place in This Document"));
        ui.selectable_value(kind, super::LinkKind::Email, tl!("E-mail Address"));
        ui.add_space(24.0);
        if ui.button(tl!("ScreenTip…")).clicked() {
            *tip_edit = Some(tip.clone());
        }
    });
    if *kind != super::LinkKind::Web && targets.is_none() {
        *targets = app.run("link.targets", json!({})).ok();
    }
    ui.add_space(4.0);
    crate::rtl::Table::new("lnk").columns(2).show(ui, |t| {
        t.row(|r| {
            let label = r.cell(|ui| ui.label(tl!("Text to display:")));
            r.cell(|ui| ui.text_edit_singleline(text).labelled_by(label.id));
        });
        if *kind == super::LinkKind::Web {
            t.row(|r| {
                let label = r.cell(|ui| ui.label(tl!("Address:")));
                r.cell(|ui| {
                    let mut out = egui::TextEdit::singleline(url).id(egui::Id::unique("link_address")).show(ui);
                    if std::mem::take(focus) {
                        out.response.request_focus();
                        let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(url.chars().count()));
                        out.state.cursor.set_char_range(Some(all));
                        out.state.store(ui.ctx(), out.response.id);
                    }
                    out.response.response.labelled_by(label.id);
                });
            });
        } else if *kind == super::LinkKind::Email {
            t.row(|r| {
                let label = r.cell(|ui| ui.label(tl!("E-mail address:")));
                r.cell(|ui| ui.text_edit_singleline(email).labelled_by(label.id));
            });
            t.row(|r| {
                let label = r.cell(|ui| ui.label(tl!("Subject:")));
                r.cell(|ui| ui.text_edit_singleline(subject).labelled_by(label.id));
            });
        }
    });
    if *kind == super::LinkKind::Email {
        ui.label(tl!("Recently used e-mail addresses:"));
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(220.0).min_scrolled_height(160.0).auto_shrink([false, true]).show(ui, |ui| {
                if let Some(emails) = targets.as_ref().and_then(|t| t.get("emails")).and_then(Value::as_array) {
                    for entry in emails {
                        let address = entry.get("address").and_then(Value::as_str).unwrap_or_default();
                        let saved_subject = entry.get("subject").and_then(Value::as_str).unwrap_or_default();
                        if ui.selectable_label(email.as_str() == address && subject.as_str() == saved_subject, address).clicked() {
                            *email = address.to_string();
                            *subject = saved_subject.to_string();
                        }
                    }
                }
            });
        });
    }
    if *kind == super::LinkKind::Place {
        ui.add_space(4.0);
        ui.label(tl!("Select a place in this document:"));
        let mut chosen: Option<(Value, String)> = None;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(220.0).min_scrolled_height(160.0).auto_shrink([false, true]).show(ui, |ui| {
                let is = |pick: &Option<(Value, String)>, key: &str, val: &Value| pick.as_ref().is_some_and(|(v, _)| v.get(key) == Some(val));
                let top = tl!("Top of the Document").to_string();
                if ui.selectable_label(is(pick, "top", &json!(true)), &top).clicked() {
                    chosen = Some((json!({"top": true}), top));
                }
                let headings = targets.as_ref().and_then(|t| t.get("headings")).and_then(Value::as_array).cloned().unwrap_or_default();
                if !headings.is_empty() {
                    ui.label(egui::RichText::new(tl!("Headings")).font(semibold(12.5)));
                }
                for h in &headings {
                    let (block, level) = (h.get("block").cloned().unwrap_or_default(), h.get("level").and_then(Value::as_u64).unwrap_or(1));
                    let name = h.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
                    ui.horizontal(|ui| {
                        ui.add_space(14.0 * level.saturating_sub(1).min(8) as f32 + 8.0);
                        if ui.selectable_label(is(pick, "heading", &block), &name).clicked() {
                            chosen = Some((json!({"heading": block}), name));
                        }
                    });
                }
                let marks = targets.as_ref().and_then(|t| t.get("bookmarks")).and_then(Value::as_array).cloned().unwrap_or_default();
                if !marks.is_empty() {
                    ui.label(egui::RichText::new(tl!("Bookmarks")).font(semibold(12.5)));
                }
                for m in marks.iter().filter_map(Value::as_str) {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        if ui.selectable_label(is(pick, "bookmark", &json!(m)), m).clicked() {
                            chosen = Some((json!({"bookmark": m}), m.to_string()));
                        }
                    });
                }
            });
        });
        if let Some(c) = chosen {
            // The text shown follows the choice until it is typed over (as in Word).
            if text.is_empty() || pick.as_ref().is_some_and(|(_, old)| old == text) {
                *text = c.1.clone();
            }
            *pick = Some(c);
        }
    }
    let (ok, cancel) = if *kind == super::LinkKind::Email {
        super::dialogs_insert::buttons(ui, "OK", "Cancel", !email.trim().is_empty())
    } else {
        buttons(ui, tl!("OK"))
    };
    let (ok, cancel) = if tip_open { (false, false) } else { (ok, cancel) };
    screen_tip_modal(ui, tip, tip_edit);
    if ok {
        // Sent also when empty, so OK clears a tip the link had.
        let mut params = if *kind == super::LinkKind::Place {
            let Some((mut params, _)) = pick.clone() else { return false };
            if !text.is_empty() {
                params["text"] = json!(text);
            }
            params
        } else if *kind == super::LinkKind::Email {
            let mut params = json!({"email": email, "subject": subject});
            if !text.is_empty() {
                params["text"] = json!(text);
            }
            params
        } else {
            json!({"url": url, "text": text})
        };
        params["screenTip"] = json!(tip);
        let _ = app.run("insert.link", params);
    }
    ok || cancel
}

/// Insert Hyperlink › ScreenTip…: Set Hyperlink ScreenTip, open while `edit` holds the text being
/// edited. OK keeps it in `tip`; Cancel (or Escape) leaves `tip` as it was.
fn screen_tip_modal(ui: &mut Ui, tip: &mut String, edit: &mut Option<String>) {
    let Some(text) = edit.as_mut() else { return };
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("wordcraft.linkScreenTip")).show(ui.ctx(), |ui| {
        ui.set_max_width(380.0);
        ui.heading(tl!("Set Hyperlink ScreenTip"));
        let label = ui.label(tl!("ScreenTip text:"));
        let field = egui::TextEdit::singleline(text).char_limit(wordcraft_engine::cmd::insert::MAX_SCREEN_TIP).desired_width(f32::INFINITY);
        ui.add(field).labelled_by(label.id);
        ui.add_space(8.0);
        ui.with_layout(crate::rtl::far_side(ui, egui::Align::Center), |ui| {
            let (okb, cancelb) = crate::dialogs::ok_cancel_responses(ui, |ui| ui.add(crate::widgets::primary_button(ui, tl!("OK"))), tl!("Cancel"));
            if okb.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                answer = Some(true);
            }
            if cancelb.clicked() {
                answer = Some(false);
            }
        });
    });
    if modal.should_close() {
        answer = answer.or(Some(false));
    }
    match answer {
        Some(true) => *tip = edit.take().unwrap_or_default(),
        Some(false) => *edit = None,
        None => {}
    }
}

fn word_count_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::WordCount { stats } = d else {
        return false;
    };
    crate::rtl::Table::new("wc").columns(2).spacing(vec2(30.0, 6.0)).show(ui, |t| {
        for (l, k) in [
            ("Pages", "pages"),
            ("Words", "words"),
            ("Characters (no spaces)", "characters"),
            ("Characters (with spaces)", "charactersWithSpaces"),
            ("Paragraphs", "paragraphs"),
            ("Lines", "lines"),
        ] {
            t.row(|r| {
                r.cell(|ui| ui.label(tl!(l)));
                r.cell(|ui| ui.label(stats.get(k).map(|v| v.to_string()).unwrap_or_default()));
            });
        }
    });
    let mut include = stats.get("includeTextBoxes").and_then(Value::as_bool).unwrap_or(true);
    if ui.checkbox(&mut include, tl!("Count words in text boxes and notes")).changed()
        && let Ok(v) = app.run("review.wordCount", json!({"includeTextBoxes": include}))
    {
        *stats = v;
    }
    let (ok, cancel) = buttons(ui, tl!("Close"));
    ok || cancel
}

fn zoom_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Zoom { percent, fit } = d else {
        return false;
    };
    // Word's "Zoom to" group: one set of radios for the three percentages and the four fits.
    // Nothing applies until OK.
    ui.label(tl!("Zoom to"));
    let percents = [(200.0, tl!("200%")), (100.0, tl!("100%")), (75.0, tl!("75%"))];
    let fits =
        [("pageWidth", tl!("Page width")), ("textWidth", tl!("Text width")), ("onePage", tl!("Whole page")), ("multiplePages", tl!("Many pages"))];
    crate::rtl::Table::new("zoom_to").columns(2).spacing(vec2(16.0, 4.0)).show(ui, |t| {
        for row in 0..fits.len() {
            t.row(|r| {
                r.cell(|ui| {
                    if let Some((p, label)) = percents.get(row)
                        && ui.radio(fit.is_empty() && *percent == *p, *label).clicked()
                    {
                        *percent = *p;
                        fit.clear();
                    }
                });
                r.cell(|ui| {
                    if let Some((f, label)) = fits.get(row)
                        && ui.radio(fit.as_str() == *f, *label).clicked()
                    {
                        *fit = (*f).to_string();
                    }
                });
            });
        }
    });
    let percent_label = ui
        .horizontal(|ui| {
            let label = ui.label(tl!("Percent:"));
            let before = *percent;
            let r = ui.add(egui::DragValue::new(percent).range(10.0..=500.0).speed(1.0).suffix("%")).labelled_by(label.id);
            // Typing a percentage replaces a fit choice.
            if r.changed() && *percent != before {
                fit.clear();
            }
            label.id
        })
        .inner;
    // Logarithmic like the status bar slider, and wide enough to aim: the default 100 pt
    // linear slider jumped from 100% to 480% within a short drag (issue #67).
    ui.spacing_mut().slider_width = 220.0;
    let before = *percent;
    let slider = crate::rtl::island(ui, |ui| {
        ui.add(egui::Slider::new(percent, 10.0..=500.0).logarithmic(true).step_by(1.0).suffix("%").show_value(false)).labelled_by(percent_label)
    });
    if slider.changed() && *percent != before {
        fit.clear();
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let value = if fit.is_empty() { json!(*percent) } else { json!(fit.as_str()) };
        let _ = app.run("view.zoom", json!({"value": value}));
    }
    ok || cancel
}

fn watermark_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Watermark { text, diagonal } = d else {
        return false;
    };
    ui.horizontal(|ui| {
        ui.label(tl!("Text:"));
        ui.text_edit_singleline(text);
    });
    ui.horizontal(|ui| {
        ui.radio_value(diagonal, true, tl!("Diagonal"));
        ui.radio_value(diagonal, false, tl!("Horizontal"));
    });
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let _ = app.run("design.watermark", json!({"text": text, "diagonal": *diagonal}));
    }
    ok || cancel
}

fn phonetic_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Phonetic { base, ruby, align, size, offset, existing, focus } = d else {
        return false;
    };
    // Base and ruby text are usually Chinese or Japanese: fonts for them load from the next frame.
    app.want_system_cjk = true;
    const ALIGNS: [(&str, &str); 6] = [
        ("center", "Center"),
        ("distributeLetter", "Distribute letters"),
        ("distributeSpace", "Distribute spacing"),
        ("left", "Left"),
        ("right", "Right"),
        ("rightVertical", "Right (vertical)"),
    ];
    crate::rtl::Table::new("phonetic").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Base text:")));
            r.cell(|ui| ui.label(egui::RichText::new(base.as_str()).strong()));
        });
        t.row(|r| {
            let ruby_label = r.cell(|ui| ui.label(tl!("Ruby text:")));
            r.cell(|ui| {
                let field = ui.text_edit_singleline(ruby).labelled_by(ruby_label.id);
                // Typing goes to the Ruby text box, not the selected text in the document (#3914).
                if std::mem::take(focus) {
                    field.request_focus();
                }
            });
        });
        t.row(|r| {
            let align_label = r.cell(|ui| ui.label(tl!("Alignment:")));
            r.cell(|ui| {
                let shown = ALIGNS.iter().find(|(k, _)| k == align).map_or("Center", |(_, l)| l);
                let combo = craft_ui::ComboBox::from_id_salt("phonetic_align").selected_text(tl!(shown)).show_ui(ui, |ui| {
                    for (k, l) in ALIGNS {
                        ui.selectable_value(align, k.to_string(), tl!(l));
                    }
                });
                combo.response.labelled_by(align_label.id);
            });
        });
        t.row(|r| {
            let offset_label = r.cell(|ui| ui.label(tl!("Offset (pt):")));
            r.cell(|ui| ui.add(egui::DragValue::new(offset).speed(0.5).range(-100.0..=100.0)).labelled_by(offset_label.id));
        });
        t.row(|r| {
            let size_label = r.cell(|ui| ui.label(tl!("Size (pt):")));
            r.cell(|ui| ui.add(egui::DragValue::new(size).speed(0.5).range(1.0..=200.0)).labelled_by(size_label.id));
        });
    });
    let mut remove = false;
    if *existing {
        ui.add_space(4.0);
        remove = ui.button(tl!("Remove")).clicked();
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if remove {
        let _ = app.run("format.phonetic", json!({"ruby": null}));
    } else if ok {
        let _ = app.run("format.phonetic", json!({"ruby": ruby, "align": align, "size": *size, "offset": *offset}));
    }
    ok || cancel || remove
}

fn new_style_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::NewStyle { name, based_on, back } = d else {
        return false;
    };
    crate::rtl::Table::new("ns").columns(2).show(ui, |t| {
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Name:")));
            r.cell(|ui| ui.text_edit_singleline(name));
        });
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Style based on:")));
            r.cell(|ui| ui.text_edit_singleline(based_on));
        });
    });
    ui.label(egui::RichText::new(tl!("The new style takes the formatting of the current paragraph.")).small().weak());
    super::styles::shortcut_menu(app, ui, None);
    let (ok, cancel) = buttons(ui, tl!("OK"));
    let mut created = None;
    if ok {
        created = app.run("styles.create", json!({"name": name, "basedOn": based_on})).ok().and_then(|v| v.get("id")?.as_str().map(str::to_string));
    }
    if (ok || cancel) && *back {
        app.dialog = Some(Dialog::ManageStyles { alphabetical: false, selected: created.unwrap_or_default() });
    }
    ok || cancel
}

fn modify_style_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::ModifyStyle { back, resume, id, name, font, size, bold, italic, color, before, after } = d else {
        return false;
    };
    crate::rtl::Table::new("ms").columns(2).show(ui, |t| {
        t.row(|r| {
            let name_label = r.cell(|ui| ui.label(tl!("Name:")));
            r.cell(|ui| ui.text_edit_singleline(name).labelled_by(name_label.id));
        });
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Font:")));
            r.cell(|ui| {
                let shown = app.previews.font_shown(font);
                let fams = app.previews.families();
                let prev = app.previews.font_preview_fn();
                if let Some(f) = crate::widgets::font_combo(ui, "ms_font", 220.0, &shown, &fams, Some(&*prev)) {
                    if f == crate::widgets::ADD_FONT {
                        let _ = app.run("font.add", json!({}));
                    } else {
                        *font = app.previews.font_stored(&f);
                    }
                }
            });
        });
        t.row(|r| {
            let size_label = r.cell(|ui| ui.label(tl!("Size:")));
            r.cell(|ui| ui.add(egui::DragValue::new(size).range(1.0..=1638.0).speed(0.5)).labelled_by(size_label.id));
        });
        t.row(|r| {
            let color_label = r.cell(|ui| ui.label(tl!("Color:")));
            r.cell(|ui| ui.text_edit_singleline(color).labelled_by(color_label.id));
        });
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Space before/after:")));
            r.cell(|ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(before).range(0.0..=1584.0)).accessible_name(tl!("Space before"));
                    ui.add(egui::DragValue::new(after).range(0.0..=1584.0)).accessible_name(tl!("Space after"));
                });
            });
        });
    });
    ui.horizontal(|ui| {
        ui.checkbox(bold, tl!("Bold"));
        ui.checkbox(italic, tl!("Italic"));
    });
    if super::styles::shortcut_menu(app, ui, Some(id)) {
        let saved = d.clone();
        if let Some(Dialog::CustomizeKeyboard { resume, .. }) = app.dialog.as_mut() {
            *resume = Some(Box::new(saved));
        }
        return true;
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        let mut chr = json!({"font": font, "size": *size, "bold": *bold, "italic": *italic});
        if let Some(c) = wordcraft_doc::Rgb::parse(color) {
            chr["color"] = json!({"Rgb": [c.0, c.1, c.2]});
        }
        let _ = app.run("styles.modify", json!({"style": id, "name": name, "chr": chr, "para": {"spaceBefore": *before, "spaceAfter": *after}}));
    }
    if (ok || cancel) && *back {
        app.dialog = Some(Dialog::ManageStyles { alphabetical: false, selected: id.clone() });
    }
    if (ok || cancel)
        && let Some(r) = resume.take()
    {
        app.dialog = Some(*r);
    }
    ok || cancel
}

fn table_style_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::TableStyle { id, name, based_on, region, regions, basis, pen } = d else {
        return false;
    };
    crate::dialogs_table_style::body(app, ui, crate::dialogs_table_style::Form { id, name, based_on, region, regions, basis, pen })
}

fn commands_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Commands { query, swallow } = d else {
        return false;
    };
    if *swallow {
        ui.ctx().input_mut(|i| i.events.retain(|e| !swallow_shortcut_text(swallow, e)));
    }
    let r = ui.add(egui::TextEdit::singleline(query).hint_text(tl!("Type a command, e.g. \"insert table\"")).desired_width(380.0));
    // Only when it lacks focus: `request_focus` always interrupts IME, and on X11 that
    // recreates the XIM context, so asking every frame left fcitx taking no keys (#2986).
    if !r.has_focus() {
        r.request_focus();
    }
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    search_results(app, ui, query, enter, 380.0) || ui.input(|i| i.key_pressed(egui::Key::Escape))
}

/// Search's results under its box (the Search Commands dialog, or the title bar's drop-down,
/// #5398): suggestions for an empty box, else the matching commands and "Find … in the
/// document". A click, or `enter` (the first command, else the find), does it: `true` when one
/// was done, so the search closes.
pub(crate) fn search_results(app: &mut WordApp, ui: &mut Ui, query: &str, enter: bool, width: f32) -> bool {
    let reg = app.session.registry.clone();
    // An empty box suggests a few common commands, as Word's Search does.
    let mut hits: Vec<&wordcraft_engine::CommandSpec> = if query.trim().is_empty() {
        ui.label(egui::RichText::new(tl!("Suggested")).font(semibold(12.5)));
        SEARCH_SUGGESTIONS.iter().filter_map(|id| reg.get(id)).collect()
    } else {
        reg.all().iter().filter(|c| !NOT_IN_SEARCH.contains(&c.id) && command_matches(c, query)).collect()
    };
    hits.truncate(14);
    let mut run: Option<&str> = enter.then(|| hits.first().map(|c| c.id)).flatten();
    for c in &hits {
        let sc = crate::widgets::shortcut_text(app, c.id);
        let label = format!("{}   —   {}", tl!(c.label), crate::i18n::location(c.location));
        if ui.add(egui::Button::new(label).shortcut_text(sc).min_size(vec2(width, 0.0))).clicked() {
            run = Some(c.id);
        }
    }
    // Search also finds the words in the document (the Navigation pane's results).
    let text = query.trim().to_string();
    let mut find = false;
    if !text.is_empty() {
        ui.separator();
        let label = crate::i18n::fmt(tl!("Find “{text}” in the document"), &[("text", &text)]);
        find = ui.add(egui::Button::new(label).min_size(vec2(width, 0.0))).clicked() || (enter && hits.is_empty());
    }
    if let Some(id) = run {
        if id == "view.paperColor" {
            // Command Search is an interactive UI entry point: open the selector instead of
            // sending the engine command an empty value. Programmatic callers still get the
            // command's normal parameter validation.
            let _ = app.run("file.options", json!({"pane": "general"}));
        } else {
            let _ = app.run(id, json!({}));
        }
    } else if find {
        find_in_navigation_pane(app, ui.ctx(), &text);
    }
    run.is_some() || find
}

fn paste_special_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::PasteSpecial { formats, choice, payload, note } = d else {
        return false;
    };
    ui.label(egui::RichText::new(tl!("Paste as:")).font(semibold(12.5)));
    if formats.is_empty() {
        // The engine's wording, sentence-cased; the format names in it aren't translated.
        let text = if note.is_empty() || note == "the clipboard is empty" {
            tl!("The clipboard is empty.").to_string()
        } else {
            let mut c = note.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>() + ".").unwrap_or_default()
        };
        ui.label(egui::RichText::new(text).weak());
    }
    for (i, f) in formats.iter().enumerate() {
        let label = wordcraft_engine::cmd::paste::FORMATS.iter().find(|(id, _)| id == f).map(|(_, l)| *l).unwrap_or(f.as_str());
        ui.radio_value(choice, i, tl!(label));
    }
    if let Some(f) = formats.get(*choice) {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tl!("Result")).font(semibold(12.5)));
        ui.label(egui::RichText::new(tl!(wordcraft_engine::cmd::paste::result_text(f))).weak());
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok && let Some(f) = formats.get(*choice) {
        let mut params = payload.clone();
        params["as"] = json!(f);
        let _ = app.run("edit.pasteSpecial", params);
    }
    ok || cancel
}

fn about_dialog(ui: &mut Ui, d: &mut Dialog, pirate_speak: bool) -> bool {
    let Dialog::About { tab } = d else {
        return false;
    };
    ui.set_width(820.0);
    let names = [tl!("About").to_string(), tl!("Contributors").to_string(), tl!("Models").to_string(), tl!("Language data").to_string()];
    if let Some(i) = crate::widgets::dialog_tabs(ui, &names, *tab as usize) {
        *tab = i as u8;
    }
    ui.add_space(6.0);
    match *tab {
        1 => crate::credits::contributors_ui(ui),
        2 => crate::credits::models_ui(ui),
        3 => crate::language_packs::about_tab(ui),
        _ => {
            use crate::brand;
            ui.label(egui::RichText::new(brand::APP_NAME).font(semibold(22.0)));
            let build = option_env!("WORDCRAFT_BUILD_SHA").map(|s| s.get(..8).unwrap_or(s)).unwrap_or(tl!("development build"));
            ui.label(crate::i18n::fmt(tl!("Version {version} ({build})"), &[("version", env!("CARGO_PKG_VERSION")), ("build", build)]));
            ui.label(crate::i18n::fmt(tl!("Part of {suite}."), &[("suite", brand::SUITE_NAME)]));
            ui.label(tl!(brand::TAGLINE));
            if let Some(flavour) = brand::about_flavour(pirate_speak) {
                ui.label(egui::RichText::new(flavour).italics());
            }
            ui.add_space(6.0);
            ui.label(tl!(brand::UPSTREAM_CREDIT));
            ui.hyperlink_to(tl!("WordCraft source code"), brand::UPSTREAM_SOURCE);
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!(brand::COPYRIGHT)).small().weak());
        }
    }
    // One Close button: About has nothing to cancel.
    super::chrome::close_button(ui)
}

fn discard_recovery_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::DiscardRecovery { name, .. } = d else {
        return false;
    };
    ui.label(crate::i18n::fmt(tl!("Discard the recovery copy of {name}?"), &[("name", name)]));
    ui.label(tl!("This cannot be undone. Your original file stays unchanged."));
    let answer = ui
        .horizontal(|ui| {
            let cancel = ui.button(tl!("Cancel"));
            let discard = ui.button(tl!("Discard"));
            if cancel.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                Some("cancel")
            } else if discard.clicked() {
                Some("discard")
            } else {
                None
            }
        })
        .inner;
    if let Some(answer) = answer {
        app.dialog = Some(d.clone());
        let result = app.run("ui.recoveryDiscard", json!({"answer": answer}));
        if let Err(e) = &result {
            app.status(e.clone());
        }
        return result.is_ok();
    }
    false
}

fn save_changes_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::SaveChanges { name, message, .. } = d else {
        return false;
    };
    ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Do you want to save changes to {name}?"), &[("name", name)])).font(semibold(15.0)));
    ui.label(tl!("Your changes will be lost if you don't save them."));
    // Why the last Save failed (#1445): the prompt came back so the user can't miss it.
    if !message.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(message.as_str()).color(ui.visuals().error_fg_color));
    }
    ui.add_space(8.0);
    let buttons = ui.horizontal(|ui| {
        ui.with_layout(crate::rtl::far_side(ui, egui::Align::Center), |ui| {
            let cancel = ui.button(tl!("Cancel"));
            let dont_save = ui.button(tl!("Don't Save"));
            let save = ui.add(crate::widgets::primary_button(ui, tl!("Save")));
            [(cancel, "cancel"), (dont_save, "dontSave"), (save, "save")]
        })
        .inner
    });
    let buttons = buttons.inner;
    // The keyboard alone answers, as in Word (#861): Save has focus when the prompt opens,
    // so Enter saves, and the arrow keys (egui's focus navigation) or Tab move between the
    // buttons, shown as Save, Don't Save, Cancel. The focus is taken from whatever held it,
    // not only from nothing: the page keeps it while typing and claims Tab and the arrows
    // (#1597).
    if !buttons.iter().any(|(b, _)| b.has_focus())
        && let Some((save, _)) = buttons.get(2)
    {
        save.request_focus();
    }
    // Enter clicks the focused button; it means Save only when none of them has focus.
    let focused = buttons.iter().any(|(b, _)| b.has_focus());
    let answer = if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        Some("cancel")
    } else if let Some((_, a)) = buttons.iter().find(|(b, _)| b.clicked()) {
        Some(*a)
    } else if !focused && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        Some("save")
    } else {
        None
    };
    let Some(answer) = answer else { return false };
    // `ui.saveChanges` reads the prompt from `app.dialog`, which `show` has taken out.
    app.dialog = Some(d.clone());
    let _ = app.run("ui.saveChanges", json!({"answer": answer}));
    true
}

fn insert_merge_field_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::InsertMergeField { fields, field } = d else {
        return false;
    };
    if fields.is_empty() {
        ui.label(egui::RichText::new(tl!("Select recipients first to choose from their fields.")).small().weak());
    } else {
        ui.label(tl!("Fields:"));
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for f in fields.iter() {
                let resp = ui.selectable_label(field == f, f);
                if resp.clicked() {
                    field.clone_from(f);
                }
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label(tl!("Field:"));
        ui.text_edit_singleline(field);
    });
    let (ok, cancel) = buttons(ui, tl!("Insert"));
    if ok && !field.trim().is_empty() {
        let _ = app.run("mailings.insertField", json!({"field": field.trim()}));
        return true;
    }
    cancel
}

fn password_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::Password { name, then, params, password, key, message } = d else {
        return false;
    };
    let modify = key == "modifyPassword";
    if modify {
        ui.label(crate::i18n::fmt(tl!("{name} has a password to modify."), &[("name", name)]));
        ui.label(egui::RichText::new(tl!("Type it to edit the document, or open it read-only.")).small());
    } else {
        ui.label(crate::i18n::fmt(tl!("{name} is protected with a password."), &[("name", name)]));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(tl!("Password:"));
        let r = ui.add(egui::TextEdit::singleline(password.as_mut_string()).password(true).desired_width(220.0));
        if password.is_empty() && !r.has_focus() {
            r.request_focus();
        }
    });
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).small().color(ui.visuals().error_fg_color));
    }
    let mut read_only = false;
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if modify && ui.button(tl!("Read Only")).clicked() {
        read_only = true;
    }
    if read_only {
        let mut p = params.clone();
        if let Some(o) = p.as_object_mut() {
            o.insert("readOnly".into(), json!(true));
        }
        match app.execute(then, p) {
            Ok(_) => {
                app.ui.backstage = false;
                return true;
            }
            Err(e) => *message = e,
        }
    }
    if ok && !password.is_empty() {
        let mut p = params.clone();
        if let Some(o) = p.as_object_mut() {
            o.insert(key.clone(), json!(password.as_str()));
        }
        // `execute`, not `run`: Save Changes was asked when the open began. The command that
        // asked runs again, so a protected template still starts a new document (#3433).
        match app.execute(then, p) {
            Ok(_) => {
                app.ui.backstage = false;
                return true;
            }
            Err(e) if wordcraft_engine::io::wrong_password(&e) || wordcraft_engine::io::wrong_modify_password(&e) => {
                *message = tl!("That password isn't right. Check it (passwords are case-sensitive) and try again.").to_string();
                password.as_mut_string().clear();
            }
            Err(e) => *message = e,
        }
    }
    cancel
}

fn encrypt_password_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::EncryptPassword { password, confirm, message } = d else {
        return false;
    };
    ui.label(tl!("Anyone who opens the document will need this password."));
    ui.add_space(4.0);
    crate::rtl::Table::new("encrypt_password").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Password:")));
            r.cell(|ui| {
                let field = ui.add(egui::TextEdit::singleline(password.as_mut_string()).password(true).desired_width(220.0));
                if password.is_empty() && confirm.is_empty() && !field.has_focus() {
                    field.request_focus();
                }
            });
        });
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Confirm password:")));
            r.cell(|ui| ui.add(egui::TextEdit::singleline(confirm.as_mut_string()).password(true).desired_width(220.0)));
        });
    });
    ui.label(
        egui::RichText::new(crate::brand::named(tl!(
            "Keep the password somewhere safe: without it nobody, WordCraft included, can open the document. Leave both boxes empty to remove the password."
        )))
        .small()
        .weak(),
    );
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).small().color(ui.visuals().error_fg_color));
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        if password != confirm {
            *message = tl!("The passwords don't match.").to_string();
            confirm.as_mut_string().clear();
            return false;
        }
        let pw = if password.is_empty() { Value::Null } else { json!(password.as_str()) };
        match app.run("file.encrypt", json!({"password": pw})) {
            Ok(_) => {
                app.status(if password.is_empty() {
                    tl!("The password is removed. Save the document to keep the change.")
                } else {
                    tl!("The document will be saved with a password. Save it to protect it.")
                });
                return true;
            }
            Err(e) => *message = e,
        }
    }
    cancel
}

fn read_only_recommended_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::ReadOnlyRecommended { name, params } = d else {
        return false;
    };
    ui.label(crate::i18n::fmt(tl!("{name} was saved with the recommendation to open it read-only."), &[("name", name)]));
    ui.label(egui::RichText::new(tl!("Open it read-only? You can still edit it, but Save will ask for another name.")).small());
    ui.add_space(4.0);
    let mut open_read_only = false;
    let mut no = false;
    ui.horizontal(|ui| {
        ui.with_layout(crate::rtl::far_side(ui, egui::Align::Center), |ui| {
            if ui.add(crate::widgets::primary_button(ui, tl!("Yes"))).clicked() {
                open_read_only = true;
            }
            if ui.button(tl!("No")).clicked() {
                no = true;
            }
        });
    });
    if open_read_only {
        let mut p = params.clone();
        if let Some(o) = p.as_object_mut() {
            o.insert("readOnly".into(), json!(true));
        }
        match app.execute("file.open", p) {
            Ok(_) => {
                app.ui.backstage = false;
                return true;
            }
            Err(e) => app.status(e),
        }
    }
    no
}

fn general_options_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::GeneralOptions { password, confirm, recommended, has_password, message } = d else {
        return false;
    };
    ui.label(tl!("A password to modify lets the document be opened read-only by anyone who doesn't know it."));
    ui.add_space(4.0);
    crate::rtl::Table::new("general_options").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Password to modify:")));
            r.cell(|ui| {
                let field = ui.add(egui::TextEdit::singleline(password.as_mut_string()).password(true).desired_width(220.0));
                if password.is_empty() && confirm.is_empty() && !field.has_focus() {
                    field.request_focus();
                }
            });
        });
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Confirm password:")));
            r.cell(|ui| ui.add(egui::TextEdit::singleline(confirm.as_mut_string()).password(true).desired_width(220.0)));
        });
    });
    ui.checkbox(recommended, tl!("Read-only recommended"));
    ui.label(egui::RichText::new(tl!("Leave both password boxes empty to remove the password to modify.")).small().weak());
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).small().color(ui.visuals().error_fg_color));
    }
    // Remove Password to Modify: only offered while one is set.
    let mut remove = false;
    if *has_password && ui.button(tl!("Remove Password to Modify")).clicked() {
        remove = true;
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if remove {
        match app.run("file.generalOptions", json!({"modifyPassword": ""})) {
            Ok(_) => {
                *has_password = false;
                app.status(tl!("The password to modify is removed. Save the document to keep the change."));
                return true;
            }
            Err(e) => *message = e,
        }
    }
    if ok {
        if password != confirm {
            *message = tl!("The passwords don't match.").to_string();
            confirm.as_mut_string().clear();
            return false;
        }
        // An empty box keeps the password that is set; Remove takes it away.
        let mut p = json!({"readOnlyRecommended": *recommended});
        if !password.is_empty() {
            p["modifyPassword"] = json!(password.as_str());
        }
        match app.run("file.generalOptions", p) {
            Ok(_) => {
                app.status(tl!("Save the document to keep the change."));
                return true;
            }
            Err(e) => *message = e,
        }
    }
    cancel
}

fn lock_tracking_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::LockTracking { password, confirm, locked, message } = d else {
        return false;
    };
    if *locked {
        ui.label(tl!("Tracking is locked. Type the password to unlock it."));
    } else {
        ui.label(tl!("Lock Tracking keeps Track Changes on. The password is optional; without it anyone can unlock it."));
    }
    ui.add_space(4.0);
    crate::rtl::Table::new("lock_tracking").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Password:")));
            r.cell(|ui| {
                let field = ui.add(egui::TextEdit::singleline(password.as_mut_string()).password(true).desired_width(220.0));
                if password.is_empty() && confirm.is_empty() && !field.has_focus() {
                    field.request_focus();
                }
            });
        });
        if !*locked {
            t.row(|r| {
                r.cell(|ui| ui.label(tl!("Confirm password:")));
                r.cell(|ui| ui.add(egui::TextEdit::singleline(confirm.as_mut_string()).password(true).desired_width(220.0)));
            });
        }
    });
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).small().color(ui.visuals().error_fg_color));
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok {
        if !*locked && password != confirm {
            *message = tl!("The passwords don't match.").to_string();
            confirm.as_mut_string().clear();
            return false;
        }
        let p = if *locked { json!({"unlock": true, "password": password.as_str()}) } else { json!({"password": password.as_str()}) };
        match app.run("review.lockTracking", p) {
            Ok(_) => return true,
            Err(e) => *message = e,
        }
    }
    cancel
}

fn field_code_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::FieldCode { code, edit, message } = d else {
        return false;
    };
    ui.label(tl!("Field codes:"));
    let r = ui.add(egui::TextEdit::singleline(code).desired_width(320.0).font(egui::TextStyle::Monospace));
    if !r.has_focus() && code.is_empty() {
        r.request_focus();
    }
    ui.label(egui::RichText::new("PAGE · NUMPAGES · DATE \\@ \"MMMM d, yyyy\" · AUTHOR · TITLE · SEQ Figure · LISTNUM").small().weak());
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).small().weak());
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok && !code.trim().is_empty() {
        let r = if *edit { app.run("fields.setCode", json!({"code": code.trim()})) } else { app.run("insert.field", json!({"instr": code.trim()})) };
        match r {
            Ok(_) => return true,
            Err(e) => *message = e,
        }
    }
    cancel
}

fn select_table_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::SelectTable { sheets, selected, headers, message, params } = d else {
        return false;
    };
    ui.label(tl!("Choose the sheet that holds the recipients."));
    ui.add_space(4.0);
    // A double-click picks the sheet as OK does.
    let mut picked = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(300.0);
        egui::ScrollArea::vertical().id_salt("select_table").max_height(220.0).show(ui, |ui| {
            for (i, name) in sheets.iter().enumerate() {
                let r = ui.selectable_label(*selected == i, name.as_str());
                if r.clicked() || r.double_clicked() {
                    *selected = i;
                }
                picked |= r.double_clicked();
            }
        });
    });
    ui.checkbox(headers, tl!("First row of data contains column headers"));
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).color(Tokens::get(ui.ctx()).red));
    }
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok || picked {
        let mut p = params.clone();
        if let Some(o) = p.as_object_mut() {
            o.insert("sheet".into(), json!(*selected + 1));
            o.insert("headers".into(), json!(*headers));
        }
        match app.load_recipients(p) {
            Ok(_) => return true,
            Err(e) => *message = e,
        }
    }
    cancel
}

fn find_recipient_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::FindRecipient { text, message } = d else {
        return false;
    };
    ui.horizontal(|ui| {
        ui.label(tl!("Find:"));
        ui.text_edit_singleline(text);
    });
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).small().weak());
    }
    let (ok, cancel) = buttons(ui, tl!("Find"));
    if ok && !text.trim().is_empty() {
        match app.run("mailings.findRecipient", json!({"text": text.trim()})) {
            Ok(_) => return true,
            Err(e) => *message = e,
        }
    }
    cancel
}

fn merge_records_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::MergeRecords { print, choice, from, to, message } = d else {
        return false;
    };
    ui.radio_value(choice, 0, tl!("All"));
    ui.radio_value(choice, 1, tl!("Current record"));
    ui.horizontal(|ui| {
        let from_label = ui.radio_value(choice, 2, tl!("From:"));
        // Typing a number picks the range, as in Word.
        if ui.add(egui::TextEdit::singleline(from).desired_width(60.0)).labelled_by(from_label.id).changed() {
            *choice = 2;
        }
        let to_label = ui.label(tl!("To:"));
        if ui.add(egui::TextEdit::singleline(to).desired_width(60.0)).labelled_by(to_label.id).changed() {
            *choice = 2;
        }
    });
    if !message.is_empty() {
        ui.label(egui::RichText::new(message.as_str()).small().weak());
    }
    let (ok, cancel) = buttons(ui, tl!(if *print { "Print" } else { "OK" }));
    if ok {
        let params = match *choice {
            0 => Some(json!({"records": "all"})),
            1 => Some(json!({"records": "current"})),
            _ => match (from.trim().parse::<u64>(), to.trim().parse::<u64>()) {
                (Ok(f), Ok(t)) => Some(json!({"from": f, "to": t})),
                _ => None,
            },
        };
        let id = if *print { "mailings.printDocuments" } else { "mailings.finish" };
        // An empty range is reported here, before Save Changes asks about the document.
        let run = params.ok_or_else(|| tl!("Enter a number.").to_string()).and_then(|p| {
            wordcraft_engine::cmd::mailings::merge_range(&app.session, &p).map_err(|e| e.to_string())?;
            app.run(id, p)
        });
        match run {
            Ok(_) => return true,
            Err(e) => *message = e,
        }
    }
    cancel
}

fn merge_rule_dialog(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::MergeRule { rule, fields, field, value, then, els } = d else {
        return false;
    };
    let skip = rule == "SKIPIF";
    ui.label(tl!(if skip {
        "Leave a recipient out of the merge when a field has this value."
    } else {
        "Insert one text or another, depending on a field's value."
    }));
    ui.add_space(4.0);
    crate::rtl::Table::new("merge_rule").columns(2).spacing(vec2(10.0, 6.0)).show(ui, |t| {
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Field:")));
            r.cell(|ui| {
                if fields.is_empty() {
                    ui.text_edit_singleline(field);
                } else {
                    craft_ui::ComboBox::from_id_salt("merge_rule_field").selected_text(field.as_str()).width(200.0).show_ui(ui, |ui| {
                        for f in fields.iter() {
                            ui.selectable_value(field, f.clone(), f);
                        }
                    });
                }
            });
        });
        t.row(|r| {
            r.cell(|ui| ui.label(tl!("Equals:")));
            r.cell(|ui| ui.text_edit_singleline(value));
        });
        if !skip {
            t.row(|r| {
                r.cell(|ui| ui.label(tl!("Then insert:")));
                r.cell(|ui| ui.text_edit_singleline(then));
            });
            t.row(|r| {
                r.cell(|ui| ui.label(tl!("Otherwise insert:")));
                r.cell(|ui| ui.text_edit_singleline(els));
            });
        }
    });
    let (ok, cancel) = buttons(ui, tl!("OK"));
    let params = rule_params(d);
    if ok && let Some(params) = params {
        let _ = app.run("mailings.rules", params);
        return true;
    }
    cancel
}

fn match_fields_dialog(ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::MatchFields { fields, address } = d else {
        return false;
    };
    if fields.is_empty() {
        ui.label(egui::RichText::new(tl!("Select recipients first to match their fields.")).small().weak());
    } else {
        ui.label(tl!("The recipient-list field that fills each part of an address block and greeting line."));
        ui.add_space(4.0);
        crate::rtl::Table::new("match_fields").columns(2).spacing(vec2(24.0, 6.0)).striped(true).show(ui, |t| {
            for (part, key) in ADDRESS_PARTS {
                t.row(|r| {
                    r.cell(|ui| ui.label(tl!(part)));
                    r.cell(|ui| match address.get(key).and_then(Value::as_str) {
                        Some(f) => ui.label(egui::RichText::new(f).strong()),
                        None => ui.label(egui::RichText::new(tl!("(not matched)")).weak()),
                    });
                });
            }
        });
    }
    let (ok, cancel) = buttons(ui, tl!("Close"));
    ok || cancel
}

/// Commands Search leaves out: they need a parameter it can't give (which Language pack), and
/// File › Options › Language does them with the list in front of the person.
const NOT_IN_SEARCH: [&str; 2] = ["proof.installPack", "proof.removePack"];

fn check_errors_dialog(ui: &mut Ui, d: &mut Dialog) -> bool {
    let Dialog::CheckErrors { unknown, records } = d else {
        return false;
    };
    if unknown.is_empty() {
        ui.label(tl!("No errors found: every merge field is in the recipient list."));
    } else {
        ui.label(tl!("These merge fields aren't in the recipient list:"));
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for f in unknown.iter() {
                ui.label(egui::RichText::new(format!("• {f}")).color(Tokens::get(ui.ctx()).red));
            }
        });
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Recipients: {count}"), &[("count", &records.to_string())])).weak());
    let (ok, cancel) = buttons(ui, tl!("Close"));
    ok || cancel
}

/// File › Info › Convert: OK upgrades the document to the current layout mode.
fn convert_document_dialog(app: &mut WordApp, ui: &mut Ui) -> bool {
    ui.label(tl!("This document will be upgraded to the newest layout mode, so it can use every feature."));
    ui.label(tl!("Its layout may change slightly."));
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok && let Err(e) = app.run("file.convert", json!({})) {
        app.status(e);
    }
    ok || cancel
}

/// Mark as Final: OK marks the document as final and saves it, as Save does (asking where for a
/// document that has never been saved).
fn mark_as_final_dialog(app: &mut WordApp, ui: &mut Ui) -> bool {
    ui.label(tl!("This document will be marked as final and then saved."));
    let (ok, cancel) = buttons(ui, tl!("OK"));
    if ok && let Err(e) = app.run("file.markAsFinal", json!({"final": true})).and_then(|_| app.run("file.save", json!({}))) {
        app.status(e);
    }
    ok || cancel
}

/// Check Compatibility: what may not carry over, and the version the document lays out like.
fn compatibility_dialog(ui: &mut Ui, issues: &[String], version: &str) -> bool {
    if issues.is_empty() {
        ui.label(tl!("No compatibility issues were found."));
    } else {
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for issue in issues {
                ui.label(format!("• {}", tl!(issue)));
            }
        });
    }
    ui.add_space(4.0);
    ui.label(crate::i18n::fmt(tl!("This document is in {version} mode."), &[("version", version)]));
    ui.add_space(8.0);
    let mut close = ui.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter));
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        close |= ui.add(crate::widgets::primary_button(ui, tl!("OK"))).clicked();
    });
    close
}

/// Select Browse Object: one button per object (tooltip "Browse by Heading"…); picking one runs
/// `nav.browseBy` and closes. Tab moves between the buttons, Enter or Space picks, Escape closes.
fn browse_object_dialog(app: &mut WordApp, ui: &mut Ui, objects: &[String], current: &str, focus: &mut bool) -> bool {
    let mut picked = None;
    egui::Grid::new("browse_objects").num_columns(6).show(ui, |ui| {
        for (i, name) in objects.iter().enumerate() {
            let Some(o) = wordcraft_engine::cmd::browse::BrowseObject::from_name(name) else { continue };
            let r = ui.selectable_label(name == current, tl!(browse_object_text(name))).on_hover_text(tl!(o.label()));
            if *focus && name == current {
                r.request_focus();
            }
            if r.clicked() {
                picked = Some(name.clone());
            }
            if i % 6 == 5 {
                ui.end_row();
            }
        }
    });
    *focus = false;
    if let Some(name) = picked {
        if let Err(e) = app.run("nav.browseBy", json!({"object": name})) {
            app.status(e);
        }
        return true;
    }
    super::chrome::cancel_key_pressed(ui)
}

/// A browse object's button text.
fn browse_object_text(name: &str) -> &'static str {
    match name {
        "goto" => "Go To",
        "find" => "Find",
        "edit" => "Edits",
        "heading" => "Heading",
        "graphic" => "Graphic",
        "table" => "Table",
        "field" => "Field",
        "endnote" => "Endnote",
        "footnote" => "Footnote",
        "comment" => "Comment",
        "section" => "Section",
        _ => "Page",
    }
}

#[cfg(test)]
mod email_link_tests {
    use super::*;
    use egui_kittest::kittest::{NodeT, Queryable};

    #[test]
    fn insert_hyperlink_screen_tip_is_sent_on_ok() {
        let mut app = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
        app.run("ui.language", json!({"value": "en"})).unwrap();
        app.run("ui.dialog", json!({"name": "link"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                app.logic(&ui.ctx().clone());
                app.ui(ui);
            },
            app,
        );
        h.run_steps(3);
        // ScreenTip… opens Set Hyperlink ScreenTip; its Cancel leaves the tip as it was.
        h.get_by_label("ScreenTip…").click();
        h.run_steps(3);
        let _ = h.get_by_label("ScreenTip text:");
        let Some(Dialog::Link { tip_edit, .. }) = &mut h.state_mut().dialog else { panic!("no link dialog") };
        *tip_edit = Some("Not kept".into());
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        let Some(Dialog::Link { url, text, tip, tip_edit, .. }) = &mut h.state_mut().dialog else { panic!("Escape closed the link dialog") };
        assert_eq!((tip.as_str(), tip_edit.is_none()), ("", true));
        *url = "https://example.com".into();
        *text = "Example".into();
        *tip = "More about it".into();
        h.get_by_label("OK").click();
        h.run_steps(3);
        assert!(h.state().dialog.is_none());
        let doc = &h.state().session.doc;
        let para = doc.body.first().and_then(|b| b.as_para()).unwrap();
        assert_eq!(para.text, "Example");
        assert!(para.run_ranges().all(|(_, c)| c.link.as_deref() == Some("https://example.com") && c.link_tip.as_deref() == Some("More about it")));
        // Opened again on the link, the dialog shows its tip.
        h.state_mut().run("select.all", json!({})).unwrap();
        h.state_mut().run("ui.dialog", json!({"name": "link"})).unwrap();
        let Some(Dialog::Link { tip, .. }) = &h.state().dialog else { panic!("no link dialog") };
        assert_eq!(tip, "More about it");
    }

    #[test]
    fn email_choice_requires_an_address_and_reuses_subject() {
        let mut app = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
        app.run("ui.language", json!({"value": "en"})).unwrap();
        app.run("insert.link", json!({"email": "ann@example.com", "subject": "Café & plans", "text": "Ann"})).unwrap();
        app.run("ui.dialog", json!({"name": "link"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                app.logic(&ui.ctx().clone());
                app.ui(ui);
            },
            app,
        );
        h.run_steps(3);
        h.get_by_label("E-mail Address").click();
        h.run_steps(3);
        assert!(h.get_by_label("OK").accesskit_node().is_disabled());
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        assert!(h.state().dialog.is_some());
        h.get_by_label("ann@example.com").click();
        h.run_steps(3);
        let Some(Dialog::Link { email, subject, .. }) = &h.state().dialog else { panic!("no link dialog") };
        assert_eq!(email, "ann@example.com");
        assert_eq!(subject, "Café & plans");
        h.get_by_label("OK").click();
        h.run_steps(3);
        assert!(h.state().dialog.is_none());
        let links = h.state_mut().run("link.targets", json!({})).unwrap();
        assert_eq!(links["emails"], json!([{"address": "ann@example.com", "subject": "Café & plans"}]));
        assert!(
            h.state()
                .session
                .doc
                .body
                .iter()
                .filter_map(|b| b.as_para())
                .any(|p| p.text.contains("mailto:ann@example.com?subject=Caf%C3%A9%20%26%20plans"))
        );
    }
}
