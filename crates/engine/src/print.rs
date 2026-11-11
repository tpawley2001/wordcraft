//! Print settings (#641): which pages go out, how many copies, as a PDF.
//!
//! WordCraft prints through a PDF (the host opens it in the system's print flow), so the settings
//! that can be delivered are the ones that shape the PDF: the page range, odd/even pages, copies,
//! collation and reverse page order. Hidden text and field codes use the per-user printing
//! options (or job overrides), independently of the on-screen display. Reverse order applies
//! only to single-page sheets. Physical sheet imposition is shared with the preview; printer
//! choice and duplex are handled by the host.
//!
//! Page ranges and copies come from users and control channels, so they are bounded: a range
//! text is at most [`MAX_SPEC_CHARS`] long, copies are clamped to [`MAX_COPIES`], and the
//! resulting page list never exceeds [`MAX_PRINT_PAGES`].

use serde_json::{Value, json};
use wordcraft_layout::DocLayout;
pub use wordcraft_pdf::imposition::Imposition;

use crate::{CmdError, Session, p};

/// Longest page-range text accepted.
pub const MAX_SPEC_CHARS: usize = 500;
/// Most copies one print job asks for.
pub const MAX_COPIES: u32 = 999;
/// Most pages (counting copies) one print job holds.
pub const MAX_PRINT_PAGES: usize = 5000;

/// Parses a custom page range like `1-3, 5, 8-` into 0-based page indexes, in the order typed
/// (Word prints them in document order; so does this: sorted, without repeats). Pages are
/// 1-based in the text; `8-` means "to the last page" and `-3` "from the first". An empty text,
/// page 0, a page past `total`, a reversed range or anything else is an error. The section forms
/// of [`parse_page_range_in`] see the whole document as section 1.
pub fn parse_page_range(spec: &str, total: usize) -> Result<Vec<usize>, String> {
    parse_page_range_in(spec, &unsectioned(total))
}

/// Page ids for `total` pages numbered 1, 2, 3… in one section.
fn unsectioned(total: usize) -> Vec<(usize, u32)> {
    (1..=total).map(|n| (1, u32::try_from(n).unwrap_or(u32::MAX))).collect()
}

/// Like [`parse_page_range`], with Word's section forms: `p2s1` is the first page whose
/// displayed number is 2 in section 1, `s3` every page of section 3, and `p2s1-p3s2` every page
/// from one to the other in document order. Plain numbers stay physical positions. `ids[i]` is
/// physical page `i`'s (1-based section, displayed page number).
pub fn parse_page_range_in(spec: &str, ids: &[(usize, u32)]) -> Result<Vec<usize>, String> {
    if spec.chars().count() > MAX_SPEC_CHARS {
        return Err("the page range is too long".into());
    }
    let total = ids.len();
    if total == 0 {
        return Err("the document has no pages".into());
    }
    let sections = ids.iter().map(|(s, _)| *s).max().unwrap_or(0);
    let mut keep = vec![false; total];
    let mut any = false;
    for item in spec.split([',', ';']) {
        let item: String = item.chars().filter(|c| !c.is_whitespace()).map(|c| c.to_ascii_lowercase()).collect();
        if item.is_empty() {
            continue;
        }
        let not_a_page = || format!("`{item}` isn't a page or a range like 2-4 or p1s2-p3s2");
        let number = |t: &str| -> Result<usize, String> {
            if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
                return Err(not_a_page());
            }
            t.parse().map_err(|_| not_a_page())
        };
        let page = |t: &str| -> Result<usize, String> {
            let n = number(t)?;
            if n == 0 || n > total {
                return Err(format!("page {n} doesn't exist; the document has {total} page{}", if total == 1 { "" } else { "s" }));
            }
            Ok(n)
        };
        let section = |t: &str| -> Result<usize, String> {
            let m = number(t)?;
            if m == 0 || m > sections {
                return Err(format!("section {m} doesn't exist; the document has {sections} section{}", if sections == 1 { "" } else { "s" }));
            }
            Ok(m)
        };
        // `pNsM` as a 1-based physical position: the first page numbered N in section M.
        let section_page = |t: &str| -> Result<usize, String> {
            let (n, m) = t.strip_prefix('p').and_then(|r| r.split_once('s')).ok_or_else(not_a_page)?;
            let (n, m) = (number(n)?, section(m)?);
            ids.iter()
                .position(|(s, num)| *s == m && usize::try_from(*num).is_ok_and(|num| num == n))
                .map(|i| i + 1)
                .ok_or_else(|| format!("section {m} has no page {n}"))
        };
        let sectioned = item.contains(['p', 's']);
        let (from, to) = if let Some(m) = item.strip_prefix('s') {
            let m = section(m)?;
            let first = ids.iter().position(|(s, _)| *s == m);
            let last = ids.iter().rposition(|(s, _)| *s == m);
            match (first, last) {
                (Some(a), Some(b)) => (a + 1, b + 1),
                _ => return Err(format!("section {m} has no pages")),
            }
        } else {
            match item.split_once('-') {
                None if sectioned => {
                    let n = section_page(&item)?;
                    (n, n)
                }
                None => {
                    let n = page(&item)?;
                    (n, n)
                }
                Some(("", "")) => return Err("`-` isn't a page or a range like 2-4".into()),
                Some((a, b)) if sectioned => {
                    if !(a.starts_with('p') && b.starts_with('p')) {
                        return Err(format!("`{item}` mixes a section page with a plain page; write both ends like p2s1"));
                    }
                    (section_page(a)?, section_page(b)?)
                }
                Some((a, b)) => {
                    let from = if a.is_empty() { 1 } else { page(a)? };
                    let to = if b.is_empty() { total } else { page(b)? };
                    (from, to)
                }
            }
        };
        if from > to {
            return Err(format!("`{item}` runs backwards"));
        }
        for flag in keep.iter_mut().take(to).skip(from.saturating_sub(1)) {
            *flag = true;
        }
        any = true;
    }
    if !any {
        return Err("type the pages to print, like 1-3, 5".into());
    }
    Ok(keep.iter().enumerate().filter(|(_, k)| **k).map(|(i, _)| i).collect())
}

/// The (1-based section, displayed page number) of each page of `lay`, for [`plan_in`].
pub fn page_ids(lay: &DocLayout) -> Vec<(usize, u32)> {
    lay.pages.iter().map(|pg| (pg.section.saturating_add(1), pg.number)).collect()
}

/// What a print job holds: the pages (0-based, copies included, in print order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrintPlan {
    /// Page indexes in print order, repeated for copies.
    pub pages: Vec<usize>,
    /// Distinct pages chosen (before copies).
    pub chosen: usize,
    pub copies: u32,
}

/// The Document Info reports File › Print offers besides the document (`what` of a print job).
pub const DOCUMENT_INFO: [&str; 5] = ["properties", "markup", "styles", "autotext", "keys"];

/// File › Print › Document Info: the paragraphs of the report Word prints for `what`
/// (`properties` | `markup` | `styles` | `autotext` | `keys`), one string per paragraph, in order.
pub fn document_info(s: &Session, what: &str) -> Result<Vec<String>, CmdError> {
    match what {
        "properties" => Ok(properties_report(s)),
        "markup" => Ok(markup_report(s)),
        "styles" => Ok(styles_report(s)),
        "autotext" => Ok(autotext_report(s)),
        "keys" => Ok(keys_report(s)),
        other => {
            Err(CmdError::Params(format!("unknown document info `{}`; use {}", other.chars().take(40).collect::<String>(), DOCUMENT_INFO.join(", "))))
        }
    }
}

/// A stored W3CDTF date as local date and time, or empty when there is none or it doesn't parse.
fn local_date(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .and_then(|d| u64::try_from(d.timestamp()).ok())
        .map(|unix| crate::cmd::insert::format_date_at("M/d/yyyy h:mm:ss am/pm", unix, crate::cmd::local_offset_secs(unix)))
        .unwrap_or_default()
}

/// Document Properties: Word's summary labels, a tab, then the value.
fn properties_report(s: &Session) -> Vec<String> {
    let c = &s.doc.core;
    let (file, dir) = s.path.as_deref().map_or_else(Default::default, |path| {
        (
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default(),
        )
    });
    let template = s.doc.settings.attached_template.as_deref().map(|t| t.rsplit(['/', '\\']).next().unwrap_or(t).to_string()).unwrap_or_default();
    let words: usize = s.counted_stories().iter().map(|st| s.doc.story_word_count(*st)).sum();
    let chars = wordcraft_doc::count_chars_no_spaces(&s.counted_text());
    let pages = s.export_layout().pages.len();
    let revision = if c.revision == 0 { String::new() } else { c.revision.to_string() };
    let line = |label: &str, value: &str| format!("{label}\t{value}");
    vec![
        line("Filename:", &file),
        line("Directory:", &dir),
        line("Template:", &template),
        line("Title:", &c.title),
        line("Subject:", &c.subject),
        line("Author:", &c.creator),
        line("Keywords:", &c.keywords),
        line("Comments:", &c.description),
        line("Creation Date:", &local_date(&c.created)),
        line("Change Number:", &revision),
        line("Last Saved On:", &local_date(&c.modified)),
        line("Last Saved By:", &c.last_modified_by),
        // The editing time isn't kept.
        line("Total Editing Time:", ""),
        line("Last Printed On:", &local_date(&c.last_printed)),
        "As of Last Complete Printing".to_string(),
        line("Number of Pages:", &pages.to_string()),
        line("Number of Words:", &words.to_string()),
        line("Number of Characters:", &chars.to_string()),
    ]
}

/// List of Markup: the tracked changes and comments in document order, each a heading line
/// (`Page N: [k] Kind`, author, date) and its text.
fn markup_report(s: &Session) -> Vec<String> {
    use wordcraft_doc::{Pos, StoryRef};
    // (reading-order key, heading kind, revision author and date, text)
    let rank: std::collections::BTreeMap<StoryRef, usize> = s.doc.shown_stories().into_iter().enumerate().map(|(i, st)| (st, i)).collect();
    let mut items: Vec<(Option<(usize, Pos)>, Pos, &str, String, String, String)> = Vec::new();
    for (a, b, kind, rev) in crate::cmd::review::tracked_changes(s) {
        let label = match kind {
            "insert" => "Inserted",
            "delete" => "Deleted",
            "format" => "Formatted",
            "moveFrom" => "Moved (from)",
            "moveTo" => "Moved (to)",
            _ => continue,
        };
        let r = rev.and_then(|r| s.doc.revisions.get(r as usize));
        let text = s.doc.copy_range(&a, &b).plain_text();
        let key = rank.get(&a.story).map(|i| (*i, a.clone()));
        items.push((key, a, label, r.map(|r| r.author.clone()).unwrap_or_default(), r.map(|r| r.date.clone()).unwrap_or_default(), text));
    }
    for (id, pos) in crate::cmd::review::comment_list(s) {
        let Some(c) = s.doc.comments.get(&id) else { continue };
        let text = s.doc.plain_text(StoryRef::Part(c.part));
        let at = pos.clone().unwrap_or_else(|| s.doc.end_of(StoryRef::Body));
        let key = pos.and_then(|p| rank.get(&p.story).map(|i| (*i, p)));
        items.push((key, at, "Comment", c.author.clone(), c.date.clone(), text));
    }
    if items.is_empty() {
        return Vec::new();
    }
    // Stable: changes and comments at one place keep their order; unanchored ones go last.
    items.sort_by(|x, y| match (&x.0, &y.0) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    let lay = s.export_layout();
    let mut out = Vec::with_capacity(items.len() * 2);
    for (k, (_, at, label, author, date, text)) in items.into_iter().enumerate() {
        let page = lay.caret_on(&at, 0).map_or(1, |c| c.page.saturating_add(1));
        out.push(format!("Page {page}: [{}] {label}\t{author}\t{}", k + 1, local_date(&date)));
        out.push(text);
    }
    out
}

/// Styles: the paragraph and character styles in use (applied to text in any story, or applied
/// while editing: Word keeps those in use) plus the document's own styles, by name, each
/// followed by its description.
fn styles_report(s: &Session) -> Vec<String> {
    use wordcraft_doc::{StoryRef, StyleKind};
    let sheet = &s.doc.styles;
    let mut used = s.applied_styles.clone();
    let stories = std::iter::once(StoryRef::Body).chain(s.doc.parts.keys().map(|id| StoryRef::Part(*id)));
    for story in stories {
        for path in s.doc.para_paths(story) {
            let Some(p) = s.doc.para(story, &path) else { continue };
            used.insert(p.props.style.clone().unwrap_or_else(|| "Normal".into()));
            used.extend(p.runs.iter().filter_map(|r| r.props.style.clone()));
        }
    }
    // A linked style's character half is listed as its paragraph style.
    let used: std::collections::BTreeSet<String> = used
        .into_iter()
        .filter_map(|id| sheet.find(&id))
        .map(|st| match st.linked.as_deref().and_then(|l| sheet.get(l)) {
            Some(para) if st.kind == StyleKind::Character && para.kind == StyleKind::Paragraph => para.id.clone(),
            _ => st.id.clone(),
        })
        .collect();
    let list = crate::cmd::para::manage_list(s, true);
    let mut out = Vec::new();
    for st in list.as_array().into_iter().flatten() {
        let id = st.get("id").and_then(Value::as_str).unwrap_or("");
        let kind = st.get("type").and_then(Value::as_str).unwrap_or("");
        let custom = !st.get("builtIn").and_then(Value::as_bool).unwrap_or(true);
        if kind == "table" || !(used.contains(id) || custom) {
            continue;
        }
        out.push(st.get("name").and_then(Value::as_str).unwrap_or(id).to_string());
        out.push(crate::cmd::para::style_description(st));
    }
    out
}

/// AutoText Entries: the document's AutoText entries by name, each followed by its text.
fn autotext_report(s: &Session) -> Vec<String> {
    let mut list: Vec<&wordcraft_doc::BuildingBlock> = s.doc.glossary.iter().filter(|e| e.gallery == wordcraft_doc::AUTO_TEXT_GALLERY).collect();
    list.sort_by_cached_key(|e| e.name.to_lowercase());
    list.into_iter()
        .flat_map(|e| {
            let frag = wordcraft_doc::edit::Fragment { blocks: e.blocks.iter().map(|b| (**b).clone()).collect(), parts: Default::default() };
            [e.name.clone(), frag.plain_text()]
        })
        .collect()
}

/// Key Assignments: the keyboard customizations (Customize Keyboard), one `name<Tab>key` line
/// each, by name then key. Built-in shortcuts and removed built-in keys aren't listed.
fn keys_report(s: &Session) -> Vec<String> {
    let commands = s.keymap.assigned.iter().filter(|(key, _)| !s.keymap.styles.contains_key(*key));
    let commands = commands.filter_map(|(key, id)| s.registry.get(id).map(|spec| (spec.label.to_string(), key)));
    let styles = s.keymap.styles.iter().map(|(key, style)| (s.doc.styles.get(style).map_or_else(|| style.clone(), |st| st.name.clone()), key));
    let mut list: Vec<(String, &String)> = commands.chain(styles).collect();
    list.sort_by_cached_key(|(name, key)| (name.to_lowercase(), (*key).clone()));
    list.into_iter().map(|(name, key)| format!("{name}\t{}", shown_key(key))).collect()
}

/// A canonical key as the Customize Keyboard dialog shows it: `Mod+` is Ctrl, or Cmd on macOS.
fn shown_key(key: &str) -> String {
    match key.strip_prefix("Mod+") {
        Some(rest) if cfg!(target_os = "macos") => format!("Cmd+{rest}"),
        Some(rest) => format!("Ctrl+{rest}"),
        None => key.to_string(),
    }
}

/// Print Selection prints no headers or footers (so no page numbers in them), as in Word.
fn no_headers(sec: &mut wordcraft_doc::section::SectionProps) {
    sec.headers = wordcraft_doc::section::HeaderSet::default();
    sec.footers = wordcraft_doc::section::HeaderSet::default();
    sec.title_page = false;
}

/// The report `lines` as a plain document: one Normal paragraph each, on the page size and
/// margins of `doc`'s first section, without its headers and footers.
pub fn report_document(doc: &wordcraft_doc::Document, lines: &[String]) -> wordcraft_doc::Document {
    let mut out = wordcraft_doc::Document::new();
    out.body = lines.iter().map(|l| wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(l, Default::default()))).collect();
    out.ensure_nonempty();
    let first = doc.sections().first().map(|(_, sec)| *sec).unwrap_or(&doc.last_section);
    let sec = &mut out.last_section;
    (sec.page_w, sec.page_h, sec.landscape) = (first.page_w, first.page_h, first.landscape);
    (sec.margin_top, sec.margin_bottom, sec.margin_left, sec.margin_right) =
        (first.margin_top, first.margin_bottom, first.margin_left, first.margin_right);
    (sec.top_fixed, sec.bottom_fixed, sec.header, sec.footer) = (first.top_fixed, first.bottom_fixed, first.header, first.footer);
    out
}

/// Works out the print plan for `v` against a layout of `total` pages whose caret and selection
/// sit on pages `current` (first, last). Params:
/// `range` (`all` | `current` | `selection` | `custom`; `custom` is implied by `pages`),
/// `pages` (text like `1-3,5`), `parity` (`all` | `odd` | `even`), `copies` (1–999),
/// `collate` (default true: 1,2,3 1,2,3; false: 1,1,2,2,3,3),
/// `reverse` (default false: reverse chosen pages before copies; ignored for N-up/booklet),
/// `duplexPass` (absent, `front` or `back`: Manually Print on Both Sides). In a duplex pass the
/// chosen pages are laid on sheets in job order, sheet k holding the (2k+1)th chosen page on its
/// front and the (2k+2)th on its back, and the pass keeps one side of each sheet. `frontReverse` /
/// `backReverse` (default false) print that pass in reverse order; they replace `reverse`, which a
/// duplex pass ignores. A duplex pass prints one copy, one page per sheet, with `parity` `all`.
pub fn plan(v: &Value, total: usize, current: (usize, usize)) -> Result<PrintPlan, CmdError> {
    plan_in(v, &unsectioned(total), current)
}

/// [`plan`] against a layout whose physical pages have these (1-based section, displayed
/// number) ids (see [`page_ids`]), so a custom range can name section pages like `p2s1`.
pub fn plan_in(v: &Value, ids: &[(usize, u32)], current: (usize, usize)) -> Result<PrintPlan, CmdError> {
    let total = ids.len();
    let bad = |m: String| CmdError::Params(m);
    let pages_text = p::str(v, "pages").filter(|t| !t.trim().is_empty());
    let range = p::str(v, "range").unwrap_or(if pages_text.is_some() { "custom" } else { "all" });
    let last = total.saturating_sub(1);
    let mut pages: Vec<usize> = match range {
        // Print Selection's layout is the selection's own: all of it prints.
        "all" | "selectedText" => (0..total).collect(),
        "current" => vec![current.0.min(last)],
        "selection" => (current.0.min(last)..=current.1.min(last).max(current.0.min(last))).collect(),
        "custom" => parse_page_range_in(pages_text.unwrap_or(""), ids).map_err(bad)?,
        other => return Err(bad(format!("unknown range `{other}`; use all, current, selection, selectedText or custom"))),
    };
    match p::str(v, "parity").unwrap_or("all") {
        "all" => {}
        // Page numbers are 1-based: the odd ones are indexes 0, 2, 4…
        "odd" => pages.retain(|i| i % 2 == 0),
        "even" => pages.retain(|i| i % 2 == 1),
        other => return Err(bad(format!("unknown parity `{other}`; use all, odd or even"))),
    }
    if pages.is_empty() {
        return Err(bad("no pages to print with these settings".into()));
    }
    let chosen = pages.len();
    if let Some(back) = duplex_pass(v)? {
        let (side, reverse_key) = if back { (1, "backReverse") } else { (0, "frontReverse") };
        pages = pages.into_iter().skip(side).step_by(2).collect();
        if pages.is_empty() {
            return Err(bad("no pages to print with these settings".into()));
        }
        if p::bool(v, reverse_key).unwrap_or(false) {
            pages.reverse();
        }
        return Ok(PrintPlan { pages, chosen, copies: 1 });
    }
    if p::bool(v, "reverse").unwrap_or(false) && imposition(v)? == Imposition::Single {
        pages.reverse();
    }
    let copies = p::u64(v, "copies").unwrap_or(1).clamp(1, u64::from(MAX_COPIES)) as u32;
    let total_out = chosen.checked_mul(copies as usize).filter(|n| *n <= MAX_PRINT_PAGES);
    let Some(total_out) = total_out else {
        return Err(bad(format!("{chosen} pages × {copies} copies is more than the {MAX_PRINT_PAGES} pages one print job can hold")));
    };
    let mut out = Vec::with_capacity(total_out);
    if p::bool(v, "collate").unwrap_or(true) {
        for _ in 0..copies {
            out.extend_from_slice(&pages);
        }
    } else {
        for &pg in &pages {
            out.extend(std::iter::repeat_n(pg, copies as usize));
        }
    }
    Ok(PrintPlan { pages: out, chosen, copies })
}

/// The `duplexPass` param of a print job: `None` for an ordinary job, `Some(false)` for the
/// fronts of the sheets, `Some(true)` for the backs. Manual duplex chooses the sides itself and
/// prints one copy of one page per sheet, so `parity`, `copies` and `pagesPerSheet` that ask for
/// anything else are errors.
fn duplex_pass(v: &Value) -> Result<Option<bool>, CmdError> {
    let back = match v.get("duplexPass") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(s)) if s == "front" => false,
        Some(Value::String(s)) if s == "back" => true,
        Some(other) => {
            let shown: String = other.to_string().chars().take(40).collect();
            return Err(CmdError::Params(format!("unknown duplexPass {shown}; use front or back")));
        }
    };
    if p::str(v, "parity").is_some_and(|s| s != "all") {
        return Err(CmdError::Params("manual duplex chooses the sides itself: use parity all with duplexPass".into()));
    }
    if p::u64(v, "copies").is_some_and(|n| n > 1) {
        return Err(CmdError::Params("manual duplex prints one copy per job: use copies 1 with duplexPass".into()));
    }
    if imposition(v)? != Imposition::Single {
        return Err(CmdError::Params("manual duplex prints one page per sheet: use pagesPerSheet 1 with duplexPass".into()));
    }
    Ok(Some(back))
}

/// The `paper` param of a print job: the standard paper (see [`wordcraft_geom::PAPER_SIZES`]) to
/// scale every page to (Scale to paper size, #2108), or `None` to print the document's own page
/// sizes (absent, null, `""`, `none` or `document`). Any other value is an error.
pub fn paper(v: &Value) -> Result<Option<(&'static str, f32, f32)>, CmdError> {
    match v.get("paper") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(name)) if name.trim().is_empty() || ["none", "document"].contains(&name.trim().to_ascii_lowercase().as_str()) => Ok(None),
        Some(Value::String(name)) => wordcraft_geom::paper_by_name(name).map(Some).ok_or_else(|| {
            let names: Vec<&str> = wordcraft_geom::PAPER_SIZES.iter().map(|(n, _, _)| *n).collect();
            CmdError::Params(format!("unknown paper `{}`; use none or one of {}", name.chars().take(40).collect::<String>(), names.join(", ")))
        }),
        Some(_) => Err(CmdError::Params("paper must be a paper size name or none".into())),
    }
}

/// Validate the physical sheet layout without touching document page setup.
pub fn imposition(v: &Value) -> Result<wordcraft_pdf::imposition::Imposition, CmdError> {
    match v.get("pagesPerSheet") {
        None => Ok(Imposition::Single),
        Some(Value::Number(n)) => match n.as_u64() {
            Some(1) => Ok(Imposition::Single),
            Some(n @ (2 | 4 | 6 | 9 | 16)) => Ok(Imposition::NUp(n as u8)),
            _ => Err(CmdError::Params("pagesPerSheet must be 1, 2, 4, 6, 9, 16, booklet or bookletReverse".into())),
        },
        Some(Value::String(s)) if s == "booklet" || s == "bookletReverse" => Ok(Imposition::Booklet { reverse: s == "bookletReverse" }),
        _ => Err(CmdError::Params("pagesPerSheet must be 1, 2, 4, 6, 9, 16, booklet or bookletReverse".into())),
    }
}

/// Resolve physical paper orientation for imposed jobs; ordinary export keeps its existing policy.
fn imposed_paper(v: &Value, lay: &DocLayout, pages: &[usize]) -> Result<Option<(f32, f32)>, CmdError> {
    let paper = paper(v)?.map(|(_, w, h)| (w, h));
    if imposition(v)? == Imposition::Single {
        return Ok(paper);
    }
    let own = pages.first().and_then(|p| lay.pages.get(*p)).map(|p| (p.w - p.markup, p.h));
    let Some((w, h)) = paper.or(own) else { return Ok(paper) };
    match v.get("orientation").and_then(Value::as_str).unwrap_or("auto") {
        "auto" => Ok(paper),
        "portrait" => Ok(Some((w.min(h), w.max(h)))),
        "landscape" => Ok(Some((w.max(h), w.min(h)))),
        _ => Err(CmdError::Params("orientation must be auto, portrait or landscape".into())),
    }
}

/// Plan physical sheet sides for one copy, using the same selection and geometry as printing.
pub fn sheets(v: &Value, lay: &DocLayout, current: (usize, usize)) -> Result<Vec<wordcraft_pdf::imposition::Sheet>, CmdError> {
    if !v.is_object() {
        return Err(CmdError::Params("print settings must be an object".into()));
    }
    let mut single = v.clone();
    single["copies"] = json!(1);
    let pages = plan_in(&single, &page_ids(lay), current)?.pages;
    imposition(v)?.sheets(lay, &pages, imposed_paper(v, lay, &pages)?).map_err(|e| CmdError::Failed(e.to_string()))
}

impl Session {
    /// The pages the caret (or selection) is on, in the output layout: (first, last).
    pub fn current_pages(&self, lay: &DocLayout) -> (usize, usize) {
        let (a, b) = self.sel.ordered();
        let first = lay.caret_on(&a, self.page_hint).map_or(0, |c| c.page);
        let last = lay.caret_on(&b, self.page_hint).map_or(first, |c| c.page);
        (first, last.max(first))
    }

    /// The document as the PDF of a print job `v` describes (see [`plan`]), with the number of
    /// distinct pages chosen and copies, as File › Print › Save these pages as PDF keeps it:
    /// links work. Does not touch the document.
    pub fn print_pdf(&self, v: &Value) -> Result<(Vec<u8>, PrintPlan), CmdError> {
        self.print_pdf_with(v, true)
    }

    /// The layout a print job with settings `v` renders. `hiddenText` and `fieldCodes` override
    /// the per-user print preferences; on-screen visibility does not affect printing.
    pub fn print_layout(&self, v: &Value) -> std::sync::Arc<DocLayout> {
        let markup = crate::cmd::file::export_options(self, v);
        let out = crate::cmd::references::output_fields(&self.doc);
        self.output_layout_of(
            out.as_ref().unwrap_or(&self.doc),
            markup.markup,
            p::bool(v, "hiddenText").unwrap_or(self.prefs.print_hidden_text),
            p::bool(v, "fieldCodes").unwrap_or(self.prefs.print_field_codes),
        )
    }

    /// The PDF sent to a printer: [`Self::print_pdf`] without link annotations, which print
    /// nothing and can prevent a printer driver fitting the page to paper (#4734).
    pub fn print_job_pdf(&self, v: &Value) -> Result<(Vec<u8>, PrintPlan), CmdError> {
        self.print_pdf_with(v, false)
    }

    /// The Document Info report a print job's `what` asks for (see [`document_info`]) as the
    /// document that prints instead ([`report_document`]) with its print layout; `None` for
    /// `document` (the default). An empty report is an error: there is nothing to print.
    pub fn print_report(&self, v: &Value) -> Result<Option<(wordcraft_doc::Document, std::sync::Arc<DocLayout>)>, CmdError> {
        let what = match v.get("what") {
            None | Some(Value::Null) => return Ok(None),
            Some(Value::String(w)) if w == "document" => return Ok(None),
            Some(Value::String(w)) => w.as_str(),
            Some(_) => return Err(CmdError::Params(format!("what must be document, {}", DOCUMENT_INFO.join(", ")))),
        };
        let lines = document_info(self, what)?;
        if lines.is_empty() {
            return Err(CmdError::Params(format!(
                "nothing to print: the document has no {}",
                match what {
                    "markup" => "tracked changes or comments",
                    "autotext" => "AutoText entries",
                    "styles" => "styles in use",
                    "keys" => "key assignments",
                    _ => "properties",
                }
            )));
        }
        let doc = report_document(&self.doc, &lines);
        let lay = self.output_layout_of(&doc, false, false, false);
        Ok(Some((doc, lay)))
    }

    /// Print Selection (`range: "selectedText"`): the selected content as the document that
    /// prints instead, with its print layout; `None` for any other range. As in Word, the
    /// selection is laid out afresh from the top of the first page with the page setup of the
    /// section it starts in, and no headers or footers print. Nothing selected is an error.
    pub fn print_selection(&self, v: &Value) -> Result<Option<(wordcraft_doc::Document, std::sync::Arc<DocLayout>)>, CmdError> {
        if p::str(v, "range") != Some("selectedText") {
            return Ok(None);
        }
        let out = crate::cmd::references::output_fields(&self.doc);
        let source = out.as_ref().unwrap_or(&self.doc);
        let Some(frag) = crate::cmd::edit::selection_fragment_in(self, source) else {
            return Err(CmdError::Params("nothing is selected to print".into()));
        };
        // The section the selection starts in: the one holding its first top-level body block
        // (a selection in another story takes the first section).
        let (a, _) = self.sel.ordered();
        let block = match a.story {
            wordcraft_doc::StoryRef::Body => a.path.0.first().map_or(0, |&i| i as usize),
            _ => 0,
        };
        let sections = source.sections();
        let mut section = sections
            .iter()
            .find(|(end, _)| *end >= block)
            .or(sections.last())
            .map(|(_, sec)| (*sec).clone())
            .unwrap_or_else(|| source.last_section.clone());
        no_headers(&mut section);
        let mut doc = source.clone();
        doc.body = frag.blocks.into_iter().map(std::sync::Arc::new).collect();
        // A section break copied with the selection prints without headers and footers too.
        for b in doc.body.iter_mut() {
            if let wordcraft_doc::Block::Para(para) = &**b
                && para.section.is_some()
                && let wordcraft_doc::Block::Para(para) = std::sync::Arc::make_mut(b)
                && let Some(sec) = para.section.as_mut()
            {
                no_headers(sec);
            }
        }
        doc.last_section = section;
        doc.ensure_nonempty();
        let markup = crate::cmd::file::export_options(self, v);
        let lay = self.output_layout_of(
            &doc,
            markup.markup,
            p::bool(v, "hiddenText").unwrap_or(self.prefs.print_hidden_text),
            p::bool(v, "fieldCodes").unwrap_or(self.prefs.print_field_codes),
        );
        Ok(Some((doc, lay)))
    }

    /// Render a shared sheet plan for a saved PDF or an annotation-free printer job.
    fn print_pdf_with(&self, v: &Value, links: bool) -> Result<(Vec<u8>, PrintPlan), CmdError> {
        if !v.is_object() {
            return Err(CmdError::Params("print settings must be an object".into()));
        }
        // Print Markup (File › Print) decides whether tracked changes and comments print.
        let mut markup = crate::cmd::file::export_options(self, v);
        let out = crate::cmd::references::output_fields(&self.doc);
        let report = self.print_report(v)?;
        // A Document Info job wins over the range; Print Selection prints the selection's own
        // document, all of it.
        let selection = if report.is_none() { self.print_selection(v)? } else { None };
        let mut settings = v.clone();
        let (doc, lay, current) = match (&report, &selection) {
            (Some((report, lay)), _) => {
                markup.markup = false;
                (report, lay.clone(), (0, 0))
            }
            (None, Some((sel, lay))) => {
                settings["range"] = json!("all");
                (sel, lay.clone(), (0, 0))
            }
            (None, None) => {
                let lay = self.print_layout(v);
                let current = self.current_pages(&lay);
                (out.as_ref().unwrap_or(&self.doc), lay, current)
            }
        };
        settings["reverse"] = json!(p::bool(v, "reverse").unwrap_or(self.prefs.print_reverse));
        settings["frontReverse"] = json!(p::bool(v, "frontReverse").unwrap_or(self.prefs.print_duplex_front));
        settings["backReverse"] = json!(p::bool(v, "backReverse").unwrap_or(self.prefs.print_duplex_back));
        let ids = page_ids(&lay);
        let plan = plan_in(&settings, &ids, current)?;
        let mut single = settings;
        single["copies"] = json!(1);
        let single_plan = plan_in(&single, &ids, current)?;
        let paper = imposed_paper(v, &lay, &single_plan.pages)?;
        let opts = wordcraft_pdf::PdfOptions {
            pages: Some(single_plan.pages),
            imposition: imposition(v)?,
            sheet_copies: plan.copies,
            sheet_collate: p::bool(v, "collate").unwrap_or(true),
            // Repeated pages would repeat the structure tree's entries too.
            tagged: plan.copies <= 1,
            include_markup: markup.markup,
            comments: markup.comments,
            paper,
            links,
            // Print only the data from a form (File › Options › Advanced): the document's own
            // setting; a Document Info report prints in full.
            forms_data_only: report.is_none() && self.doc.settings.print_forms_data,
            ..Default::default()
        };
        let bytes = match self.properties_appendix(v, report.is_some())? {
            Some((adoc, alay)) => wordcraft_pdf::export_layout_with_appendix(doc, &lay, (&adoc, &alay), &opts),
            None => wordcraft_pdf::export_layout(doc, &lay, &opts),
        };
        Ok((bytes.map_err(|e| CmdError::Failed(e.to_string()))?, plan))
    }

    /// File › Options › Display › Print document properties: the properties report (laid out as
    /// [`Self::print_report`] lays it out) that prints after the document's pages. The job param
    /// `properties` overrides the option. `None` for a Document Info job (`report`), a manual
    /// duplex pass and a job with several pages per sheet, which print as they would without it.
    fn properties_appendix(&self, v: &Value, report: bool) -> Result<Option<(wordcraft_doc::Document, std::sync::Arc<DocLayout>)>, CmdError> {
        if report || !p::bool(v, "properties").unwrap_or(self.prefs.print_properties) {
            return Ok(None);
        }
        if duplex_pass(v)?.is_some() || imposition(v)? != Imposition::Single {
            return Ok(None);
        }
        let lines = properties_report(self);
        if lines.is_empty() {
            return Ok(None);
        }
        let doc = report_document(&self.doc, &lines);
        let lay = self.output_layout_of(&doc, false, false, false);
        Ok(Some((doc, lay)))
    }
}

/// `file.printPdf`: write the print job's PDF to `path` (no dialogs).
pub(crate) fn print_pdf_to_file(s: &mut Session, v: &Value) -> Result<Value, CmdError> {
    let path = p::req_str(v, "path")?;
    let path = if path.to_ascii_lowercase().ends_with(".pdf") { path.to_string() } else { format!("{path}.pdf") };
    let (bytes, plan) = s.print_pdf(v)?;
    // Through `io`, so an agent confined to an automation write root (#769) can't print elsewhere.
    #[cfg(not(target_arch = "wasm32"))]
    crate::io::write_file(&path, &bytes).map_err(CmdError::Failed)?;
    s.doc.core.last_printed = crate::cmd::now_iso();
    s.dirty = true;
    Ok(json!({"path": path, "pages": plan.pages.len(), "chosen": plan.chosen, "copies": plan.copies, "bytes": bytes.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_parse_and_hostile_ones_are_errors() {
        assert_eq!(parse_page_range("1-3, 5", 6), Ok(vec![0, 1, 2, 4]));
        assert_eq!(parse_page_range("5,1-2,2", 6), Ok(vec![0, 1, 4]), "document order, no repeats");
        assert_eq!(parse_page_range("4-", 6), Ok(vec![3, 4, 5]));
        assert_eq!(parse_page_range("-2", 6), Ok(vec![0, 1]));
        for bad in ["", " , ", "0", "7", "3-1", "a", "1-2-3", "99999999999999999999999", "-", "1..3", "+3", "1-+3", "+2-+4", &"1,".repeat(400)] {
            assert!(parse_page_range(bad, 6).is_err(), "`{bad}` is refused");
        }
        assert!(parse_page_range("1", 0).is_err());
    }

    #[test]
    fn section_ranges_refuse_hostile_text() {
        let ids = [(1, 1), (1, 2), (2, 1)];
        for bad in ["p99999999999999999999999s1", "p1s99999999999999999999", "-p1s1", "p1s1-", "s1-s2", "p1s", "1s1", "pp1s1", "p1-p2s1"] {
            assert!(parse_page_range_in(bad, &ids).is_err(), "`{bad}` is refused");
        }
        assert_eq!(parse_page_range_in("p1s2-p1s2; s1", &ids), Ok(vec![0, 1, 2]));
    }

    #[test]
    fn plans_apply_range_parity_copies_and_caps() {
        let p = |v: Value| plan(&v, 6, (2, 3)).map(|p| p.pages);
        assert_eq!(p(json!({})), Ok(vec![0, 1, 2, 3, 4, 5]));
        assert_eq!(p(json!({"range": "current"})), Ok(vec![2]));
        assert_eq!(p(json!({"range": "selection"})), Ok(vec![2, 3]));
        // Print Selection's own layout prints whole.
        assert_eq!(p(json!({"range": "selectedText"})), Ok(vec![0, 1, 2, 3, 4, 5]));
        assert_eq!(p(json!({"pages": "2-4", "copies": 2})), Ok(vec![1, 2, 3, 1, 2, 3]));
        assert_eq!(p(json!({"pages": "2-3", "copies": 2, "collate": false})), Ok(vec![1, 1, 2, 2]));
        assert_eq!(p(json!({"parity": "even"})), Ok(vec![1, 3, 5]));
        assert!(p(json!({"range": "custom"})).is_err(), "custom needs pages");
        assert!(p(json!({"range": "everything"})).is_err());
        assert!(p(json!({"range": "current", "parity": "even"})).is_err(), "page 3 is odd: nothing left");
        // Hostile copies are clamped; a job that would be too big is refused, not allocated.
        assert_eq!(p(json!({"range": "current", "copies": 1e12})).map(|v| v.len()), Ok(MAX_COPIES as usize));
        assert!(plan(&json!({"copies": 999}), 600, (0, 0)).is_err());
        assert_eq!(p(json!({"copies": -5, "range": "current"})), Ok(vec![2]));
    }

    #[test]
    fn reverse_is_ignored_for_imposed_sheets() {
        for imposition in [json!(2), json!(4), json!("booklet"), json!("bookletReverse")] {
            let v = json!({"reverse": true, "copies": 2, "pagesPerSheet": imposition});
            assert_eq!(plan(&v, 3, (0, 0)).unwrap().pages, vec![0, 1, 2, 0, 1, 2]);
            let mut s = Session::new(crate::sample::sample_document());
            s.prefs.print_reverse = true;
            let (_, actual) = s.print_job_pdf(&v).unwrap();
            let mut forward = v.clone();
            forward["reverse"] = json!(false);
            assert_eq!(actual, s.print_job_pdf(&forward).unwrap().1);
        }
    }

    #[test]
    fn print_options_reject_non_booleans_before_changing_settings() {
        for key in ["printHiddenText", "printFieldCodes", "printReverse"] {
            let mut s = Session::new(wordcraft_doc::Document::new());
            let before = s.prefs.clone();
            assert!(s.run("file.options", &json!({"ignoreUppercase": false, key: "true"})).is_err());
            assert_eq!(s.prefs, before);
        }
    }

    #[test]
    fn print_pdf_writes_only_the_chosen_pages() {
        let mut s = Session::new(crate::sample::sample_document());
        let pages = s.export_layout().pages.len();
        let (all, _) = s.print_pdf(&json!({})).unwrap();
        let (one, plan) = s.print_pdf(&json!({"range": "current", "copies": 2})).unwrap();
        assert!(all.starts_with(b"%PDF") && one.starts_with(b"%PDF"));
        assert_eq!(plan.pages.len(), 2);
        if pages > 2 {
            assert!(one.len() < all.len());
        }
        assert!(s.print_pdf(&json!({"pages": "9999"})).is_err());
        let r = s.run("file.printPdf", &json!({})).is_err();
        assert!(r, "printPdf needs a path");
    }

    /// Print document properties with copies: the report follows each collated copy, or all the
    /// document's pages when copies aren't collated; manual duplex passes don't get it.
    #[test]
    fn properties_page_follows_every_copy() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.run("text.insert", &json!({"text": "One page"})).unwrap();
        s.prefs.print_properties = true;
        let count = |bytes: &[u8]| String::from_utf8_lossy(bytes).matches("/MediaBox[").count();
        for collate in [true, false] {
            let (bytes, plan) = s.print_job_pdf(&json!({"copies": 2, "collate": collate})).unwrap();
            assert_eq!(plan.pages.len(), 2, "the plan describes the document's pages");
            assert_eq!(count(&bytes), 4, "collate {collate}");
        }
        let (front, _) = s.print_job_pdf(&json!({"duplexPass": "front"})).unwrap();
        assert_eq!(count(&front), 1);
    }

    /// #4734: the PDF sent to a printer has no link annotations, which a printer driver can't fit
    /// to the paper ("flatten page annotations before using shrink-to-fit"). An exported PDF and
    /// one saved with Save these pages as PDF (`file.printPdf`) keep them.
    #[test]
    fn only_the_print_job_leaves_out_link_annotations() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.run("insert.link", &json!({"url": "https://example.com", "text": "a link"})).unwrap();
        let lay = s.export_layout();
        let exported = wordcraft_pdf::export_layout(&s.doc, &lay, &wordcraft_pdf::PdfOptions::default()).unwrap();
        let links = |bytes: &[u8]| {
            let raw = String::from_utf8_lossy(bytes);
            raw.contains("/Annots") && raw.contains("/Link")
        };
        assert!(links(&exported), "an exported PDF keeps the link");
        for params in [json!({}), json!({"paper": "A4"}), json!({"markup": true})] {
            let (job, _) = s.print_job_pdf(&params).unwrap();
            assert!(!links(&job) && !String::from_utf8_lossy(&job).contains("/Annots"), "{params}");
            let (saved, _) = s.print_pdf(&params).unwrap();
            assert!(links(&saved), "{params}");
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = std::env::temp_dir().join(format!("wc-print-links-{}.pdf", std::process::id()));
            s.run("file.printPdf", &json!({"path": path.to_string_lossy()})).unwrap();
            let saved = std::fs::read(&path).unwrap();
            let _ = std::fs::remove_file(&path);
            assert!(links(&saved), "Save these pages as PDF keeps the link");
        }
    }

    /// Document Info: comments are listed with their own text, a hostile `what` is refused, and
    /// a report prints on the document's paper without touching it.
    #[test]
    fn document_info_lists_comments_and_refuses_hostile_what() {
        let mut s = Session::new(wordcraft_doc::Document::from_text("Synthetic note here"));
        s.run("select.text", &json!({"text": "note"})).unwrap();
        s.run("review.newComment", &json!({"text": "Check this"})).unwrap();
        let lines = document_info(&s, "markup").unwrap();
        assert!(lines.first().is_some_and(|l| l.starts_with("Page 1: [1] Comment\t")), "{lines:?}");
        assert_eq!(lines.get(1).map(String::as_str), Some("Check this"));
        let e = document_info(&s, &"x".repeat(10_000)).unwrap_err().to_string();
        assert!(e.contains("properties, markup, styles, autotext") && e.len() < 200, "{e}");
        for bad in [json!(5), json!(["markup"]), json!({}), json!("Markup")] {
            assert!(s.print_pdf(&json!({"what": bad})).is_err(), "{bad}");
        }
        let rev = s.rev();
        let (_, plan) = s.print_job_pdf(&json!({"what": "markup", "copies": 2, "pagesPerSheet": 2})).unwrap();
        assert_eq!((plan.chosen, plan.copies), (1, 2));
        assert!(s.print_pdf(&json!({"what": "autotext"})).is_err(), "no AutoText entries: nothing to print");
        assert_eq!(s.rev(), rev);
    }

    /// Scale to paper size (#2108): the PDF goes on the chosen paper; the document keeps its size.
    #[test]
    fn print_pdf_scales_to_the_chosen_paper() {
        // Page dictionaries are written uncompressed: `/MediaBox[0 0 w h]`.
        let media = |bytes: &[u8]| {
            let raw = String::from_utf8_lossy(bytes).into_owned();
            raw.split("/MediaBox[")
                .skip(1)
                .filter_map(|r| r.split(']').next())
                .map(|r| r.split_whitespace().filter_map(|n| n.parse::<f32>().ok()).collect::<Vec<_>>())
                .map(|n| (n[2].round(), n[3].round()))
                .collect::<Vec<_>>()
        };
        let s = Session::new(crate::sample::sample_document());
        let (w, h) = (s.doc.last_section.page_w, s.doc.last_section.page_h);
        let rev = s.rev();
        let (own, _) = s.print_pdf(&json!({"range": "current"})).unwrap();
        assert_eq!(media(&own), vec![(w.round(), h.round())]);
        for (name, want) in [("A4", (595.0, 842.0)), ("letter", (612.0, 792.0)), ("Legal", (612.0, 1008.0))] {
            let (bytes, _) = s.print_pdf(&json!({"range": "current", "paper": name})).unwrap();
            assert_eq!(media(&bytes), vec![want], "{name}");
        }
        for none in [json!(null), json!(""), json!("none"), json!("Document")] {
            let (bytes, _) = s.print_pdf(&json!({"range": "current", "paper": none})).unwrap();
            assert_eq!(media(&bytes), media(&own));
        }
        for bad in [json!("Quarto"), json!(5), json!({"w": 1}), json!("x".repeat(10_000))] {
            assert!(s.print_pdf(&json!({"paper": bad})).is_err(), "{bad}");
        }
        assert_eq!((s.doc.last_section.page_w, s.doc.last_section.page_h, s.rev()), (w, h, rev), "the document is unchanged");
    }

    /// `file.printPdf` writes only inside the automation write root when there is one (#769).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn print_pdf_to_file_is_held_to_the_write_root() {
        use crate::file_access::{AutomationRoots, confine};
        let base = std::env::temp_dir().join(format!("wc-print-roots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for d in ["out", "outside"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        let roots = AutomationRoots::new(None, Some(&base.join("out"))).unwrap();
        let mut s = Session::new(crate::sample::sample_document());
        let inside = base.join("out/job").to_string_lossy().into_owned();
        let outside = base.join("outside/job").to_string_lossy().into_owned();
        confine(roots.as_ref(), || {
            s.dirty = false;
            let e = s.run("file.printPdf", &json!({"path": outside})).unwrap_err().to_string();
            assert!(e.contains("outside the write root"), "{e}");
            assert!(s.doc.core.last_printed.is_empty());
            assert!(!s.dirty);
            s.run("file.printPdf", &json!({"path": inside})).unwrap();
            assert!(s.dirty);
            assert!(chrono::DateTime::parse_from_rfc3339(&s.doc.core.last_printed).is_ok());
        });
        assert!(!base.join("outside/job.pdf").exists(), "nothing printed outside the root");
        assert!(std::fs::read(base.join("out/job.pdf")).unwrap().starts_with(b"%PDF"));
        // Without roots nothing is checked: the behaviour is unchanged.
        s.run("file.printPdf", &json!({"path": outside})).unwrap();
        assert!(base.join("outside/job.pdf").is_file());
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod imposition_tests {
    use super::*;
    /// Sheet printing leaves document bytes, dirty state, selection and undo history untouched.
    #[test]
    fn print_imposition_is_pure_and_rejects_hostile_modes() {
        let s = Session::new(wordcraft_doc::Document::from_text("one\u{c}two\u{c}three\u{c}four\u{c}five"));
        let before = (s.rev(), s.dirty, s.sel.clone(), serde_json::to_value(&s.doc).unwrap());
        for (mode, sides) in
            [(json!(2), 3), (json!(4), 2), (json!(6), 1), (json!(9), 1), (json!(16), 1), (json!("booklet"), 4), (json!("bookletReverse"), 4)]
        {
            let params = json!({"pagesPerSheet":mode, "copies":2});
            let lay = s.export_layout();
            assert_eq!(sheets(&params, &lay, s.current_pages(&lay)).unwrap().len(), sides);
            let (bytes, plan) = s.print_pdf(&params).unwrap();
            assert_eq!(plan.copies, 2);
            assert_eq!(String::from_utf8_lossy(&bytes).matches("/MediaBox[").count(), sides * 2);
        }
        for bad in [json!(0), json!(3), json!(-1), json!(1.5), json!(true), json!(null), json!({}), json!("2"), json!("x".repeat(10000))] {
            assert!(s.print_pdf(&json!({"pagesPerSheet":bad})).is_err());
        }
        for params in [json!(null), json!([]), json!(false)] {
            assert!(s.print_pdf(&params).is_err());
        }
        assert_eq!((s.rev(), s.dirty, s.sel.clone(), serde_json::to_value(&s.doc).unwrap()), before);
    }
}
