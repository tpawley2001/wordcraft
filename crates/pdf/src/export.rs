use super::exporter::Exporter;
use super::metadata::metadata;
use super::options::{PdfError, PdfOptions};
use super::paint::{clamp_side, ok, rect_path};
use super::{
    DocLayout, Document, FillRule, HashMap, HashSet, KrillaError, LayoutCache, Page, PageSettings, Rect, Size, Transform, WriteError, Writer,
    imposition, krilla, layout, write,
};

/// Lay out `doc` and write it as PDF. Without markup the layout is the final text: tracked
/// deletions are left out, as in Word's "No Markup" view. With `comments` each page carries the
/// markup area with the comment balloons.
pub fn export(doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    write::guard(|| {
        let lay = layout(doc, &mut LayoutCache::new(), &opts.layout_options());
        export_layout_inner(doc, &lay, None, opts)
    })
}

/// Write an existing layout of `doc` as PDF.
pub fn export_layout(doc: &Document, lay: &DocLayout, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    write::guard(|| export_layout_inner(doc, lay, None, opts))
}

/// [`export_layout`] followed by every page of a second document's layout, one page per sheet
/// (File › Options › Print document properties: the properties report after each copy of the
/// document, or after all of its pages when copies aren't collated). The appendix pages are
/// drawn in full even with [`PdfOptions::forms_data_only`] and take the paper of
/// [`PdfOptions::paper`]; the PDF has no structure tree, since its two documents would share one.
pub fn export_layout_with_appendix(
    doc: &Document,
    lay: &DocLayout,
    appendix: (&Document, &DocLayout),
    opts: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    write::guard(|| export_layout_inner(doc, lay, Some(appendix), opts))
}

/// Validate selected pages and retry embedding failures with outlined fonts.
fn export_layout_inner(doc: &Document, lay: &DocLayout, appendix: Option<(&Document, &DocLayout)>, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    let mut adjusted = opts.clone();
    adjusted.tagged &= adjusted.sheet_copies <= 1 && appendix.is_none();
    let opts = &adjusted;
    let pages: Vec<usize> = match &opts.pages {
        Some(v) => {
            if let Some(bad) = v.iter().find(|p| **p >= lay.pages.len()) {
                return Err(PdfError::BadPage(bad.saturating_add(1)));
            }
            v.clone()
        }
        None => (0..lay.pages.len()).collect(),
    };
    if pages.is_empty() {
        return Err(PdfError::NoPages);
    }
    // krilla subsets fonts only when the document is finished, so one font it can't subset fails
    // the whole export. Draw that font's glyphs as outlines instead and write the document again.
    let mut outlined = HashSet::new();
    loop {
        match write_attempt(doc, lay, appendix, opts, &pages, &outlined)? {
            Attempt::Done(bytes) => return Ok(bytes),
            Attempt::BadFont(id, msg) if outlined.len() < MAX_OUTLINED_FONTS && outlined.insert(id) => {
                log::warn!("PDF: {msg}; its text is drawn as outlines");
            }
            Attempt::BadFont(_, msg) => return Err(PdfError::Write(msg)),
        }
    }
}

/// How many unsubsettable fonts we fall back to outlines for before giving up.
const MAX_OUTLINED_FONTS: usize = 32;

/// One try at writing the PDF.
enum Attempt {
    Done(Vec<u8>),
    /// The face with this id could not be embedded (and why).
    BadFont(u32, String),
}

/// Apply a print job's optional paper substitution through the existing fit and slot geometry.
fn print_sheets(lay: &DocLayout, pages: &[usize], mode: imposition::Imposition, opts: &PdfOptions) -> Result<Vec<imposition::Sheet>, PdfError> {
    let mut sheets = mode.sheets(lay, pages, opts.paper)?;
    if opts.paper.is_none()
        && let Some(((sw, sh), (tw, th))) = opts.paper_substitution
    {
        for sheet in &mut sheets {
            let (w, h) = (sheet.width, sheet.height);
            if ((w - sw).abs() <= 2.0 && (h - sh).abs() <= 2.0) || ((h - sw).abs() <= 2.0 && (w - sh).abs() <= 2.0) {
                let fit = wordcraft_geom::fit_to_paper(w, h, tw, th);
                for slot in &mut sheet.slots {
                    slot.x *= fit.paper_w / w;
                    slot.y *= fit.paper_h / h;
                    slot.width *= fit.paper_w / w;
                    slot.height *= fit.paper_h / h;
                }
                sheet.width = fit.paper_w;
                sheet.height = fit.paper_h;
            }
        }
    }
    Ok(sheets)
}

/// Serialize physical sheet sides while mapping link and outline destinations onto their cells.
fn write_attempt<'a>(
    doc: &'a Document,
    lay: &'a DocLayout,
    appendix: Option<(&'a Document, &'a DocLayout)>,
    opts: &'a PdfOptions,
    pages: &[usize],
    outlined: &'a HashSet<u32>,
) -> Result<Attempt, PdfError> {
    let one_copy = print_sheets(lay, pages, opts.imposition, opts)?;
    let extra = match appendix {
        Some((_, alay)) if !alay.pages.is_empty() => {
            let all: Vec<usize> = (0..alay.pages.len()).collect();
            print_sheets(alay, &all, imposition::Imposition::Single, opts)?
        }
        _ => Vec::new(),
    };
    let copies = opts.sheet_copies.clamp(1, 999) as usize;
    let per_copy = one_copy.len().saturating_add(extra.len());
    if (opts.imposition != imposition::Imposition::Single || copies > 1 || !extra.is_empty()) && per_copy.checked_mul(copies).is_none_or(|n| n > 5000)
    {
        return Err(PdfError::Write("the print job exceeds 5000 sheet sides".into()));
    }
    // Each sheet side with whether it shows the appendix.
    let sheets: Vec<(bool, &imposition::Sheet)> = if opts.sheet_collate {
        (0..copies).flat_map(|_| one_copy.iter().map(|s| (false, s)).chain(extra.iter().map(|s| (true, s)))).collect()
    } else {
        let booklet = matches!(opts.imposition, imposition::Imposition::Booklet { .. });
        let main = repeat(&one_copy, copies, booklet).into_iter().map(|s| (false, s));
        main.chain(repeat(&extra, copies, false).into_iter().map(|s| (true, s))).collect()
    };
    let mut destinations = [HashMap::new(), HashMap::new()];
    for (output, (in_appendix, sheet)) in sheets.iter().enumerate() {
        let (part, part_lay) = match appendix {
            Some((_, alay)) if *in_appendix => (1, alay),
            _ => (0, lay),
        };
        for slot in &sheet.slots {
            let page = part_lay.pages.get(slot.page).ok_or(PdfError::BadPage(slot.page.saturating_add(1)))?;
            if let Some(map) = destinations.get_mut(part) {
                map.entry(slot.page).or_insert((output, Fit::in_slot(page, *slot)));
            }
        }
    }
    let [destinations, mut appendix_destinations] = destinations;
    let settings = krilla::SerializeSettings { compress_content_streams: opts.compress, enable_tagging: opts.tagged, ..Default::default() };
    let language = doc.styles.default_chr.lang.clone().filter(|l| !l.is_empty() && l.is_ascii());
    let mut writer = Writer::new_with_language(settings, metadata(doc, opts), language);
    let pdf = writer.document_mut();

    let mut ex = Exporter {
        doc,
        lay,
        opts,
        forms_data_only: opts.forms_data_only,
        background: opts.background,
        drawings: opts.drawings,
        out_index: destinations,
        fonts: HashMap::new(),
        missing_advances: HashMap::new(),
        font_error: None,
        outlined,
        cmaps: HashMap::new(),
        images: HashMap::new(),
        vectors: HashMap::new(),
        tags: Vec::new(),
        tag_index: HashMap::new(),
        links: Vec::new(),
        bookmarks: std::cell::OnceCell::new(),
    };
    if let Some(o) = ex.outline(pages) {
        pdf.set_outline(o);
    }
    let mut showing_appendix = false;
    for (in_appendix, sheet) in sheets {
        if let Some((adoc, alay)) = appendix
            && in_appendix != showing_appendix
        {
            // Switch documents: links and bookmarks resolve within the document drawn.
            showing_appendix = in_appendix;
            (ex.doc, ex.lay) = if in_appendix { (adoc, alay) } else { (doc, lay) };
            // Print only the data from a form is the document's setting: the appendix is a
            // report about the document and prints in full.
            ex.forms_data_only = opts.forms_data_only && !in_appendix;
            ex.background = opts.background || in_appendix;
            ex.drawings = opts.drawings || in_appendix;
            std::mem::swap(&mut ex.out_index, &mut appendix_destinations);
            ex.bookmarks = std::cell::OnceCell::new();
        }
        let page_lay = ex.lay;
        let size = Size::from_wh(clamp_side(sheet.width), clamp_side(sheet.height)).ok_or(PdfError::NoPages)?;
        let mut kp = pdf.start_page_with(PageSettings::new(size));
        let mut annotations = Vec::new();
        let mut surface = kp.surface();
        for slot in &sheet.slots {
            let page = page_lay.pages.get(slot.page).ok_or(PdfError::BadPage(slot.page.saturating_add(1)))?;
            let fit = Fit::in_slot(page, *slot);
            ex.links.clear();
            let clip = rect_path(&Rect::new(slot.x, slot.y, slot.width, slot.height));
            if let Some(clip) = &clip {
                surface.push_clip_path(clip, &FillRule::NonZero);
            }
            surface.push_transform(&Transform::from_row(fit.k, 0.0, 0.0, fit.k, fit.dx, fit.dy));
            ex.page(&mut surface, page, clamp_side(fit.page_w), clamp_side(page.h));
            surface.pop();
            if clip.is_some() {
                surface.pop();
            }
            if opts.links {
                annotations.extend(ex.annotations(fit));
            }
        }
        surface.finish();
        for annotation in annotations {
            kp.add_annotation(annotation);
        }
        kp.finish();
    }
    if opts.tagged {
        pdf.set_tag_tree(ex.tag_tree());
    }
    if let Some(error) = ex.font_error {
        return Err(error);
    }
    match writer.finish() {
        Ok(bytes) => Ok(Attempt::Done(bytes)),
        Err(WriteError::Serialization(error)) => match *error {
            KrillaError::Font(font, msg) => {
                let bad = ex.fonts.iter().find(|(_, (_, f))| f.as_ref() == Some(&font));
                match bad {
                    Some(((id, _), (face, _))) => {
                        Ok(Attempt::BadFont(*id, format!("font {} {} could not be embedded ({msg})", face.family, face.style)))
                    }
                    None => Err(PdfError::Write(format!("a font could not be embedded ({msg})"))),
                }
            }
            error => Err(PdfError::Write(format!("{error:?}"))),
        },
        Err(error) => Err(error.into()),
    }
}

/// Uncollated copies: each sheet side `copies` times in a row; a booklet's front/back pair must
/// remain one physical sheet, so it repeats as a pair.
fn repeat(one_copy: &[imposition::Sheet], copies: usize, booklet: bool) -> Vec<&imposition::Sheet> {
    if booklet {
        one_copy.chunks(2).flat_map(|pair| (0..copies).flat_map(move |_| pair.iter())).collect()
    } else {
        one_copy.iter().flat_map(|s| std::iter::repeat_n(s, copies)).collect()
    }
}

/// How a layout page maps onto its PDF page: a page with a markup area (Print Markup) is scaled
/// by `k` to its paper width and centred vertically (`dy`). With [`PdfOptions::paper`] that
/// sheet is then scaled to the chosen paper and centred on it (`dx`, `dy`). Any other page is
/// drawn as it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fit {
    /// The PDF page.
    paper_w: f32,
    paper_h: f32,
    /// The document page's width in layout points (without the markup area).
    page_w: f32,
    k: f32,
    dx: f32,
    dy: f32,
}

impl Fit {
    /// The physical output page dimensions, shared with the imposition planner.
    pub(crate) fn paper_size(self) -> (f32, f32) {
        (self.paper_w, self.paper_h)
    }

    /// Fit the complete logical page (including markup) within one fixed sheet cell.
    fn in_slot(page: &Page, slot: imposition::Slot) -> Self {
        let own = Self::of(page, None);
        let k = (slot.width / page.w).min(slot.height / page.h);
        Self {
            paper_w: slot.width,
            paper_h: slot.height,
            page_w: own.page_w,
            k,
            dx: slot.x + (slot.width - page.w * k) / 2.0,
            dy: slot.y + (slot.height - page.h * k) / 2.0,
        }
    }

    /// Map one document page and its markup area onto its requested paper.
    pub(crate) fn of(page: &Page, paper: Option<(f32, f32)>) -> Fit {
        let paper_w = page.w - page.markup;
        let own = if page.markup > 0.0 && ok(page.markup) && ok(page.w) && paper_w > 1.0 {
            let k = paper_w / page.w;
            Fit { paper_w, paper_h: page.h, page_w: paper_w, k, dx: 0.0, dy: page.h * (1.0 - k) / 2.0 }
        } else {
            Fit { paper_w: page.w, paper_h: page.h, page_w: page.w, k: 1.0, dx: 0.0, dy: 0.0 }
        };
        let Some((w, h)) = paper else { return own };
        let f = wordcraft_geom::fit_to_paper(own.paper_w, own.paper_h, w, h);
        Fit { paper_w: f.paper_w, paper_h: f.paper_h, page_w: own.page_w, k: own.k * f.k, dx: f.dx, dy: own.dy * f.k + f.dy }
    }

    /// A layout point on the PDF page.
    pub(crate) fn point(self, x: f32, y: f32) -> (f32, f32) {
        (x * self.k + self.dx, y * self.k + self.dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N-up fits the whole markup page once, with the same centering as the physical-sheet preview.
    #[test]
    fn imposed_markup_pages_fit_the_slot_without_double_scaling() {
        let page = wordcraft_layout::Page { w: 732.0, h: 792.0, markup: 120.0, ..Default::default() };
        for (width, height, scale) in [(1000.0, 396.0, 0.5), (183.0, 1000.0, 0.25)] {
            let slot = imposition::Slot { page: 0, x: 17.0, y: 23.0, width, height };
            let fit = Fit::in_slot(&page, slot);
            assert!((fit.k - scale).abs() < 0.00001);
            assert!((fit.dx - (17.0 + (width - 732.0 * scale) / 2.0)).abs() < 0.00001);
            assert!((fit.dy - (23.0 + (height - 792.0 * scale) / 2.0)).abs() < 0.00001);
            assert_eq!(fit.page_w, 612.0, "the unscaled document width excludes the balloon area");
            assert_eq!((fit.paper_w, fit.paper_h), (width, height));
        }
    }
}
