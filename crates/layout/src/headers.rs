use super::boxes::{BoxInput, layout_box};
use super::cache::{Totals, hash_of};
use super::ctx::{Ctx, with_pct_base, with_pin};
use super::floats::WrapArea;
use super::flow::again;
use super::page::{PageBuilder, PageFrame, page_border_rules};
use super::placed::{HfSlot, Placed};
use super::{
    Arc, Document, HashMap, HashSet, HeaderSet, InlineObject, Page, Path, Rect, SectionProps, SectionStart, StoryRef, ViewMode, fields, styleref,
    work,
};

/// Whether page `i` is the first page of its section (the one its First Page header and footer,
/// `w:titlePg`, go on): the section at the top of the page starts on it, its first block being the
/// first body content there and not continued from the page before. A section that starts part-way
/// down a page (a Continuous break) has no first page of its own: the next page is an ordinary
/// one, as Word prints it.
pub(crate) fn starts_section(pages: &[Page], i: usize, sections: &[(usize, &SectionProps)]) -> bool {
    let Some(page) = pages.get(i) else { return false };
    let start = match page.section.checked_sub(1) {
        None => 0,
        Some(k) => match sections.get(k) {
            Some((end, _)) => end.saturating_add(1),
            None => return false,
        },
    };
    if page.first_block != start {
        return false;
    }
    let Some(prev) = i.checked_sub(1).and_then(|k| pages.get(k)) else { return true };
    let continued = prev.items.iter().any(|it| match it {
        Placed::Lines { story: StoryRef::Body, path, .. } | Placed::Image { story: StoryRef::Body, path, .. } => {
            path.0.first().is_some_and(|&b| b as usize == start)
        }
        _ => false,
    });
    !continued
}

/// Whether a section that starts on a new page needs a blank page first so it lands on the right
/// side, and the page number it restarts at. `last` is the number of the page before it and
/// `pages` how many pages there are so far.
/// - An odd- or even-page break counts on from the previous section's numbers. A section that
///   restarts its numbering gets no blank page unless the document prints both sides (different
///   odd and even headers, or mirror margins); a restart at the wrong parity takes the next number.
/// - With different odd and even headers or mirror margins, a next-page break that restarts the
///   numbers at an odd (even) number starts on an odd (even) sheet, the side told by the number of
///   the page before (not the page count), so a blank page goes first when the restart has the
///   same parity as `last`.
///
/// (Word's output for docxide-pdf's fixtures: bush_fires_act_comparison has a blank page 2.)
pub(crate) fn section_side(start: SectionStart, restart: Option<u32>, last: u32, pages: usize, even_odd: bool, mirror: bool) -> (bool, Option<u32>) {
    match start {
        SectionStart::OddPage | SectionStart::EvenPage => {
            let want_odd = start == SectionStart::OddPage;
            let continuing_odd = last.saturating_add(1) % 2 == 1;
            let filler = pages > 0 && continuing_odd != want_odd && (restart.is_none() || even_odd || mirror);
            let restart = restart.map(|n| if (n % 2 == 1) == want_odd { n } else { n.saturating_add(1) });
            (filler, restart)
        }
        SectionStart::NextPage => {
            let filler = (even_odd || mirror) && pages > 0 && restart.is_some_and(|n| n % 2 == last % 2);
            (filler, restart)
        }
        SectionStart::Continuous | SectionStart::NextColumn => (false, restart),
    }
}

/// The number the document's first page takes from its section's start type and restart
/// (`None`: it counts on from 0, so it is page 1). Word puts no blank page in front of an
/// even-page first section: it numbers that page 2 (or the first even number at or after the
/// restart), so later odd- and even-page breaks are decided against those numbers. An odd-page
/// first section restarting at an even number takes the next one, as later sections do.
pub(crate) fn first_page_number(start: SectionStart, restart: Option<u32>) -> Option<u32> {
    let to_even = |n: u32| if n.is_multiple_of(2) { n } else { n.saturating_add(1) };
    match start {
        SectionStart::EvenPage => Some(to_even(restart.unwrap_or(2))),
        SectionStart::OddPage => restart.map(|n| if n % 2 == 1 { n } else { n.saturating_add(1) }),
        SectionStart::NextPage | SectionStart::Continuous | SectionStart::NextColumn => restart,
    }
}

/// The document's sections with their headers and footers (Word's "link to previous"), each
/// section's resolved once for the whole layout in document order rather than by looking back
/// over the sections before it whenever one is needed.
pub(crate) struct HfLinks<'a> {
    pub(crate) sections: Vec<(usize, &'a SectionProps)>,
    pub(crate) heads: Linked,
    pub(crate) feet: Linked,
}

impl<'a> HfLinks<'a> {
    pub(crate) fn new(sections: Vec<(usize, &'a SectionProps)>) -> Self {
        let heads = Linked::new(&sections, |s| s.headers);
        let feet = Linked::new(&sections, |s| s.footers);
        HfLinks { sections, heads, feet }
    }

    /// The text width of the section that defines header (or, `footer`, footer) `id`, when
    /// section `si` inherits it rather than naming it (`None` when it names it). Measured in
    /// Word: an inherited part's percentage-width tables are sized by that section, though it's
    /// laid out on `si`'s pages.
    pub(crate) fn defining_width(&self, footer: bool, si: usize, id: Option<u32>) -> Option<f32> {
        let id = id?;
        work::steps(1);
        let linked = if footer { &self.feet } else { &self.heads };
        let own = self.sections.get(si)?.1;
        if linked.own.get(si).is_some_and(|h| [h.default, h.first, h.even].contains(&Some(id))) {
            return None;
        }
        // The last section before `si` naming it.
        let namers = linked.namers.get(&id)?;
        let k = *namers.get(namers.partition_point(|&k| k < si).checked_sub(1)?)?;
        self.sections.get(k).map(|(_, s)| s.text_width()).filter(|w| (w - own.text_width()).abs() > 0.01)
    }
}

/// The headers (or footers) of every section: those it names, and with those it leaves out
/// inherited from the sections before it.
pub(crate) struct Linked {
    own: Vec<HeaderSet>,
    linked: Vec<HeaderSet>,
    /// The sections naming each part, in order.
    namers: HashMap<u32, Vec<usize>>,
}

impl Linked {
    fn new(sections: &[(usize, &SectionProps)], get: fn(&SectionProps) -> HeaderSet) -> Self {
        let own: Vec<HeaderSet> = sections.iter().map(|(_, s)| get(s)).collect();
        let mut linked = Vec::with_capacity(own.len());
        let mut namers: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut prev = HeaderSet::default();
        for (si, o) in own.iter().enumerate() {
            work::steps(1);
            prev = HeaderSet { default: o.default.or(prev.default), first: o.first.or(prev.first), even: o.even.or(prev.even) };
            linked.push(prev);
            for id in [o.default, o.first, o.even].into_iter().flatten() {
                let v = namers.entry(id).or_default();
                if v.last() != Some(&si) {
                    v.push(si);
                }
            }
        }
        Linked { own, linked, namers }
    }

    /// Section `si`'s headers (or footers), those it leaves out inherited.
    pub(crate) fn set(&self, si: usize) -> HeaderSet {
        self.linked.get(si).copied().unwrap_or_default()
    }
}

/// Header rows are re-placed on every page a table spills onto, so a table is O(header items ×
/// pages) placed items — and an exact-height header row keeps all its overflowing text without
/// growing, so a few KB of document could otherwise fill memory. Once this many items (a row
/// counts at least one) have been repeated across the whole layout (every table together), later pages carry no header rows;
/// pagination still finishes. Far above any real table (three ordinary header rows over some
/// 5 000 pages).
pub(crate) const MAX_HEADER_ITEMS: usize = 500_000;

#[cfg(test)]
thread_local! {
    /// A smaller [`MAX_HEADER_ITEMS`] for tests, so a short table can reach it.
    pub(crate) static HEADER_ITEMS_CAP: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_HEADER_ITEMS) };
}

/// [`MAX_HEADER_ITEMS`], or the test cap ([`HEADER_ITEMS_CAP`]) in tests.
pub(crate) fn header_items_cap() -> usize {
    #[cfg(test)]
    return HEADER_ITEMS_CAP.with(|c| c.get());
    #[cfg(not(test))]
    MAX_HEADER_ITEMS
}

/// What re-placing `it` costs against [`MAX_HEADER_ITEMS`] (and laying it out against
/// [`textbox::MAX_FLOW_WORK`]): one, and for a clipped group (a floating Drawing Canvas) every
/// member below it, nested groups included, as placing clones them all. Counted with an explicit
/// stack, and counting stops once the total passes `limit` (the result is then just over it), so
/// neither deep nesting nor a huge group makes the count itself costly.
pub(crate) fn placed_cost(it: &Placed, limit: usize) -> usize {
    placed_cost_by(it, limit, |_| 1)
}

/// [`placed_cost`] with each item (the group itself and every member) costing `cost` of it.
pub(crate) fn placed_cost_by(it: &Placed, limit: usize, cost: impl Fn(&Placed) -> usize) -> usize {
    let mut count = 0usize;
    let mut stack: Vec<std::slice::Iter<Placed>> = Vec::new();
    let mut next = Some(it);
    loop {
        let cur = match next.take() {
            Some(c) => c,
            None => loop {
                match stack.last_mut() {
                    None => return count,
                    Some(iter) => match iter.next() {
                        Some(c) => break c,
                        None => {
                            stack.pop();
                        }
                    },
                }
            },
        };
        count = count.saturating_add(cost(cur));
        if count > limit {
            return count;
        }
        if let Placed::Clip { items, .. } = cur {
            stack.push(items.iter());
        }
    }
}

/// Header tables can displace a non-overlapping body table. Page-positioned drawings keep
/// their own wrapping rules, so only table areas are retained from the header layout.
pub(crate) fn header_table_areas(ctx: &mut Ctx, pb: &PageBuilder) -> Vec<Rect> {
    header_obstacles(ctx, pb).0
}

/// The wrap areas of the text frames of the page's header (page coordinates), the part below
/// the body's top: body text wraps around them like around a floating object with the frame's
/// wrap (Word's PDF export, every compatibility mode). A footer's frames are left alone.
pub(crate) fn header_frame_wraps(ctx: &mut Ctx, pb: &PageBuilder) -> Vec<WrapArea> {
    header_obstacles(ctx, pb).1
}

/// [`header_table_areas`] and [`header_frame_wraps`], from one layout of the page's header
/// (with that page's fields), cached by section, page number and whether it is the section's
/// first page.
fn header_obstacles(ctx: &mut Ctx, pb: &PageBuilder) -> (Vec<Rect>, Vec<WrapArea>) {
    if pb.web || ctx.opts.view == ViewMode::Read {
        return Default::default();
    }
    let Some(page) = pb.pages.last() else { return Default::default() };
    let first = starts_section(&pb.pages, pb.pages.len().saturating_sub(1), &ctx.hf.sections);
    let key = (page.section, page.number, first);
    if let Some(areas) = ctx.header_tables.get(&key) {
        return areas.clone();
    }
    let hf = Arc::clone(&ctx.hf);
    let Some((_, sect)) = hf.sections.get(page.section) else { return Default::default() };
    if sect.text_direction.is_turned() {
        return Default::default();
    }
    let set = hf.heads.set(page.section);
    let id = if sect.title_page && first {
        set.first
    } else if ctx.doc.settings.even_odd_headers && page.number.is_multiple_of(2) {
        set.even
    } else {
        set.default
    };
    let Some(id) = id else { return Default::default() };
    let Some(part) = ctx.doc.parts.get(&id) else { return Default::default() };
    let blocks = part.blocks.clone();
    let x = sect.text_left(ctx.mirror);
    let body_top = page.body.y;
    let frame = PageFrame { sect, margin_top: Some(page.body.y), origin: (x, sect.header), number: page.number, mirror: ctx.mirror };
    let base = hf.defining_width(false, page.section, Some(id));
    let counters = std::mem::take(&mut ctx.counters);
    let boxes = std::mem::take(&mut ctx.boxes);
    let chains = std::mem::take(&mut ctx.chains);
    let fields = ctx.fields.clone();
    let grid = std::mem::take(&mut ctx.grid);
    let track = std::mem::replace(&mut ctx.track_totals, false);
    ctx.fields.page_format = sect.page_num_format;
    ctx.fields.page = page.number;
    let laid = again(ctx, 0, |ctx| {
        with_pct_base(ctx, base, |ctx| layout_box(ctx, BoxInput::other(StoryRef::Part(id), &blocks, sect.text_width(), 0, Some(frame))))
    });
    let (mut areas, mut frames) = (laid.table_areas, laid.frame_wraps);
    if !sect.top_fixed
        && let Some(cut) = half_page_cut(&laid.items, StoryRef::Part(id), sect.page_h)
    {
        areas.retain_mut(|r| clamp_rect(r, cut, 0.01));
    }
    ctx.counters = counters;
    ctx.boxes = boxes;
    ctx.chains = chains;
    ctx.fields = fields;
    ctx.grid = grid;
    ctx.track_totals = track;
    for r in &mut areas {
        r.x += x;
        r.y += sect.header;
    }
    for (r, ..) in &mut frames {
        r.x += x;
        r.y += sect.header;
    }
    frames.retain(|(r, ..)| r.bottom() > body_top);
    let out = (areas, frames);
    ctx.header_tables.insert(key, out.clone());
    out
}

/// Whether a header, a footer or a text box shows a `PAGEREF` field, which they resolve from the
/// paginated body (see [`bookmark_pages`]).
pub(crate) fn headers_footers_refer_to_pages(doc: &Document) -> bool {
    use wordcraft_doc::PartKind;
    doc.parts.iter().filter(|(_, part)| matches!(part.kind, PartKind::Header | PartKind::Footer | PartKind::TextBox)).any(|(id, _)| {
        let story = StoryRef::Part(*id);
        doc.para_paths(story)
            .iter()
            .filter_map(|path| doc.para(story, path))
            .any(|p| p.objects.iter().any(|o| matches!(o, InlineObject::Field { instr, .. } if fields::field_name(instr) == "PAGEREF")))
    })
}

/// Where each bookmark in the body starts: the number and number format of the page it is placed
/// on (with two of the same name, the first in the document).
pub(crate) fn bookmark_pages(
    doc: &Document,
    pages: &[Page],
    sections: &[(usize, &SectionProps)],
) -> (fields::BookmarkPages, HashMap<String, String>) {
    // Each paragraph's bookmarks: name, offset, and its line in the paragraph layout last looked
    // up (the pieces of a paragraph usually share one).
    let mut wanted: HashMap<Path, Vec<(String, usize, Option<(usize, usize)>)>> = HashMap::new();
    let mut seen = HashSet::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        if !p.objects.iter().any(|o| matches!(o, InlineObject::BookmarkStart { .. })) {
            continue;
        }
        for (off, o) in p.object_offsets().into_iter().zip(&p.objects) {
            if let InlineObject::BookmarkStart { name } = o
                && seen.insert(name.as_str())
            {
                wanted.entry(path.clone()).or_default().push((name.clone(), off, None));
            }
        }
    }
    let mut out = fields::BookmarkPages::new();
    let mut chapters = HashMap::new();
    for page in pages {
        if wanted.is_empty() {
            break;
        }
        let format = sections.get(page.section).map(|(_, s)| s.page_num_format).unwrap_or_default();
        for it in &page.items {
            let Placed::Lines { story: StoryRef::Body, path, para, l0, l1, .. } = it else { continue };
            let Some(marks) = wanted.get_mut(path) else { continue };
            let layout = Arc::as_ptr(para) as usize;
            marks.retain_mut(|(name, off, line)| {
                let li = match *line {
                    Some((k, li)) if k == layout => li,
                    _ => {
                        let li = para.line_of(*off);
                        *line = Some((layout, li));
                        li
                    }
                };
                if !(*l0..*l1).contains(&li) {
                    return true;
                }
                chapters.insert(name.clone(), page.chapter.clone());
                out.insert(std::mem::take(name), (page.number, format));
                false
            });
            if marks.is_empty() {
                wanted.remove(path);
            }
        }
    }
    (out, chapters)
}

/// A key for caches standing for `marks` (0 for none).
pub(crate) fn marks_key(marks: &fields::BookmarkPages) -> u64 {
    if marks.is_empty() {
        return 0;
    }
    let mut v: Vec<_> = marks.iter().collect();
    v.sort_by(|a, b| a.0.cmp(b.0));
    hash_of(&v)
}

pub(crate) fn chapter_numbers(doc: &Document, pages: &mut [Page], sections: &[(usize, &SectionProps)]) {
    if !sections.iter().any(|(_, s)| s.chapter_style.is_some()) {
        return;
    }
    let mut counters = wordcraft_doc::numbering::Counters::default();
    let mut headings = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(p) = doc.para(StoryRef::Body, &path) else { continue };
        let props = doc.styles.resolve_para(&p.props);
        let Some(n) = p.props.numbering.or(props.numbering).filter(|n| n.num != 0) else { continue };
        let Some((label, definition)) = counters.next_label(&doc.numbering, n.num, n.level) else { continue };
        if matches!(definition.format, wordcraft_doc::section::NumFormat::Bullet | wordcraft_doc::section::NumFormat::None) {
            continue;
        }
        let level = p.props.outline_level.or(props.outline_level).filter(|l| *l < 9).or_else(|| {
            p.props
                .style
                .as_deref()
                .and_then(|s| s.strip_prefix("Heading"))
                .and_then(|s| s.trim().parse::<u8>().ok())
                .filter(|n| (1..=9).contains(n))
                .map(|n| n - 1)
        });
        if let Some(level) = level {
            let label = label.trim_end_matches(|c: char| !c.is_alphanumeric()).to_string();
            if !label.is_empty() {
                headings.push((path, level + 1, label));
            }
        }
    }
    for page in pages {
        let Some((_, sect)) = sections.get(page.section) else { continue };
        let Some(level) = sect.chapter_style else { continue };
        let top = page
            .items
            .iter()
            .filter_map(|it| match it {
                Placed::Lines { story: StoryRef::Body, path, .. } => Some(path.clone()),
                _ => None,
            })
            .min()
            .unwrap_or_else(|| Path::top(page.first_block));
        if let Some((_, _, label)) = headings.iter().rev().find(|(path, l, _)| *l == level && *path <= top) {
            use wordcraft_doc::section::ChapterSep;
            let sep = match sect.chapter_sep.unwrap_or(ChapterSep::Hyphen) {
                ChapterSep::Hyphen => "-",
                ChapterSep::Period => ".",
                ChapterSep::Colon => ":",
                ChapterSep::EmDash => "—",
                ChapterSep::EnDash => "–",
            };
            page.chapter = format!("{label}{sep}");
        }
    }
}

pub(crate) fn headers_footers(ctx: &mut Ctx, pages: &mut [Page], sections: &[(usize, &SectionProps)], totals: &Totals) {
    let total = pages.len() as u32;
    let hf = Arc::clone(&ctx.hf);
    let even_odd = ctx.doc.settings.even_odd_headers;
    let styles = styleref::headers_use_styleref(ctx.doc).then(|| ctx.cache.style_index(ctx.doc));
    let first_of_section: HashSet<usize> = (0..pages.len()).filter(|&i| starts_section(pages, i, sections)).collect();
    for (i, page) in pages.iter_mut().enumerate() {
        let Some((_, sect)) = sections.get(page.section) else { continue };
        // Inherit header/footer references from earlier sections (Word's "link to previous").
        let pick = |linked: &Linked| -> Option<u32> {
            let set = linked.set(page.section);
            if sect.title_page && first_of_section.contains(&i) {
                return set.first;
            }
            if even_odd && page.number % 2 == 0 {
                return set.even;
            }
            set.default
        };
        let hid = pick(&hf.heads);
        let fid = pick(&hf.feet);
        page.hf_slot = if sect.title_page && first_of_section.contains(&i) {
            HfSlot::First
        } else if even_odd && page.number % 2 == 0 {
            HfSlot::Even
        } else {
            HfSlot::Default
        };
        // The blank page putting the next section on its side shows no header or footer.
        if page.blank {
            page.header.clear();
            page.footer.clear();
            (page.header_story, page.footer_story) = (None, None);
            continue;
        }
        ctx.fields.page = page.number;
        ctx.fields.page_format = sect.page_num_format;
        ctx.fields.chapter = page.chapter.clone();
        ctx.fields.pages = total;
        ctx.fields.section_pages = totals.section(page.section);
        ctx.fields.section = page.section as u32 + 1;
        // STYLEREF: the body paragraphs on this page.
        ctx.fields.styleref = styles.as_ref().map(|ix| {
            let paths = page.items.iter().filter_map(|it| match it {
                Placed::Lines { story: StoryRef::Body, path, .. } => Some(path),
                _ => None,
            });
            (ix.clone(), ix.page_at(paths, page.first_block))
        });
        let w = sect.text_width();
        let x = sect.text_left(ctx.mirror);
        if let Some(id) = hid
            && let Some(part) = ctx.doc.parts.get(&id)
        {
            let blocks = part.blocks.clone();
            let frame = PageFrame { sect, margin_top: Some(page.body.y), origin: (x, sect.header), number: page.number, mirror: ctx.mirror };
            let base = hf.defining_width(false, page.section, hid);
            let (mut items, behind, front) = with_pin(ctx, id, sect, page.number, frame.margin_top, |ctx| {
                with_pct_base(ctx, base, |c| layout_box(c, BoxInput::other(StoryRef::Part(id), &blocks, w, 0, Some(frame))).items)
            });
            if !sect.top_fixed
                && let Some(cut) = half_page_cut(&items, StoryRef::Part(id), sect.page_h)
            {
                clip_header(&mut items, StoryRef::Part(id), cut);
            }
            for it in &mut items {
                it.translate(x, sect.header);
            }
            page.header = behind.into_iter().chain(items).chain(front).collect();
            page.header_story = Some(id);
        }
        if let Some(id) = fid
            && let Some(part) = ctx.doc.parts.get(&id)
        {
            let blocks = part.blocks.clone();
            let base = hf.defining_width(true, page.section, fid);
            // The footer's height fixes where it sits, which page-relative floats need.
            let (h, ..) = with_pin(ctx, id, sect, page.number, None, |ctx| {
                with_pct_base(ctx, base, |c| layout_box(c, BoxInput::other(StoryRef::Part(id), &blocks, w, 0, None)).height)
            });
            let frame = PageFrame { sect, margin_top: None, origin: (x, sect.page_h - sect.footer - h), number: page.number, mirror: ctx.mirror };
            let (mut items, behind, front) = with_pin(ctx, id, sect, page.number, frame.margin_top, |ctx| {
                with_pct_base(ctx, base, |c| layout_box(c, BoxInput::other(StoryRef::Part(id), &blocks, w, 0, Some(frame))).items)
            });
            for it in &mut items {
                it.translate(x, sect.page_h - sect.footer - h);
            }
            page.footer = behind.into_iter().chain(items).chain(front).collect();
            page.footer_story = Some(id);
        }
    }
    ctx.fields.styleref = None;
    ctx.fields.chapter.clear();
}

/// Mirrored margins: even pages have the inside margin (and gutter) on the right, so their
/// header, text, notes and footer move to start at the outside margin. Page borders, measured
/// from the page edge, stay.
pub(crate) fn mirror_even_pages(pages: &mut [Page], sections: &[(usize, &SectionProps)]) {
    for page in pages.iter_mut().filter(|p| p.number % 2 == 0) {
        let Some((_, s)) = sections.get(page.section) else { continue };
        let dx = s.margin_right - s.margin_left - s.gutter;
        if dx.abs() < 0.001 || !dx.is_finite() {
            continue;
        }
        let borders = page_border_rules(s).len();
        for it in page.items.iter_mut().skip(borders).chain(page.header.iter_mut()).chain(page.footer.iter_mut()) {
            it.translate(dx, 0.0);
        }
        page.body.x += dx;
    }
}

/// Where a header stops: the top of its first in-flow line or top-level table row whose bottom
/// lies more than half the page height below the top of the header's content (box coordinates,
/// the header's origin at 0). A header is never taller than that: the line or row that crosses
/// it is not printed, nor is anything after it, and the body starts below what is kept (a row
/// is dropped whole, as header rows never split). `None` for a header within the limit, which
/// lays out unchanged. Measured in Word on Letter, Letter landscape and A4 pages with header
/// distances of 18, 36 and 72pt: the limit doesn't depend on the margins.
pub(crate) fn half_page_cut(items: &[Placed], story: StoryRef, page_h: f32) -> Option<f32> {
    let limit = page_h / 2.0;
    if !limit.is_finite() || limit <= 0.0 {
        return None;
    }
    let mut cut = f32::INFINITY;
    // Top-level tables' rows: (table, row) -> (top, bottom).
    let mut rows: HashMap<(u32, usize), (f32, f32)> = HashMap::new();
    for it in items {
        match it {
            Placed::Lines { story: s, path, para, l0, l1, y, turn, .. } if *s == story && path.0.len() == 1 => {
                if turn.is_turned() || para.rp.frame.is_some() {
                    continue;
                }
                let Some(first) = para.lines.get(*l0) else { continue };
                for line in para.lines.iter().take(*l1).skip(*l0) {
                    let top = y + line.top - first.top;
                    if top + line.height > limit + 0.01 {
                        cut = cut.min(top);
                        break;
                    }
                }
            }
            Placed::Cell { rect, table, row, story: s, .. } if *s == story && table.0.len() == 1 => {
                let Some(&t) = table.0.first() else { continue };
                let e = rows.entry((t, *row)).or_insert((rect.y, rect.bottom()));
                e.0 = e.0.min(rect.y);
                e.1 = e.1.max(rect.bottom());
            }
            _ => {}
        }
    }
    // A row ends where the next one starts (a merged cell reaches further down), the last at
    // its cells' bottom.
    for (&(t, r), &(top, bottom)) in &rows {
        let bottom = rows.get(&(t, r.saturating_add(1))).map_or(bottom, |next| next.0);
        if bottom > limit + 0.01 {
            cut = cut.min(top);
        }
    }
    cut.is_finite().then_some(cut)
}

/// Leaves out of a header's `items` (box coordinates) what lies below `cut` (see
/// [`half_page_cut`]): its lines, table cells, shading, borders and inline pictures. Floating
/// objects and text boxes anchored in the header stay as they are.
pub(crate) fn clip_header(items: &mut Vec<Placed>, story: StoryRef, cut: f32) {
    const EPS: f32 = 0.01;
    let mut cut_paths: Vec<Path> = Vec::new();
    items.retain_mut(|it| match it {
        Placed::Lines { story: s, path, para, l0, l1, y, .. } if *s == story => {
            let Some(first) = para.lines.get(*l0) else { return true };
            let first_top = first.top;
            let keep = para.lines.iter().take(*l1).skip(*l0).take_while(|l| *y + l.top - first_top < cut - EPS).count();
            let end = l0.saturating_add(keep);
            if end < *l1 {
                cut_paths.push(path.clone());
                *l1 = end;
            }
            *l1 > *l0
        }
        Placed::Cell { rect, story: s, .. } if *s == story => clamp_rect(rect, cut, EPS),
        Placed::Fill { rect, .. } => clamp_rect(rect, cut, EPS),
        Placed::Rule { y0, y1, .. } => {
            if y0.min(*y1) >= cut - EPS && y0.max(*y1) > cut + EPS {
                return false;
            }
            // A border running on past the cut (a cell's side) stops there.
            *y0 = if *y0 > cut + EPS { cut } else { *y0 };
            *y1 = if *y1 > cut + EPS { cut } else { *y1 };
            true
        }
        _ => true,
    });
    // Inline pictures on the lines left out.
    items.retain(|it| match it {
        Placed::Image { rect, story: s, path, .. } | Placed::Graphic { rect, story: s, path, .. } if *s == story => {
            !(rect.y >= cut - EPS && cut_paths.contains(path))
        }
        _ => true,
    });
}

/// Keeps a box that starts above `cut`, cut off there; `false` for one that starts at or below it.
fn clamp_rect(rect: &mut Rect, cut: f32, eps: f32) -> bool {
    if rect.y >= cut - eps {
        return false;
    }
    if rect.bottom() > cut {
        rect.h = cut - rect.y;
    }
    true
}
