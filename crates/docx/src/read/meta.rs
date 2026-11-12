use super::{
    Arc, Blocks, Comment, DocxError, El, HashMap, MAX_PARTS, MAX_SOURCES, MAX_TEMPLATE_PATH, NoteKind, NumFormat, Paragraph, PartKind, Reader, Rels,
    Seen, StoryCtx, Zoom, ZoomFit, int, math, on_off, para_block, props, tw, u32_of,
};

/// `w:stylePaneFormatFilter`'s attributes whose bit in `w:val` was measured, with that bit.
const PANE_FILTER_ATTRS: [(&str, u16); 7] = [
    ("w:allStyles", 0x0001),
    ("w:customStyles", 0x0002),
    ("w:latentStyles", 0x0004),
    ("w:stylesInUse", 0x0008),
    ("w:headingStyles", 0x0020),
    ("w:clearFormatting", 0x1000),
    ("w:top3HeadingStyles", 0x2000),
];

/// `w:stylePaneFormatFilter/@w:val`: 1–4 hex digits, any case.
fn pane_filter_hex(v: &str) -> Option<u16> {
    let v = v.trim();
    if v.is_empty() || v.len() > 4 {
        return None;
    }
    u16::from_str_radix(v, 16).ok()
}

/// An ST_OnOff attribute value.
fn attr_on_off(v: &str) -> Option<bool> {
    match v.trim() {
        "1" | "true" | "on" => Some(true),
        "0" | "false" | "off" => Some(false),
        _ => None,
    }
}

impl Reader<'_> {
    pub(super) fn read_settings(&mut self, path: &str) -> Result<Option<u32>, DocxError> {
        let Some(root) = self.xml(path)? else { return Ok(None) };
        let mut explicit_mode = None;
        // The drawing grid's display elements are absent from most files, and absent means 1, not
        // the blank document's 0/2: start from the schema's defaults and let the file say.
        self.doc.settings.grid_every_h = 1;
        self.doc.settings.grid_every_v = 1;
        if let Some(b) = self.pkg.get(path).filter(|b| crate::write::settings_has_unowned(b)) {
            self.doc.passthrough.insert(crate::package::SETTINGS_ORIG.into(), Arc::new(b.to_vec()));
        }
        let rels = self.pkg.rels(path);
        let s = &mut self.doc.settings;
        for k in root.els() {
            match k.name.as_str() {
                // §17.15.1.6: the template, by a relationship (usually external) of the settings part.
                "w:attachedTemplate" => {
                    s.attached_template = k
                        .attr("r:id")
                        .and_then(|id| rels.by_id(id))
                        .map(|r| r.target.trim().to_string())
                        .filter(|t| !t.is_empty() && t.len() <= MAX_TEMPLATE_PATH);
                }
                "w:linkStyles" => s.link_styles = on_off(k),
                "w:stylePaneFormatFilter" => {
                    if let Some(filter) = k.attr("w:val").and_then(pane_filter_hex) {
                        let mut pane = s.style_pane.take().unwrap_or_default();
                        pane.filter = filter;
                        pane.filter_attrs = false;
                        // The Word 2010 attribute form wins for the bits it names.
                        for (name, bit) in PANE_FILTER_ATTRS {
                            if let Some(on) = k.attr(name).and_then(attr_on_off) {
                                pane.filter_attrs = true;
                                if on { pane.filter |= bit } else { pane.filter &= !bit }
                            }
                        }
                        s.style_pane = Some(pane);
                    }
                }
                "w:stylePaneSortMethod" => {
                    let mut pane = s.style_pane.take().unwrap_or_default();
                    pane.sort = k.attr("w:val").and_then(wordcraft_doc::StylePaneSort::parse);
                    s.style_pane = Some(pane);
                }
                "w:removePersonalInformation" => s.remove_personal_info = on_off(k),
                "w:removeDateAndTime" => s.remove_date_time = on_off(k),
                "w:compat" => {
                    s.allow_space_of_same_style_in_table = k.child("w:allowSpaceOfSameStyleInTable").is_some_and(on_off);
                    s.split_pg_break_and_para_mark = k.child("w:splitPgBreakAndParaMark").is_some_and(on_off);
                    // Only Word's own setting: another `w:uri` names a different application's.
                    let mode = k
                        .children("w:compatSetting")
                        .find(|c| {
                            c.attr("w:name") == Some("compatibilityMode")
                                && c.attr("w:uri").is_none_or(|u| u == "http://schemas.microsoft.com/office/word")
                        })
                        .and_then(|c| c.attr("w:val"));
                    if let Some(m) = mode.and_then(u32_of) {
                        explicit_mode = Some(m);
                        s.compat_mode = m.clamp(11, 99);
                    }
                    s.sum_paragraph_spacing = k.children("w:doNotUseHTMLParagraphAutoSpacing").any(on_off);
                    s.no_leading = k.children("w:noLeading").any(on_off);
                    s.wp_justification = k.children("w:wpJustification").any(on_off);
                    s.do_not_expand_shift_return = k.children("w:doNotExpandShiftReturn").any(on_off);
                    s.wrap_trail_spaces = k.children("w:wrapTrailSpaces").any(on_off);
                }
                "w:trackRevisions" => s.track_changes = on_off(k),
                "w:doNotTrackMoves" => s.track_moves = !on_off(k),
                "w:defaultTabStop" => {
                    if let Some(v) = tw(k, "w:val").filter(|v| *v > 0.0) {
                        s.default_tab = v.clamp(1.0, 1584.0);
                    }
                }
                "w:evenAndOddHeaders" => s.even_odd_headers = on_off(k),
                "w:drawingGridHorizontalSpacing" | "w:drawingGridVerticalSpacing" => {
                    if let Some(v) = tw(k, "w:val").filter(|v| *v > 0.0) {
                        let v = v.clamp(0.5, 1584.0);
                        if k.name == "w:drawingGridHorizontalSpacing" { s.grid_h = v } else { s.grid_v = v }
                    }
                }
                "w:displayHorizontalDrawingGridEvery" | "w:displayVerticalDrawingGridEvery" => {
                    if let Some(v) = k.attr("w:val").and_then(u32_of) {
                        let v = v.min(127) as u8;
                        if k.name == "w:displayHorizontalDrawingGridEvery" { s.grid_every_h = v } else { s.grid_every_v = v }
                    }
                }
                "w:doNotUseMarginsForDrawingGridOrigin" => s.grid_use_margins = !on_off(k),
                "w:drawingGridHorizontalOrigin" | "w:drawingGridVerticalOrigin" => {
                    if let Some(v) = tw(k, "w:val").filter(|v| v.is_finite()) {
                        let v = v.clamp(0.0, 1584.0);
                        if k.name == "w:drawingGridHorizontalOrigin" { s.grid_origin_x = v } else { s.grid_origin_y = v }
                    }
                }
                "m:mathPr" => s.math = Some(math::read_math_pr(k)),
                "w:mirrorMargins" => s.mirror_margins = on_off(k),
                "w:printFormsData" => s.print_forms_data = on_off(k),
                "w:autoHyphenation" => s.auto_hyphenation = on_off(k),
                "w:doNotHyphenateCaps" => s.hyphenate_caps = !on_off(k),
                "w:hyphenationZone" => {
                    if let Some(v) = tw(k, "w:val").filter(|v| v.is_finite() && *v >= 0.0) {
                        s.hyphenation_zone = v.min(1584.0);
                    }
                }
                "w:consecutiveHyphenLimit" => {
                    if let Some(v) = k.attr("w:val").and_then(u32_of) {
                        s.consecutive_hyphen_limit = v.min(32_767);
                    }
                }
                "w:footnotePr" => {
                    if let Some(f) = k.child_val("w:numFmt") {
                        s.footnote_format = NumFormat::from_ooxml(f);
                    }
                    s.footnote_pr = props::note_pr(k, false);
                }
                "w:endnotePr" => {
                    if let Some(f) = k.child_val("w:numFmt") {
                        s.endnote_format = NumFormat::from_ooxml(f);
                    }
                    s.endnote_pr = props::note_pr(k, false);
                }
                "w:documentProtection" => read_protection(k, s),
                "w:writeProtection" => read_write_protection(k, s),
                "w:captions" => s.captions = read_captions(k),
                "w:docVars" => s.doc_vars = read_doc_vars(k),
                "w:zoom" => {
                    let percent = k.attr("w:percent").and_then(|v| u32_of(v.trim_end_matches('%'))).unwrap_or(100).clamp(10, 500);
                    let fit = match k.attr("w:val") {
                        Some("fullPage") => Some(ZoomFit::OnePage),
                        Some("bestFit") => Some(ZoomFit::PageWidth),
                        Some("textFit") => Some(ZoomFit::TextWidth),
                        _ => None,
                    };
                    s.zoom = Some(Zoom { percent, fit }).filter(|z| z.percent != 100 || z.fit.is_some());
                }
                _ => {}
            }
        }
        Ok(explicit_mode)
    }

    pub(super) fn read_notes(&mut self, path: &str, foot: bool) -> Result<(), DocxError> {
        let Some(root) = self.xml(path)? else { return Ok(()) };
        let rels = self.pkg.rels(path);
        let tag = if foot { "w:footnote" } else { "w:endnote" };
        for n in root.children(tag) {
            match n.attr("w:type") {
                None | Some("normal") => {}
                Some(t @ ("separator" | "continuationSeparator" | "continuationNotice")) => {
                    self.read_note_sep(n, &rels, foot, t);
                    continue;
                }
                Some(_) => continue,
            }
            let Some(fid) = n.attr("w:id").and_then(int) else { continue };
            if self.doc.parts.len() >= MAX_PARTS {
                break;
            }
            // Reserve the id first: the note's own reference mark points at it.
            let (pk, kind) = if foot { (PartKind::Footnote, NoteKind::Footnote) } else { (PartKind::Endnote, NoteKind::Endnote) };
            let id = self.doc.add_part(pk, Blocks::new());
            self.doc.missing_pictures.part_names.insert(id, self.story.clone());
            let mut sc = StoryCtx::default();
            let mut blocks = Blocks::new();
            self.current_note = Some((kind, id));
            self.read_blocks(&mut sc, n, &rels, &mut blocks, self.chunks.depth);
            self.flush_pending(&mut sc, &mut blocks);
            self.current_note = None;
            if let Some(p) = self.doc.parts.get_mut(&id)
                && !blocks.is_empty()
            {
                p.blocks = blocks;
            }
            if foot {
                self.footnotes.insert(fid, id);
            } else {
                self.endnotes.insert(fid, id);
            }
        }
        Ok(())
    }

    /// A document's own separator, continuation separator or continuation notice (`kind`, the
    /// note's `w:type`) for its footnotes or endnotes. The first of each kind counts.
    fn read_note_sep(&mut self, n: &El, rels: &Rels, foot: bool, kind: &str) {
        let seps = if foot { &self.doc.footnote_separators } else { &self.doc.endnote_separators };
        let taken = match kind {
            "separator" => seps.separator.is_some(),
            "continuationSeparator" => seps.continuation_separator.is_some(),
            _ => seps.continuation_notice.is_some(),
        };
        if taken {
            return;
        }
        let mut sc = StoryCtx::default();
        let mut blocks = Blocks::new();
        self.in_note_sep = true;
        self.read_blocks(&mut sc, n, rels, &mut blocks, self.chunks.depth);
        self.flush_pending(&mut sc, &mut blocks);
        self.in_note_sep = false;
        if blocks.is_empty() {
            blocks.push(para_block(Paragraph::new()));
        }
        let seps = if foot { &mut self.doc.footnote_separators } else { &mut self.doc.endnote_separators };
        let slot = match kind {
            "separator" => &mut seps.separator,
            "continuationSeparator" => &mut seps.continuation_separator,
            _ => &mut seps.continuation_notice,
        };
        *slot = Some(blocks);
    }

    /// The extended properties a save carries over ([`crate::package::APP_KEPT_PROPS`]).
    pub(super) fn read_app(&mut self, path: &str) {
        let Ok(Some(root)) = self.xml(path) else { return };
        let mut kept = String::new();
        for e in root.els() {
            let name = e.local();
            let value = e.text();
            if crate::package::APP_KEPT_PROPS.contains(&name) && value.len() <= 2048 && !value.contains(['\t', '\n']) {
                kept.push_str(&format!("{name}\t{value}\n"));
            }
        }
        if !kept.is_empty() {
            self.doc.passthrough.insert(crate::package::APP_KEPT.into(), Arc::new(kept.into_bytes()));
        }
    }

    pub(super) fn read_comments(&mut self, path: &str, ext: Option<&str>, ids: Option<&str>, cex: Option<&str>) -> Result<(), DocxError> {
        let Some(root) = self.xml(path)? else { return Ok(()) };
        let rels = self.pkg.rels(path);
        let mut by_para: HashMap<String, u32> = HashMap::new();
        let mut next_id = 0u32;
        // Straight into the document, beside the comments chunks in notes brought in.
        for c in root.children("w:comment") {
            if self.doc.parts.len() >= MAX_PARTS {
                break;
            }
            let file_id = c.attr("w:id").unwrap_or("").to_string();
            let id = match file_id.parse::<u32>() {
                Ok(v) if !self.doc.comments.contains_key(&v) => v,
                _ => {
                    // Every id handed out so far is a key of `doc.comments` (inserted below).
                    while self.doc.comments.contains_key(&next_id) {
                        next_id += 1;
                    }
                    next_id
                }
            };
            // Taken before its text is read: a chunk there brings comments of its own.
            self.doc.comments.insert(id, Comment::default());
            let mut sc = StoryCtx::default();
            let mut blocks = Blocks::new();
            self.read_blocks(&mut sc, c, &rels, &mut blocks, self.chunks.depth);
            self.flush_pending(&mut sc, &mut blocks);
            let part = self.doc.add_part(PartKind::Comment, blocks);
            self.doc.missing_pictures.part_names.insert(part, self.story.clone());
            if let Some(pid) = c.children("w:p").last().and_then(|p| p.attr("w14:paraId")) {
                by_para.insert(pid.to_string(), id);
            }
            self.doc.comments.insert(
                id,
                Comment {
                    author: c.attr("w:author").unwrap_or("").to_string(),
                    initials: c.attr("w:initials").unwrap_or("").to_string(),
                    date: c.attr("w:date").unwrap_or("").to_string(),
                    parent: None,
                    resolved: false,
                    part,
                    ..Default::default()
                },
            );
            self.comment_map.entry(file_id).or_default().push_back(id);
        }
        if let Some(ext) = ext
            && let Ok(Some(x)) = self.xml(ext)
        {
            for e in x.els().filter(|e| e.local() == "commentEx") {
                let Some(cid) = e.attr("w15:paraId").and_then(|p| by_para.get(p)).copied() else { continue };
                let parent = e.attr("w15:paraIdParent").and_then(|p| by_para.get(p)).copied().filter(|p| *p != cid);
                let done = e.attr("w15:done").is_some_and(|v| v == "1" || v == "true");
                if let Some(c) = self.doc.comments.get_mut(&cid) {
                    c.parent = parent;
                    c.resolved = done;
                }
            }
        }
        // Durable ids (by paragraph id) and UTC dates (by durable id), which the writer
        // carries over (`write_comment_ids`). Their namespaces (w16cid, w16cex) have no canonical
        // prefix here, so attributes are found by local name.
        let local = |e: &crate::xml::El, name: &str| -> Option<String> {
            e.attrs.iter().find(|(k, _)| k.rsplit(':').next() == Some(name)).map(|(_, v)| v.clone())
        };
        let mut by_durable: HashMap<String, u32> = HashMap::new();
        if let Some(ids) = ids
            && let Ok(Some(x)) = self.xml(ids)
        {
            for e in x.els().filter(|e| e.local() == "commentId") {
                let Some(cid) = local(e, "paraId").and_then(|p| by_para.get(&p).copied()) else { continue };
                let Some(durable) = local(e, "durableId").filter(|d| crate::write::durable_id_ok(d)) else { continue };
                if let Some(c) = self.doc.comments.get_mut(&cid) {
                    c.durable_id = Some(durable.to_ascii_uppercase());
                    by_durable.insert(durable.to_ascii_uppercase(), cid);
                }
            }
        }
        if let Some(cex) = cex
            && let Ok(Some(x)) = self.xml(cex)
        {
            for e in x.els().filter(|e| e.local() == "commentExtensible") {
                let Some(cid) = local(e, "durableId").and_then(|d| by_durable.get(&d.to_ascii_uppercase()).copied()) else { continue };
                let date = local(e, "dateUtc").filter(|d| d.len() <= 40 && d.chars().all(|c| c.is_ascii_alphanumeric() || ":-.+".contains(c)));
                if let (Some(c), Some(date)) = (self.doc.comments.get_mut(&cid), date) {
                    c.date_utc = Some(date);
                }
            }
        }
        Ok(())
    }

    /// Sources from a `b:Sources` custom XML part (other custom XML is left alone); at most
    /// [`MAX_SOURCES`] in all, and the document's citation style from `StyleName`.
    pub(super) fn read_bibliography(&mut self, path: &str, seen: &mut Seen) -> Result<(), DocxError> {
        let Some(root) = self.xml(path)? else { return Ok(()) };
        if root.local() != "Sources" {
            return Ok(());
        }
        let room = MAX_SOURCES.saturating_sub(self.doc.sources.len());
        let bib = crate::bibliography::parse(&root, seen, room);
        if self.doc.settings.bib_style.is_none() {
            self.doc.settings.bib_style = bib.style;
        }
        self.doc.sources.extend(bib.sources);
        Ok(())
    }

    pub(super) fn read_core(&mut self, path: &str) {
        // Word's DOCPROPERTY tells a summary property with no element (an error) from an empty
        // one (nothing): each is missing until its element is read, also without a core part.
        use wordcraft_doc::InFile::Present;
        self.doc.core.in_file = wordcraft_doc::SummaryInFile::all(wordcraft_doc::InFile::Missing);
        let Ok(Some(root)) = self.xml(path) else { return };
        let c = &mut self.doc.core;
        for k in root.els() {
            let t = k.text().trim().to_string();
            match k.name.as_str() {
                "dc:title" => (c.title, c.in_file.title) = (t, Present),
                "dc:subject" => (c.subject, c.in_file.subject) = (t, Present),
                "dc:creator" => (c.creator, c.in_file.creator) = (t, Present),
                "cp:keywords" => (c.keywords, c.in_file.keywords) = (t, Present),
                "dc:description" => (c.description, c.in_file.description) = (t, Present),
                "cp:lastModifiedBy" => c.last_modified_by = t,
                "cp:revision" => c.revision = u32_of(&t).unwrap_or(0),
                // A date that isn't a W3CDTF date isn't one: leave the field empty.
                "dcterms:created" if crate::w3cdtf::is_w3cdtf(&t) => c.created = t,
                "dcterms:modified" if crate::w3cdtf::is_w3cdtf(&t) => c.modified = t,
                "cp:category" => c.category = t,
                "cp:contentStatus" => c.content_status = t,
                "cp:lastPrinted" if crate::w3cdtf::is_w3cdtf(&t) => c.last_printed = t,
                "dc:language" => c.language = t,
                "cp:version" => c.version = t,
                "dc:identifier" => c.identifier = t,
                "cp:contentType" => c.content_type = t,
                _ => {}
            }
        }
    }
}

/// The password hash attributes of `w:documentProtection` that are kept (ECMA-376 §17.15.1.29:
/// the current `algorithmName`/`hashValue`/`saltValue`/`spinCount`, and the older transitional
/// `crypt*`/`hash`/`salt` set).
pub(crate) const PROTECTION_HASH_ATTRS: [&str; 16] = [
    "w:algorithmName",
    "w:hashValue",
    "w:saltValue",
    "w:spinCount",
    "w:cryptProviderType",
    "w:cryptAlgorithmClass",
    "w:cryptAlgorithmType",
    "w:cryptAlgorithmSid",
    "w:cryptSpinCount",
    "w:cryptProvider",
    "w:algIdExt",
    "w:algIdExtSource",
    "w:cryptProviderTypeExt",
    "w:cryptProviderTypeExtSource",
    "w:hash",
    "w:salt",
];

/// Restrict Editing (`w:documentProtection`, §17.15.1.29): the editing restriction, whether
/// formatting is limited to unlocked styles, and the password hash. Only enforced protection is
/// kept; a hostile attribute value can't be longer than a hash needs.
/// Longest caption label or object type name kept.
const MAX_CAPTION_NAME: usize = 255;

/// `w:captions` (ECMA-376 §17.15.1.18): caption labels (`w:caption`) and AutoCaption entries
/// (`w:autoCaptions/w:autoCaption`), [`wordcraft_doc::MAX_CAPTION_ENTRIES`] of each. An attribute
/// with a value the schema doesn't have is ignored; an entry without a name is skipped.
/// Most document variables read from a file (hostile-input cap).
const MAX_DOC_VARS: usize = 4096;
/// Longest variable name and value kept (ST_String limits of `w:docVar`, ECMA-376 §17.15.1.31).
const MAX_DOC_VAR_NAME: usize = 255;
const MAX_DOC_VAR_VALUE: usize = 65_280;

/// `w:docVars` (§17.15.1.32): one variable per `w:docVar` with a name, in file order.
fn read_doc_vars(k: &El) -> Vec<wordcraft_doc::DocVar> {
    let mut vars = Vec::new();
    for e in k.children("w:docVar") {
        if vars.len() >= MAX_DOC_VARS {
            break;
        }
        let Some(name) = e.attr("w:name").filter(|n| !n.is_empty() && n.chars().count() <= MAX_DOC_VAR_NAME) else { continue };
        let value = e.attr("w:val").unwrap_or("");
        if value.chars().count() > MAX_DOC_VAR_VALUE {
            continue;
        }
        vars.push(wordcraft_doc::DocVar { name: name.to_string(), value: value.to_string() });
    }
    vars
}

pub(crate) fn read_captions(k: &El) -> wordcraft_doc::Captions {
    use wordcraft_doc::{AutoCaption, CAPTION_NUM_FORMATS, CaptionLabel, CaptionSep, MAX_CAPTION_ENTRIES};
    let name = |e: &El, attr: &str| {
        e.attr(attr).map(str::trim).filter(|n| !n.is_empty() && n.len() <= MAX_CAPTION_NAME && !n.chars().any(char::is_control)).map(str::to_string)
    };
    let flag = |e: &El, attr: &str| match e.attr(attr) {
        Some("1" | "true" | "on") => Some(true),
        Some("0" | "false" | "off") => Some(false),
        _ => None,
    };
    let mut c = wordcraft_doc::Captions::default();
    for e in k.children("w:caption") {
        if c.labels.len() >= MAX_CAPTION_ENTRIES {
            break;
        }
        let Some(n) = name(e, "w:name") else { continue };
        if c.labels.iter().any(|l| l.name.eq_ignore_ascii_case(&n)) {
            continue;
        }
        let mut l = CaptionLabel::new(&n);
        match e.attr("w:pos") {
            Some("above") => l.above = true,
            Some("below") => l.above = false,
            _ => {}
        }
        if let Some(f) = e.attr("w:numFmt").filter(|f| CAPTION_NUM_FORMATS.contains(f)) {
            l.num_fmt = f.to_string();
        }
        l.chapter = flag(e, "w:chapNum").unwrap_or(false);
        if let Some(h) = e.attr("w:heading").and_then(|h| h.trim().parse::<u8>().ok()).filter(|h| *h <= 9) {
            l.heading = h;
        }
        if let Some(sep) = e.attr("w:sep").and_then(CaptionSep::from_ooxml) {
            l.separator = sep;
        }
        l.no_label = flag(e, "w:noLabel").unwrap_or(false);
        c.labels.push(l);
    }
    for e in k.children("w:autoCaptions").flat_map(|a| a.children("w:autoCaption")) {
        if c.auto.len() >= MAX_CAPTION_ENTRIES {
            break;
        }
        let (Some(object), Some(label)) = (name(e, "w:name"), name(e, "w:caption")) else { continue };
        if !c.auto.iter().any(|a| a.object == object) {
            c.auto.push(AutoCaption { object, label });
        }
    }
    c
}

/// Read-only recommended and Password to modify (`w:writeProtection`, §17.15.1.93): the
/// recommendation flag and the password hash attributes, in the same forms as
/// `w:documentProtection`'s.
fn read_write_protection(k: &El, s: &mut wordcraft_doc::Settings) {
    let on = |v: &str| !matches!(v, "0" | "false" | "off");
    let recommended = k.attr("w:recommended").is_some_and(on);
    let hash: Vec<(String, String)> = PROTECTION_HASH_ATTRS
        .iter()
        .filter_map(|n| k.attr(n).filter(|v| !v.is_empty() && v.len() <= 1024).map(|v| (n.to_string(), v.to_string())))
        .collect();
    s.write_protection = Some(wordcraft_doc::WriteProtection { recommended, hash }).filter(|w| !w.is_empty());
}

fn read_protection(k: &El, s: &mut wordcraft_doc::Settings) {
    let on = |v: &str| !matches!(v, "0" | "false" | "off");
    if !k.attr("w:enforcement").is_some_and(on) {
        return;
    }
    let edit = k.attr("w:edit").filter(|e| matches!(*e, "readOnly" | "comments" | "trackedChanges" | "forms"));
    let formatting = k.attr("w:formatting").is_some_and(on);
    if edit.is_none() && !formatting {
        return;
    }
    s.protection = Some(edit.unwrap_or("none").to_string());
    s.protect_formatting = formatting;
    s.protect_hash = PROTECTION_HASH_ATTRS
        .iter()
        .filter_map(|n| k.attr(n).filter(|v| !v.is_empty() && v.len() <= 1024).map(|v| (n.to_string(), v.to_string())))
        .collect();
}
