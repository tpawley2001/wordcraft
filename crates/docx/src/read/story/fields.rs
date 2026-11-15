use super::{CharProps, El, FieldState, InlineObject, Item, MAX_FIELD_DEPTH, PB, Phase, Reader, StoryCtx};
use wordcraft_doc::link::{hyperlink_target, hyperlink_tip};

/// Enclosed characters (`eq \o\ac(○,字)`), with their style from the code's font sizes: Word
/// enlarges the symbol (bigger than the text, which keeps the field's size) or shrinks the text
/// (smaller than the symbol).
fn enclosure(f: &FieldState) -> Option<wordcraft_doc::eq::Enclosure> {
    use wordcraft_doc::eq::{EncloseShape, EncloseStyle, Enclosure};
    let mut e = Enclosure::parse(&f.instr)?;
    let size_at = |at: usize| f.sizes.iter().rev().find(|(start, _)| *start <= at).and_then(|(_, s)| *s);
    let sym = f.instr.char_indices().find(|(_, c)| EncloseShape::from_symbol(&c.to_string()) == Some(e.shape)).map(|(i, _)| i)?;
    let txt = e.text.chars().next().and_then(|c| f.instr.get(sym..)?.find(c).map(|i| i + sym)).unwrap_or(sym);
    let base = f.props.size;
    e.style = match (size_at(sym), size_at(txt)) {
        (Some(s), Some(t)) if s > t + 0.01 && base.is_none_or(|b| t >= b - 0.01) => EncloseStyle::Enlarge,
        (Some(s), None) if base.is_none_or(|b| s > b + 0.01) => EncloseStyle::Enlarge,
        _ => EncloseStyle::Shrink,
    };
    Some(e)
}

/// Fields kept as ranges: those whose result another program writes, with formatting and often
/// several paragraphs (TOCs and citation managers' `ADDIN` fields).
fn is_range_instr(instr: &str) -> bool {
    crate::read::is_toc(instr) || instr.trim_start().get(..5).is_some_and(|k| k.eq_ignore_ascii_case("ADDIN"))
}

impl Reader<'_> {
    // ---- fields ----

    pub(super) fn begin_field(&mut self, sc: &mut StoryCtx, props: CharProps, instr: String, locked: bool, dirty: bool, form: String) {
        if sc.fields.len() >= MAX_FIELD_DEPTH || sc.ignored_begins > 0 {
            sc.ignored_begins += 1;
            return;
        }
        // Fields nested deeper than the cap are ignored (their content flows to the enclosing field).
        sc.fields.push(FieldState {
            instr,
            phase: Phase::Instr,
            spilled: false,
            buf: Vec::new(),
            props,
            locked,
            dirty,
            range: false,
            sizes: Vec::new(),
            form,
            marks: Vec::new(),
            nested: Vec::new(),
            first_after: (None, None),
        });
    }

    /// Whether the field below the innermost one is still collecting its code (a field nested
    /// in another's code contributes text to that code, never content).
    fn parent_in_instr(sc: &StoryCtx) -> bool {
        let n = sc.fields.len();
        n >= 2 && sc.fields.get(n - 2).is_some_and(|p| !p.spilled && !p.range && p.phase == Phase::Instr)
    }

    /// `separate`: the code is complete; a range field starts its result here.
    pub(super) fn separate_field(&mut self, sc: &mut StoryCtx, pb: &mut PB) {
        if sc.ignored_begins > 0 {
            return;
        }
        let nested_in_instr = Self::parent_in_instr(sc);
        let Some(f) = sc.fields.last_mut() else { return };
        if f.range {
            return;
        }
        f.phase = Phase::Result;
        if !f.spilled && !nested_in_instr && is_range_instr(&f.instr) {
            f.range = true;
            let (obj, props) = (InlineObject::FieldStart { instr: f.instr.trim().to_string(), locked: f.locked, dirty: f.dirty }, f.props.clone());
            self.emit_obj(sc, pb, obj, &props);
        }
    }

    pub(super) fn end_field(&mut self, sc: &mut StoryCtx, pb: &mut PB) {
        if sc.ignored_begins > 0 {
            sc.ignored_begins -= 1;
            return;
        }
        let nested_in_instr = Self::parent_in_instr(sc);
        let Some(f) = sc.fields.pop() else { return };
        if f.range {
            self.emit_obj(sc, pb, InlineObject::FieldEnd, &f.props);
            return;
        }
        if f.spilled {
            return;
        }
        if f.phase == Phase::Instr && !nested_in_instr && is_range_instr(&f.instr) {
            // A range field with no result yet.
            self.emit_obj(sc, pb, InlineObject::FieldStart { instr: f.instr.trim().to_string(), locked: f.locked, dirty: f.dirty }, &f.props);
            self.emit_obj(sc, pb, InlineObject::FieldEnd, &f.props);
            return;
        }
        // A `HYPERLINK` field is a link, as a `w:hyperlink` is: its result stays ordinary content
        // carrying the link, so a tab in it reaches its tab stop and the `PAGEREF` in a table of
        // contents entry (Word for the web writes them this way) stays a field of its own (#2174).
        if !nested_in_instr && let Some(target) = hyperlink_target(&f.instr) {
            let tip = hyperlink_tip(&f.instr).map(Box::<str>::from);
            for (it, mut p) in f.buf {
                p.link.get_or_insert_with(|| target.clone());
                if let Some(tip) = &tip {
                    p.link_tip.get_or_insert_with(|| tip.clone());
                }
                match it {
                    Item::Text(t) => self.emit_text(sc, pb, &t, &p),
                    Item::Obj(o) => self.emit_obj(sc, pb, *o, &p),
                }
            }
            return;
        }
        // A cached result holding a picture, chart or shape (`INCLUDEPICTURE`'s embedded
        // picture) can't be a text result: keep the field as a range around it so the picture
        // stays in the document and is saved again. The linked source is never read.
        // A tracked insertion or deletion inside the result (one the field itself isn't wrapped
        // in) keeps the field a range too: a cached text result has one revision at most, so
        // each piece stays an ordinary run with its own `w:ins` / `w:del` (#1037).
        let tracked_inside = f.buf.iter().any(|(_, p)| (p.ins, p.del) != (f.props.ins, f.props.del));
        if !nested_in_instr && (tracked_inside || f.buf.iter().any(|(it, _)| matches!(it, Item::Obj(o) if o.frame().is_some()))) {
            self.emit_obj(sc, pb, InlineObject::FieldStart { instr: f.instr.trim().to_string(), locked: f.locked, dirty: f.dirty }, &f.props);
            for (it, p) in f.buf {
                match it {
                    Item::Text(t) => self.emit_text(sc, pb, &t, &p),
                    Item::Obj(o) => self.emit_obj(sc, pb, *o, &p),
                }
            }
            self.emit_obj(sc, pb, InlineObject::FieldEnd, &f.props);
            return;
        }
        let mut result = String::new();
        for (it, _) in &f.buf {
            match it {
                Item::Text(t) => result.push_str(t),
                Item::Obj(o) => result.push_str(o.plain_text()),
            }
        }
        let mut code_paras = None;
        let (instr, result) = match enclosure(&f) {
            // Word writes no result for enclosed characters: the text is their plain text.
            Some(e) => (e.instr(), if result.is_empty() { e.text } else { result }),
            None => {
                let instr = f.instr.trim().to_string();
                // Paragraph marks inside the code, at their offsets in the trimmed code.
                if (!f.marks.is_empty() || !f.nested.is_empty()) && !nested_in_instr {
                    let lead = f.instr.len().saturating_sub(f.instr.trim_start().len());
                    let marks = f
                        .marks
                        .into_iter()
                        .map(|mut m| {
                            m.at = m.at.saturating_sub(lead).min(instr.len());
                            m
                        })
                        .collect();
                    code_paras = Some(Box::new(wordcraft_doc::para::CodeParas {
                        code: wordcraft_doc::para::code_hash(&instr),
                        first_after: f.first_after.0,
                        first_after_auto: f.first_after.1,
                        marks,
                        nested: f
                            .nested
                            .into_iter()
                            .map(|mut n| {
                                n.at = n.at.saturating_sub(lead).min(instr.len());
                                n
                            })
                            .collect(),
                    }));
                }
                (instr, result)
            }
        };
        // The visible result owns the field's formatting; empty results keep the begin's.
        let props = f
            .buf
            .iter()
            .find_map(|(it, p)| match it {
                Item::Text(t) if !t.is_empty() => Some(p),
                Item::Obj(o) if !o.plain_text().is_empty() => Some(p),
                _ => None,
            })
            .unwrap_or(&f.props)
            .clone();
        let obj = InlineObject::Field { instr, result, locked: f.locked, dirty: f.dirty, code: false, form: f.form, code_paras };
        if let Some(parent) = sc.fields.last_mut()
            && !parent.spilled
            && parent.phase == Phase::Instr
        {
            if let InlineObject::Field { instr, .. } = &obj {
                parent.nested.push(wordcraft_doc::para::NestedCode { at: parent.instr.len(), instr: instr.clone() });
            }
            parent.instr.push_str(obj.plain_text());
            return;
        }
        self.emit_obj(sc, pb, obj, &props);
    }

    /// At a paragraph end, open fields can't keep buffering: emit them (empty result) and
    /// let the rest of their result flow as ordinary text.
    pub(super) fn spill_fields(&mut self, sc: &mut StoryCtx, pb: &mut PB) {
        for f in sc.fields.iter_mut() {
            if f.spilled || f.range {
                continue;
            }
            f.spilled = true;
            let form = std::mem::take(&mut f.form);
            pb.push_obj(
                InlineObject::Field {
                    instr: f.instr.trim().to_string(),
                    result: String::new(),
                    locked: f.locked,
                    dirty: f.dirty,
                    code: false,
                    form,
                    code_paras: None,
                },
                &f.props,
            );
            for (it, p) in f.buf.drain(..) {
                match it {
                    Item::Text(t) => pb.push_text(&t, &p),
                    Item::Obj(o) => pb.push_obj(*o, &p),
                }
            }
        }
    }
}

/// Most paragraphs a field's code may run on into (more: the field ends at the paragraph end).
const MAX_CODE_PARAS: usize = 64;

/// How many of the paragraphs `next` (the siblings after a paragraph whose field is still
/// collecting its code) the code runs on into: the last one holds the code's `separate` or `end`.
/// `None` unless they are all plain sibling paragraphs (no section break), those wholly inside the
/// code hold nothing but code, field characters, proofing and bookmark marks and empty runs, and
/// no field nested in the code is open at a paragraph mark.
pub(super) fn code_paragraphs(next: &[&El]) -> Option<usize> {
    let mut depth = 1usize;
    for (j, p) in next.iter().enumerate().take(MAX_CODE_PARAS) {
        if p.name != "w:p" || p.child("w:pPr").is_some_and(|ppr| ppr.child("w:sectPr").is_some()) {
            return None;
        }
        for k in p.els() {
            match k.name.as_str() {
                "w:pPr" | "w:proofErr" | "w:bookmarkStart" | "w:bookmarkEnd" => {}
                "w:r" => {
                    for c in k.els() {
                        match c.name.as_str() {
                            "w:rPr" | "w:instrText" | "w:delInstrText" => {}
                            "w:fldChar" => match c.attr("w:fldCharType") {
                                Some("begin") => depth = depth.saturating_add(1),
                                Some("separate") if depth == 1 => return Some(j + 1),
                                Some("end") if depth == 1 => return Some(j + 1),
                                Some("end") => depth = depth.saturating_sub(1),
                                _ => {}
                            },
                            _ => return None,
                        }
                    }
                }
                _ => return None,
            }
        }
        if depth != 1 {
            return None;
        }
    }
    None
}

impl Reader<'_> {
    /// At a paragraph end: whether the innermost field is still collecting its code and is the
    /// only one doing so (a paragraph mark in a nested field's code is not joined).
    pub(super) fn code_open_at_mark(sc: &StoryCtx) -> bool {
        sc.ignored_begins == 0 && sc.fields.last().is_some_and(|f| !f.spilled && !f.range && f.phase == Phase::Instr) && !Self::parent_in_instr(sc)
    }

    /// A paragraph mark inside the code of the innermost field: one space in the code where the
    /// mark is (none after white space), and the next paragraph's properties. `after` is the
    /// space after of the paragraph the mark ends.
    pub(super) fn code_mark(sc: &mut StoryCtx, props: wordcraft_doc::props::ParaProps, mark: CharProps, after: (Option<f32>, Option<bool>)) {
        let Some(f) = sc.fields.last_mut() else { return };
        if f.marks.is_empty() {
            f.first_after = after;
        }
        if f.instr.len() < super::MAX_INSTR && !f.instr.is_empty() && !f.instr.ends_with(char::is_whitespace) {
            f.instr.push(' ');
        }
        if f.marks.len() < MAX_CODE_PARAS {
            f.marks.push(wordcraft_doc::para::CodeMark { at: f.instr.len(), props, mark });
        }
    }
}

#[cfg(test)]
mod tests;
