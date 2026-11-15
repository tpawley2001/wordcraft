use super::export::HEAD_SIZES;
use super::state::{Pn, PnKind, St, pattern_shading};
use super::{Align, Fmt, Kind, ListInfo, Para, Reader, model};
use model::Label;
use std::collections::{HashMap, HashSet};
use wordcraft_doc::TextColor;
use wordcraft_doc::props::{CharProps, ParaProps};
use wordcraft_doc::styles::{Style, StyleKind, StyleSheet};

impl Reader {
    pub(super) fn commit_style(&mut self) {
        let name = std::mem::take(&mut self.style_name).trim().trim_end_matches(';').trim().to_string();
        let def = std::mem::take(&mut self.style_def_cur);
        if self.styles.len() < 4096 {
            if self.styles.insert(self.style_cur, name).is_none() {
                self.style_order.push(self.style_cur);
            }
            self.style_defs.insert(self.style_cur, def);
            self.style_ids = std::cell::OnceCell::new();
        }
        // The next entry is style 0 (Normal) unless it names a number.
        self.style_cur = 0;
    }

    /// The document style id of each paragraph and character entry of the stylesheet.
    fn style_ids(&self) -> &StyleIds {
        self.style_ids.get_or_init(|| self.make_style_ids())
    }

    /// The entry's name: the first of its comma-separated names (`Body Text 2,bt2`).
    fn entry_name(&self, key: i32) -> &str {
        self.styles.get(&key).and_then(|n| n.split(',').next()).map_or("", str::trim)
    }

    /// The document style id entry `key` (a paragraph or character style) stands for when it is
    /// one the flow model's kinds or formatting stand for (Normal, the headings, Title, Quote,
    /// Code, Default Paragraph Font…), or `None` when it is one of the document's own.
    fn fixed_entry_id(&self, key: i32, kind: StyleKind, builtin: &StyleSheet) -> Option<String> {
        let name = self.entry_name(key);
        // A built-in table style (Normal Table, Table Grid…) keeps WordCraft's definition.
        if kind == StyleKind::Table {
            return builtin_id(builtin, name, kind);
        }
        if kind == StyleKind::Paragraph {
            if key == 0 {
                return Some("Normal".into());
            }
            match self.name_kind(key) {
                Some(Kind::Heading(n)) => {
                    let n = name.get(7..).and_then(|r| r.trim().parse::<u8>().ok()).filter(|n| (1..=9).contains(n)).unwrap_or(n);
                    return Some(format!("Heading{n}"));
                }
                Some(Kind::Title) => return Some("Title".into()),
                Some(Kind::Quote) if name.eq_ignore_ascii_case("intense quote") => return Some("IntenseQuote".into()),
                Some(Kind::Quote) => return Some("Quote".into()),
                Some(Kind::Code) => return Some(model::CODE_STYLE.into()),
                _ => {}
            }
        }
        let id = builtin_id(builtin, name, kind).unwrap_or_else(|| model::style_id(name));
        let fixed = if kind == StyleKind::Paragraph { model::is_fixed_style(&id) } else { model::is_fixed_char_style(&id) };
        fixed.then_some(id)
    }

    fn make_style_ids(&self) -> StyleIds {
        let builtin = StyleSheet::builtin();
        let mut out = StyleIds::default();
        let entries: Vec<(i32, StyleKind)> = self
            .style_order
            .iter()
            .filter_map(|k| match self.style_defs.get(k).map(|d| d.entry) {
                Some(StyleEntry::Para) => Some((*k, StyleKind::Paragraph)),
                Some(StyleEntry::Char) => Some((*k, StyleKind::Character)),
                Some(StyleEntry::Table) => Some((*k, StyleKind::Table)),
                _ => None,
            })
            .collect();
        // The fixed styles first, so the document's own don't take their ids.
        let mut taken: HashSet<String> = HashSet::new();
        for &(key, kind) in &entries {
            if let Some(id) = self.fixed_entry_id(key, kind, &builtin) {
                taken.insert(id.clone());
                out.ids.insert(key, (id, false));
            }
        }
        for &(key, kind) in &entries {
            if out.ids.contains_key(&key) {
                continue;
            }
            let name = self.entry_name(key);
            let base = builtin_id(&builtin, name, kind).unwrap_or_else(|| model::style_id(name));
            // An id another entry has, or a built-in style of the other kind or a fixed one has,
            // takes a number.
            let free = |id: &str| {
                !taken.contains(id) && !model::is_fixed_style(id) && !model::is_fixed_char_style(id) && builtin.get(id).is_none_or(|s| s.kind == kind)
            };
            let mut id = base.clone();
            let mut n = 2u32;
            while !free(&id) {
                id = format!("{base}{n}");
                n = n.saturating_add(1);
            }
            taken.insert(id.clone());
            if kind == StyleKind::Character {
                out.own_chars.insert(id.clone(), key);
            }
            out.ids.insert(key, (id, true));
            out.own.push((key, kind));
        }
        out
    }

    /// The document style id of the entry numbered `n` that a `\sbasedon`, `\snext` or `\slink`
    /// names: a character entry's first when `char_first`, else a paragraph entry's.
    fn style_ref(&self, n: Option<i32>, char_first: bool) -> Option<String> {
        let n = n.filter(|n| *n >= 0)?;
        let ids = &self.style_ids().ids;
        let ck = -1 - n;
        let keys = if char_first { [ck, n] } else { [n, ck] };
        keys.iter().find_map(|k| ids.get(k)).map(|(id, _)| id.clone()).or_else(|| (n == 0 && !char_first).then(|| "Normal".into()))
    }

    /// The document's own paragraph style that `\sN` names, if entry `n` became one.
    pub(super) fn own_para_style(&self, n: i32) -> Option<String> {
        self.style_defs.get(&n).filter(|d| d.entry == StyleEntry::Para)?;
        self.style_ids().ids.get(&n).filter(|(_, own)| *own).map(|(id, _)| id.clone())
    }

    /// The document style id of the table style `\tsN` names: its stylesheet entry's, if
    /// there is one.
    pub(super) fn table_style_id(&self, n: i32) -> Option<String> {
        let key = -1 - n.saturating_abs();
        self.style_defs.get(&key).filter(|d| d.entry == StyleEntry::Table)?;
        self.style_ids().ids.get(&key).map(|(id, _)| id.clone())
    }

    /// A table look word of a row (`\tbllkhdrrows`…, RTF 1.9.1, "Table row definitions"). The
    /// first one sets the look to what the row's words say: no header, total row or first or
    /// last column unless named, and banded rows and columns unless turned off.
    pub(super) fn look_word(&mut self, w: &str, on: bool) {
        use wordcraft_doc::props::TableLook;
        let l = self.row_style.look.get_or_insert(TableLook {
            header_row: false,
            total_row: false,
            banded_rows: true,
            first_column: false,
            last_column: false,
            banded_columns: true,
        });
        match w {
            "tbllkhdrrows" => l.header_row = on,
            "tbllklastrow" => l.total_row = on,
            "tbllkhdrcols" => l.first_column = on,
            "tbllklastcol" => l.last_column = on,
            "tbllknorowband" => l.banded_rows = !on,
            "tbllknocolband" => l.banded_columns = !on,
            _ => {}
        }
    }

    /// A row formatting word in a table style's stylesheet entry; false when `word` isn't one,
    /// for the entry's other words (based on, flags, paragraph and character formatting).
    pub(super) fn table_style_word(&mut self, word: &str, value: i32) -> bool {
        let color = |i: i32| usize::try_from(i).ok().and_then(|i| self.colors.get(i).copied().flatten());
        let t = &mut self.style_def_cur.table;
        match word {
            "tsrowd" => t.side = None,
            "trbrdrt" | "trbrdrl" | "trbrdrb" | "trbrdrr" | "trbrdrh" | "trbrdrv" => t.side = word.chars().last(),
            // A cell-level default border: not kept.
            _ if word.starts_with("tsbrdr") => t.side = None,
            "brdrw" => {
                let width = (value as f32 / 20.0).clamp(0.25, 6.0);
                if let Some(Some(b)) = t.slot() {
                    b.width = width;
                }
            }
            "brdrcf" => {
                let c = color(value);
                if let Some(Some(b)) = t.slot() {
                    b.color = c;
                }
            }
            "trcbpat" => t.fill = color(value),
            "trpaddl" | "trpaddt" | "trpaddb" | "trpaddr" | "trpaddfl" | "trpaddft" | "trpaddfb" | "trpaddfr" => {
                let i = match word.chars().last() {
                    Some('l') => 0,
                    Some('t') => 1,
                    Some('b') => 2,
                    _ => 3,
                };
                if let Some(p) = t.pad.get_mut(i) {
                    if word.starts_with("trpaddf") {
                        p.1 = Some(value);
                    } else {
                        p.0 = Some(value);
                    }
                }
            }
            _ => {
                let Some(style) = super::word::border_style(word) else { return false };
                let b = (style != wordcraft_doc::props::BorderStyle::None)
                    .then_some(wordcraft_doc::props::Border { style, ..wordcraft_doc::props::Border::single(0.5) });
                match t.slot() {
                    Some(slot) => *slot = b,
                    None => return false,
                }
            }
        }
        true
    }

    /// The document's own character style that `\csN` names, if entry `n` became one.
    pub(super) fn own_char_style(&self, n: i32) -> Option<String> {
        let key = -1 - n.saturating_abs();
        self.style_defs.get(&key).filter(|d| d.entry == StyleEntry::Char)?;
        self.style_ids().ids.get(&key).filter(|(_, own)| *own).map(|(id, _)| id.clone())
    }

    /// The document's own styles the stylesheet defines, in the file's order: each with its
    /// based-on, next and linked styles, its flags, and the formatting its entry itself sets.
    pub(super) fn own_styles(&self) -> Vec<Style> {
        let builtin = StyleSheet::builtin();
        let ids = self.style_ids();
        let mut out = Vec::with_capacity(ids.own.len());
        for &(key, kind) in &ids.own {
            let (Some(d), Some((id, _))) = (self.style_defs.get(&key), ids.ids.get(&key)) else { continue };
            let char = kind == StyleKind::Character;
            let table = kind == StyleKind::Table;
            let (para, chr) = self.def_props(d);
            out.push(Style {
                id: id.clone(),
                name: self.entry_name(key).to_string(),
                kind,
                // A table style's `\sbasedon` names a table entry, numbered as a character one.
                based_on: self.style_ref(d.based_on, char || table),
                next: if table { None } else { self.style_ref(d.next, false) },
                linked: if table { None } else { self.style_ref(d.link, !char) },
                table: if table { d.table.parts() } else { None },
                para: if char { ParaProps::default() } else { para },
                chr,
                priority: d.priority,
                quick: d.quick,
                hidden: d.hidden,
                locked: d.locked,
                builtin: builtin.get(id).is_some(),
                ..Default::default()
            });
        }
        out
    }

    /// The paragraph and character formatting stylesheet entry `d` itself sets, converted as
    /// the same words on a paragraph and its text are.
    fn def_props(&self, d: &StyleDef) -> (ParaProps, CharProps) {
        let pt = |v: i32| (v as f32 / 20.0).clamp(-1584.0, 1584.0);
        let gap = |v: i32| pt(v).max(0.0);
        let chars = |v: i32| v as f32 / 100.0;
        let line = d.sl.and_then(|sl| spacing(&St { sl, slmult: d.slmult.unwrap_or(false), ..Default::default() }).line);
        let para = ParaProps {
            align: d.align,
            indent_left: d.li.map(pt),
            indent_right: d.ri.map(pt),
            indent_first: d.fi.map(pt),
            space_before: d.sb.map(gap),
            space_after: d.sa.map(gap),
            line_spacing: line,
            keep_next: d.keep_next,
            keep_lines: d.keep_lines,
            tabs: (!d.tabs.is_empty()).then(|| d.tabs.clone()),
            indent_first_chars: d.cufi.map(chars),
            indent_left_chars: d.culi.map(chars),
            indent_right_chars: d.curi.map(chars),
            shading: self.def_para_shading(d),
            borders: self.def_para_borders(d),
            text_alignment: d.font_align,
            // A paragraph entry without `\adjustright` has it off; other entries leave it unset.
            adjust_right_indent: (d.entry == StyleEntry::Para).then_some(d.adjust_right),
            ..Default::default()
        };
        let color = |i: i32| usize::try_from(i).ok().and_then(|i| self.colors.get(i).copied().flatten()).map_or(TextColor::Auto, TextColor::Rgb);
        let chr = CharProps {
            font: d.font.and_then(|i| self.fonts.get(&i)).map(|n| n.to_string()),
            size: d.size,
            bold: d.bold,
            italic: d.italic,
            color: d.color.map(color),
            spacing: d.spacing,
            ..self.def_char_lines(d)
        };
        (para, chr)
    }

    /// The character border, underline and strikethrough entry `d` states (`None` for what it
    /// doesn't); the border's colour is looked up in the colour table as a run's `\brdrcf` is.
    fn def_char_lines(&self, d: &StyleDef) -> CharProps {
        let border = d.border.map(|mut b| {
            b.color = d.border_color.and_then(|i| self.colors.get(i.max(0) as usize).copied().flatten());
            b
        });
        let (highlight, shading) = self.def_highlight_and_shading(d);
        CharProps {
            border,
            underline: d.underline,
            underline_color: d.underline_color.filter(|i| *i > 0).and_then(|i| self.color_at(i)),
            strike: d.strike,
            double_strike: d.double_strike,
            caps: d.caps,
            small_caps: d.small_caps,
            hidden: d.hidden_text,
            snap_to_grid: d.snap_off.then_some(false),
            vert_align: d.vert_align,
            highlight,
            shading,
            ..Default::default()
        }
    }

    /// Colour table entry `i`; `None` for the automatic colour, a negative index or one past
    /// the table.
    fn color_at(&self, i: i32) -> Option<wordcraft_doc::Rgb> {
        usize::try_from(i).ok().and_then(|i| self.colors.get(i).copied().flatten())
    }

    /// The highlight and character shading entry `d` states, as a run's words give them: a
    /// highlight in one of the highlighter colours is the highlight, any other colour is
    /// shading; a shading pattern (`\chcbpat`, `\chcfpat`, `\chshdng`) is the shading.
    fn def_highlight_and_shading(&self, d: &StyleDef) -> (Option<wordcraft_doc::props::Highlight>, Option<wordcraft_doc::Rgb>) {
        use wordcraft_doc::props::Highlight;
        let marked = d.highlight.and_then(|i| self.color_at(i));
        let highlight = marked.and_then(|c| Highlight::ALL.into_iter().find(|h| *h != Highlight::None && h.rgb() == Some(c)));
        let pattern = d.ch_fill.is_some() || d.ch_ink.is_some() || d.ch_pct.is_some();
        let shading = if pattern {
            pattern_shading(d.ch_pct.unwrap_or(0), d.ch_ink.and_then(|i| self.color_at(i)), d.ch_fill.and_then(|i| self.color_at(i)))
        } else {
            marked.filter(|_| highlight.is_none())
        };
        (highlight, shading)
    }

    /// The paragraph shading entry `d` states (`\cbpat`, `\cfpat`, `\shading`), as a paragraph's.
    fn def_para_shading(&self, d: &StyleDef) -> Option<wordcraft_doc::Rgb> {
        pattern_shading(d.para_pct.unwrap_or(0), d.para_ink.and_then(|i| self.color_at(i)), d.para_fill.and_then(|i| self.color_at(i)))
    }

    /// The paragraph borders entry `d` states, their colours looked up; `None` when it states no side.
    fn def_para_borders(&self, d: &StyleDef) -> Option<wordcraft_doc::props::Borders> {
        let mut sides = d.para_borders;
        for (b, c) in sides.iter_mut().zip(d.para_border_colors) {
            if let Some(b) = b {
                b.color = c.and_then(|i| self.color_at(i));
            }
        }
        let [top, left, bottom, right, between] = sides;
        sides.iter().any(Option::is_some).then_some(wordcraft_doc::props::Borders { top, left, bottom, right, between, ..Default::default() })
    }

    /// Whether another program wrote the file: its `{\*\generator …}` names one that isn't
    /// WordCraft. Such a file's styles have only what their stylesheet entries set.
    pub(super) fn foreign(&self) -> bool {
        self.generator.as_deref().is_some_and(|g| !g.trim_start().starts_with("WordCraft"))
    }

    /// The formatting style `id` has in this file: its stylesheet entry's,
    /// then its `\sbasedon` ancestors' (a bounded walk: loops and missing ids end it).
    fn file_style(&self, id: i32) -> StyleDef {
        let mut out = StyleDef::default();
        let mut seen: Vec<i32> = Vec::new();
        let mut cur = Some(id);
        while let Some(i) = cur {
            if seen.len() >= 16 || seen.contains(&i) {
                break;
            }
            seen.push(i);
            let Some(d) = self.style_defs.get(&i) else { break };
            out.bold = out.bold.or(d.bold);
            out.italic = out.italic.or(d.italic);
            out.size = out.size.or(d.size);
            out.font = out.font.or(d.font);
            out.color = out.color.or(d.color);
            out.sb = out.sb.or(d.sb);
            out.sa = out.sa.or(d.sa);
            out.sl = out.sl.or(d.sl);
            out.slmult = out.slmult.or(d.slmult);
            out.li = out.li.or(d.li);
            out.ri = out.ri.or(d.ri);
            out.fi = out.fi.or(d.fi);
            out.keep_next = out.keep_next.or(d.keep_next);
            out.keep_lines = out.keep_lines.or(d.keep_lines);
            out.align = out.align.or(d.align);
            // Tab stops add up along the chain; a derived style's stop replaces one at its position.
            for t in &d.tabs {
                if out.tabs.len() < super::state::MAX_TABS && !out.tabs.iter().any(|o| o.pos == t.pos) {
                    out.tabs.push(*t);
                }
            }
            out.cufi = out.cufi.or(d.cufi);
            out.culi = out.culi.or(d.culi);
            out.curi = out.curi.or(d.curi);
            out.spacing = out.spacing.or(d.spacing);
            if out.border.is_none() {
                (out.border, out.border_color) = (d.border, d.border_color);
            }
            out.underline = out.underline.or(d.underline);
            out.strike = out.strike.or(d.strike);
            out.double_strike = out.double_strike.or(d.double_strike);
            out.caps = out.caps.or(d.caps);
            out.small_caps = out.small_caps.or(d.small_caps);
            out.hidden_text = out.hidden_text.or(d.hidden_text);
            out.snap_off |= d.snap_off;
            out.vert_align = out.vert_align.or(d.vert_align);
            out.highlight = out.highlight.or(d.highlight);
            out.underline_color = out.underline_color.or(d.underline_color);
            out.ch_fill = out.ch_fill.or(d.ch_fill);
            out.ch_ink = out.ch_ink.or(d.ch_ink);
            out.ch_pct = out.ch_pct.or(d.ch_pct);
            out.para_fill = out.para_fill.or(d.para_fill);
            out.para_ink = out.para_ink.or(d.para_ink);
            out.para_pct = out.para_pct.or(d.para_pct);
            for ((o, oc), (b, c)) in
                out.para_borders.iter_mut().zip(out.para_border_colors.iter_mut()).zip(d.para_borders.iter().zip(d.para_border_colors.iter()))
            {
                if o.is_none() {
                    (*o, *oc) = (*b, *c);
                }
            }
            cur = d.based_on;
        }
        out
    }

    /// The formatting a paragraph of kind `kind` in style `id` gives its runs. A style whose
    /// entry and `\sbasedon` chain set no character formatting (a bare `{\s2 heading 2;}`) has
    /// WordCraft's own (`style_fmt`); otherwise what the chain doesn't set is RTF's default (not
    /// bold, not italic, 12 pt). The font alone falls back per property: WordCraft writes the code
    /// font on each code paragraph, not in its Code entry. In another program's file a heading,
    /// title or quote style always has the chain's formatting and RTF's defaults (see
    /// [`Reader::foreign_styles`]).
    pub(super) fn style_base(&self, kind: Kind, id: i32) -> Fmt {
        let mut f = style_fmt(kind);
        let d = self.file_style(id);
        let own = self.foreign() && foreign_style_id(kind).is_some();
        if own || d.bold.is_some() || d.italic.is_some() || d.size.is_some() || d.font.is_some() {
            f.bold = d.bold.unwrap_or(false);
            f.italic = d.italic.unwrap_or(false);
            f.size = Some(d.size.unwrap_or(12.0));
        }
        // A font is matched the way `\f` sets a run's: a monospaced one is the code font.
        if let Some(name) = d.font.and_then(|i| self.fonts.get(&i)) {
            f.code = model::is_mono(name);
            f.font = (!f.code).then(|| name.clone());
        }
        f
    }

    /// In another program's file, the document style each heading, title and quote entry of the
    /// stylesheet stands for, with exactly the formatting the entry and its `\sbasedon` chain set
    /// and RTF's defaults for the rest (RTF 1.9.1, "Style sheet": no space before or after, no
    /// keeps, single spacing, no indents, left aligned, not bold or italic, 12 pt, auto colour).
    /// None of WordCraft's built-in formatting for those styles comes in; the outline level does.
    /// Empty for WordCraft's own files and files that don't name their program.
    pub(super) fn foreign_styles(&self) -> Vec<(String, ParaProps, CharProps)> {
        use wordcraft_doc::props::LineSpacing;
        if !self.foreign() {
            return Vec::new();
        }
        let mut ids: Vec<i32> = self.styles.keys().copied().collect();
        ids.sort_unstable();
        let mut out: Vec<(String, ParaProps, CharProps)> = Vec::new();
        for id in ids {
            let Some(style) = self.name_kind(id).and_then(foreign_style_id) else { continue };
            // The first entry naming a style gives it its formatting.
            if out.iter().any(|(s, _, _)| *s == style) {
                continue;
            }
            let d = self.file_style(id);
            let pt = |v: i32| (v as f32 / 20.0).clamp(-1584.0, 1584.0);
            let st = St { sl: d.sl.unwrap_or(0), slmult: d.slmult.unwrap_or(false), ..Default::default() };
            let line = spacing(&st).line.unwrap_or(LineSpacing::Multiple(1.0));
            let para = ParaProps {
                align: Some(d.align.unwrap_or(Align::Left)),
                indent_left: Some(d.li.map_or(0.0, pt)),
                indent_right: Some(d.ri.map_or(0.0, pt)),
                indent_first: Some(d.fi.map_or(0.0, pt)),
                space_before: Some(d.sb.map_or(0.0, |v| pt(v).max(0.0))),
                space_after: Some(d.sa.map_or(0.0, |v| pt(v).max(0.0))),
                line_spacing: Some(line),
                contextual_spacing: Some(false),
                keep_next: Some(d.keep_next.unwrap_or(false)),
                keep_lines: Some(d.keep_lines.unwrap_or(false)),
                tabs: (!d.tabs.is_empty()).then(|| d.tabs.clone()),
                indent_first_chars: d.cufi.map(|v| v as f32 / 100.0),
                indent_left_chars: d.culi.map(|v| v as f32 / 100.0),
                indent_right_chars: d.curi.map(|v| v as f32 / 100.0),
                ..Default::default()
            };
            let color = d.color.and_then(|i| usize::try_from(i).ok()).and_then(|i| self.colors.get(i).copied().flatten());
            let chr = CharProps {
                font: d.font.and_then(|i| self.fonts.get(&i)).map(|n| n.to_string()),
                size: Some(d.size.unwrap_or(12.0)),
                bold: Some(d.bold.unwrap_or(false)),
                italic: Some(d.italic.unwrap_or(false)),
                color: Some(color.map_or(TextColor::Auto, TextColor::Rgb)),
                spacing: d.spacing,
                ..self.def_char_lines(&d)
            };
            out.push((style, para, chr));
        }
        out
    }

    /// The paragraph kind the stylesheet entry `id`'s name gives.
    fn name_kind(&self, id: i32) -> Option<Kind> {
        // Only a paragraph style's.
        if self.style_defs.get(&id).is_some_and(|d| d.entry != StyleEntry::Para) {
            return None;
        }
        let name = self.styles.get(&id).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
        if let Some(n) = name.strip_prefix("heading").and_then(|r| r.trim().parse::<u8>().ok()) {
            return Some(Kind::Heading(n.clamp(1, 6)));
        }
        if id != 0 {
            match name.as_str() {
                "title" => return Some(Kind::Title),
                "quote" | "intense quote" => return Some(Kind::Quote),
                "code" | "html preformatted" | "plain text" => return Some(Kind::Code),
                _ => {}
            }
        }
        None
    }

    /// The paragraph kind `st`'s style names, if it names one.
    pub(super) fn style_kind(&self, st: &St) -> Option<Kind> {
        self.name_kind(st.style)
    }

    fn para_kind(&self, st: &St) -> Kind {
        // Only a style's name makes a heading: an outline level alone keeps the paragraph's own
        // style and formatting (as Word reads `\outlinelevelN`).
        self.style_kind(st).unwrap_or(Kind::Normal)
    }

    /// The paragraph formatting of `st` (kind, alignment, page break, spacing, borders, list).
    pub(super) fn para_props(&self, st: &St) -> Para {
        let mut p = Para::new(self.para_kind(st));
        p.align = st.align;
        p.page_break = st.pagebb;
        if st.widow.is_some() {
            p.extra.get_or_insert_with(Default::default).widow_control = st.widow;
        }
        if st.culi.is_some() || st.curi.is_some() || st.cufi.is_some() {
            let chars = |v: Option<i32>| v.map(|v| v as f32 / 100.0);
            let extra = p.extra.get_or_insert_with(Default::default);
            extra.indent_left_chars = chars(st.culi);
            extra.indent_right_chars = chars(st.curi);
            extra.indent_first_chars = chars(st.cufi);
        }
        p.spacing = spacing(st);
        p.borders = st.borders.any_visible().then_some(st.borders);
        if !st.tabs.is_empty() {
            p.extra.get_or_insert_with(Default::default).tabs = Some(st.tabs.clone());
        }
        p.keep_next = st.keepn;
        if st.keep || st.noline {
            let x = p.extra.get_or_insert_with(Default::default);
            x.keep_lines = st.keep.then_some(true);
            x.suppress_line_numbers = st.noline.then_some(true);
        }
        if !st.asp_alpha || !st.asp_num {
            let x = p.extra.get_or_insert_with(Default::default);
            x.auto_space_de = (!st.asp_alpha).then_some(false);
            x.auto_space_dn = (!st.asp_num).then_some(false);
        }
        let flags = [st.contextual, st.mirror, st.sb_auto, st.sa_auto, st.no_hyph, st.snap_grid];
        if flags.iter().any(Option::is_some) {
            let x = p.extra.get_or_insert_with(Default::default);
            x.contextual_spacing = st.contextual;
            x.mirror_indents = st.mirror;
            x.space_before_auto = st.sb_auto;
            x.space_after_auto = st.sa_auto;
            x.suppress_hyphens = st.no_hyph;
            x.snap_to_grid = st.snap_grid;
        }
        // Adjust right indent only where it differs from the paragraph's style: in RTF a
        // paragraph without `\adjustright` has it off, where the document's styles default to on.
        let adjust = (st.adjust_right != self.style_adjust_right(st.style)).then_some(st.adjust_right);
        if st.font_align.is_some() || adjust.is_some() {
            let x = p.extra.get_or_insert_with(Default::default);
            x.text_alignment = st.font_align;
            x.adjust_right_indent = adjust;
        }
        if let Some(f) = st.frame.as_deref() {
            f.apply(p.extra.get_or_insert_with(Default::default));
        }
        if let Some(id) = self.own_para_style(st.style) {
            p.extra.get_or_insert_with(Default::default).style = Some(id);
        }
        if let Some(ls) = st.ls {
            let list = self.overrides.get(&ls).and_then(|id| self.lists.get(id));
            let lv = st.ilvl.min(8);
            let ordered = list.and_then(|l| l.get(lv as usize).copied()).unwrap_or(false);
            p.list = Some(ListInfo { ordered, level: lv, start: 1, ..Default::default() });
        } else if let Some(pn) = st.pn.as_deref() {
            p.list = self.pn_list(pn);
        }
        if let Some(c) = self.para_shading(st) {
            p.extra.get_or_insert_with(Default::default).shading = Some(c);
        }
        // An outline level apart from a heading style, the paragraph's own.
        if !matches!(p.kind, Kind::Heading(_))
            && let Some(o) = st.outline.filter(|o| *o < 9)
        {
            p.extra.get_or_insert_with(Default::default).outline_level = Some(o);
        }
        p
    }

    /// The Automatically adjust right indent a paragraph in style `\sN` gets from its style: its
    /// paragraph entry's, else the Normal entry's (a style the stylesheet doesn't define is
    /// Normal), else on (no stylesheet: the document's built-in styles).
    fn style_adjust_right(&self, n: i32) -> bool {
        let entry = |k: i32| self.style_defs.get(&k).filter(|d| d.entry == StyleEntry::Para).map(|d| d.adjust_right);
        entry(n).or_else(|| entry(0)).unwrap_or(true)
    }

    /// The font alignment and Automatically adjust right indent of each paragraph entry that
    /// stands for one of the document's fixed styles (Normal, the headings…), which those styles
    /// take from the file; the document's own styles have them in [`Reader::own_styles`].
    pub(super) fn fixed_style_flags(&self) -> Vec<(String, Option<wordcraft_doc::props::TextAlignment>, bool)> {
        let ids = self.style_ids();
        let mut out: Vec<(String, Option<wordcraft_doc::props::TextAlignment>, bool)> = Vec::new();
        for key in &self.style_order {
            let Some(d) = self.style_defs.get(key).filter(|d| d.entry == StyleEntry::Para) else { continue };
            let Some((id, false)) = ids.ids.get(key) else { continue };
            // The first entry naming a style gives it its formatting.
            if !out.iter().any(|(s, _, _)| s == id) {
                out.push((id.clone(), d.font_align, d.adjust_right));
            }
        }
        out
    }

    /// The paragraph's shading colour (`\cbpat`, `\cfpat`, `\shading`): the colour the pattern
    /// averages to (see [`pattern_shading`]); `None` for no pattern over an automatic background.
    fn para_shading(&self, st: &St) -> Option<wordcraft_doc::props::Rgb> {
        let color = |i: Option<i32>| i.and_then(|i| usize::try_from(i).ok()).and_then(|i| self.colors.get(i).copied().flatten());
        pattern_shading(st.shading, color(st.cfpat), color(st.cbpat))
    }

    /// A Word 6.0/95 `{\*\pn …}` group as the paragraph's list level. A continuation has the
    /// `none` format: it belongs to the list, but shows no label and doesn't advance its count.
    fn pn_list(&self, pn: &Pn) -> Option<ListInfo> {
        use wordcraft_doc::section::NumFormat;
        let (ordered, level) = match pn.kind {
            PnKind::None => return None,
            PnKind::Bullet => (false, 0),
            PnKind::Body | PnKind::Cont => (true, 0),
            PnKind::Outline(n) => (true, n.saturating_sub(1).min(8)),
        };
        let format = if pn.kind == PnKind::Cont {
            NumFormat::None
        } else if ordered {
            pn.format.unwrap_or(NumFormat::Decimal)
        } else {
            NumFormat::Bullet
        };
        let label = Label {
            format,
            prefix: model::Affix::new(&pn.before),
            suffix: model::Affix::new(&pn.after),
            shown: 1,
            // Legacy numbering uses the paragraph's own indents, not gallery defaults.
            indent: Some(0),
            hanging: Some(0),
            align: None,
        };
        let font = pn.font.and_then(|f| self.fonts.get(&f)).and_then(|f| model::LabelFont::new(f));
        Some(ListInfo { ordered, level, start: pn.start.unwrap_or(1), list: 0, label: Some(label), font })
    }
}

/// The document style a paragraph of kind `kind` takes, when another program's stylesheet entry
/// gives it its formatting. Code paragraphs keep WordCraft's code style.
fn foreign_style_id(kind: Kind) -> Option<String> {
    match kind {
        Kind::Heading(n) => Some(format!("Heading{}", n.clamp(1, 6))),
        Kind::Title => Some("Title".into()),
        Kind::Quote => Some("Quote".into()),
        Kind::Code | Kind::Normal | Kind::Rule => None,
    }
}

/// The character formatting the paragraph style of `kind` gives its runs, as the writer's
/// stylesheet and paragraph properties set it.
pub(super) fn style_fmt(kind: Kind) -> Fmt {
    match kind {
        Kind::Heading(n) => Fmt { bold: true, size: HEAD_SIZES.get(usize::from(n.clamp(1, 6)) - 1).copied(), ..Default::default() },
        Kind::Title => Fmt { size: Some(28.0), ..Default::default() },
        Kind::Quote => Fmt { italic: true, ..Default::default() },
        Kind::Code => Fmt { code: true, size: Some(10.0), ..Default::default() },
        Kind::Normal | Kind::Rule => Fmt::default(),
    }
}

/// `f` with what it doesn't set directly taken from its paragraph style's formatting `base`.
pub(super) fn styled(base: &Fmt, f: &Fmt) -> Fmt {
    let mut e = f.clone();
    e.bold |= base.bold;
    e.italic |= base.italic;
    if e.size.is_none() {
        e.size = base.size;
    }
    if !e.code && e.font.is_none() {
        e.code = base.code;
    }
    e
}

/// The built-in style of kind `kind` whose name is `name` (case aside), as its id.
fn builtin_id(builtin: &StyleSheet, name: &str, kind: StyleKind) -> Option<String> {
    builtin.styles.iter().find(|s| s.kind == kind && s.name.eq_ignore_ascii_case(name)).map(|s| s.id.clone())
}

/// The document style ids of a stylesheet's paragraph and character entries.
#[derive(Debug, Default)]
pub(super) struct StyleIds {
    /// By entry: the id, and whether it is one of the document's own styles.
    ids: HashMap<i32, (String, bool)>,
    /// The entries that are the document's own styles, in the file's order.
    own: Vec<(i32, StyleKind)>,
    /// The entry of each of the document's own character styles, by id.
    own_chars: HashMap<String, i32>,
}

/// What a `\stylesheet` entry defines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum StyleEntry {
    /// A paragraph style (`\sN`, or no number: style 0).
    #[default]
    Para,
    /// A character style (`\*\csN`).
    Char,
    /// A table style (`\*\tsN`).
    Table,
    /// A section style (`\dsN`).
    Other,
}

/// A `\stylesheet` entry's formatting; `None` where the entry doesn't set it. Spacing and
/// indents are in twips, as written.
#[derive(Clone, Debug, Default)]
pub(super) struct StyleDef {
    pub(super) entry: StyleEntry,
    pub(super) based_on: Option<i32>,
    /// The style of the next paragraph (`\snext`), the linked style (`\slink`).
    pub(super) next: Option<i32>,
    pub(super) link: Option<i32>,
    /// In the Quick Styles gallery (`\sqformat`), its gallery order (`\spriority`), hidden
    /// (`\ssemihidden`, `\shidden`), locked (`\slocked`).
    pub(super) quick: bool,
    pub(super) priority: Option<u32>,
    pub(super) hidden: bool,
    pub(super) locked: bool,
    pub(super) bold: Option<bool>,
    pub(super) italic: Option<bool>,
    pub(super) size: Option<f32>,
    /// A `\fonttbl` index, named when it's used (the table may follow).
    pub(super) font: Option<i32>,
    /// A `\colortbl` index (`\cf`).
    pub(super) color: Option<i32>,
    pub(super) sb: Option<i32>,
    pub(super) sa: Option<i32>,
    pub(super) sl: Option<i32>,
    pub(super) slmult: Option<bool>,
    pub(super) li: Option<i32>,
    pub(super) ri: Option<i32>,
    pub(super) fi: Option<i32>,
    pub(super) keep_next: Option<bool>,
    pub(super) keep_lines: Option<bool>,
    pub(super) align: Option<Align>,
    /// Font alignment (`\fa…`) and Automatically adjust right indent (`\adjustright`; a
    /// paragraph entry without it has the setting off).
    pub(super) font_align: Option<wordcraft_doc::props::TextAlignment>,
    pub(super) adjust_right: bool,
    /// Tab stops (`\tx`, `\tb`), and the alignment and leader words read for the next one.
    pub(super) tabs: Vec<wordcraft_doc::props::TabStop>,
    pub(super) tab_align: wordcraft_doc::props::TabAlign,
    pub(super) tab_leader: wordcraft_doc::props::TabLeader,
    /// Indents in hundredths of a character (`\cufi`, `\culi`, `\curi`).
    pub(super) cufi: Option<i32>,
    pub(super) culi: Option<i32>,
    pub(super) curi: Option<i32>,
    /// Character spacing in points (`\expnd`, `\expndtw`; the later wins).
    pub(super) spacing: Option<f32>,
    /// The character border (`\chbrdr` and the border words after it), its colour as a
    /// `\colortbl` index (`\brdrcf`, looked up when the style is made), and whether border words
    /// still describe it (until a paragraph border side starts).
    pub(super) border: Option<wordcraft_doc::props::Border>,
    pub(super) border_color: Option<i32>,
    pub(super) border_open: bool,
    /// Underline (`\ul…`; `\ulnone` is a stated none), strikethrough (`\strike`) and double
    /// strikethrough (`\striked1`).
    pub(super) underline: Option<wordcraft_doc::props::Underline>,
    pub(super) strike: Option<bool>,
    pub(super) double_strike: Option<bool>,
    /// All caps (`\caps`), small caps (`\scaps`), hidden (`\v`); a stated off is kept.
    pub(super) caps: Option<bool>,
    pub(super) small_caps: Option<bool>,
    pub(super) hidden_text: Option<bool>,
    /// Snap to grid turned off (`\cgrid0`); on, the default, is kept as nothing.
    pub(super) snap_off: bool,
    /// Superscript or subscript (`\super`, `\sub`, `\nosupersub`; the later wins).
    pub(super) vert_align: Option<wordcraft_doc::props::VertAlign>,
    /// `\colortbl` indexes: the highlight (`\highlight`) and the underline colour (`\ulc`).
    pub(super) highlight: Option<i32>,
    pub(super) underline_color: Option<i32>,
    /// Character shading (`\chcbpat`/`\cb`, `\chcfpat`, `\chshdng`): background and pattern
    /// colour indexes and the pattern's coverage; `None` where the entry doesn't state it.
    pub(super) ch_fill: Option<i32>,
    pub(super) ch_ink: Option<i32>,
    pub(super) ch_pct: Option<i32>,
    /// Paragraph shading (`\cbpat`, `\cfpat`, `\shading`), as for characters.
    pub(super) para_fill: Option<i32>,
    pub(super) para_ink: Option<i32>,
    pub(super) para_pct: Option<i32>,
    /// The paragraph border side the border words describe (`t`, `l`, `b`, `r`, `h` between,
    /// `x` all four, `v` the bar border, which has no model value), and the borders by side
    /// (top, left, bottom, right, between) with their `\brdrcf` colour indexes.
    pub(super) para_side: Option<char>,
    pub(super) para_borders: [Option<wordcraft_doc::props::Border>; 5],
    pub(super) para_border_colors: [Option<i32>; 5],
    /// A table style's row formatting (after `\tsrowd`).
    pub(super) table: TableDef,
}

/// The row formatting a table style's stylesheet entry sets (RTF 1.9.1, "Table styles"): its
/// borders (`\trbrdr*`), shading (`\trcbpat`) and cell padding (`\trpadd*`, `\trpaddf*`).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TableDef {
    /// The border side (`t`, `l`, `b`, `r`, `h`, `v`) the border words describe.
    side: Option<char>,
    borders: wordcraft_doc::props::Borders,
    fill: Option<wordcraft_doc::Rgb>,
    /// Cell padding by side (left, top, bottom, right): the value and its unit (3: twips).
    pad: [(Option<i32>, Option<i32>); 4],
}

impl TableDef {
    fn slot(&mut self) -> Option<&mut Option<wordcraft_doc::props::Border>> {
        let b = &mut self.borders;
        Some(match self.side? {
            't' => &mut b.top,
            'l' => &mut b.left,
            'b' => &mut b.bottom,
            'r' => &mut b.right,
            'h' => &mut b.between,
            'v' => &mut b.inside_v,
            _ => return None,
        })
    }

    /// The table style parts these words give, `None` when they give none.
    fn parts(&self) -> Option<wordcraft_doc::styles::TableStyleParts> {
        let borders = [self.borders.top, self.borders.left, self.borders.bottom, self.borders.right, self.borders.between, self.borders.inside_v]
            .iter()
            .any(Option::is_some)
            .then_some(self.borders);
        // Only twips count; another unit (or none) leaves the side unset.
        let twips = self.pad.map(|(v, unit)| v.filter(|_| unit == Some(3)).map(|v| (v as f32 / 20.0).clamp(0.0, 1584.0)));
        let [l, t, b, r] = twips;
        let cell_margins = twips.iter().any(Option::is_some).then(|| [t, l, b, r].map(|v| v.unwrap_or(0.0)));
        (borders.is_some() || self.fill.is_some() || cell_margins.is_some()).then(|| wordcraft_doc::styles::TableStyleParts {
            borders,
            fill: self.fill,
            cell_margins,
            ..Default::default()
        })
    }
}

/// Clear from `f` the formatting a paragraph of kind `kind` gets from its style, whose
/// formatting is `base`: a heading's or title's size and bold when they equal the style's, a
/// quote's italic, a code run's font when it's the style's (and, as before, its size and bold).
pub(super) fn drop_style_formatting(kind: Kind, base: &Fmt, f: &mut Fmt) {
    match kind {
        Kind::Heading(_) | Kind::Title => {
            if f.size.is_some() && f.size == base.size {
                f.size = None;
            }
            if base.bold {
                f.bold = false;
            }
        }
        Kind::Code => {
            f.size = None;
            f.bold = false;
            let same_font = match (&f.font, &base.font) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                (a, b) => a.is_none() && b.is_none(),
            };
            if f.code == base.code && same_font {
                f.code = false;
                f.font = None;
            }
        }
        Kind::Quote => {
            if base.italic {
                f.italic = false;
            }
        }
        Kind::Normal | Kind::Rule => {}
    }
}

/// A character or paragraph formatting word: what old formatting (`\oldcprops`,
/// `\oldpprops`) may hold.
pub(super) fn is_format_word(w: &str) -> bool {
    matches!(
        w,
        "plain"
            | "b"
            | "i"
            | "ulnone"
            | "strike"
            | "striked"
            | "super"
            | "up"
            | "sub"
            | "dn"
            | "nosupersub"
            | "fs"
            | "f"
            | "cf"
            | "highlight"
            | "chcbpat"
            | "chcfpat"
            | "chshdng"
            | "chbrdr"
            | "brsp"
            | "cb"
            | "v"
            | "pard"
            | "ql"
            | "qc"
            | "qr"
            | "qj"
            | "qd"
            | "s"
            | "outlinelevel"
            | "pagebb"
            | "widctlpar"
            | "nowidctlpar"
            | "intbl"
            | "ls"
            | "ilvl"
            | "sb"
            | "sa"
            | "sl"
            | "slmult"
            | "li"
            | "lin"
            | "ri"
            | "rin"
            | "fi"
            | "culi"
            | "curi"
            | "cufi"
            | "brdrw"
            | "brdrcf"
            | "box"
            | "cbpat"
            | "cfpat"
            | "shading"
    ) || (w.starts_with("ul") && w.len() <= 8)
        || w.starts_with("brdr")
}

/// A character formatting word: what an `\oldcprops` group's words are replayed from.
pub(super) fn is_char_format_word(w: &str) -> bool {
    matches!(
        w,
        "plain"
            | "b"
            | "i"
            | "strike"
            | "striked"
            | "super"
            | "up"
            | "sub"
            | "dn"
            | "nosupersub"
            | "fs"
            | "f"
            | "cf"
            | "highlight"
            | "chcbpat"
            | "chcfpat"
            | "chshdng"
            | "chbrdr"
            | "brsp"
            | "cb"
            | "v"
    ) || (w.starts_with("ul") && w.len() <= 8)
        || w.starts_with("brdr")
}

/// The spacing and indents in points of the paragraph formatting `st`.
fn spacing(st: &St) -> model::Spacing {
    use wordcraft_doc::props::LineSpacing;
    let pt = |v: i32| (v as f32 / 20.0).clamp(-1584.0, 1584.0);
    let gap = |v: Option<i32>| v.map(|v| pt(v).max(0.0));
    let sl = st.sl;
    let line = if sl == 0 {
        None
    } else if sl < 0 {
        Some(LineSpacing::Exactly(pt(sl.saturating_neg()).max(1.0)))
    } else if st.slmult {
        // \sl240\slmult1 is single spacing.
        Some(LineSpacing::Multiple((sl as f32 / 240.0).clamp(0.06, 132.0)))
    } else {
        Some(LineSpacing::AtLeast(pt(sl)))
    };
    model::Spacing {
        before: gap(st.sb),
        after: gap(st.sa),
        line,
        left: st.li.map(pt),
        right: st.ri.map(pt),
        first: st.fi.map(pt),
        ..Default::default()
    }
}

/// Text in one of the document's own styles shows only the formatting its own words state, as
/// Word opens the file (RTF 1.9.1, "Style Sheet": an entry defines its style; a paragraph repeats
/// the style's formatting). Whatever the text does not have of its style's look becomes direct
/// formatting over the style; the style itself and its definition stay as read.
impl Reader {
    /// Whether WordCraft wrote the file: its `{\*\generator …}` names it.
    fn wordcraft_file(&self) -> bool {
        self.generator.as_deref().is_some_and(|g| g.trim_start().starts_with("WordCraft"))
    }

    /// Lays the paragraph's and its runs' own words over their styles: a property a run turns
    /// off stays off (every file); in a file WordCraft did not write, a property the RTF state
    /// does not hold has RTF's default (not bold…, 12 pt, the `\deff` font, automatic colour; no
    /// spacing, indents or borders, left aligned, single spaced). Only where the style sets
    /// another value. A paragraph in a heading, title, quote or code style keeps its kind's
    /// reading; its runs only lose what they stated off.
    pub(super) fn lay_own_words(&self, p: &mut Para) {
        let in_kind = self.style_kind(&self.st).is_some();
        let own = if in_kind { None } else { self.own_para_style(self.st.style).map(|_| self.def_props(&self.file_style(self.st.style))) };
        let all = !self.wordcraft_file();
        let (para, para_chr) = own.unzip();
        if all && let Some(s) = &para {
            unstated_para(p, s);
        }
        let para_chr = para_chr.unwrap_or_default();
        let deff = self.fonts.get(&self.deff).filter(|n| !model::is_mono(n)).cloned();
        let look = StateLook { all, deff };
        // Each character style is resolved once per paragraph.
        let mut chars: HashMap<String, CharProps> = HashMap::new();
        for i in &mut p.inlines {
            let (super::Inline::Text(_, f) | super::Inline::Field { fmt: f, .. }) = i else { continue };
            {
                let s = self.run_style(&para_chr, f, &mut chars, in_kind);
                look.apply(f, s);
            }
            if let Some(c) = f.fmt_change.as_deref_mut() {
                let s = self.run_style(&para_chr, &c.old, &mut chars, in_kind);
                look.apply(&mut c.old, s);
            }
        }
    }

    /// The style formatting of a run: its paragraph style's (`para`), with its own character
    /// style's over it (none in a paragraph of a built-in kind).
    fn run_style<'a>(&self, para: &'a CharProps, f: &Fmt, chars: &'a mut HashMap<String, CharProps>, in_kind: bool) -> &'a CharProps {
        let id = f.extra.as_deref().and_then(|x| x.style.as_deref()).filter(|_| !in_kind);
        let Some(key) = id.and_then(|id| self.style_ids().own_chars.get(id).copied()) else { return para };
        let id = id.unwrap_or_default();
        if !chars.contains_key(id) {
            let (_, top) = self.def_props(&self.file_style(key));
            chars.insert(id.to_string(), over(para, &top));
        }
        chars.get(id).unwrap_or(para)
    }
}

/// The character properties a run's style resolution looks at: `top`'s, else `base`'s.
fn over(base: &CharProps, top: &CharProps) -> CharProps {
    CharProps {
        bold: top.bold.or(base.bold),
        italic: top.italic.or(base.italic),
        underline: top.underline.or(base.underline),
        strike: top.strike.or(base.strike),
        double_strike: top.double_strike.or(base.double_strike),
        caps: top.caps.or(base.caps),
        small_caps: top.small_caps.or(base.small_caps),
        hidden: top.hidden.or(base.hidden),
        vert_align: top.vert_align.or(base.vert_align),
        highlight: top.highlight.or(base.highlight),
        color: top.color.or(base.color),
        size: top.size.or(base.size),
        font: top.font.clone().or_else(|| base.font.clone()),
        spacing: top.spacing.or(base.spacing),
        ..Default::default()
    }
}

/// The paragraph properties the RTF state does not hold, where the paragraph's style sets
/// another value: RTF's defaults, as direct formatting. Shading and tab stops have no "none
/// over the style" value and stay as read.
fn unstated_para(p: &mut Para, s: &ParaProps) {
    use wordcraft_doc::props::LineSpacing;
    let zero = |have: &mut Option<f32>, style: Option<f32>| {
        if have.is_none() && style.is_some_and(|v| v != 0.0) {
            *have = Some(0.0);
        }
    };
    zero(&mut p.spacing.before, s.space_before);
    zero(&mut p.spacing.after, s.space_after);
    zero(&mut p.spacing.left, s.indent_left);
    zero(&mut p.spacing.right, s.indent_right);
    zero(&mut p.spacing.first, s.indent_first);
    if p.align.is_none() && s.align.is_some_and(|a| a != Align::Left) {
        p.align = Some(Align::Left);
    }
    let single = LineSpacing::Multiple(1.0);
    if p.spacing.line.is_none() && s.line_spacing.is_some_and(|l| l != single) {
        p.spacing.line = Some(single);
    }
    if p.borders.is_none() && s.borders.is_some_and(|b| b.any_visible()) {
        p.borders = Some(Default::default());
    }
}

/// How a run's formatting is laid over its style: `all` in a file WordCraft did not write (the
/// state is the whole look), else only what the run stated off; `deff` the default font.
struct StateLook {
    all: bool,
    deff: Option<super::Arc<str>>,
}

impl StateLook {
    /// `f` with direct "off" formatting where its style `s` turns a property on that the text
    /// stated off (or, with `all`, does not have); every other stated-off mark is dropped.
    fn apply(&self, f: &mut Fmt, s: &CharProps) {
        use wordcraft_doc::props::{Highlight, Underline, VertAlign};
        let mut x = f.extra.take().map(|b| *b).unwrap_or_default();
        let off = |on: bool, stated: bool| on && (self.all || stated);
        let stated = x.bold == Some(false);
        x.bold = (!f.bold && off(s.bold == Some(true), stated)).then_some(false);
        let stated = x.italic == Some(false);
        x.italic = (!f.italic && off(s.italic == Some(true), stated)).then_some(false);
        if !f.underline {
            let stated = x.underline == Some(Underline::None);
            x.underline = off(s.underline.is_some_and(|u| u != Underline::None), stated).then_some(Underline::None);
        }
        // Strikethrough off takes a double one with it (see `model::to_doc::char_props`).
        let (single, double) = (x.strike == Some(false), x.double_strike == Some(false));
        x.strike = None;
        if double {
            x.double_strike = None;
        }
        if !f.strike && x.double_strike.is_none() && (off(s.strike == Some(true), single) || off(s.double_strike == Some(true), double)) {
            x.strike = Some(false);
        }
        if x.caps != Some(true) {
            let stated = x.caps == Some(false);
            x.caps = off(s.caps == Some(true), stated).then_some(false);
        }
        if x.small_caps != Some(true) {
            let stated = x.small_caps == Some(false);
            x.small_caps = off(s.small_caps == Some(true), stated).then_some(false);
        }
        if !f.hidden {
            let stated = x.hidden == Some(false);
            x.hidden = off(s.hidden == Some(true), stated).then_some(false);
        }
        if !f.sup && !f.sub {
            let stated = x.vert_align == Some(VertAlign::Baseline);
            x.vert_align = off(s.vert_align.is_some_and(|v| v != VertAlign::Baseline), stated).then_some(VertAlign::Baseline);
        }
        if x.highlight.is_none_or(|h| h == Highlight::None) {
            let stated = x.highlight == Some(Highlight::None);
            x.highlight = off(s.highlight.is_some_and(|h| h != Highlight::None), stated).then_some(Highlight::None);
        }
        let stated = x.color == Some(TextColor::Auto);
        x.color = (f.color.is_none() && off(matches!(s.color, Some(TextColor::Rgb(_))), stated)).then_some(TextColor::Auto);
        if self.all {
            if f.size.is_none() && s.size.is_some_and(|v| v != 12.0) {
                f.size = Some(12.0);
            }
            if f.font.is_none()
                && !f.code
                && let Some(d) = &self.deff
                && s.font.as_deref().is_some_and(|n| n != &**d)
            {
                f.font = Some(d.clone());
            }
            if x.spacing.is_none() && s.spacing.is_some_and(|v| v != 0.0) {
                x.spacing = Some(0.0);
            }
        }
        f.extra = (x != CharProps::default()).then(|| Box::new(x));
    }
}

/// The character properties `off` says the text's own words turned off, marked on `f` as "off"
/// values for [`Reader::lay_own_words`], where `f` does not have them on.
pub(super) fn mark_stated_off(f: &mut Fmt, off: super::state::StatedOff) {
    use wordcraft_doc::props::{Highlight, Underline, VertAlign};
    if off == super::state::StatedOff::default() {
        return;
    }
    let mut x = f.extra.take().map(|b| *b).unwrap_or_default();
    if off.bold && !f.bold {
        x.bold = Some(false);
    }
    if off.italic && !f.italic {
        x.italic = Some(false);
    }
    if off.underline && !f.underline && x.underline.is_none() {
        x.underline = Some(Underline::None);
    }
    if off.strike && !f.strike {
        x.strike = Some(false);
    }
    if off.double_strike && x.double_strike.is_none() {
        x.double_strike = Some(false);
    }
    if off.caps && x.caps.is_none() {
        x.caps = Some(false);
    }
    if off.small_caps && x.small_caps.is_none() {
        x.small_caps = Some(false);
    }
    if off.hidden && !f.hidden {
        x.hidden = Some(false);
    }
    if off.script && !f.sup && !f.sub {
        x.vert_align = Some(VertAlign::Baseline);
    }
    if off.highlight && x.highlight.is_none() {
        x.highlight = Some(Highlight::None);
    }
    if off.color && f.color.is_none() {
        x.color = Some(TextColor::Auto);
    }
    f.extra = (x != CharProps::default()).then(|| Box::new(x));
}

#[cfg(test)]
mod legacy_numbering_tests {
    use wordcraft_doc::{Block, numbering::Counters};

    fn labels(body: &str) -> Vec<Option<String>> {
        let doc = crate::rtf::import(format!("{{\\rtf1 {body}}}").as_bytes()).unwrap();
        let mut counters = Counters::default();
        doc.body
            .iter()
            .filter_map(|block| match &**block {
                Block::Para(p) => Some(p.props.numbering.and_then(|n| counters.next_label(&doc.numbering, n.num, n.level)).map(|(text, _)| text)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn outline_parameter_and_default_start() {
        let doc = crate::rtf::import(br"{\rtf1\pard{\*\pn\pnlvl3\pndec{\pntxta .}}a\par}").unwrap();
        let Block::Para(p) = &**doc.body.first().unwrap() else { panic!("paragraph") };
        assert_eq!(p.props.numbering.unwrap().level, 2);
        assert_eq!(labels(r"\pard{\*\pn\pnlvlbody\pndec{\pntxta .}}a\par"), [Some("1.".into())]);
    }

    #[test]
    fn unicode_label_and_paragraph_indents() {
        let doc = crate::rtf::import(br"{\rtf1\pard\li240\fi-120{\*\pn\pnlvlblt{\pntxtb\u8226?}}a\par}").unwrap();
        let Block::Para(p) = &**doc.body.first().unwrap() else { panic!("paragraph") };
        assert_eq!(p.props.indent_left, Some(12.0));
        assert_eq!(p.props.indent_first, Some(-6.0));
        let n = p.props.numbering.unwrap();
        let (text, level) = Counters::default().next_label(&doc.numbering, n.num, n.level).unwrap();
        assert_eq!(text, "•");
        assert_eq!((level.indent, level.hanging), (0.0, 0.0));
    }

    #[test]
    fn bullet_continuation_and_format_change() {
        assert_eq!(
            labels(r"\pard{\*\pn\pnlvlblt{\pntxtb *}}a\par\pard{\*\pn\pnlvlcont}b\par\pard{\*\pn\pnlvlblt{\pntxtb *}}c\par"),
            [Some("*".into()), None, Some("*".into())]
        );
        assert_eq!(
            labels(r"\pard{\*\pn\pnlvlbody\pndec{\pntxta .}}a\par\pard{\*\pn\pnlvlbody\pnucrm{\pntxta .}}b\par"),
            [Some("1.".into()), Some("I.".into())]
        );
    }
}

#[cfg(test)]
mod stated_off_tests {
    use wordcraft_doc::Document;
    use wordcraft_doc::props::VertAlign;
    use wordcraft_doc::resolve::ResolvedChar;

    /// A WordCraft-written file (only words stated off count) or one with no generator (the
    /// state is the whole look), with style `\s20` and character style `\cs10` from `sheet`.
    fn open(wordcraft: bool, sheet: &str, body: &str) -> Document {
        let generator = if wordcraft { r"{\*\generator WordCraft;}" } else { "" };
        let head = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}{\f1 Arial;}}{\colortbl;\red255\green0\blue0;}";
        let rtf = format!(r"{head}{generator}{{\*\revtbl {{Unknown;}}{{Alice;}}}}{{\stylesheet{{\s0 Normal;}}{sheet}}}{body}}}");
        super::super::import(rtf.as_bytes()).unwrap()
    }

    fn chars(d: &Document, text: &str) -> ResolvedChar {
        let p = d.body.iter().filter_map(|b| b.as_para()).find(|p| p.text.contains(text)).unwrap();
        let at = p.text.find(text).unwrap();
        let (_, c) = p.run_ranges().find(|(r, _)| r.contains(&at)).unwrap();
        d.styles.resolve_char(p.props.style.as_deref(), c)
    }

    #[test]
    fn double_strikethrough_off_in_a_double_struck_style() {
        let d = open(true, r"{\s20\striked1 Twice;}", r"\pard\plain\s20\striked1 on {\striked0 offrun} on\par");
        assert!(chars(&d, "on").double_strike);
        let off = chars(&d, "offrun");
        assert!(!off.double_strike && !off.strike);
    }

    #[test]
    fn hidden_off_in_a_hidden_style_is_shown() {
        let d = open(true, r"{\s20\v Secret;}", r"\pard\plain\s20\v on {\v0 offrun} on\par");
        assert!(chars(&d, "on").hidden);
        assert!(!chars(&d, "offrun").hidden);
    }

    #[test]
    fn subscript_off_in_a_subscript_style_is_on_the_baseline() {
        let d = open(true, r"{\s20\sub Low;}", r"\pard\plain\s20\sub on {\sub0 offrun} on\par");
        assert_eq!(chars(&d, "on").vert_align, VertAlign::Subscript);
        assert_eq!(chars(&d, "offrun").vert_align, VertAlign::Baseline);
    }

    #[test]
    fn character_spacing_not_stated_is_normal_outside_wordcraft_files() {
        let sheet = r"{\s20\expnd40\expndtw200 Wide;}";
        let body = r"\pard\plain\s20 Unstated\par\pard\plain\s20\expnd40\expndtw200 Stated\par";
        let d = open(false, sheet, body);
        assert_eq!(chars(&d, "Unstated").spacing, 0.0);
        assert_eq!(chars(&d, "Stated").spacing, 10.0);
        let d = open(true, sheet, body);
        assert_eq!(chars(&d, "Unstated").spacing, 10.0);
    }

    #[test]
    fn a_tracked_changes_old_formatting_is_laid_over_the_style_too() {
        // Bold now, not bold before the change, in a bold style.
        let d = open(true, r"{\s20\b Loud;}", r"\pard\plain\s20\b on {\crauth1\crdate-1480361314{\*\oldcprops \b0}changed} on\par");
        let p = d.body.iter().filter_map(|b| b.as_para()).find(|p| p.text.contains("changed")).unwrap();
        let at = p.text.find("changed").unwrap();
        let c = p.props_of_char(p.text[..at].chars().count());
        assert!(chars(&d, "changed").bold);
        let old = &c.fmt_change.as_deref().expect("a tracked formatting change").old;
        assert_eq!(old.bold, Some(false));
        assert!(!d.styles.resolve_char(p.props.style.as_deref(), old).bold);
    }

    #[test]
    fn stated_off_outside_any_style_adds_nothing() {
        let d = open(true, "", r"\pard\plain {\b0\i0\ulnone\strike0\caps0\v0\nosupersub\highlight0\cf0 plainrun} end\par");
        let p = d.body.iter().filter_map(|b| b.as_para()).find(|p| p.text.contains("plainrun")).unwrap();
        assert_eq!(p.runs.len(), 1, "{:?}", p.runs);
        assert!(p.runs.iter().all(|r| r.props == Default::default()), "{:?}", p.runs);
    }
}
