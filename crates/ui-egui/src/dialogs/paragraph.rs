use super::{BTreeSet, Dialog, MeasureExt, Ui, Value, WordApp, json};

/// The Paragraph dialog as one paragraph's formatting fills it, on page `tab`.
pub(super) fn paragraph_dialog(rp: &wordcraft_doc::resolve::ResolvedPara, tab: u8) -> Dialog {
    let (line_rule, line_at) = line_box(rp.line_spacing);
    let (vl, vr) = rp.visual_indents();
    let unit = crate::options::unit().pt_per_unit();
    Dialog::Paragraph {
        rtl: rp.bidi,
        align: format!("{:?}", rp.align.visual(rp.bidi)).to_lowercase(),
        left: vl / unit,
        right: vr / unit,
        first: rp.indent_first / unit,
        before: rp.space_before,
        after: rp.space_after,
        line_rule,
        line_at,
        keep_next: rp.keep_next,
        keep_lines: rp.keep_lines,
        page_break: rp.page_break_before,
        widow: rp.widow_control,
        tab,
        outline: 9,
        outline_fixed: false,
        asian: [rp.kinsoku, rp.word_wrap, rp.overflow_punct, rp.top_line_punct, rp.auto_space_de, rp.auto_space_dn],
        opts: [rp.mirror_indents, rp.contextual_spacing, rp.suppress_line_numbers, rp.suppress_hyphens],
        start: serde_json::Map::new(),
        mixed: BTreeSet::new(),
    }
}

/// The Paragraph dialog's fields as `para.set` takes them: `align`, `left` and `right` as seen
/// on the page (its `onPage`), the rest as paragraph properties.
pub(super) fn para_fields(d: &Dialog) -> serde_json::Map<String, Value> {
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
        asian,
        opts,
        ..
    } = d
    else {
        return serde_json::Map::new();
    };
    let unit = crate::options::unit().pt_per_unit();
    let [kinsoku, word_wrap, overflow, top_line, de, dn] = asian;
    let [mirror, contextual, no_numbers, no_hyphens] = opts;
    let fields = json!({
        "bidi": rtl, "align": align, "left": left * unit, "right": right * unit, "indentFirst": first * unit,
        "spaceBefore": before, "spaceAfter": after, "lineSpacing": line_spacing(*line_rule, *line_at),
        "keepNext": keep_next, "keepLines": keep_lines, "pageBreakBefore": page_break, "widowControl": widow,
        "mirrorIndents": mirror, "contextualSpacing": contextual, "suppressLineNumbers": no_numbers, "suppressHyphens": no_hyphens,
        "kinsoku": kinsoku, "wordWrap": word_wrap, "overflowPunct": overflow, "topLinePunct": top_line, "autoSpaceDE": de, "autoSpaceDN": dn,
    });
    match fields {
        Value::Object(m) => m,
        _ => serde_json::Map::new(),
    }
}

/// The Line spacing box's entries, in Word's order: the index is the dialog's `line_rule`.
pub(super) const LINE_RULES: [&str; 6] = ["Single", "1.5 lines", "Double", "At least", "Exactly", "Multiple"];
pub(super) const LINE_AT_LEAST: u8 = 3;
pub(super) const LINE_EXACTLY: u8 = 4;
pub(super) const LINE_MULTIPLE: u8 = 5;

/// The Line spacing box's entry and the At box's value for a paragraph's line spacing: 1, 1.5
/// and 2 lines have entries of their own, any other multiple is Multiple with At in lines.
pub(super) fn line_box(spacing: wordcraft_doc::props::LineSpacing) -> (u8, f32) {
    use wordcraft_doc::props::LineSpacing;
    match spacing {
        LineSpacing::Multiple(m) => {
            let rule = [1.0, 1.5, 2.0].iter().position(|x| (m - x).abs() < 1e-4).and_then(|i| u8::try_from(i).ok());
            (rule.unwrap_or(LINE_MULTIPLE), m)
        }
        LineSpacing::AtLeast(pt) => (LINE_AT_LEAST, pt),
        LineSpacing::Exactly(pt) => (LINE_EXACTLY, pt),
    }
}

/// Whether the At box holds points (At least, Exactly) rather than lines.
pub(super) fn line_in_points(rule: u8) -> bool {
    matches!(rule, LINE_AT_LEAST | LINE_EXACTLY)
}

/// `lineSpacing` as `para.set` takes it for the Line spacing entry `rule` and the At value, kept
/// to the ranges Word's At box allows (ECMA-376 §17.3.1.33 `w:line`, `w:lineRule`).
pub(super) fn line_spacing(rule: u8, at: f32) -> Value {
    let (rule, value) = match rule {
        0 => ("multiple", 1.0),
        1 => ("multiple", 1.5),
        2 => ("multiple", 2.0),
        LINE_AT_LEAST => ("atLeast", at.clamp(0.0, 1584.0)),
        LINE_EXACTLY => ("exactly", at.clamp(0.7, 1584.0)),
        _ => ("multiple", at.clamp(0.06, 132.0)),
    };
    json!({"rule": rule, "value": value})
}

/// Pick `rule` in the Line spacing box: a points rule after a lines one starts At at Word's
/// 12 pt, Multiple after another rule at 3 lines.
pub(super) fn pick_line_rule(rule: &mut u8, at: &mut f32, picked: u8, was_shown: bool) {
    let before = if was_shown { Some(*rule) } else { None };
    if line_in_points(picked) && !before.is_some_and(line_in_points) {
        *at = 12.0;
    } else if picked == LINE_MULTIPLE && before != Some(LINE_MULTIPLE) {
        *at = 3.0;
    }
    *rule = picked;
}

/// The Paragraph dialog's Line spacing and At boxes. While the selected paragraphs differ in
/// line spacing both show blank, and OK leaves each its own until a rule is picked.
pub(super) fn line_spacing_boxes(ui: &mut Ui, rule: &mut u8, at: &mut f32, mixed: &mut BTreeSet<String>, label: &egui::Response) {
    let blank = mixed.contains("lineSpacing");
    let shown = if blank { "" } else { LINE_RULES.get(usize::from(*rule)).map_or("", |l| tl!(l)) };
    let combo = craft_ui::ComboBox::from_id_salt("para_line_rule").selected_text(shown).show_ui(ui, |ui| {
        for (k, l) in (0u8..).zip(LINE_RULES) {
            if ui.selectable_label(!blank && *rule == k, tl!(l)).clicked() {
                pick_line_rule(rule, at, k, !blank);
                mixed.remove("lineSpacing");
            }
        }
    });
    combo.response.labelled_by(label.id);
    let at_label = ui.label(tl!("At:"));
    let enabled = !blank && (line_in_points(*rule) || *rule == LINE_MULTIPLE);
    let field = if line_in_points(*rule) {
        egui::DragValue::new(at).speed(1.0).range(0.0..=1584.0).clamp_existing_to_range(false)
    } else {
        egui::DragValue::new(at).speed(0.05).range(0.06..=132.0).clamp_existing_to_range(false)
    };
    let field = if !enabled {
        field.custom_formatter(|_, _| String::new())
    } else if !line_in_points(*rule) {
        field
    } else if crate::rtl::on() {
        field.suffix(format!("{} {RLM}", tl!("pt")))
    } else {
        field.suffix(format!(" {}", tl!("pt")))
    };
    ui.add_enabled(enabled, field).labelled_by(at_label.id);
}

/// The Paragraph dialog's number boxes: a length in the measurement unit (indents), points
/// (spacing).
#[derive(Clone, Copy)]
pub(super) enum ParaNumber {
    Length,
    Points,
}

/// The right-to-left mark.
const RLM: char = '\u{200F}';

/// A Paragraph dialog number box for the field `key`. While the selected paragraphs differ in
/// it, it shows nothing, not even the unit, until a value is typed or dragged in.
pub(super) fn para_number(ui: &mut Ui, value: &mut f32, kind: ParaNumber, key: &str, mixed: &mut BTreeSet<String>, label: &egui::Response) {
    let unit = crate::options::unit();
    let blank = mixed.contains(key);
    // A value from the document outside the range stays until edited.
    let field = match kind {
        ParaNumber::Length => egui::DragValue::new(value).speed(3.6 / unit.pt_per_unit()).max_decimals(2),
        ParaNumber::Points => egui::DragValue::new(value).speed(1.0).range(0.0..=1584.0).clamp_existing_to_range(false),
    };
    let field = match kind {
        ParaNumber::Length if blank => field.measure_blank(unit),
        ParaNumber::Length => field.measure(unit),
        _ if blank => field.custom_formatter(|_, _| String::new()),
        // Right to left, the unit's space goes after the word and a right-to-left mark closes it: a
        // plain leading space between the digits and an Arabic unit is drawn with no width.
        ParaNumber::Points if crate::rtl::on() => field.suffix(format!("{} {RLM}", tl!("pt"))),
        ParaNumber::Points => field.suffix(format!(" {}", tl!("pt"))),
    };
    if ui.add(field).labelled_by(label.id).changed() {
        mixed.remove(key);
    }
}

/// A Paragraph dialog check box for the field `key`. While the selected paragraphs differ in
/// it, it shows as neither on nor off, and a click turns it on for all of them.
pub(super) fn para_check(ui: &mut Ui, on: &mut bool, text: &str, key: &str, mixed: &mut BTreeSet<String>) {
    if !mixed.contains(key) {
        ui.checkbox(on, text);
    } else if ui.add(egui::Checkbox::new(&mut false, text).indeterminate(true)).clicked() {
        *on = true;
        mixed.remove(key);
    }
}

/// The Paragraph dialog's OK, as one `para.set` and so one undo step (#1346): the fields that
/// differ from what every selected paragraph had when it opened, not the blank ones, which
/// stay as each paragraph has them. The alignment and indents are as seen on the page, so a
/// new direction keeps them where the dialog shows them.
pub(super) fn apply_paragraph(app: &mut WordApp, d: &Dialog) {
    let Dialog::Paragraph { start, mixed, .. } = d else { return };
    let fields = para_fields(d);
    let turned = !mixed.contains("bidi") && fields.get("bidi") != start.get("bidi");
    let (mut props, mut page) = (serde_json::Map::new(), serde_json::Map::new());
    for (k, v) in fields {
        let on_page = matches!(k.as_str(), "align" | "left" | "right");
        if mixed.contains(&k) || (start.get(&k) == Some(&v) && !(turned && on_page)) {
            continue;
        }
        if on_page {
            page.insert(k, v);
        } else {
            props.insert(k, v);
        }
    }
    if !props.is_empty() || !page.is_empty() {
        let _ = app.run("para.set", json!({"props": props, "onPage": page}));
    }
}
