use wordcraft_doc::SectionProps;
use wordcraft_doc::para::{CodeSplit, code_hash};
use wordcraft_doc::props::ParaProps;

use super::{
    ChangeAttrs, CharProps, InlineObject, Paragraph, PartRels, Piece, PropChange, RevisionKind, W, Writer, open_change, ppr_inner, rpr, rpr_inner, rt,
};

impl Writer<'_> {
    pub(super) fn para(&mut self, w: &mut W, p: &Paragraph, rels: &mut PartRels, top: bool, depth: usize) {
        // Which content control markers are written inline (set by `blocks`).
        let inline = std::mem::take(&mut self.inline_controls);
        // A drop cap is a separate framed paragraph holding the first character.
        if let Some(lines) = p.props.drop_cap.filter(|l| *l > 0)
            && let Some(first) = p.text.chars().next().filter(|c| !matches!(*c, wordcraft_doc::para::OBJ | '\t' | '\n' | '\u{c}' | '\u{e}'))
            && p.text.len() > first.len_utf8()
        {
            let mut head = p.clone();
            let tail = head.split_off(first.len_utf8()).ok();
            if let Some(mut tail) = tail {
                head.props.drop_cap = Some(lines);
                if p.props.drop_cap_margin == Some(true) {
                    head.props.indent_first = p.props.drop_cap_indent_first;
                    head.props.indent_first_chars = None;
                }
                // The letter's own font goes on its run (`w:rFonts`).
                if let Some(f) = p.props.drop_cap_font.as_ref().filter(|f| !f.trim().is_empty()) {
                    for r in &mut head.runs {
                        r.props.font = Some(f.clone());
                    }
                }
                head.section = None;
                self.para_inner(w, &head, rels, false, depth, true);
                tail.props.drop_cap = None;
                tail.section = p.section.clone();
                self.inline_controls = inline.iter().filter_map(|o| o.checked_sub(first.len_utf8())).collect();
                self.para_inner(w, &tail, rels, top, depth, false);
                return;
            }
        }
        self.inline_controls = inline;
        self.para_inner(w, p, rels, top, depth, false);
    }

    fn write_mark_rpr(&mut self, w: &mut W, mark: &CharProps, mark_revs: &[(u32, &str)]) {
        w.open("w:rPr", &[]);
        for (idx, tag) in mark_revs {
            let (id, author, date) = self.rev_attrs(*idx);
            if date.is_empty() {
                w.empty(tag, &[("w:id", &id), ("w:author", &author)]);
            } else {
                w.empty(tag, &[("w:id", &id), ("w:author", &author), ("w:date", &date)]);
            }
        }
        rpr_inner(w, mark);
        if let Some(ch) = mark.fmt_change.as_deref() {
            self.rpr_change(w, ch);
        }
        w.close("w:rPr");
    }

    fn para_inner(&mut self, w: &mut W, p: &Paragraph, rels: &mut PartRels, top: bool, depth: usize, framed: bool) {
        let para_id = self.take_para_id(p);
        match &para_id {
            Some(id) => w.open("w:p", &[("w14:paraId", id), ("w14:textId", "77777777")]),
            None => w.open("w:p", &[]),
        }
        let mut pp = p.props.clone();
        if !framed {
            pp.drop_cap = None;
        }
        let section = if top { p.section.as_deref() } else { None };
        // A field code that ran over several paragraphs is saved in them again: this paragraph
        // is the first, with its own space after.
        self.code_splits = if framed { Vec::new() } else { self.plan_code_splits(p) };
        if !self.code_splits.is_empty()
            && let Some(cp) = p.code_paras.as_deref()
        {
            pp.space_after = cp.first_after;
            pp.space_after_auto = cp.first_after_auto;
        }
        self.write_ppr(w, &pp, &p.mark, section, rels, framed);
        // A note's first paragraph starts with its reference mark. When the paragraph holds the
        // note's reference to itself (notes read from a file, or made by references.footnote),
        // the mark is written there instead, with that reference's formatting.
        if !self.holds_own_ref(p)
            && let Some(mark) = self.pending_mark.take()
        {
            w.raw("<w:r><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr>");
            w.empty(mark, &[]);
            w.raw("</w:r>");
        }
        // A header's watermark, in its first top-level paragraph (not a table cell or text box).
        if depth == 0
            && let Some(run) = self.pending_watermark.take()
        {
            w.raw(&run);
        }
        // Taken before the content so a nested story (a text box in an entry) can't claim them.
        if std::mem::take(&mut self.toc_begin_here)
            && let Some((instr, locked, dirty, props)) = self.toc_field.take()
        {
            self.rev_open(w, &props);
            self.field_start(w, &instr, locked, dirty, "", &props);
            self.rev_close(w, &props);
        }
        let toc_end = std::mem::take(&mut self.toc_end_here);
        self.para_content(w, p, rels, depth);
        if toc_end {
            w.raw(r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#);
        }
        // Move ranges stay open across a moved paragraph mark; others end with the paragraph.
        let mark_moves = self.moves_of(&p.mark);
        self.move_ranges(w, &mark_moves);
        if top {
            w.set_body_comment_anchor();
        }
        w.close("w:p");
    }

    /// A paragraph's `w:pPr`: its properties, the mark's formatting and its section.
    fn write_ppr(&mut self, w: &mut W, pp: &ParaProps, mark: &CharProps, section: Option<&SectionProps>, rels: &mut PartRels, framed: bool) {
        // A tracked paragraph mark: `w:ins` / `w:del` first in the mark's `w:rPr` (CT_ParaRPr).
        let mark_revs: Vec<(u32, &str)> = [(mark.ins, "w:ins"), (mark.del, "w:del")]
            .into_iter()
            .filter_map(|(idx, tag)| idx.filter(|i| self.rev_kind(Some(*i)).is_some_and(|k| k != RevisionKind::Format)).map(|i| (i, tag)))
            .map(|(i, tag)| (i, self.rev_tag(i, tag)))
            .collect();
        let has_mark = crate::write::props::has_rpr(mark) || !mark_revs.is_empty() || mark.fmt_change.is_some();
        if pp.is_empty() && !has_mark && section.is_none() {
            return;
        }
        w.open("w:pPr", &[]);
        let num_change = pp.num_change.as_ref().map(|c| {
            let mut a = self.change_attrs(c.rev);
            a.push(("w:original", c.original.clone()));
            a
        });
        ppr_inner(w, pp, framed, num_change.as_ref());
        if has_mark {
            self.write_mark_rpr(w, mark, &mark_revs);
        }
        if let Some(s) = section {
            self.sectpr(w, s, rels);
        }
        // The paragraph properties before a tracked change come last (§17.13.5.29).
        if let Some(ch) = &pp.fmt_change {
            let a = self.change_attrs(ch.rev);
            open_change(w, "w:pPrChange", &a);
            w.open("w:pPr", &[]);
            ppr_inner(w, &ch.old, false, None);
            w.close("w:pPr");
            w.close("w:pPrChange");
        }
        w.close("w:pPr");
    }

    /// The fields of `p` whose code is saved over several paragraphs (see [`CodeParas`]), with
    /// the `w:pPr` of each paragraph after the first. A field whose code changed since it was
    /// read, or that is written inside a link, a tracked change, a content control or a
    /// direction element (none of which may contain a paragraph end), is written in one.
    fn plan_code_splits(&mut self, p: &Paragraph) -> Vec<(usize, Vec<(usize, String)>)> {
        let Some(cp) = p.code_paras.as_deref() else { return Vec::new() };
        if !self.inline_controls.is_empty() || p.text.chars().any(|c| ('\u{202A}'..='\u{202E}').contains(&c)) {
            return Vec::new();
        }
        // (field offset, split) in order: each split goes to the next field with its code.
        let mut found: Vec<(usize, &CodeSplit)> = Vec::new();
        let mut splits = cp.fields.iter().peekable();
        let mut k = 0usize;
        for (off, c) in p.text.char_indices() {
            if c != wordcraft_doc::para::OBJ {
                continue;
            }
            let o = p.objects.get(k);
            k += 1;
            let (Some(InlineObject::Field { instr, .. }), Some(split)) = (o, splits.peek()) else { continue };
            if code_hash(instr) != split.code {
                continue;
            }
            let props = p.props_of_char(off);
            let plain = props.link.is_none() && props.ins.is_none() && props.del.is_none();
            if plain && split.parts(instr).is_some() && wordcraft_doc::eq::Enclosure::parse(instr).is_none() && !super::toc::is_toc(instr) {
                found.push((off, split));
            }
            splits.next();
        }
        let last = found.len().saturating_sub(1);
        let mut out = Vec::with_capacity(found.len());
        for (i, (off, split)) in found.into_iter().enumerate() {
            let n = split.marks.len().saturating_sub(1);
            let mut marks = Vec::with_capacity(split.marks.len());
            for (j, m) in split.marks.iter().enumerate() {
                let mut pp = m.props.clone();
                pp.drop_cap = None;
                // The paragraph mark that ends the line has the paragraph's space after.
                if i == last && j == n {
                    pp.space_after = p.props.space_after;
                    pp.space_after_auto = p.props.space_after_auto;
                }
                let mut t = W::default();
                let mut rels = PartRels::default();
                self.write_ppr(&mut t, &pp, &m.mark, None, &mut rels, false);
                marks.push((m.at, t.s));
            }
            out.push((off, marks));
        }
        out
    }

    fn para_content(&mut self, w: &mut W, p: &Paragraph, rels: &mut PartRels, depth: usize) {
        // Taken first: a text box in this paragraph writes paragraphs of its own.
        let inline = std::mem::take(&mut self.inline_controls);
        let mut code_splits = std::mem::take(&mut self.code_splits);
        // Split into pieces with their props.
        let mut pieces: Vec<(Piece, &CharProps)> = Vec::new();
        let mut k = 0usize;
        // Runs of mixed direction are split so Word orders them as WordCraft does (#1543).
        let rtl_para = self.doc.styles.resolve_para(&p.props).bidi;
        let styles = &self.doc.styles;
        let para_style = p.props.style.as_deref();
        let para_lang = crate::write::bidi::para_inherits_lang_bidi(styles, para_style);
        let spans = crate::write::bidi::direction_spans(&p.text, rtl_para, p.run_ranges(), |c| {
            para_lang || crate::write::bidi::char_style_has_lang_bidi(styles, c)
        });
        // Embedding and override characters written as `w:dir` / `w:bdo`.
        let dirs = crate::write::bidi::direction_elements(p);
        for (range, props) in &spans {
            let props: &CharProps = props;
            let range = range.clone();
            let base = range.start;
            let Some(text) = p.text.get(range) else { continue };
            let sym_font = props.sym_font.as_deref().filter(|f| !f.trim().is_empty());
            let mut buf = String::new();
            for (ci, c) in text.char_indices() {
                let special = match c {
                    '\u{202A}'..='\u{202E}' if dirs.contains_key(&base.saturating_add(ci)) => {
                        dirs.get(&base.saturating_add(ci)).map(|d| Piece::Dir(*d))
                    }
                    '\t' => Some(Piece::Tab),
                    '\n' => Some(Piece::Br(None)),
                    '\u{000C}' => Some(Piece::Br(Some("page"))),
                    '\u{000E}' => Some(Piece::Br(Some("column"))),
                    '\u{2011}' => Some(Piece::NbHyphen),
                    '\u{00AD}' => Some(Piece::SoftHyphen),
                    wordcraft_doc::para::OBJ => {
                        let o = p.objects.get(k);
                        k += 1;
                        Some(match o {
                            Some(o) => Piece::Obj(o, base + ci),
                            None => Piece::Text(String::new()),
                        })
                    }
                    // `w:char` is a two-byte code (ST_ShortHexNumber) from U+0020, as the reader
                    // takes it; anything else stays text.
                    c if (0x20..=0xFFFF).contains(&u32::from(c)) => sym_font.map(|f| Piece::Sym(f, c)),
                    _ => None,
                };
                match special {
                    Some(sp) => {
                        if !buf.is_empty() {
                            pieces.push((Piece::Text(std::mem::take(&mut buf)), props));
                        }
                        pieces.push((sp, props));
                    }
                    None => buf.push(c),
                }
            }
            if !buf.is_empty() {
                pieces.push((Piece::Text(buf), props));
            }
        }

        // The open `w:hyperlink`: its target and ScreenTip. Neighbouring runs with one target but
        // different tips are separate links.
        let mut open_link: Option<(String, Option<String>)> = None;
        let mut i = 0;
        while i < pieces.len() {
            let Some((piece, props)) = pieces.get(i) else { break };
            let props: &CharProps = props;
            // Content control boundaries: written inline when the plan says so (else they are
            // written around the paragraph, or were left without a partner). A hyperlink never
            // crosses one.
            if let Piece::Obj(o @ (InlineObject::ControlStart { .. } | InlineObject::ControlEnd), off) = piece {
                if inline.binary_search(off).is_ok() {
                    if open_link.take().is_some() {
                        w.close("w:hyperlink");
                    }
                    match o {
                        InlineObject::ControlStart { control } => crate::write::sdt::open(w, control),
                        _ => crate::write::sdt::close(w),
                    }
                }
                i += 1;
                continue;
            }
            // Hyperlink grouping.
            let want = props.link.as_deref().filter(|l| !l.is_empty()).map(|l| (l, props.link_tip.as_deref().filter(|t| !t.is_empty())));
            if want != open_link.as_ref().map(|(l, t)| (l.as_str(), t.as_deref())) {
                if open_link.take().is_some() {
                    w.close("w:hyperlink");
                }
                if let Some((l, tip)) = want {
                    let id;
                    let mut attrs = match l.strip_prefix('#') {
                        Some(anchor) => vec![("w:anchor", anchor)],
                        None => {
                            id = rels.add(rt::HYPERLINK, l, true);
                            vec![("r:id", id.as_str())]
                        }
                    };
                    if let Some(tip) = tip {
                        attrs.push(("w:tooltip", tip));
                    }
                    attrs.push(("w:history", "1"));
                    w.open("w:hyperlink", &attrs);
                    open_link = Some((l.to_string(), tip.map(str::to_string)));
                }
            }
            match piece {
                Piece::Dir(crate::write::bidi::DirElement::Open(tag, val)) => {
                    w.open(tag, &[("w:val", val)]);
                    i += 1;
                }
                Piece::Dir(crate::write::bidi::DirElement::Close(tag)) => {
                    w.close(tag);
                    i += 1;
                }
                Piece::Obj(o, off) => {
                    if let Some(at) = code_splits.iter().position(|(o, _)| o == off) {
                        self.code_marks = code_splits.swap_remove(at).1;
                    }
                    self.object(w, o, props, rels, depth);
                    self.code_marks.clear();
                    i += 1;
                }
                _ => {
                    // Gather simple pieces sharing these props into one run.
                    let mut j = i;
                    while let Some((pc, pr)) = pieces.get(j) {
                        if matches!(pc, Piece::Obj(..) | Piece::Dir(..)) || !(std::ptr::eq(*pr, props) || *pr == props) {
                            break;
                        }
                        j += 1;
                    }
                    let del = self.is_del(props);
                    let moves = self.moves_of(props);
                    self.move_ranges(w, &moves);
                    self.rev_open(w, props);
                    w.open("w:r", &[]);
                    self.rpr(w, props);
                    for (pc, _) in pieces.get(i..j).unwrap_or(&[]) {
                        match pc {
                            Piece::Text(t) if !t.is_empty() => w.leaf(if del { "w:delText" } else { "w:t" }, &[("xml:space", "preserve")], t),
                            Piece::Tab => w.empty("w:tab", &[]),
                            Piece::Br(None) => w.empty("w:br", &[]),
                            Piece::Br(Some(t)) => w.empty("w:br", &[("w:type", t)]),
                            Piece::NbHyphen => w.empty("w:noBreakHyphen", &[]),
                            Piece::SoftHyphen => w.empty("w:softHyphen", &[]),
                            Piece::Sym(font, c) => w.empty("w:sym", &[("w:font", font), ("w:char", &format!("{:04X}", u32::from(*c)))]),
                            _ => {}
                        }
                    }
                    w.close("w:r");
                    self.rev_close(w, props);
                    i = j.max(i + 1);
                }
            }
        }
        if open_link.is_some() {
            w.close("w:hyperlink");
        }
    }

    fn rev_kind(&self, idx: Option<u32>) -> Option<RevisionKind> {
        idx.and_then(|i| self.doc.revisions.get(i as usize)).map(|r| r.kind)
    }

    /// Deleted text goes in `w:delText`; text moved away stays in `w:t` (§17.13.5.22).
    pub(super) fn is_del(&self, props: &CharProps) -> bool {
        props.del.is_some() && self.rev_kind(props.del) != Some(RevisionKind::Format) && self.doc.move_name(props.del).is_none()
    }

    /// `w:ins` / `w:del`, or `w:moveTo` / `w:moveFrom` when revision `idx` is one end of a move.
    fn rev_tag(&self, idx: u32, tag: &'static str) -> &'static str {
        match (self.doc.move_name(Some(idx)).is_some(), tag) {
            (true, "w:ins") => "w:moveTo",
            (true, "w:del") => "w:moveFrom",
            _ => tag,
        }
    }

    pub(super) fn rev_attrs(&mut self, idx: u32) -> (String, String, String) {
        self.next_rev_id += 1;
        let r = self.doc.revisions.get(idx as usize);
        let author = crate::write::annotation_author(self.doc, r.map_or("", |r| r.author.as_str())).to_string();
        // Remove date and time (§17.15.1.69): tracked changes are saved without `w:date`.
        let date = if self.doc.settings.remove_date_time { String::new() } else { r.map(|r| r.date.clone()).unwrap_or_default() };
        (self.next_rev_id.to_string(), author, date)
    }

    /// `w:id`, `w:author` and `w:date` of a tracked change by revision `idx`.
    pub(super) fn change_attrs(&mut self, idx: u32) -> ChangeAttrs {
        let (id, author, date) = self.rev_attrs(idx);
        let mut a = vec![("w:id", id), ("w:author", author)];
        if !date.is_empty() {
            a.push(("w:date", date));
        }
        a
    }

    /// A run's `w:rPr`, with its tracked formatting change last (§17.13.5.31).
    pub(super) fn rpr(&mut self, w: &mut W, c: &CharProps) {
        let Some(ch) = c.fmt_change.as_deref() else { return rpr(w, c) };
        w.open("w:rPr", &[]);
        rpr_inner(w, c);
        self.rpr_change(w, ch);
        w.close("w:rPr");
    }

    fn rpr_change(&mut self, w: &mut W, ch: &PropChange<CharProps>) {
        let a = self.change_attrs(ch.rev);
        open_change(w, "w:rPrChange", &a);
        w.open("w:rPr", &[]);
        rpr_inner(w, &ch.old);
        w.close("w:rPr");
        w.close("w:rPrChange");
    }

    pub(super) fn rev_open(&mut self, w: &mut W, props: &CharProps) {
        for (idx, tag) in [(props.ins, "w:ins"), (props.del, "w:del")] {
            let Some(idx) = idx else { continue };
            if self.rev_kind(Some(idx)) == Some(RevisionKind::Format) {
                continue;
            }
            let tag = self.rev_tag(idx, tag);
            let (id, author, date) = self.rev_attrs(idx);
            if date.is_empty() {
                w.open(tag, &[("w:id", &id), ("w:author", &author)]);
            } else {
                w.open(tag, &[("w:id", &id), ("w:author", &author), ("w:date", &date)]);
            }
        }
    }

    pub(super) fn rev_close(&mut self, w: &mut W, props: &CharProps) {
        for (idx, tag) in [(props.del, "w:del"), (props.ins, "w:ins")] {
            if let Some(i) = idx
                && self.rev_kind(idx) != Some(RevisionKind::Format)
            {
                w.close(self.rev_tag(i, tag));
            }
        }
    }
}
