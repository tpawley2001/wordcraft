use super::reader::Reader;
use super::state::{CommentData, Dest, DocOpts, HeaderFooter, HfPages, MARK_CLOSE, MARK_OPEN, MAX_PICT, NoteData, PageGeom};
use super::{Arc, Block, Blocks, Cell, Document, FBlock, Flow, HashMap, InlineObject, Para, Paragraph, model};

/// Parse RTF bytes into the flow model.
pub fn parse(bytes: &[u8]) -> Result<Flow, String> {
    let mut flow = parse_with_page(bytes)?.0;
    super::frames::settle_flow(&mut flow.blocks, 0);
    Ok(flow)
}

/// The flow, the page, and the default font (`\deff`, font 0 when the file names none), which
/// is the font of all text that doesn't select one.
fn parse_with_page(bytes: &[u8]) -> Result<(Flow, PageGeom, Option<String>), String> {
    parse_full(bytes, false).map(|r| (r.flow, r.page, r.default_font))
}

/// Everything an import takes from an RTF file.
struct Parsed {
    flow: Flow,
    /// The information group beyond the flow's metadata, and the user properties.
    info: Info,
    page: PageGeom,
    /// The document formatting words that go into the document's settings.
    doc_opts: DocOpts,
    /// The last section's properties (the others are on the paragraphs that end them).
    last_section: wordcraft_doc::SectionProps,
    default_font: Option<String>,
    /// Each section's headers and footers, in order, the last section's last.
    headers_footers: Vec<Vec<(HeaderFooter, Vec<FBlock>)>>,
    /// Another program's heading, title and quote styles: the document style each stands for,
    /// with the formatting its stylesheet entry gives it.
    foreign_styles: Vec<(String, wordcraft_doc::props::ParaProps, wordcraft_doc::props::CharProps)>,
    /// The document's own paragraph and character styles the stylesheet defines.
    own_styles: Vec<wordcraft_doc::styles::Style>,
    notes: Vec<NoteData>,
    comments: Vec<CommentData>,
    /// The note separator stories (`\ftnsep`…), in the order of [`super::state::SEP_WORDS`].
    note_seps: [Option<Vec<FBlock>>; 6],
    /// The document's footnote and endnote options; `num_fmt` is the number format.
    doc_notes: (wordcraft_doc::section::NoteProps, wordcraft_doc::section::NoteProps),
    /// The document's `\fetN`, when it names one.
    fet: Option<i32>,
    /// The document settings the header names.
    doc_set: super::reader::DocSettingWords,
    /// The latent-style table (`{\*\latentstyles …}`) as `w:latentStyles` XML.
    latent_styles: Option<String>,
}

/// What the information group gives the document's properties beyond [`model::Meta`], with the user
/// properties (`{\*\userprops …}`).
#[derive(Debug, Default)]
struct Info {
    more: super::state::InfoMore,
    /// `\revtim` and `\printim` as `YYYY-MM-DDTHH:MM:00Z` (empty when absent).
    modified: String,
    printed: String,
}

impl Info {
    fn apply(self, doc: &mut Document) {
        let c = &mut doc.core;
        c.last_modified_by = self.more.last_modified_by;
        c.category = self.more.category;
        c.revision = self.more.revision;
        c.modified = self.modified;
        c.last_printed = self.printed;
        doc.custom_props = self.more.props;
        doc.settings.doc_vars = self.more.doc_vars;
        let mut kept = String::new();
        for (name, value) in [("Manager", self.more.manager), ("Company", self.more.company), ("HyperlinkBase", self.more.hyperlink_base)] {
            if let Some(v) = app_kept_value(value) {
                kept.push_str(name);
                kept.push('\t');
                kept.push_str(&v);
                kept.push('\n');
            }
        }
        if !kept.is_empty() {
            doc.passthrough.insert(wordcraft_doc::APP_KEPT.into(), Arc::new(kept.into_bytes()));
        }
    }
}

/// The longest value the kept extended properties hold (as the DOCX reader keeps them).
const MAX_APP_KEPT_VALUE: usize = 2048;

/// An information-group value as the line-based `wordcraft:app.kept` store holds it: trimmed, tabs
/// and line ends turned into spaces (they would break the line), at most 2048 bytes; `None` when
/// empty.
fn app_kept_value(value: String) -> Option<String> {
    let mut v: String = value.trim().chars().map(|c| if matches!(c, '\t' | '\n' | '\r') { ' ' } else { c }).collect();
    super::state::truncate_chars(&mut v, MAX_APP_KEPT_VALUE);
    (!v.is_empty()).then_some(v)
}

/// An information-group date read from `\yr \mo \dy \hr \min`, as `YYYY-MM-DDTHH:MM:00Z`; empty
/// when it wasn't given or has no year.
fn info_date(d: Option<(u32, u32, u32, u32, u32)>) -> String {
    match d {
        Some((y, mo, d, h, mi)) if y > 0 => {
            format!("{y:04}-{:02}-{:02}T{:02}:{:02}:00Z", mo.clamp(1, 12), d.clamp(1, 31), h.min(23), mi.min(59))
        }
        _ => String::new(),
    }
}

/// Where the document `{\rtf…}` starting at `start` ends: at the `}` that first brings the depth
/// back to 0 (left for `finalize`, which ends a last paragraph without `\par` before the outer
/// group's state is restored), or the end of the bytes if the outer group never closes (RTF
/// spec, "RTF Syntax": the file is one group). Anything after that brace is not part of the document: text,
/// control words, raw bytes and groups, another `{\rtf` group included. A stray `}` inside the
/// document closes the outer group there, so what follows it is ignored too. `\binN` data is
/// skipped by the count the parser reads, so braces in it don't count.
fn document_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while let Some(&c) = bytes.get(i) {
        match c {
            b'{' => {
                depth = depth.saturating_add(1);
                i += 1;
            }
            b'}' => {
                if depth <= 1 {
                    return i;
                }
                depth -= 1;
                i += 1;
            }
            b'\\' => i = past_control(bytes, i),
            _ => i += 1,
        }
    }
    bytes.len()
}

/// Where the control symbol or word starting with the `\` at `i` ends, lexed as `parse_full`
/// lexes it: up to 32 letters, an optional `-`, at most 10 digits (clamped to `i32`) and one
/// optional space. After `\bin` that is past its data too. An escaped brace or backslash is text.
fn past_control(bytes: &[u8], i: usize) -> usize {
    if !bytes.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
        return i.saturating_add(2);
    }
    let mut j = i + 1;
    while bytes.get(j).is_some_and(u8::is_ascii_alphabetic) && j - i <= 32 {
        j += 1;
    }
    let is_bin = bytes.get(i + 1..j) == Some(b"bin".as_slice());
    let mut k = j;
    let neg = bytes.get(k) == Some(&b'-');
    if neg {
        k += 1;
    }
    let d0 = k;
    let mut v = 0i64;
    while let Some(d) = bytes.get(k).filter(|d| d.is_ascii_digit()) {
        if k - d0 >= 10 {
            break;
        }
        v = v * 10 + i64::from(d - b'0');
        k += 1;
    }
    if k == d0 && neg {
        k -= 1;
    }
    if bytes.get(k) == Some(&b' ') {
        k += 1;
    }
    if !is_bin {
        return k;
    }
    let v = if neg { -v } else { v };
    let n = usize::try_from(v.clamp(0, i64::from(i32::MAX))).unwrap_or(0);
    k.saturating_add(n)
}

/// `marks`: also read footnotes, endnotes and comments (as place markers in the flow's text, for
/// [`import`] to turn into objects); without it they are skipped.
fn parse_full(bytes: &[u8], marks: bool) -> Result<Parsed, String> {
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(0);
    if !bytes.get(start..).is_some_and(|b| b.starts_with(b"{\\rtf")) {
        return Err("not an RTF document (it doesn't start with {\\rtf)".into());
    }
    // Whatever follows the document's closing brace isn't part of it, as in Word. The brace
    // itself is closed by `finalize`.
    let bytes = bytes.get(..document_end(bytes, start)).unwrap_or(bytes);
    let mut r = Reader::default();
    r.marks_on = marks;
    let mut i = start;
    let mut first_in_group = false;
    let mut starred = false;
    while let Some(&c) = bytes.get(i) {
        match c {
            b'{' => {
                r.open();
                first_in_group = true;
                starred = false;
                i += 1;
                continue;
            }
            b'}' => {
                r.close();
                if r.info_depth.is_some_and(|d| r.depth() < d) {
                    r.info_depth = None;
                }
                first_in_group = false;
                starred = false;
                i += 1;
                continue;
            }
            b'\\' => {
                let Some(&n) = bytes.get(i + 1) else { break };
                if n.is_ascii_alphabetic() {
                    let mut j = i + 1;
                    while bytes.get(j).is_some_and(u8::is_ascii_alphabetic) && j - i <= 32 {
                        j += 1;
                    }
                    let word = std::str::from_utf8(bytes.get(i + 1..j).unwrap_or(b"")).unwrap_or("");
                    let mut k = j;
                    let neg = bytes.get(k) == Some(&b'-');
                    if neg {
                        k += 1;
                    }
                    let d0 = k;
                    while bytes.get(k).is_some_and(u8::is_ascii_digit) && k - d0 < 10 {
                        k += 1;
                    }
                    let param = if k > d0 {
                        let v = std::str::from_utf8(bytes.get(d0..k).unwrap_or(b"0")).ok().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                        let v = if neg { -v } else { v };
                        // A date (DTTM) is a 32-bit pattern some writers put unsigned.
                        let v = if matches!(word, "revdttm" | "revdttmdel" | "crdate" | "prdate")
                            && v > i64::from(i32::MAX)
                            && v <= i64::from(u32::MAX)
                        {
                            v - (1i64 << 32)
                        } else {
                            v
                        };
                        Some(v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
                    } else {
                        if neg {
                            k -= 1;
                        }
                        None
                    };
                    if bytes.get(k) == Some(&b' ') {
                        k += 1;
                    }
                    i = k;
                    if word == "bin" {
                        let n = param.unwrap_or(0).max(0) as usize;
                        let end = i.saturating_add(n).min(bytes.len());
                        if r.st.dest == Dest::Pict
                            && let Some(data) = bytes.get(i..end)
                            && r.pict_hex.len() + data.len() <= MAX_PICT
                        {
                            r.pict_hex.extend_from_slice(data);
                        }
                        i = end;
                        continue;
                    }
                    if r.skip_chars > 0
                        && matches!(
                            r.st.dest,
                            Dest::Body
                                | Dest::Info(_)
                                | Dest::FldInst
                                | Dest::FfText(_)
                                | Dest::Bookmark(_)
                                | Dest::FontTbl
                                | Dest::RevTbl
                                | Dest::Atn(_)
                        )
                    {
                        r.skip_chars -= 1;
                        first_in_group = false;
                        continue;
                    }
                    if word == "info" {
                        r.info_depth = Some(r.depth());
                    }
                    r.word(word, param, starred, first_in_group);
                    first_in_group = false;
                    starred = false;
                    continue;
                }
                if r.skip_chars > 0 && n != b'\'' {
                    r.skip_chars -= 1;
                    i += 2;
                    first_in_group = false;
                    continue;
                }
                match n {
                    b'\'' => {
                        // `from_str_radix` takes a leading `+`: a pair that isn't both hex digits
                        // is not an escape at all.
                        let h = bytes
                            .get(i + 2..i + 4)
                            .and_then(|h| std::str::from_utf8(h).ok())
                            .filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()))
                            .and_then(|h| u8::from_str_radix(h, 16).ok());
                        match h {
                            Some(v) => {
                                if r.st.dest == Dest::Pict {
                                    // Hex digits, two at a time, without a String per picture byte.
                                    const HEX: &[u8; 16] = b"0123456789abcdef";
                                    let h = [HEX[usize::from(v >> 4)], HEX[usize::from(v & 0x0f)]];
                                    if let Ok(s) = std::str::from_utf8(&h) {
                                        r.text(s);
                                    }
                                } else {
                                    r.byte(v);
                                }
                                i += 4;
                            }
                            // Not two hex digits: the escape is malformed. Consume the escape and
                            // any hex digits of a truncated pair, and leave every other byte,
                            // `}` or a control word's `\`, to be read as the structure it is.
                            None => {
                                i += 2;
                                let mut n = 0;
                                while n < 2 && bytes.get(i).is_some_and(u8::is_ascii_hexdigit) {
                                    i += 1;
                                    n += 1;
                                }
                            }
                        }
                    }
                    b'*' => {
                        starred = true;
                        i += 2;
                        continue;
                    }
                    b'~' | b' ' => {
                        r.ch('\u{00A0}');
                        i += 2;
                    }
                    b'-' => {
                        r.ch('\u{00AD}');
                        i += 2;
                    }
                    b'_' => {
                        r.ch('\u{2011}');
                        i += 2;
                    }
                    b'\n' | b'\r' => {
                        r.word("par", None, false, false);
                        i += 2;
                    }
                    b'\\' | b'{' | b'}' => {
                        r.ch(n as char);
                        i += 2;
                    }
                    _ => i += 2,
                }
                first_in_group = false;
            }
            b'\r' | b'\n' | 0 => i += 1,
            _ => {
                // A run of plain text.
                let mut j = i;
                while bytes.get(j).is_some_and(|b| !matches!(b, b'{' | b'}' | b'\\' | b'\r' | b'\n' | 0)) {
                    j += 1;
                }
                let chunk = bytes.get(i..j).unwrap_or(b"");
                if r.st.dest == Dest::Pict && r.skip_chars == 0 && chunk.is_ascii() {
                    // Picture hex is not text: it needs no decoder, so neither a pending lead byte
                    // nor the code page's single-byte path applies to it.
                    if let Ok(s) = std::str::from_utf8(chunk) {
                        r.text(s);
                    }
                } else if r.skip_chars == 0 && chunk.is_ascii() && r.mb_decoder.is_none() {
                    // Plain ASCII with no multibyte run open: append it as it is.
                    if let Ok(s) = std::str::from_utf8(chunk) {
                        r.text(s);
                    }
                } else {
                    r.run(chunk);
                }
                i = j;
                first_in_group = false;
            }
        }
    }
    Ok(finalize(r))
}

fn finalize(mut r: Reader) -> Parsed {
    // A multibyte run ends here, and so does a note, comment, header or footer left open by a
    // truncated file.
    r.flush_high_surrogate();
    r.finish_multibyte();
    // A last paragraph with no `\par` (it ends at the closing brace, or the file is cut short)
    // takes the paragraph properties in force there, as in Word, not those the open groups
    // restore when they close.
    if r.st.dest == Dest::Body && r.sub.is_none() && r.hf.is_none() && !r.in_shape_text() && (r.para_has || !r.para.inlines.is_empty()) {
        r.end_para(false);
    }
    while r.depth() > 0 {
        r.close();
    }
    r.end_sub();
    r.end_header_footer();
    if r.para_has || !r.para.inlines.is_empty() {
        r.end_para(false);
    }
    r.close_table();
    let [created, modified, printed] = r.dates.map(info_date);
    r.meta.created = created;
    r.info_more.commit_user_prop();
    let info = Info { more: std::mem::take(&mut r.info_more), modified, printed };
    let mut blocks = std::mem::take(&mut r.body);
    // A file that ends with `\sect` ends its last section there: no empty one follows, and
    // header or footer groups after that `\sect` (a section without text) are dropped.
    let mut last_section = r.sec.props(&r.page);
    last_section.no_endnote = r.no_endnote();
    last_section.fmt_change = r.section_change();
    let mut headers_footers = std::mem::take(&mut r.section_parts);
    if let Some(FBlock::Para(p)) = blocks.last_mut()
        && let Some(s) = p.section.take()
    {
        last_section = *s;
    } else {
        headers_footers.push(std::mem::take(&mut r.hf_parts));
    }
    if blocks.is_empty() {
        blocks.push(FBlock::Para(Para::default()));
    }
    let default_font = r.fonts.get(&r.deff).map(|n| n.to_string());
    let foreign_styles = r.foreign_styles();
    let own_styles = r.own_styles();
    Parsed {
        info,
        flow: Flow { blocks, meta: r.meta },
        page: r.page,
        doc_opts: r.doc_opts,
        last_section,
        default_font,
        headers_footers,
        foreign_styles,
        own_styles,
        notes: r.notes,
        comments: r.comments,
        note_seps: std::mem::take(&mut r.note_seps),
        doc_notes: r.doc_notes,
        fet: r.fet,
        doc_set: r.doc_set,
        latent_styles: r.latent.take(),
    }
}

/// The column grid of a table from its rows' cell right edges: every distinct edge starts a grid
/// column, and each cell spans the grid columns between its left and right edges, as Word lays
/// out RTF rows whose cells don't line up. Returns the widths in points, or none (equal columns)
/// when the edges don't make a grid.
pub(super) fn column_grid(rows: &mut [Vec<Cell>], rights: &[Vec<i32>]) -> Vec<f32> {
    let mut edges: Vec<i32> = rights.iter().flatten().copied().filter(|&x| x > 0).collect();
    edges.sort_unstable();
    edges.dedup();
    let ok = rights.len() == rows.len()
        && rows
            .iter()
            .zip(rights)
            .all(|(r, x)| r.len() == x.len() && x.windows(2).all(|w| w.first() < w.get(1)) && x.first().is_some_and(|&v| v > 0))
        && !edges.is_empty()
        && edges.len() <= 63;
    if !ok {
        return Vec::new();
    }
    for (row, x) in rows.iter_mut().zip(rights) {
        let mut left = 0;
        for (c, &right) in row.iter_mut().zip(x) {
            let span = edges.iter().filter(|&&e| e > left && e <= right).count().max(1);
            c.colspan = span as u32;
            left = right;
        }
    }
    let mut prev = 0;
    edges
        .iter()
        .map(|&e| {
            let w = (e - prev) as f32 / 20.0;
            prev = e;
            w
        })
        .collect()
}

/// Gives the sections' tracked formatting changes (`\oldsprops`) their revision: RTF names no
/// author or date for one, so it is the "Unknown" author's, undated.
fn set_section_revisions(doc: &mut Document) {
    use wordcraft_doc::{Block, Revision, RevisionKind};
    let changed = doc.last_section.fmt_change.is_some()
        || doc.body.iter().any(|b| matches!(&**b, Block::Para(p) if p.section.as_ref().is_some_and(|s| s.fmt_change.is_some())));
    if !changed {
        return;
    }
    let found =
        doc.revisions.iter().position(|r| r.kind == RevisionKind::Format && r.author == "Unknown" && r.date.is_empty() && r.move_name.is_none());
    let rev = match found {
        Some(i) => u32::try_from(i).unwrap_or(u32::MAX),
        None => {
            doc.revisions.push(Revision::new(RevisionKind::Format, "Unknown", ""));
            u32::try_from(doc.revisions.len().saturating_sub(1)).unwrap_or(u32::MAX)
        }
    };
    for b in &mut doc.body {
        if let Block::Para(p) = &**b
            && p.section.as_ref().is_some_and(|s| s.fmt_change.is_some())
            && let Block::Para(p) = std::sync::Arc::make_mut(b)
            && let Some(c) = p.section.as_deref_mut().and_then(|s| s.fmt_change.as_deref_mut())
        {
            c.rev = rev;
        }
    }
    if let Some(c) = doc.last_section.fmt_change.as_deref_mut() {
        c.rev = rev;
    }
}

/// Parse RTF bytes into a document.
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    model::begin_import();
    let Parsed {
        flow,
        info,
        last_section,
        default_font,
        headers_footers,
        foreign_styles,
        own_styles,
        mut notes,
        comments,
        note_seps,
        doc_notes,
        fet,
        doc_opts,
        doc_set,
        latent_styles,
        ..
    } = parse_full(bytes, true)?;
    let mut doc = model::to_doc_in(&flow, last_section);
    info.apply(&mut doc);
    doc_opts.apply(&mut doc.settings);
    doc_set.apply(&mut doc.settings);
    let (mut foot, mut end) = doc_notes;
    if let Some(f) = foot.num_fmt.take() {
        doc.settings.footnote_format = f;
    }
    if let Some(f) = end.num_fmt.take() {
        doc.settings.endnote_format = f;
    }
    // Word reads a file that never says where footnotes go (no `\ftnbj` or `\ftntj` in the
    // document formatting, `\fet` absent or 0) as holding endnotes: every `{\footnote}` without
    // `\ftnalt` is an endnote then, numbered i, ii… whatever `\aftnn…` says.
    if foot.pos.is_none() && fet.unwrap_or(0) == 0 && notes.iter().any(|n| !n.endnote) {
        for n in &mut notes {
            n.endnote = true;
        }
        doc.settings.endnote_format = wordcraft_doc::section::NumFormat::LowerRoman;
    }
    (doc.settings.footnote_pr, doc.settings.endnote_pr) = (foot, end);
    // Each section's own headers and footers; those it leaves out stay linked to the previous.
    let mut sets = Vec::with_capacity(headers_footers.len());
    for parts in &headers_footers {
        let (mut headers, mut footers) = (wordcraft_doc::section::HeaderSet::default(), wordcraft_doc::section::HeaderSet::default());
        for (which, blocks) in parts {
            let blocks = model::part_blocks(&mut doc, blocks);
            let id = doc.add_part(if which.footer { wordcraft_doc::PartKind::Footer } else { wordcraft_doc::PartKind::Header }, blocks);
            let set = if which.footer { &mut footers } else { &mut headers };
            match which.pages {
                HfPages::Default => set.default = Some(id),
                HfPages::First => set.first = Some(id),
                HfPages::Even => set.even = Some(id),
            }
        }
        sets.push((headers, footers));
    }
    let mut sets = sets.into_iter();
    for b in &mut doc.body {
        if let wordcraft_doc::Block::Para(p) = &**b
            && p.section.is_some()
            && let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(b)
            && let Some(s) = p.section.as_deref_mut()
            && let Some((h, f)) = sets.next()
        {
            (s.headers, s.footers) = (h, f);
        }
    }
    if let Some((h, f)) = sets.next() {
        (doc.last_section.headers, doc.last_section.footers) = (h, f);
    }
    set_section_revisions(&mut doc);
    place_notes_and_comments(&mut doc, notes, comments);
    place_note_separators(&mut doc, note_seps);
    // A drop cap's letter paragraph joins the paragraph it starts.
    super::frames::merge_drop_caps(&mut doc);
    // RTF's paragraph defaults are single spacing and no space around paragraphs; what a file
    // wants beyond that it states on each paragraph.
    let dp = &mut Arc::make_mut(&mut doc.styles).default_para;
    dp.space_before = Some(0.0);
    dp.space_after = Some(0.0);
    dp.line_spacing = Some(wordcraft_doc::props::LineSpacing::Multiple(1.0));
    if latent_styles.is_some() {
        Arc::make_mut(&mut doc.styles).latent_styles = latent_styles;
    }
    if default_font.is_some() {
        Arc::make_mut(&mut doc.styles).default_chr.font = default_font;
    }
    // Another program's styles have what its stylesheet sets, not WordCraft's built-in
    // formatting; the outline level a heading style gives stays.
    for (id, para, chr) in foreign_styles {
        if let Some(s) = Arc::make_mut(&mut doc.styles).get_mut(&id) {
            s.para = wordcraft_doc::props::ParaProps { outline_level: s.para.outline_level, ..para };
            s.chr = chr;
        }
    }
    // The document's own styles, which its paragraphs and runs already name.
    for style in own_styles {
        Arc::make_mut(&mut doc.styles).upsert(style);
    }
    // Without `\widowctrl` Word has widow control off for every paragraph that doesn't say.
    if !doc_opts.widow_control {
        set_widow_control_off(&mut doc);
    }
    Ok(doc)
}

/// Turns widow control off for every paragraph that doesn't set it, in every story: the body,
/// table cells (nested too), headers and footers, notes, comments and text boxes.
fn set_widow_control_off(doc: &mut Document) {
    use wordcraft_doc::StoryRef;
    let stories: Vec<StoryRef> = std::iter::once(StoryRef::Body).chain(doc.parts.keys().map(|&id| StoryRef::Part(id))).collect();
    for story in stories {
        for path in doc.para_paths(story) {
            let unset = doc.para(story, &path).is_some_and(|p| p.props.widow_control.is_none());
            if !unset {
                continue;
            }
            if let Ok(p) = doc.para_mut(story, &path) {
                p.props.widow_control = Some(false);
                p.touch();
            }
        }
    }
}

/// Turn the place markers the reader left in the text into note references and comment ranges,
/// and add the notes' and comments' stories to `doc`.
fn place_notes_and_comments(doc: &mut Document, notes: Vec<NoteData>, comments: Vec<CommentData>) {
    use wordcraft_doc::para::NoteKind;
    use wordcraft_doc::{Comment, PartKind};
    if notes.is_empty() && comments.is_empty() {
        return;
    }
    // (kind, part id, custom mark) per note, by index.
    let mut note_parts: Vec<(NoteKind, u32, String)> = Vec::new();
    for n in notes {
        let (pk, nk, style) =
            if n.endnote { (PartKind::Endnote, NoteKind::Endnote, "EndnoteText") } else { (PartKind::Footnote, NoteKind::Footnote, "FootnoteText") };
        let mut blocks = model::part_blocks(doc, &n.blocks);
        for b in &mut blocks {
            if let Block::Para(p) = Arc::make_mut(b)
                && p.props.style.is_none()
            {
                p.props.style = Some(style.to_string());
            }
        }
        note_parts.push((nk, doc.add_part(pk, blocks), n.custom));
    }
    // RTF range id to the comment's id in the document.
    let mut by_rtf: HashMap<i64, u32> = HashMap::new();
    for (k, c) in comments.into_iter().enumerate() {
        let id = k as u32;
        let blocks = model::part_blocks(doc, &c.blocks);
        let part = doc.add_part(PartKind::Comment, blocks);
        doc.comments
            .insert(id, Comment { author: c.author, initials: c.initials, date: c.date, parent: None, resolved: false, part, ..Default::default() });
        by_rtf.insert(i64::from(c.rtf_id), id);
    }
    let ctx = Placer { note_parts, by_rtf };
    let mut body = std::mem::take(&mut doc.body);
    ctx.blocks(&mut body, None, 0);
    doc.body = body;
    let ids: Vec<u32> = doc.parts.keys().copied().collect();
    for id in ids {
        let Some(part) = doc.parts.get_mut(&id) else { continue };
        let own = ctx.note_parts.iter().find(|(_, p, _)| *p == id).map(|(k, p, c)| (*k, *p, c.as_str()));
        ctx.blocks(&mut part.blocks, own, 0);
        // A note starts with its own number (or custom mark), as Word's do.
        if let Some((kind, pid, custom)) = own
            && let Some(first) = part.blocks.first_mut()
            && let Block::Para(p) = Arc::make_mut(first)
            && !p.objects.iter().any(|o| matches!(o, InlineObject::NoteRef { id, .. } if *id == pid))
        {
            let props = Placer::ref_props(kind);
            if p.insert_object(0, InlineObject::NoteRef { kind, id: pid, custom: custom.to_string() }, &props).is_err() {
                // The paragraph can't take it; the note simply has no number mark.
            }
        }
    }
}

/// The document's own note separator stories, in the order of [`super::state::SEP_WORDS`].
fn place_note_separators(doc: &mut Document, seps: [Option<Vec<FBlock>>; 6]) {
    let ctx = Placer { note_parts: Vec::new(), by_rtf: HashMap::new() };
    for (k, story) in seps.into_iter().enumerate() {
        let Some(story) = story else { continue };
        let mut blocks = model::part_blocks(doc, &story);
        ctx.blocks(&mut blocks, None, 0);
        // Word's default groups (one paragraph holding only the kind's own line, no paragraph
        // formatting of its own) store nothing: the built-in separator is drawn.
        if is_default_separator(&blocks, k % 3 == 1) {
            continue;
        }
        let s = if k < 3 { &mut doc.footnote_separators } else { &mut doc.endnote_separators };
        let slot = match k % 3 {
            0 => &mut s.separator,
            1 => &mut s.continuation_separator,
            _ => &mut s.continuation_notice,
        };
        *slot = Some(blocks);
    }
}

/// One paragraph without text or direct paragraph formatting holding only the separator line
/// (`continuation`: the continuation line).
fn is_default_separator(blocks: &Blocks, continuation: bool) -> bool {
    let [b] = blocks.as_slice() else { return false };
    let Block::Para(p) = &**b else { return false };
    let [o] = p.objects.as_slice() else { return false };
    // Every RTF paragraph without `\aspalpha`/`\aspnum` says no automatic Asian spacing, and
    // `\ql` is the default alignment (written for every paragraph): not formatting of its own.
    let plain = wordcraft_doc::props::ParaProps {
        style: p.props.style.clone(),
        align: p.props.align.filter(|a| *a == wordcraft_doc::Align::Left),
        auto_space_de: p.props.auto_space_de,
        auto_space_dn: p.props.auto_space_dn,
        ..Default::default()
    };
    o.as_note_separator() == Some(continuation) && p.plain_text().trim().is_empty() && p.props == plain
}

struct Placer {
    note_parts: Vec<(wordcraft_doc::para::NoteKind, u32, String)>,
    by_rtf: HashMap<i64, u32>,
}

impl Placer {
    fn ref_props(kind: wordcraft_doc::para::NoteKind) -> wordcraft_doc::CharProps {
        let style = if kind == wordcraft_doc::para::NoteKind::Endnote { "EndnoteReference" } else { "FootnoteReference" };
        wordcraft_doc::CharProps { style: Some(style.into()), ..Default::default() }
    }

    fn blocks(&self, bl: &mut Blocks, own: Option<(wordcraft_doc::para::NoteKind, u32, &str)>, depth: usize) {
        for b in bl.iter_mut() {
            let needs = match &**b {
                Block::Para(p) => p.text.contains(MARK_OPEN),
                Block::Table(_) => depth < 8,
            };
            if !needs {
                continue;
            }
            match Arc::make_mut(b) {
                Block::Para(p) => self.para(p, own),
                Block::Table(t) => {
                    for r in &mut t.rows {
                        for c in &mut r.cells {
                            self.blocks(&mut c.blocks, own, depth + 1);
                        }
                    }
                }
            }
        }
    }

    fn para(&self, p: &mut Paragraph, own: Option<(wordcraft_doc::para::NoteKind, u32, &str)>) {
        // Every pass removes one marker, so this ends; the bound is only a seat belt.
        for _ in 0..100_000 {
            let Some(a) = p.text.find(MARK_OPEN) else { return };
            let Some(len) = p.text.get(a..).and_then(|t| t.find(MARK_CLOSE)) else { return };
            let end = a + len + MARK_CLOSE.len_utf8();
            let inner = p.text.get(a + MARK_OPEN.len_utf8()..a + len).unwrap_or("").to_string();
            let mut chars = inner.chars();
            let tag = chars.next().unwrap_or(' ');
            let n = chars.as_str().parse::<i64>().unwrap_or(i64::MIN);
            let (obj, props) = match tag {
                'N' => match usize::try_from(n).ok().and_then(|i| self.note_parts.get(i)) {
                    Some((kind, id, custom)) => {
                        (Some(InlineObject::NoteRef { kind: *kind, id: *id, custom: custom.clone() }), Self::ref_props(*kind))
                    }
                    None => (None, Default::default()),
                },
                'S' => match own {
                    Some((kind, id, custom)) => (Some(InlineObject::NoteRef { kind, id, custom: custom.to_string() }), Self::ref_props(kind)),
                    None => (None, Default::default()),
                },
                // A separator line (`\chftnsep`, `\chftnsepc`: 1).
                'L' => (Some(InlineObject::note_separator(n == 1)), Default::default()),
                'B' => (self.by_rtf.get(&n).map(|&id| InlineObject::CommentStart { id }), Default::default()),
                'E' => (self.by_rtf.get(&n).map(|&id| InlineObject::CommentEnd { id }), Default::default()),
                _ => (None, Default::default()),
            };
            if p.delete(a, end).is_err() {
                return;
            }
            if let Some(o) = obj
                && p.insert_object(a, o, &props).is_err()
            {
                return;
            }
        }
    }
}
