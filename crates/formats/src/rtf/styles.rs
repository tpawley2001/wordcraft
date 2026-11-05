use super::export::HEAD_SIZES;
use super::state::{Pn, PnKind, St};
use super::{Fmt, Kind, ListInfo, Para, Reader, model};
use model::Label;

impl Reader {
    pub(super) fn commit_style(&mut self) {
        let name = std::mem::take(&mut self.style_name).trim().trim_end_matches(';').trim().to_string();
        let def = std::mem::take(&mut self.style_def_cur);
        if self.styles.len() < 4096 {
            self.styles.insert(self.style_cur, name);
            self.style_defs.insert(self.style_cur, def);
        }
        // The next entry is style 0 (Normal) unless it names a number.
        self.style_cur = 0;
    }

    /// The character formatting style `id` gives its runs in this file: its stylesheet entry's,
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
            cur = d.based_on;
        }
        out
    }

    /// The formatting a paragraph of kind `kind` in style `id` gives its runs. A style whose
    /// entry and `\sbasedon` chain set no character formatting (a bare `{\s2 heading 2;}`) has
    /// WordCraft's own (`style_fmt`); otherwise what the chain doesn't set is RTF's default (not
    /// bold, not italic, 12 pt). The font alone falls back per property: WordCraft writes the code
    /// font on each code paragraph, not in its Code entry.
    pub(super) fn style_base(&self, kind: Kind, id: i32) -> Fmt {
        let mut f = style_fmt(kind);
        let d = self.file_style(id);
        if d.bold.is_some() || d.italic.is_some() || d.size.is_some() || d.font.is_some() {
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

    /// The paragraph kind `st`'s style names, if it names one.
    pub(super) fn style_kind(&self, st: &St) -> Option<Kind> {
        let name = self.styles.get(&st.style).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
        if let Some(n) = name.strip_prefix("heading").and_then(|r| r.trim().parse::<u8>().ok()) {
            return Some(Kind::Heading(n.clamp(1, 6)));
        }
        if st.style != 0 {
            match name.as_str() {
                "title" => return Some(Kind::Title),
                "quote" | "intense quote" => return Some(Kind::Quote),
                "code" | "html preformatted" | "plain text" => return Some(Kind::Code),
                _ => {}
            }
        }
        None
    }

    fn para_kind(&self, st: &St) -> Kind {
        if let Some(k) = self.style_kind(st) {
            return k;
        }
        if let Some(o) = st.outline.filter(|o| *o < 9) {
            return Kind::Heading((o + 1).min(6));
        }
        Kind::Normal
    }

    /// The paragraph formatting of `st` (kind, alignment, page break, spacing, borders, list).
    pub(super) fn para_props(&self, st: &St) -> Para {
        let mut p = Para::new(self.para_kind(st));
        p.align = st.align;
        p.page_break = st.pagebb;
        p.spacing = spacing(st);
        p.borders = st.borders.any_visible().then_some(st.borders);
        if !st.tabs.is_empty() {
            p.extra = Some(Box::new(wordcraft_doc::props::ParaProps { tabs: Some(st.tabs.clone()), ..Default::default() }));
        }
        if let Some(ls) = st.ls {
            let list = self.overrides.get(&ls).and_then(|id| self.lists.get(id));
            let lv = st.ilvl.min(8);
            let ordered = list.and_then(|l| l.get(lv as usize).copied()).unwrap_or(false);
            p.list = Some(ListInfo { ordered, level: lv, start: 1, ..Default::default() });
        } else if let Some(pn) = st.pn.as_deref() {
            p.list = self.pn_list(pn);
        }
        p
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
            indent: None,
            hanging: None,
            align: None,
        };
        let font = pn.font.and_then(|f| self.fonts.get(&f)).and_then(|f| model::LabelFont::new(f));
        Some(ListInfo { ordered, level, start: pn.start.unwrap_or(1), list: 0, label: Some(label), font })
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

/// A `\stylesheet` entry's character formatting; `None` where the entry doesn't set it.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StyleDef {
    pub(super) based_on: Option<i32>,
    pub(super) bold: Option<bool>,
    pub(super) italic: Option<bool>,
    pub(super) size: Option<f32>,
    /// A `\fonttbl` index, named when it's used (the table may follow).
    pub(super) font: Option<i32>,
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
            | "brdrw"
            | "brdrcf"
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
            | "cb"
            | "v"
    ) || (w.starts_with("ul") && w.len() <= 8)
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
        let Block::Para(p) = &**doc.body.iter().next().unwrap() else { panic!("paragraph") };
        assert_eq!(p.props.numbering.unwrap().level, 2);
        assert_eq!(labels(r"\pard{\*\pn\pnlvlbody\pndec{\pntxta .}}a\par"), [Some("1.".into())]);
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
