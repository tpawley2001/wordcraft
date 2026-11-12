//! Paragraphs whose mark is a tracked deletion, joined to the next paragraph where the layout
//! hides deletions (the final text: PDF export, print, no markup; deletions in balloons).
//!
//! Word lays such a paragraph out as part of the paragraph after it: the break is gone, its text
//! runs on into the next paragraph's, and the joined paragraph takes the paragraph formatting
//! (alignment, indents, spacing, numbering…) of the chain's last paragraph, whose mark survives.
//! Each run keeps its own character formatting.
//!
//! The join is a matter of layout only. [`plan`] builds the document as laid out: the chain's
//! last paragraph holds the text and objects of the whole chain, and each paragraph joined
//! away is an empty paragraph with its deleted mark, which takes no room. After the layout,
//! [`Joins::split`] gives each source paragraph its own share of the joined lines
//! ([`ParaLayout::share_view`]) under its own path, so the caret, hit testing, selection and
//! drawing still address the paragraphs of the document.
//!
//! Left as they are: a paragraph carrying a section break, one followed by a table or by
//! nothing in its container (the last paragraph of the body or of a table cell), and every
//! story other than the body (headers, footers, notes, text boxes). Joins stay inside one
//! container: body with body, a cell with the same cell.

use super::placed::Placed;
use super::{
    Arc, Block, Blocks, DocLayout, Document, Hash, HashMap, HashSet, Hasher, HideDeleted, LayoutOptions, ParaLayout, Paragraph, Path, StoryRef,
    turn_point,
};
use wordcraft_doc::para::{OBJ, Run};
use wordcraft_doc::props::NumRef;

/// The most paragraphs one joined paragraph is made of. Hostile input can delete thousands of
/// marks in a row: the paragraph at the cap ends its chain as a surviving mark would, and a new
/// chain may start after it, so no joined paragraph grows past this many.
pub const MAX_JOINED_PARAS: usize = 256;

/// How deep in nested tables cells are joined; deeper cells are left as they are.
const MAX_JOIN_DEPTH: usize = 32;

/// A source paragraph of a joined paragraph.
#[derive(Clone, Debug)]
struct Source {
    path: Path,
    /// Its byte range and inline object range in the joined paragraph.
    bytes: (usize, usize),
    objects: (usize, usize),
}

/// The document as laid out with the joins made, and where each joined paragraph came from.
pub(crate) struct Joins {
    pub(crate) doc: Document,
    /// The chain's last paragraph (where the joined one is) → its sources, in order.
    hosts: HashMap<Path, Vec<Source>>,
    /// The paragraphs joined away (laid out empty, in no room).
    away: HashSet<Path>,
}

/// Whether paragraph `p` joins the next paragraph when deletions are hidden: its mark is a
/// deletion, it carries no section break, and its runs and objects agree with its text (so its
/// text can be put before another's without shifting what follows).
fn joins_next(p: &Paragraph) -> bool {
    p.mark.del.is_some()
        && p.section.is_none()
        && p.runs.iter().try_fold(0usize, |n, r| n.checked_add(r.len)) == Some(p.text.len())
        && p.text.chars().filter(|c| *c == OBJ).count() == p.objects.len()
}

/// The joins to make in `doc` laid out with `opts`, or `None` when there are none.
pub(crate) fn plan(doc: &Document, opts: &LayoutOptions) -> Option<Joins> {
    if opts.hides_deleted(StoryRef::Body) != HideDeleted::ALL {
        return None;
    }
    let mut hosts = HashMap::new();
    let mut away = HashSet::new();
    let body = join_blocks(&doc.body, &[], 0, (&mut hosts, &mut away))?;
    let mut out = doc.clone();
    out.body = body;
    Some(Joins { doc: out, hosts, away })
}

type Found<'a> = (&'a mut HashMap<Path, Vec<Source>>, &'a mut HashSet<Path>);

/// `blocks` (at `prefix`) with their chains joined, or `None` when nothing in them joins.
fn join_blocks(blocks: &Blocks, prefix: &[u32], depth: usize, found: Found<'_>) -> Option<Blocks> {
    let (hosts, away) = found;
    let mut out: Option<Blocks> = None;
    let path = |i: usize| u32::try_from(i).ok().map(|i| Path(prefix.iter().copied().chain([i]).collect()));
    let para = |i: usize| blocks.get(i).and_then(|b| b.as_para());
    let mut i = 0;
    while let Some(block) = blocks.get(i) {
        match &**block {
            Block::Table(t) if depth < MAX_JOIN_DEPTH => {
                let Some(here) = path(i) else { break };
                let mut table: Option<wordcraft_doc::Table> = None;
                for (r, row) in t.rows.iter().enumerate() {
                    for (c, cell) in row.cells.iter().enumerate() {
                        let (Ok(r32), Ok(c32)) = (u32::try_from(r), u32::try_from(c)) else { continue };
                        let mut at = here.0.clone();
                        at.extend([r32, c32]);
                        if let Some(b) = join_blocks(&cell.blocks, &at, depth + 1, (&mut *hosts, &mut *away))
                            && let Some(cell) = table.get_or_insert_with(|| t.clone()).rows.get_mut(r).and_then(|row| row.cells.get_mut(c))
                        {
                            cell.blocks = b;
                        }
                    }
                }
                if let Some(t) = table
                    && let Some(slot) = out.get_or_insert_with(|| blocks.clone()).get_mut(i)
                {
                    *slot = Arc::new(Block::Table(t));
                }
                i += 1;
            }
            Block::Table(_) => i += 1,
            Block::Para(p) => {
                if !joins_next(p) || para(i + 1).is_none() {
                    i += 1;
                    continue;
                }
                // The chain is i..=j: every paragraph but j joins the next; j's mark survives
                // (or the chain is at its cap).
                let mut j = i + 1;
                while j + 1 - i < MAX_JOINED_PARAS && para(j).is_some_and(joins_next) && para(j + 1).is_some() {
                    j += 1;
                }
                let Some(host_path) = path(j) else { break };
                let chain: Vec<(Path, &Paragraph)> = (i..=j).filter_map(|k| Some((path(k)?, para(k)?))).collect();
                let (joined, sources) = join_chain(&chain);
                let slots = out.get_or_insert_with(|| blocks.clone());
                for (k, (src, p)) in (i..j).zip(&chain) {
                    if let Some(slot) = slots.get_mut(k) {
                        *slot = Arc::new(Block::Para(placeholder(p)));
                        away.insert(src.clone());
                    }
                }
                if let Some(slot) = slots.get_mut(j) {
                    *slot = Arc::new(Block::Para(joined));
                    hosts.insert(host_path, sources);
                }
                i = j + 1;
            }
        }
    }
    out
}

/// A layout revision of its own for a paragraph made here, from the revisions of the
/// paragraphs it was made from: an edit in any of them lays it out again. The top bit keeps it
/// apart from the document's own revisions (a counter).
fn rev_of(tag: u8, revs: impl Iterator<Item = u64>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    tag.hash(&mut h);
    for r in revs {
        r.hash(&mut h);
    }
    h.finish() | (1 << 63)
}

/// The paragraphs of `chain` as one: their text, runs and objects in order, with the last
/// paragraph's paragraph formatting and mark; and where each one's text went.
fn join_chain(chain: &[(Path, &Paragraph)]) -> (Paragraph, Vec<Source>) {
    let Some((_, last)) = chain.last() else { return (Paragraph::new(), Vec::new()) };
    let bytes: usize = chain.iter().map(|(_, p)| p.text.len()).sum();
    let mut text = String::with_capacity(bytes);
    let mut runs: Vec<Run> = Vec::new();
    let mut objects = Vec::new();
    let mut sources = Vec::with_capacity(chain.len());
    for (path, p) in chain {
        let (b0, o0) = (text.len(), objects.len());
        text.push_str(&p.text);
        runs.extend(p.runs.iter().cloned());
        objects.extend(p.objects.iter().cloned());
        sources.push(Source { path: path.clone(), bytes: (b0, text.len()), objects: (o0, objects.len()) });
    }
    let joined = Paragraph {
        text,
        runs,
        props: last.props.clone(),
        mark: last.mark.clone(),
        objects,
        section: last.section.clone(),
        rev: rev_of(1, chain.iter().map(|(_, p)| p.rev)),
    };
    (joined, sources)
}

/// What stands in for a paragraph joined away: empty, with its deleted mark (so it takes no
/// room), no list number, nothing to break a page before it, and kept with the next paragraph
/// so a keep-with-next chain running into it reaches the joined paragraph.
fn placeholder(p: &Paragraph) -> Paragraph {
    let mut q = Paragraph::new();
    q.mark = p.mark.clone();
    q.props.numbering = Some(NumRef { num: 0, level: 0 });
    q.props.page_break_before = Some(false);
    q.props.keep_next = Some(true);
    q.rev = rev_of(2, std::iter::once(p.rev));
    q
}

impl Joins {
    /// Give each source paragraph its own share of the joined layouts in `l`, under its own path.
    pub(crate) fn split(&self, l: &mut DocLayout) {
        let mut views: HashMap<usize, Vec<Option<(Arc<ParaLayout>, usize, usize)>>> = HashMap::new();
        for page in &mut l.pages {
            let items = std::mem::take(&mut page.items);
            page.items = self.split_items(items, &mut views);
        }
        l.index = super::pass::index_lines(&l.pages);
    }

    /// Each source's view of joined layout `pl` with its first and last joined line.
    fn views(&self, sources: &[Source], pl: &ParaLayout) -> Vec<Option<(Arc<ParaLayout>, usize, usize)>> {
        let n = sources.len();
        sources
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let share = crate::para::Share { bytes: s.bytes, objects: s.objects, first: k == 0, last: k + 1 == n };
                pl.share_view(share).map(|(v, f)| {
                    let t = f + v.lines.len().saturating_sub(1);
                    (Arc::new(v), f, t)
                })
            })
            .collect()
    }

    fn split_items(&self, items: Vec<Placed>, views: &mut HashMap<usize, Vec<Option<(Arc<ParaLayout>, usize, usize)>>>) -> Vec<Placed> {
        let mut out = Vec::with_capacity(items.len());
        for it in items {
            match it {
                Placed::Lines { story: StoryRef::Body, ref path, .. } if self.away.contains(path) => {}
                Placed::Lines { story: StoryRef::Body, path, para, l0, l1, x, y, turn } if self.hosts.contains_key(&path) => {
                    let Some(sources) = self.hosts.get(&path) else { continue };
                    let key = Arc::as_ptr(&para) as usize;
                    let vs = views.entry(key).or_insert_with(|| self.views(sources, &para));
                    let top = para.lines.get(l0).map_or(0.0, |l| l.top);
                    let before = out.len();
                    for (s, v) in sources.iter().zip(vs.iter()) {
                        let Some((view, f, t)) = v else { continue };
                        let (a, b) = (l0.max(*f), l1.min(t.saturating_add(1)));
                        if a >= b {
                            continue;
                        }
                        let dy = para.lines.get(a).map_or(0.0, |l| l.top) - top;
                        let (x, y) = turn_point(turn, x, y, 0.0, dy);
                        out.push(Placed::Lines { story: StoryRef::Body, path: s.path.clone(), para: view.clone(), l0: a - f, l1: b - f, x, y, turn });
                    }
                    if out.len() == before {
                        // No share to show it as: keep the joined lines (they draw the same).
                        out.push(Placed::Lines { story: StoryRef::Body, path, para, l0, l1, x, y, turn });
                    }
                }
                mut other => {
                    self.remap(&mut other);
                    out.push(other);
                }
            }
        }
        out
    }

    /// The source paragraph and offset of byte `off` of the joined paragraph at `host`.
    fn source_of(&self, host: &Path, off: usize) -> Option<(Path, usize)> {
        let sources = self.hosts.get(host)?;
        let s = sources.iter().find(|s| off < s.bytes.1).or(sources.last())?;
        Some((s.path.clone(), off.saturating_sub(s.bytes.0)))
    }

    /// Point an object placed from a joined paragraph at the source paragraph it is in.
    fn remap(&self, it: &mut Placed) {
        match it {
            Placed::Image { story: StoryRef::Body, path, off, .. }
            | Placed::Graphic { story: StoryRef::Body, path, off, .. }
            | Placed::Object { story: StoryRef::Body, path, off, .. } => {
                if let Some((p, o)) = self.source_of(path, *off) {
                    (*path, *off) = (p, o);
                }
            }
            Placed::Shape { object: Some(pos), .. } if pos.story == StoryRef::Body => {
                if let Some((p, o)) = self.source_of(&pos.path, pos.off) {
                    (pos.path, pos.off) = (p, o);
                }
            }
            Placed::Clip { items, .. } => {
                for i in items {
                    self.remap(i);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::para_block;
    use wordcraft_doc::section::SectionProps;

    fn para(text: &str) -> Paragraph {
        Paragraph::with_text(text, Default::default())
    }

    fn gone(text: &str) -> Paragraph {
        let mut p = para(text);
        p.mark.del = Some(0);
        p
    }

    fn hidden() -> LayoutOptions {
        LayoutOptions { hide_deleted: true, ..Default::default() }
    }

    fn doc(blocks: Vec<Block>) -> Document {
        let mut d = Document::new();
        d.body = blocks.into_iter().map(Arc::new).collect();
        d
    }

    /// The joined paragraphs' texts, in document order.
    fn joined(d: &Document) -> Vec<String> {
        let Some(j) = plan(d, &hidden()) else { return Vec::new() };
        let mut hosts: Vec<&Path> = j.hosts.keys().collect();
        hosts.sort();
        hosts.iter().filter_map(|p| j.doc.para(StoryRef::Body, p)).map(|p| p.text.clone()).collect()
    }

    #[test]
    fn a_chain_ends_at_the_cap_and_a_new_one_starts_after_it() {
        let mut blocks: Vec<Block> = (0..=MAX_JOINED_PARAS).map(|_| Block::Para(gone("a"))).collect();
        blocks.push(Block::Para(para("z")));
        let d = doc(blocks);
        let j = joined(&d);
        assert_eq!(j.len(), 2, "{} joined paragraphs", j.len());
        assert_eq!(j[0], "a".repeat(MAX_JOINED_PARAS));
        assert_eq!(j[1], "az");
        let plan = plan(&d, &hidden()).unwrap();
        // 255 joined away in the first chain, 1 in the second.
        assert_eq!(plan.away.len(), MAX_JOINED_PARAS);
        assert!(plan.hosts.values().all(|s| s.len() <= MAX_JOINED_PARAS));
    }

    #[test]
    fn a_paragraph_before_a_table_or_at_the_end_is_left_alone() {
        let table = Block::Table(wordcraft_doc::Table::new(1, 1, 100.0));
        assert!(plan(&doc(vec![Block::Para(gone("A")), table]), &hidden()).is_none());
        assert!(plan(&doc(vec![Block::Para(para("X")), Block::Para(gone("A"))]), &hidden()).is_none());
    }

    #[test]
    fn a_paragraph_carrying_a_section_break_is_left_alone() {
        let mut a = gone("A");
        a.section = Some(Box::new(SectionProps::default()));
        assert!(plan(&doc(vec![Block::Para(a), Block::Para(para("B"))]), &hidden()).is_none());
    }

    #[test]
    fn the_last_paragraph_of_a_cell_is_left_alone_and_cells_join_inside_themselves() {
        let mut t = wordcraft_doc::Table::new(1, 2, 100.0);
        if let Some(row) = t.rows.first_mut() {
            if let Some(c) = row.cells.get_mut(0) {
                c.blocks = vec![para_block(gone("A")), para_block(para("B"))];
            }
            if let Some(c) = row.cells.get_mut(1) {
                c.blocks = vec![para_block(gone("C"))];
            }
        }
        let d = doc(vec![Block::Table(t), Block::Para(para("D"))]);
        assert_eq!(joined(&d), ["AB"]);
        let plan = plan(&d, &hidden()).unwrap();
        assert!(plan.hosts.contains_key(&Path(vec![0, 0, 0, 1])));
        assert!(plan.away.contains(&Path(vec![0, 0, 0, 0])));
    }

    #[test]
    fn nothing_joins_while_deletions_are_shown() {
        let d = doc(vec![Block::Para(gone("A")), Block::Para(para("B"))]);
        assert!(plan(&d, &LayoutOptions::default()).is_none());
    }

    /// The joined paragraph is laid out again when a paragraph it was made from changes, though
    /// the chain's last paragraph did not.
    #[test]
    fn an_edit_in_a_joined_away_paragraph_lays_the_joined_one_out_again() {
        crate::test_fonts();
        let mut d = doc(vec![Block::Para(gone("Alpha")), Block::Para(para("Beta"))]);
        let mut cache = crate::LayoutCache::new();
        let opts = hidden();
        let width = |l: &DocLayout| {
            l.pages.iter().flat_map(|p| &p.items).find_map(|it| match it {
                Placed::Lines { path, para, .. } if path.0 == [1] => para.lines.first().and_then(|l| l.xs.first().copied()),
                _ => None,
            })
        };
        let before = width(&crate::layout(&d, &mut cache, &opts)).unwrap();
        if let Some(Block::Para(p)) = d.body.first_mut().map(Arc::make_mut) {
            p.text = "Alpha alpha alpha".into();
            p.runs = vec![Run { len: p.text.len(), props: Default::default() }];
            p.touch();
        }
        let after = width(&crate::layout(&d, &mut cache, &opts)).unwrap();
        assert!(after > before + 10.0, "Beta starts at {before}, then at {after}");
    }
}
