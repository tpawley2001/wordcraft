//! WordCraft reader for the Word 97-2003 binary file format (`.doc`, [MS-DOC]), and for the
//! text and basic formatting of Word 6.0/95 files ([`word6`]).
//!
//! [`read`] turns the bytes of a `.doc` file into a [`wordcraft_doc::Document`]. There is no
//! writer: saving always goes through the other formats. The reader is written from the public
//! specification ([MS-DOC], Word (.doc) Binary File Format, Microsoft Open Specifications) and,
//! like every WordCraft parser, is lenient by design (unknown structures are skipped, bad numbers
//! fall back to defaults) and bounded (stream sizes, piece counts and property counts are
//! capped), so hostile input yields an error or a best-effort document, never a panic.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod crypt;
mod dop;
mod drawing;
mod fib;
mod fkp;
mod fmt;
mod list;
mod media;
mod metadata;
mod notes;
mod officeart;
mod ole;
mod piece;
mod sections;
mod sprm;
mod stsh;
mod table;
mod word6;

use std::io::{Read, Seek};
use std::sync::Arc;

use wordcraft_doc::link::hyperlink_target;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::para::NoteKind;
use wordcraft_doc::para::Paragraph;
use wordcraft_doc::para::Run;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::props::NumRef;
use wordcraft_doc::section::HeaderSet;
use wordcraft_doc::styles::StyleSheet;
use wordcraft_doc::{Document, PartKind};

use fkp::Bins;
use piece::{PieceTable, PrmRef};
use sprm::Prl;
use table::ParaOut;

/// Largest stream we materialise (bytes); the engine already caps whole files at 2 GB.
const MAX_STREAM: u64 = 1 << 30;
/// Most header/footer parts we create (matches the docx reader's cap).
const MAX_PARTS: usize = 50_000;
/// Hard ceiling on characters materialised across every story walk of one document (body,
/// headers, notes, text boxes), whatever the file's size. The working budget is tied to the
/// WordDocument stream (see [`read`]); this caps even a maximal stream.
const MAX_TOTAL_CHARS: u64 = 64 << 20;
/// Most paragraph and cell marks across every story walk of one document. The character
/// budget does not see them: a mark is one byte of the file but about 1 KB of document, so a
/// file of nothing but marks would grow memory about a thousand times faster than its size.
/// The cap bounds a flood to about 2 GB while reading (about 4 GB at the very cap, when the
/// blocks are assembled). Past it the read fails with [`DocbinError::Limit`]. Real documents stay far below it:
/// 50 paragraphs on each of 10,000 pages is 500,000, and a 10,000-page one-column table (46
/// rows, each a cell mark plus a row mark) is about 920,000.
const MAX_PARAGRAPHS: usize = 2_000_000;
/// Bytes of repeated work per document beyond the text itself: piece Prm grpprls (Prcs, up
/// to 64 KB each) interpreted (#3692), hyperlink targets (#3708), picture alt text (#3709)
/// and floating objects (#3710) copied into every run or anchor. Past it Prms are skipped,
/// copied strings left empty and floating objects dropped, each kind logged once a document
/// ([`Work::log`]).
const MAX_WORK: u64 = 64 << 20;

/// What the [`MAX_WORK`] budget pays for, and leaves out once it is spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cost {
    /// A piece's Prm grpprl: past the budget the piece keeps its CHPX formatting only.
    Formatting,
    /// A hyperlink target copied into a run of its result: past it the text is kept, unlinked.
    LinkTarget,
    /// A picture's alt text copied into an anchor: past it the picture has none.
    AltText,
    /// A floating object copied into its anchor: past it the object is left out.
    FloatingObject,
}

impl Cost {
    const ALL: [Cost; 4] = [Cost::Formatting, Cost::LinkTarget, Cost::AltText, Cost::FloatingObject];

    /// What is left out, for the log.
    fn left_out(self) -> &'static str {
        match self {
            Cost::Formatting => "some text keeps only part of its formatting",
            Cost::LinkTarget => "some hyperlinks' text is kept without its link",
            Cost::AltText => "some pictures are left without their alt text",
            Cost::FloatingObject => "some floating shapes and pictures are left out",
        }
    }
}

/// One document's [`MAX_WORK`] budget, and what it has left out (each kind logged once when
/// the document has been read, [`Work::log`]).
struct Work {
    left: std::cell::Cell<u64>,
    dropped: std::cell::Cell<[bool; 4]>,
}

impl Work {
    fn new(budget: u64) -> Self {
        Work { left: std::cell::Cell::new(budget), dropped: std::cell::Cell::new([false; 4]) }
    }

    /// Spends `cost` bytes on `what`: false (and `what` noted as left out) when they aren't left.
    fn charge(&self, cost: u64, what: Cost) -> bool {
        match self.left.get().checked_sub(cost) {
            Some(left) => {
                self.left.set(left);
                true
            }
            None => {
                let mut dropped = self.dropped.get();
                if let Some(d) = Cost::ALL.iter().position(|c| *c == what).and_then(|i| dropped.get_mut(i)) {
                    *d = true;
                }
                self.dropped.set(dropped);
                false
            }
        }
    }

    /// The kinds of work left out so far, in [`Cost::ALL`] order.
    fn dropped(&self) -> Vec<Cost> {
        Cost::ALL.iter().zip(self.dropped.get()).filter(|(_, d)| *d).map(|(c, _)| *c).collect()
    }

    /// Logs one warning for each kind left out.
    fn log(&self) {
        let dropped = self.dropped();
        for c in &dropped {
            log::warn!(".doc: the document repeats more than WordCraft copies ({MAX_WORK} bytes): {}", c.left_out());
        }
        #[cfg(test)]
        LAST_DROPPED.with(|d| *d.borrow_mut() = dropped);
    }
}

#[cfg(test)]
thread_local! {
    /// What the last document read on this thread left out for the work budget ([`Work::log`]).
    static LAST_DROPPED: std::cell::RefCell<Vec<Cost>> = const { std::cell::RefCell::new(Vec::new()) };
}
/// `sprmCPicLocation`: the fc of a picture in the Data stream.
const C_PIC_LOCATION: u16 = 0x6A03;
/// `sprmCSymbol`: the record font and character of a symbol character ([MS-DOC] §2.6.1); the
/// text stream holds the placeholder U+0028 at its position.
const C_SYMBOL: u16 = 0x6A09;
/// `sprmPIlvl` / `sprmPIlfo`: the paragraph's list level and list.
const P_ILVL: u16 = 0x260A;
const P_ILFO: u16 = 0x460B;
/// `sprmPHugePapx`: the whole PAPX of a paragraph that is too large for its FKP page (a table row with
/// many cells) is this one sprm, whose operand is the offset of the real properties in the Data stream.
const P_HUGE_PAPX: u16 = 0x6646;

/// Errors from reading a Word 97-2003 binary file.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum DocbinError {
    /// The bytes are not a readable compound file (OLE2/CFB container).
    #[error("not a Word document (compound file): {0}")]
    Container(String),
    /// The compound file is readable but is not a Word 97-2003 document.
    #[error("not a Word 97-2003 document: {0}")]
    NotWord(String),
    /// The document is password-protected and no password was given.
    #[error("this document is protected with a password")]
    PasswordRequired,
    /// The password does not open the document.
    #[error("the password is incorrect")]
    WrongPassword,
    /// The document is encrypted in a way WordCraft can't read (AES, an external provider, an
    /// unknown header).
    #[error("encrypted document: {0}")]
    Encryption(String),
    /// The input exceeds a safety limit (stream size, piece count…).
    #[error("limit exceeded: {0}")]
    Limit(String),
    /// A structure is malformed beyond recovery.
    #[error("malformed document: {0}")]
    Malformed(String),
}

/// Is this `.doc` password-protected (XOR obfuscation, RC4 or RC4 CryptoAPI)? False for
/// anything that is not a readable Word 97-2003 file.
pub fn is_encrypted(bytes: &[u8]) -> bool {
    let Ok(mut comp) = open_container(bytes) else { return false };
    stream(&mut comp, "WordDocument").is_some_and(|w| fib::peek_encryption(&w).is_some())
}

/// Repair tolerable compound-file defects, then check the FAT layout before opening a `.doc`.
fn open_container(bytes: &[u8]) -> Result<cfb::CompoundFile<std::io::Cursor<Vec<u8>>>, DocbinError> {
    let bytes = wordcraft_doc::cfb_repair::repair(bytes);
    wordcraft_doc::cfb_guard::check(&bytes).map_err(DocbinError::Container)?;
    cfb::CompoundFile::open(std::io::Cursor::new(bytes.into_owned())).map_err(|e| DocbinError::Container(e.to_string()))
}

/// Read a `.doc` (Word 97-2003 binary) file into a [`Document`]. A password-protected file
/// gives [`DocbinError::PasswordRequired`]; see [`read_with_password`].
pub fn read(bytes: &[u8]) -> Result<Document, DocbinError> {
    read_with_password(bytes, None)
}

/// [`read`] for a file that may be password-protected: it is checked against the file's stored
/// verifier ([`DocbinError::WrongPassword`] when it does not match) and then decrypted. The
/// password is ignored for an unprotected file.
pub fn read_with_password(bytes: &[u8], password: Option<&str>) -> Result<Document, DocbinError> {
    read_limited(bytes, password, MAX_PARAGRAPHS)
}

/// [`read_with_password`] with the paragraph cap as a parameter, so tests need not build a
/// file of [`MAX_PARAGRAPHS`] marks.
fn read_limited(bytes: &[u8], password: Option<&str>, max_paragraphs: usize) -> Result<Document, DocbinError> {
    let mut comp = open_container(bytes)?;
    let mut word = stream(&mut comp, "WordDocument").ok_or_else(|| DocbinError::NotWord("no WordDocument stream".into()))?;
    if word.len() as u64 > MAX_STREAM {
        return Err(DocbinError::Limit(format!("WordDocument stream is {} bytes", word.len())));
    }
    let enc = fib::peek_encryption(&word);
    // Word 6.0/95 files have their own FIB and keep everything in the WordDocument stream.
    if enc.is_none()
        && let Some(w6) = fib::Fib::parse(&word)?.word6
    {
        return word6::read(&mut comp, &word, &w6);
    }
    // The FIB names the Table stream (0Table or 1Table); fall back to whichever exists,
    // since files written by other writers sometimes carry the wrong bit.
    let which = if let Some(e) = enc { e.which_tbl_stm } else { fib::Fib::parse(&word)?.which_tbl_stm };
    let named = if which { "1Table" } else { "0Table" };
    let other = if which { "0Table" } else { "1Table" };
    let mut table = stream(&mut comp, named).or_else(|| stream(&mut comp, other)).unwrap_or_default();
    if table.len() as u64 > MAX_STREAM {
        return Err(DocbinError::Limit(format!("Table stream is {} bytes", table.len())));
    }
    let mut data = stream(&mut comp, "Data").unwrap_or_default();
    if data.len() as u64 > MAX_STREAM {
        return Err(DocbinError::Limit(format!("Data stream is {} bytes", data.len())));
    }
    if let Some(e) = enc {
        // Verify the password first; only then is anything decrypted.
        let password = password.ok_or(DocbinError::PasswordRequired)?;
        let cipher = crypt::Cipher::open(e.flags, e.lkey, &table, password)?;
        cipher.decrypt_word(&mut word);
        cipher.decrypt_table(&mut table);
        cipher.decrypt_data(&mut data);
    }
    let fib = fib::Fib::parse(&word)?;

    // The piece table locates every character in the WordDocument stream.
    let (fc_clx, lcb_clx) =
        fib.pair(fib::pair::CLX).filter(|(_, lcb)| *lcb > 0).ok_or_else(|| DocbinError::Malformed("the FIB has no piece table (Clx)".into()))?;
    let pieces = piece::PieceTable::parse(&table, fc_clx, lcb_clx)?;

    // Styles, fonts and the formatting bin tables; each is optional in broken files.
    let fonts = fib.pair(fib::pair::STTBF_FFN).map(|(f, l)| stsh::fonts(&table, f, l)).unwrap_or_default();
    let raw_styles = match fib.pair(fib::pair::STSHF) {
        Some((f, l)) => stsh::parse(&table, f, l)?,
        None => Vec::new(),
    };
    let standard_fonts = fib.pair(fib::pair::STSHF).map(|(f, l)| stsh::standard_fonts(&table, f, l)).unwrap_or_default();
    let chpx_bins = match fib.pair(fib::pair::PLCF_BTE_CHPX) {
        Some((f, l)) => Bins::parse(&table, f, l)?,
        None => Bins::parse(&[], 0, 0)?,
    };
    let papx_bins = match fib.pair(fib::pair::PLCF_BTE_PAPX) {
        Some((f, l)) => Bins::parse(&table, f, l)?,
        None => Bins::parse(&[], 0, 0)?,
    };

    let mut doc = Document::new();
    // Word lays out every Word 97-2003 file in Compatibility Mode, with the file's options.
    doc.settings.compat_mode = wordcraft_doc::WORD2003_COMPAT_MODE;
    dop::apply(&table, fib.pair(fib::pair::DOP), &mut doc.settings);
    if let Some(core) = metadata::read(&mut comp) {
        doc.core = core;
    }
    doc.body.clear();
    let style_ids = fmt::style_ids(&raw_styles);
    let mut styles = fmt::stylesheet(&raw_styles, &style_ids, &fonts, standard_fonts);
    // With no quick flag read, the Home › Styles gallery shows Word's usual set, as the DOCX
    // reader does.
    if !styles.styles.iter().any(|s| s.quick) {
        styles.add_default_gallery();
    }
    doc.styles = Arc::new(styles);
    doc.numbering = Arc::new(list::parse(&table, &fib, &fonts));
    let sheet: StyleSheet = StyleSheet::clone(&doc.styles);
    let sects = sections::parse(&word, &table, &fib);
    let stories = sections::header_stories(&table, &fib);
    // Subdocument layout in the CP space: main document, footnotes, headers, comments,
    // endnotes, text boxes, header text boxes.
    // Story ranges from the Table stream are hostile: each is clamped to its subdocument's
    // length here and to the last piece's CP inside `walk`.
    let ftn_base = fib.ccp.text;
    let hdd_base = fib.ccp.text.saturating_add(fib.ccp.ftn);
    let edn_base = hdd_base.saturating_add(fib.ccp.hdd).saturating_add(fib.ccp.atn);
    let mut pics = media::Pictures::for_input(word.len().saturating_add(data.len()));
    // Object storages are not decrypted; an encrypted file keeps its Data stream pictures.
    let prints = if enc.is_none() { ole::Prints::read(&mut comp) } else { ole::Prints::default() };
    let kept = std::cell::RefCell::new(ole::Kept::default());
    // Every materialised character comes from at least one byte of the WordDocument stream, so a
    // legitimate document's total text (body plus every header, footnote and text box, each read
    // once) cannot exceed its stream. A hostile piece or story table that aliases the same bytes
    // many times can, so the walk is held to twice the stream's size — generous for real files,
    // tight against aliasing — and never past the absolute ceiling.
    let budget = std::cell::Cell::new((word.len() as u64).saturating_mul(2).min(MAX_TOTAL_CHARS));
    let work = Work::new(MAX_WORK);
    let paragraphs = ParaBudget { left: std::cell::Cell::new(max_paragraphs), exceeded: std::cell::Cell::new(false) };

    let ctx = WalkCtx {
        word: &word,
        data: &data,
        pieces: &pieces,
        chpx_bins: &chpx_bins,
        papx_bins: &papx_bins,
        fonts: &fonts,
        raw_styles: &raw_styles,
        style_ids: &style_ids,
        sheet: &sheet,
        notes: &[],
        own_note: None,
        marks: &[],
        drawn: &[],
        sect_ends: &[],
        prints: &prints,
        kept: &kept,
        budget: &budget,
        paragraphs: &paragraphs,
        work: &work,
    };

    // Drawings: text box stories become parts first, then the floating shapes and pictures
    // anchored in the main document and the headers, found by their anchor CP while walking.
    let txbx_base = edn_base.saturating_add(fib.ccp.edn);
    let hdr_txbx_base = txbx_base.saturating_add(fib.ccp.txbx);
    let art = fib
        .pair(fib::pair::DGG_INFO)
        .and_then(|(f, l)| table.get(f as usize..(f as usize).saturating_add(l as usize)))
        .map(officeart::content)
        .unwrap_or_default();
    let mut tb_stories = std::collections::HashMap::new();
    for (pair, base, len) in [(fib::pair::PLCF_TXBX_TXT, txbx_base, fib.ccp.txbx), (fib::pair::PLCF_HDR_TXBX_TXT, hdr_txbx_base, fib.ccp.hdr_txbx)] {
        for (spid, ab) in drawing::textbox_stories(&table, fib.pair(pair)) {
            let Some((a, b)) = story(base, len, ab) else { continue };
            if doc.parts.len() >= MAX_PARTS || tb_stories.contains_key(&spid) {
                continue;
            }
            let blocks = story_blocks(&ctx, a, b, &mut pics, max_paragraphs)?;
            tb_stories.insert(spid, doc.add_part(PartKind::TextBox, blocks));
        }
    }
    // Each story's box: its internal margins and vertical alignment ([MS-ODRAW] §2.3.21).
    let mut nodes: Vec<&officeart::Node> = art.main.values().chain(art.header.values()).collect();
    while let Some(node) = nodes.pop() {
        if let officeart::Node::Group(_, members) = node {
            nodes.extend(members);
        }
        let sp = node.sp();
        if let Some(part) = tb_stories.get(&sp.spid).and_then(|id| doc.parts.get_mut(id)) {
            part.body = sp.props.text_body(part.body);
        }
    }
    let build = drawing::Build { bstore: &art.bstore, delay: &word, stories: &tb_stories };
    let mut drawn = drawing::anchors(&table, fib.pair(fib::pair::PLC_SPA_MOM), 0, &art.main, &build, &mut pics);
    drawn.extend(drawing::anchors(&table, fib.pair(fib::pair::PLC_SPA_HDR), hdd_base, &art.header, &build, &mut pics));
    drawn.sort_by_key(|(cp, _)| *cp);
    let ctx = WalkCtx { drawn: &drawn, ..ctx };

    // Header and footer parts: skip the six separator stories, then six stories per section
    // in the order even header, odd (default) header, even footer, odd (default) footer,
    // first header, first footer. Empty stories inherit from the previous section.
    let mut hf_ids: Vec<(usize, usize, u32)> = Vec::new();
    for si in 0..sects.len() {
        for k in 0..6usize {
            let Some(&ab) = stories.get(6 + si * 6 + k) else { break };
            let Some((a, b)) = story(hdd_base, fib.ccp.hdd, ab) else { continue };
            if doc.parts.len() >= MAX_PARTS {
                continue;
            }
            let kind = match k {
                0 | 1 | 4 => PartKind::Header,
                _ => PartKind::Footer,
            };
            let blocks = header_story_blocks(&ctx, a, b, &mut pics, max_paragraphs)?;
            if !blocks.is_empty() {
                let id = doc.add_part(kind, blocks);
                hf_ids.push((si, k, id));
            }
        }
    }

    // Footnote and endnote stories: each text range becomes a part; the reference CPs then
    // link to the part ids during the main walk.
    let note_kinds = [
        (fib::pair::PLCF_FND_REF, fib::pair::PLCF_FND_TXT, ftn_base, fib.ccp.ftn, NoteKind::Footnote, PartKind::Footnote),
        (fib::pair::PLCF_END_REF, fib::pair::PLCF_END_TXT, edn_base, fib.ccp.edn, NoteKind::Endnote, PartKind::Endnote),
    ];
    let mut note_links: Vec<(u32, NoteKind, u32)> = Vec::new();
    for (ref_pair, txt_pair, base, len, kind, part_kind) in note_kinds {
        let found = notes::parse_notes(&table, &fib, ref_pair, txt_pair);
        let stories = found.into_iter().filter_map(|(cp, ab)| Some((cp, story(base, len, ab)?)));
        note_parts(&mut doc, &ctx, stories, (kind, part_kind), (&mut pics, max_paragraphs), &mut note_links)?;
    }
    // Sorted so the main walk finds each reference by binary search.
    note_links.sort_by_key(|(cp, _, _)| *cp);
    let marks = notes::parse_bookmarks(&table, &fib);
    let mut sect_ends: Vec<u32> = sects.iter().map(|s| s.end_cp).collect();
    sect_ends.sort_unstable();
    let ctx = WalkCtx { notes: &note_links, marks: &marks, sect_ends: &sect_ends, ..ctx };

    // Sections: each section's properties attach to the paragraph that ends it (its story of
    // header/footer parts included); the section reaching the end of the text is the final one.
    let mut paras = walk(&ctx, 0, fib.ccp.text, &mut pics);
    // The first paragraph ending at each CP, found once rather than by a scan per section.
    let mut ending: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (i, p) in paras.iter().enumerate() {
        ending.entry(p.end_cp).or_insert(i);
    }
    let last = sects.last().map(|s| s.props.clone());
    for (si, sec) in sects.iter().enumerate() {
        let mut props = sec.props.clone();
        let id_of = |k: usize| hf_ids.iter().find(|(s, kk, _)| *s == si && *kk == k).map(|(_, _, id)| *id);
        props.headers = HeaderSet { even: id_of(0), default: id_of(1), first: id_of(4) };
        props.footers = HeaderSet { even: id_of(2), default: id_of(3), first: id_of(5) };
        let is_final = si + 1 == sects.len() || sec.end_cp >= fib.ccp.text;
        if is_final {
            continue; // handled below through `last`
        }
        if let Some(p) = ending.get(&sec.end_cp).and_then(|&i| paras.get_mut(i)) {
            p.para.section = Some(Box::new(props));
        }
    }
    if let Some(mut l) = last {
        let si = sects.len().saturating_sub(1);
        let id_of = |k: usize| hf_ids.iter().find(|(s, kk, _)| *s == si && *kk == k).map(|(_, _, id)| *id);
        l.headers = HeaderSet { even: id_of(0), default: id_of(1), first: id_of(4) };
        l.footers = HeaderSet { even: id_of(2), default: id_of(3), first: id_of(5) };
        doc.last_section = l;
    }

    if paragraphs.exceeded.get() {
        return Err(paragraph_limit(max_paragraphs));
    }
    doc.media.extend(pics.media);
    doc.linked_media.extend(pics.links);
    kept.into_inner().finish(&mut doc.passthrough);
    doc.body = table::assemble(paras);
    doc.ensure_nonempty();
    work.log();
    Ok(doc)
}

fn paragraph_limit(max_paragraphs: usize) -> DocbinError {
    DocbinError::Limit(format!("the document has more than {max_paragraphs} paragraphs and table cells"))
}

/// The parts of one kind of note: each note's story becomes a part, and `links` gets its
/// reference CP. Empty stories leave no part.
fn note_parts(
    doc: &mut Document,
    ctx: &WalkCtx<'_>,
    stories: impl Iterator<Item = (u32, (u32, u32))>,
    (kind, part_kind): (NoteKind, PartKind),
    (pics, max_paragraphs): (&mut media::Pictures, usize),
    links: &mut Vec<(u32, NoteKind, u32)>,
) -> Result<(), DocbinError> {
    for (cp, (a, b)) in stories {
        if doc.parts.len() >= MAX_PARTS {
            continue;
        }
        // The part's id first, so the note's own number in its text can name it.
        let Ok(id) = doc.try_add_part(part_kind, Vec::new()) else { continue };
        let blocks = story_blocks(&WalkCtx { own_note: Some((kind, id)), ..*ctx }, a, b, pics, max_paragraphs)?;
        if blocks.is_empty() {
            doc.parts.remove(&id);
            continue;
        }
        if let Some(part) = doc.parts.get_mut(&id) {
            part.blocks = blocks;
        }
        links.push((cp, kind, id));
    }
    Ok(())
}

/// Walks one story into blocks. A walk that spent the shared paragraph budget is refused before
/// its partial list is assembled, so a flood does not build blocks it will throw away.
fn story_blocks(ctx: &WalkCtx<'_>, a: u32, b: u32, pics: &mut media::Pictures, max_paragraphs: usize) -> Result<wordcraft_doc::Blocks, DocbinError> {
    let paras = walk(ctx, a, b, pics);
    if ctx.paragraphs.exceeded.get() {
        return Err(paragraph_limit(max_paragraphs));
    }
    Ok(table::assemble(paras))
}

/// A header or footer story's blocks without its guard paragraph mark. [MS-DOC] 2.3.3: a
/// non-empty header story ends with a paragraph mark that only separates it from the next story
/// and is not part of its contents, so a one-line header is stored as `text\r\r`. Laid out, the
/// guard would add an empty paragraph to every header and footer and push the body down when
/// the header reaches past the top margin (#5922).
///
/// Only an empty last paragraph after an ordinary one is dropped: a story that is nothing but a
/// mark stays a part (an empty header, not one linked to the previous section), and a mark right
/// after a table row stays the paragraph that closes the table.
fn header_story_blocks(
    ctx: &WalkCtx<'_>,
    a: u32,
    b: u32,
    pics: &mut media::Pictures,
    max_paragraphs: usize,
) -> Result<wordcraft_doc::Blocks, DocbinError> {
    let mut paras = walk(ctx, a, b, pics);
    if ctx.paragraphs.exceeded.get() {
        return Err(paragraph_limit(max_paragraphs));
    }
    let outside_table = |p: &table::ParaOut| p.row.as_ref().is_none_or(|r| !r.in_table && !r.ttp && !r.cell_mark);
    if let [.., prev, last] = paras.as_slice() {
        let guard = outside_table(last) && last.para.text.is_empty() && last.para.objects.is_empty() && last.para.section.is_none();
        if guard && outside_table(prev) {
            paras.pop();
        }
    }
    Ok(table::assemble(paras))
}

/// A story's CP range `[a, b)`, relative to a subdocument starting at `base` with `len`
/// CPs, as absolute CPs clamped to that subdocument; `None` when empty.
fn story(base: u32, len: u32, (a, b): (u32, u32)) -> Option<(u32, u32)> {
    let (a, b) = (a.min(len), b.min(len));
    if b <= a {
        return None;
    }
    Some((base.checked_add(a)?, base.checked_add(b)?))
}

/// The sprms a `sprmPHugePapx` in `papx` points to in the Data stream: a `PrcData`, a two-byte byte
/// count and then the grpprl. `None` when `papx` has none, or when it points outside the stream.
fn huge_papx<'d>(data: &'d [u8], papx: &[u8]) -> Option<&'d [u8]> {
    let prl = sprm::iter(papx).find(|p| p.op == P_HUGE_PAPX)?;
    let [a, b, c, d, ..] = *prl.operand else { return None };
    let at = u32::from_le_bytes([a, b, c, d]) as usize;
    let body = at.checked_add(2)?;
    let cb = u16::from_le_bytes(data.get(at..body)?.try_into().ok()?) as usize;
    data.get(body..body.checked_add(cb)?)
}

/// Everything the walker needs besides its CP range.
struct WalkCtx<'a> {
    word: &'a [u8],
    data: &'a [u8],
    pieces: &'a PieceTable,
    chpx_bins: &'a Bins,
    papx_bins: &'a Bins,
    fonts: &'a [String],
    raw_styles: &'a [stsh::RawStyle],
    style_ids: &'a [String],
    sheet: &'a StyleSheet,
    /// (reference CP in the main document, kind, part id) for notes.
    notes: &'a [(u32, NoteKind, u32)],
    /// The note whose text is being walked: its reference character is the note's own number.
    own_note: Option<(NoteKind, u32)>,
    /// Bookmark boundaries (main-document CPs), sorted for a sequential walk.
    marks: &'a [(u32, notes::Mark)],
    /// Floating shapes and pictures by anchor CP (absolute), sorted.
    drawn: &'a [(u32, InlineObject)],
    /// Where sections end (main-document CPs, sorted): the section mark (0x0C) just before
    /// each ends the section's last paragraph, it isn't a page break inside it.
    sect_ends: &'a [u32],
    /// Embedded objects' print presentations, by storage id.
    prints: &'a ole::Prints,
    /// Embedded objects kept whole for writing back, with the parts they use.
    kept: &'a std::cell::RefCell<ole::Kept>,
    /// Characters still allowed across every story walk of this document. Story ranges (headers,
    /// footnotes, text boxes) are hostile and can each span a whole subdocument, so thousands of
    /// them could otherwise materialise the same text thousands of times; this bounds the total.
    budget: &'a std::cell::Cell<u64>,
    /// Paragraph and cell marks still allowed across every story walk ([`MAX_PARAGRAPHS`]).
    paragraphs: &'a ParaBudget,
    /// Repeated-work bytes left ([`MAX_WORK`]), and what they left out.
    work: &'a Work,
}

/// The paragraph and cell marks still allowed in this document, and whether a walk ran into
/// the end of them (the read then fails).
struct ParaBudget {
    left: std::cell::Cell<usize>,
    exceeded: std::cell::Cell<bool>,
}

impl ParaBudget {
    /// Spends one mark; false (and the overrun noted) when none are left.
    fn spend(&self) -> bool {
        match self.left.get().checked_sub(1) {
            Some(left) => {
                self.left.set(left);
                true
            }
            None => {
                self.exceeded.set(true);
                false
            }
        }
    }
}

/// State while assembling one paragraph from per-CP formatting.
#[derive(Default)]
struct ParaBuild {
    text: String,
    runs: Vec<Run>,
    props: wordcraft_doc::props::ParaProps,
    mark: CharProps,
    /// Which resolved formatting ([`FmtState::generation`]) the last character came from, while
    /// the last run is still the one it extended: the same generation means equal properties.
    run_gen: Option<u64>,
    /// Inline objects, each anchored at a U+FFFC pushed into `text`.
    objects: Vec<InlineObject>,
}

impl ParaBuild {
    fn push(&mut self, c: char, props: CharProps) {
        self.run_gen = None;
        let len = c.len_utf8();
        match self.runs.last_mut() {
            Some(r) if r.props == props => r.len += len,
            _ => self.runs.push(Run { len, props }),
        }
        self.text.push(c);
    }

    /// [`ParaBuild::push`] for properties that are resolved once and shared by many characters:
    /// they are cloned only when a run starts, and not even compared while `generation` stays
    /// the same.
    fn push_shared(&mut self, c: char, props: &CharProps, generation: u64) {
        let len = c.len_utf8();
        match self.runs.last_mut() {
            Some(r) if self.run_gen == Some(generation) || r.props == *props => r.len += len,
            _ => self.runs.push(Run { len, props: props.clone() }),
        }
        self.run_gen = Some(generation);
        self.text.push(c);
    }

    /// Append `c` to the current run, whose properties the caller knows it matches.
    fn extend(&mut self, c: char) {
        if let Some(r) = self.runs.last_mut() {
            r.len += c.len_utf8();
        }
        self.text.push(c);
    }
}

/// Walk a CP range, splitting paragraphs at 0x0D/0x07 marks, applying the direct character
/// formatting of each CP and the paragraph formatting of each mark. Works for the main
/// document and for any subdocument range (headers, footers, notes…). Field characters,
/// note references, bookmarks and inline pictures become inline objects; pictures go into
/// `pics`, which shares one media entry per Data-stream location.
///
/// The range is clamped to the last piece's CP, and unreadable stretches (gaps, truncated
/// pieces) are skipped a whole piece at a time, so the work is bounded by the text that
/// really exists, not by the CP numbers a file claims.
fn walk<'a>(ctx: &WalkCtx<'a>, cp_start: u32, cp_end: u32, pics: &mut media::Pictures) -> Vec<ParaOut> {
    let cp_end = cp_end.min(ctx.pieces.cp_end());
    let mut out = Vec::new();
    let mut pb = ParaBuild::default();
    let mut cp = cp_start;
    let mut marks = ctx.marks.iter().peekable();
    // Open fields, innermost last: instruction characters are swallowed, the result is kept until
    // the end character, then emitted as one object (hyperlinks keep their result text). A field
    // in another's instruction is flattened into it; one in a result is read on its own.
    let mut fields: Vec<OpenField> = Vec::new();
    // Storage id of the embedded object the open field's code references (sprmCFOle2).
    let mut object_id: Option<u32> = None;
    let mut fmt = FmtState::default();
    // The paragraph being built has had its mark looked up ([`para_head`]): its style is known
    // before its characters are resolved, and the mark's PAPX is read once.
    let mut head: Option<Option<ParaHead<'a>>> = None;
    while cp < cp_end {
        // Bookmark boundaries at this CP (main-document walks only; subdocument walks get
        // an empty table and `cp` never matches the main-document CPs).
        // `<=` so a boundary inside a skipped stretch is still emitted (late) rather than
        // blocking every later one.
        while marks.peek().is_some_and(|&(mcp, _)| *mcp <= cp) {
            if let Some((_, m)) = marks.next() {
                let obj = match m {
                    notes::Mark::Start(name) => InlineObject::BookmarkStart { name: name.clone() },
                    notes::Mark::End(name) => InlineObject::BookmarkEnd { name: name.clone() },
                };
                pb.push('\u{FFFC}', CharProps::default());
                pb.objects.push(obj);
            }
        }
        let (c, units) = match ctx.pieces.char_at(ctx.word, cp) {
            Ok(found) => found,
            Err(next) => {
                cp = next;
                continue;
            }
        };
        // Every real character counts against the document-wide budget; once it is spent, stop
        // (a hostile file with thousands of story ranges over the same text can't multiply on).
        if ctx.budget.get() == 0 {
            break;
        }
        if head.is_none() {
            // Toggle operands (0x80/0x81) resolve against the paragraph's own style, which only
            // its mark's PAPX names: look it up before the first character is resolved.
            let found = para_head(ctx, cp, cp_end);
            pb.props.style = found.and_then(|h| ctx.style_ids.get(usize::from(h.istd))).filter(|id| !id.is_empty()).cloned();
            head = Some(found);
        }
        ctx.budget.set(ctx.budget.get() - 1);
        // A section mark (0x0C) at a section's end is that section's last paragraph mark.
        let section_mark = c == '\u{C}' && ctx.sect_ends.binary_search(&cp.saturating_add(1)).is_ok();
        match c {
            '\r' | '\u{7}' | '\u{C}' if c != '\u{C}' || section_mark => {
                if !ctx.paragraphs.spend() {
                    break;
                }
                // Paragraph mark: its PAPX covers the FC range of the paragraph it ends, so
                // querying at the mark's own FC finds that paragraph's properties.
                let (istd, papx) = match head.take().flatten().filter(|h| h.mark == cp) {
                    Some(h) => (h.istd, h.papx),
                    None => mark_papx(ctx, cp),
                };
                pb.props.style = ctx.style_ids.get(usize::from(istd)).filter(|id| !id.is_empty()).cloned();
                let mut row = table::decode(papx);
                row.cell_mark = c == '\u{7}';
                // Only table marks keep their table context; a plain paragraph would carry
                // a `RowInfo` (about 300 bytes) it never uses.
                let row = row.is_table().then(|| Box::new(row));
                let mut ilvl: Option<u8> = None;
                let mut ilfo: Option<i32> = None;
                // Direct frame sprms merge into the style's frame field by field, so start from it.
                if sprm::iter(papx).any(|prl| fmt::is_frame_sprm(prl.op)) {
                    pb.props.frame = fmt::style_frame(ctx.raw_styles, istd as usize);
                }
                for prl in sprm::iter(papx) {
                    if prl.op == P_ILVL {
                        ilvl = Some(ilvl_operand(prl.operand));
                        continue;
                    }
                    if prl.op == P_ILFO {
                        ilfo = Some(ilfo_operand(prl.operand));
                        continue;
                    }
                    if prl.sgc() == 1 {
                        fmt::apply_para(&mut pb.props, &prl);
                    }
                }
                // The paragraph's own list, or its style's list at its own level. Switching the
                // style's list off is `num: 0`, which also drops the style's indents, so only a
                // style that has a list gets it.
                let style_list = pb.props.style.as_deref().and_then(|id| ctx.sheet.get(id)).and_then(|s| s.para.numbering).filter(|n| n.num != 0);
                pb.props.numbering = match (ilfo.map(num_of), style_list) {
                    (Some(Some(num)), _) => Some(NumRef { num, level: ilvl.or(style_list.map(|n| n.level)).unwrap_or(0) }),
                    (Some(None), Some(s)) => Some(NumRef { num: 0, level: s.level }),
                    (None, Some(s)) => ilvl.map(|level| NumRef { num: s.num, level }),
                    _ => None,
                };
                let fc = ctx.pieces.fc_of_cp(cp).unwrap_or(0);
                let base = pb.props.style.as_deref().and_then(|id| ctx.sheet.get(id)).map(|style| style.chr.clone()).unwrap_or_default();
                pb.mark = char_props(ctx.chpx_bins.chpx(ctx.word, fc), ctx.fonts, &base, ctx.raw_styles, ctx.style_ids);
                let done = std::mem::take(&mut pb);
                head = None;
                out.push(ParaOut {
                    para: Paragraph {
                        text: done.text,
                        runs: done.runs,
                        props: done.props,
                        mark: done.mark,
                        objects: done.objects,
                        ..Default::default()
                    },
                    end_cp: cp.saturating_add(1),
                    row,
                });
            }
            // Field begin/separator/end ([MS-DOC] §2.9.84 Plcfld).
            '\u{13}' => {
                let props = || {
                    let base = pb.props.style.as_deref().and_then(|id| ctx.sheet.get(id)).map(|style| style.chr.clone()).unwrap_or_default();
                    let fc = ctx.pieces.fc_of_cp(cp).unwrap_or(0);
                    let mut props = char_props(ctx.chpx_bins.chpx(ctx.word, fc), ctx.fonts, &base, ctx.raw_styles, ctx.style_ids);
                    apply_prm(&mut props, ctx.pieces.piece_prm(cp), ctx.fonts, &base, ctx.work);
                    props
                };
                if field_begin(&mut fields, props) {
                    object_id = None;
                }
                cp = cp.saturating_add(1);
                continue;
            }
            '\u{14}' => {
                field_separator(&mut fields, &mut pb);
                cp = cp.saturating_add(1);
                continue;
            }
            '\u{15}' => {
                if field_end(&mut fields, &mut pb, ctx.work) {
                    object_id = None;
                }
                cp = cp.saturating_add(1);
                continue;
            }
            '\u{1}' => {
                // A range field's result is ordinary content: its pictures are not the field's.
                let top = fields.last().filter(|f| f.build.depth == 1 && !f.range && f.over_hidden == 0 && f.over_shown == 0).map(|f| &f.build);
                let open = top.map(|f| f.sep.is_some());
                let instr = top.map(|f| f.instr.as_str());
                if inline_picture(ctx, cp, (open, instr), &mut object_id, &mut pb, pics)
                    && let Some(f) = fields.last_mut()
                {
                    f.build.object = true;
                }
            }
            '\u{8}' => {
                // Drawn object anchor: the floating shape or picture anchored at this CP.
                let at = ctx.drawn.partition_point(|(c, _)| *c < cp);
                // Each copy (a story can be walked more than once) is charged to the work budget
                // by what a clone copies: a picture's alt text, a group with all its members (#3710).
                for (_, obj) in ctx.drawn.get(at..).unwrap_or_default().iter().take_while(|(c, _)| *c == cp) {
                    if !ctx.work.charge(object_cost(obj), Cost::FloatingObject) {
                        continue;
                    }
                    pb.push('\u{FFFC}', CharProps::default());
                    pb.objects.push(obj.clone());
                }
            }
            '\u{2}' => {
                // Footnote or endnote reference; custom symbols share the same character. In a
                // note's own text it is that note's number, as `w:footnoteRef` is in DOCX.
                let at = ctx.notes.partition_point(|(c, _, _)| *c < cp);
                let note = ctx.notes.get(at).filter(|(c, _, _)| *c == cp).map(|&(_, kind, id)| (kind, id)).or(ctx.own_note);
                if let Some((kind, id)) = note {
                    pb.push('\u{FFFC}', CharProps::default());
                    pb.objects.push(InlineObject::NoteRef { kind, id, custom: String::new() });
                }
            }
            '\t' => push_formatted(&mut pb, cp, '\t', ctx, fields.last_mut(), &mut fmt),
            '\u{B}' => push_formatted(&mut pb, cp, '\n', ctx, fields.last_mut(), &mut fmt),
            '\u{C}' => push_formatted(&mut pb, cp, '\u{C}', ctx, fields.last_mut(), &mut fmt),
            '\u{E}' => push_formatted(&mut pb, cp, '\u{E}', ctx, fields.last_mut(), &mut fmt),
            '\u{1E}' => push_formatted(&mut pb, cp, '\u{2011}', ctx, fields.last_mut(), &mut fmt),
            '\u{1F}' => push_formatted(&mut pb, cp, '\u{AD}', ctx, fields.last_mut(), &mut fmt),
            // Object anchors, comment references and other special characters are dropped
            // (or handled by later passes).
            c if (c as u32) < 0x20 => {}
            c => push_formatted(&mut pb, cp, c, ctx, fields.last_mut(), &mut fmt),
        }
        cp = cp.saturating_add(units);
    }
    // A table of contents still open when the story ends (a cut-off file) is closed here, so its
    // `FieldStart` doesn't reach a save without its end.
    for _ in fields.iter().filter(|f| f.range) {
        push_field_end(&mut pb);
    }
    if !pb.text.is_empty() {
        let done = pb;
        out.push(ParaOut {
            para: Paragraph { text: done.text, runs: done.runs, props: done.props, mark: done.mark, objects: done.objects, ..Default::default() },
            end_cp: cp_end,
            row: None,
        });
    }
    out
}

/// A paragraph's mark, found before its characters are walked: its CP and the istd and grpprl
/// of its PAPX.
#[derive(Clone, Copy)]
struct ParaHead<'a> {
    mark: u32,
    istd: u16,
    papx: &'a [u8],
}

/// The PAPX of the paragraph whose mark is at `cp`: it covers the FC range of the paragraph it
/// ends, so querying at the mark's own FC finds that paragraph's properties.
fn mark_papx<'a>(ctx: &WalkCtx<'a>, cp: u32) -> (u16, &'a [u8]) {
    let fc_mark = ctx.pieces.fc_of_cp(cp).unwrap_or(0);
    let (istd, papx) = ctx.papx_bins.papx(ctx.word, fc_mark);
    (istd, huge_papx(ctx.data, papx).unwrap_or(papx))
}

/// The mark of the paragraph starting at `cp` (the next 0x0D, 0x07 or section-end 0x0C before
/// `cp_end`) and its PAPX. The scan reads no more characters than the walk may still read
/// ([`WalkCtx::budget`]), and the walk then reads the same ones, so it at most doubles the
/// work; `None` when there is no mark within that.
fn para_head<'a>(ctx: &WalkCtx<'a>, cp: u32, cp_end: u32) -> Option<ParaHead<'a>> {
    let mut left = ctx.budget.get();
    let mut at = cp;
    while at < cp_end {
        match ctx.pieces.char_at(ctx.word, at) {
            Ok((c, units)) => {
                left = left.checked_sub(1)?;
                let mark = match c {
                    '\r' | '\u{7}' => true,
                    '\u{C}' => ctx.sect_ends.binary_search(&at.saturating_add(1)).is_ok(),
                    _ => false,
                };
                if mark {
                    let (istd, papx) = mark_papx(ctx, at);
                    return Some(ParaHead { mark: at, istd, papx });
                }
                at = at.saturating_add(units.max(1));
            }
            Err(next) if next > at => at = next,
            Err(_) => return None,
        }
    }
    None
}

/// An inline picture anchor (`\x01`): the CHPX carries sprmCPicLocation. `open` is whether the
/// innermost open field (if any, and not nested) is in its result; in its code, an embedded
/// object's reference (sprmCFOle2) sets `object_id`, and the result's picture then prints from
/// that object's print presentation, when it has one. A result `\x01` that is itself an object
/// reference (no object in the code) is drawn from the object's storage: its location is the
/// storage id, not a Data stream fc.
///
/// In an `EMBED` field's result (`instr` is the field code), the picture of an object whose
/// storage is kept carries the object itself (its `w:object`): true then, and the field leaves
/// no trace of its own (the object replaces it, as in a .docx).
fn inline_picture(
    ctx: &WalkCtx<'_>,
    cp: u32,
    (open, instr): (Option<bool>, Option<&str>),
    object_id: &mut Option<u32>,
    pb: &mut ParaBuild,
    pics: &mut media::Pictures,
) -> bool {
    let fc = ctx.pieces.fc_of_cp(cp).unwrap_or(0);
    let chpx = ctx.chpx_bins.chpx(ctx.word, fc);
    // The anchor takes its run's formatting as a text character there would: Word draws an
    // inline picture at its run's size and raised or lowered position (sprmCHpsPos). Only the
    // sprms that make it a picture anchor (sprmCFSpec, sprmCPicLocation) have no visible property.
    let base = pb.props.style.as_deref().and_then(|id| ctx.sheet.get(id)).map(|style| style.chr.clone()).unwrap_or_default();
    let mut props = char_props(chpx, ctx.fonts, &base, ctx.raw_styles, ctx.style_ids);
    apply_prm(&mut props, ctx.pieces.piece_prm(cp), ctx.fonts, &base, ctx.work);
    let mut object = None;
    match open {
        Some(false) => {
            if let Some(id) = ole::object_id(chpx) {
                *object_id = Some(id);
            }
        }
        Some(true) => object = object_id.and_then(|id| Some((id, ctx.prints.get(id)?))),
        None => {}
    }
    let result_object = if open == Some(true) && object_id.is_none() { ole::object_id(chpx) } else { None };
    let pic = if let Some(id) = result_object {
        ctx.prints.result_object(id).and_then(|o| pics.ole_object(id, o.bytes, o.ext, o.w, o.h))
    } else {
        let pic = pic_location(chpx).and_then(|at| pics.get(ctx.data, at)).cloned();
        match (pic, object) {
            (Some(p), Some((id, emf))) => pics.ole(id, emf, &p).or(Some(p)),
            (p, _) => p,
        }
    };
    match pic {
        // Empty shapes keep their space; horizontal rules fill their line via the rule metadata.
        Some(p) if p.empty || p.rule.is_some() => {
            pb.push('\u{FFFC}', props);
            pb.objects.push(InlineObject::Shape {
                kind: wordcraft_doc::para::ShapeKind::Rectangle,
                w: p.w,
                h: p.h,
                fill: p.rule.map(|(color, _)| wordcraft_doc::fill::Fill::solid(color)),
                stroke: None,
                stroke_width: 0.0,
                float: Default::default(),
                story: None,
                freeform: None,
                effects: Default::default(),
                extra: wordcraft_doc::connector::ShapeExtra { hr: p.rule.map(|(_, hr)| hr), ..Default::default() },
            });
        }
        Some(p) => {
            // Every anchor of a picture gets its own copy of the alt text; the
            // copies are charged to the work budget (#3709).
            let alt = if ctx.work.charge(p.alt.len() as u64, Cost::AltText) { p.alt } else { String::new() };
            let ole = match (open, instr, result_object.or(*object_id)) {
                (Some(true), Some(instr), Some(id)) if ole::is_embed(instr) => {
                    ctx.kept.borrow_mut().object(ctx.prints, id, instr, &p.key, &pics.media, (p.w, p.h))
                }
                _ => None,
            };
            let object = ole.is_some();
            pb.push('\u{FFFC}', props);
            pb.objects.push(InlineObject::Image {
                media: p.key,
                w: p.w,
                h: p.h,
                alt,
                float: Default::default(),
                crop: p.crop,
                fx: Default::default(),
                ole,
            });
            return object;
        }
        None => pics.dropped(cp),
    }
    false
}

/// Field assembly: `instr` collects everything before the separator (nested fields
/// flattened), `sep` holds the cached result once the separator was seen.
struct FieldBuild {
    depth: u32,
    instr: String,
    sep: Option<FieldResult>,
    props: CharProps,
    /// An `EMBED` field whose object is kept whole: the object replaces the field.
    object: bool,
}

/// An open field in the walk's stack: the field being assembled and how it nests.
struct OpenField {
    build: FieldBuild,
    /// A range field: its result is ordinary content after a `FieldStart` marker.
    range: bool,
    /// Fields past [`MAX_FIELD_NEST`] opened in this one's result, only counted: those still
    /// in their code (whose characters are dropped), and those showing their result here.
    over_hidden: u32,
    over_shown: u32,
}

/// A field's cached result: its text and, as runs, the formatting of that text. A run is added
/// only when the formatting changes, so a result of millions of equally formatted characters
/// costs one `CharProps`, not one per character (365 bytes each).
#[derive(Default)]
struct FieldResult {
    text: String,
    /// (byte length in `text`, formatting) in order; adjacent runs differ.
    runs: Vec<(usize, CharProps)>,
}

impl FieldResult {
    fn push(&mut self, c: char, props: CharProps) {
        let len = c.len_utf8();
        match self.runs.last_mut() {
            Some((n, last)) if *last == props => *n += len,
            _ => self.runs.push((len, props)),
        }
        self.text.push(c);
    }

    /// Another field's result, appended with its formatting.
    fn append(&mut self, other: Self) {
        let mut at = 0usize;
        for (len, props) in other.runs {
            let end = at.saturating_add(len);
            if let Some(text) = other.text.get(at..end) {
                match self.runs.last_mut() {
                    Some((n, last)) if *last == props => *n = n.saturating_add(len),
                    _ => self.runs.push((len, props)),
                }
                self.text.push_str(text);
            }
            at = end;
        }
    }
}

/// Fields open inside one another's results at most; deeper ones are only counted by the
/// innermost (`over_hidden`, `over_shown`), so a hostile file can't grow the stack without bound.
const MAX_FIELD_NEST: usize = 32;

/// Fields whose result stays ordinary content between `FieldStart` and `FieldEnd` markers, as
/// the DOCX reader keeps them: a table of contents (its entries are paragraphs) and ADDIN fields.
fn is_range_instr(instr: &str) -> bool {
    let t = instr.trim_start();
    t.split_whitespace().next().is_some_and(|k| k.eq_ignore_ascii_case("TOC")) || t.get(..5).is_some_and(|k| k.eq_ignore_ascii_case("ADDIN"))
}

/// A field begin (0x13): a new field, or one more level in the innermost field's code. `props`
/// gives a new field's formatting at its begin. True when a new field was opened.
fn field_begin(fields: &mut Vec<OpenField>, props: impl FnOnce() -> CharProps) -> bool {
    let full = fields.len() >= MAX_FIELD_NEST;
    match fields.last_mut() {
        Some(f) if f.over_hidden > 0 => f.over_hidden = f.over_hidden.saturating_add(1),
        Some(f) if f.build.sep.is_none() && !f.range => f.build.depth = f.build.depth.saturating_add(1),
        // Past the cap, a field in a result is counted, not kept: its code is
        // hidden and its result shows in the innermost kept field.
        Some(f) if full => f.over_hidden = 1,
        _ => {
            let props = props();
            let build = FieldBuild { depth: 1, instr: String::new(), sep: None, props, object: false };
            fields.push(OpenField { build, range: false, over_hidden: 0, over_shown: 0 });
            return true;
        }
    }
    false
}

/// A field separator (0x14): the innermost field's result follows.
fn field_separator(fields: &mut [OpenField], pb: &mut ParaBuild) {
    if let Some(f) = fields.last_mut()
        && f.over_hidden > 0
    {
        // The separator of the innermost counted field: its result follows.
        if f.over_hidden == 1 {
            f.over_hidden = 0;
            f.over_shown = f.over_shown.saturating_add(1);
        }
    } else if let Some(f) = fields.last_mut()
        && f.build.depth == 1
        && f.build.sep.is_none()
        && !f.range
        && f.over_shown == 0
    {
        if is_range_instr(&f.build.instr) {
            // A table of contents keeps its entries as ordinary paragraphs between
            // markers, as the DOCX reader does.
            f.range = true;
            push_field_start(pb, &f.build.instr);
        } else {
            f.build.sep = Some(FieldResult::default());
        }
    }
}

/// A field end (0x15): the innermost field closes, into its outer field's result or the paragraph.
/// True when a field (not one more level of a code, nor a counted one) closed.
fn field_end(fields: &mut Vec<OpenField>, pb: &mut ParaBuild, work: &Work) -> bool {
    if let Some(f) = fields.last_mut()
        && (f.over_hidden > 0 || f.over_shown > 0)
    {
        if f.over_hidden > 0 {
            f.over_hidden -= 1;
        } else {
            f.over_shown -= 1;
        }
    } else if let Some(mut f) = fields.pop() {
        if f.build.depth > 1 {
            f.build.depth -= 1;
            fields.push(f);
        } else if f.range {
            push_field_end(pb);
        } else if f.build.sep.is_none() && is_range_instr(&f.build.instr) {
            push_field_start(pb, &f.build.instr);
            push_field_end(pb);
        } else {
            match fields.last_mut().filter(|o| !o.range).and_then(|o| o.build.sep.as_mut()) {
                // An embedded object kept whole replaces its field: nothing of it is left.
                Some(_) if f.build.object => {}
                // A field in another field's kept result shows its own result there.
                Some(outer) => outer.append(f.build.sep.unwrap_or_default()),
                None => finish_field(pb, f.build, work),
            }
        }
        return true;
    }
    false
}

fn push_field_start(pb: &mut ParaBuild, instr: &str) {
    pb.push('\u{FFFC}', CharProps::default());
    pb.objects.push(InlineObject::FieldStart { instr: instr.trim().to_string(), locked: false, dirty: false });
}

fn push_field_end(pb: &mut ParaBuild) {
    pb.push('\u{FFFC}', CharProps::default());
    pb.objects.push(InlineObject::FieldEnd);
}

/// Bytes a clone of `obj` copies: the object itself plus a picture's alt text, a group with all
/// its members (#3710).
fn object_cost(obj: &InlineObject) -> u64 {
    let shallow = std::mem::size_of::<InlineObject>() as u64;
    match obj {
        InlineObject::Image { alt, .. } => shallow.saturating_add(alt.len() as u64),
        InlineObject::Group { children, .. } => children.iter().fold(shallow, |n, c| n.saturating_add(object_cost(&c.obj))),
        _ => shallow,
    }
}

/// Emit a finished field: HYPERLINK keeps its result text as a linked run; every other
/// field becomes one inline object with its cached result.
///
/// The target is copied once per run of equally formatted result text, not per character,
/// and each copy is charged to `work` ([`MAX_WORK`]); once that is spent the rest of the
/// result is kept as plain text (#3708).
fn finish_field(pb: &mut ParaBuild, f: FieldBuild, work: &Work) {
    if f.object {
        return;
    }
    let instr = f.instr.trim().to_string();
    let result = f.sep.unwrap_or_default();
    if let Some(target) = hyperlink_target(&instr) {
        let cost = target.len() as u64;
        let mut chars = result.text.chars();
        for (len, props) in result.runs {
            let mut linked = props;
            if work.charge(cost, Cost::LinkTarget) {
                linked.link = Some(target.clone());
            }
            let mut left = len;
            let Some(first) = chars.next() else { break };
            pb.push(first, linked);
            left = left.saturating_sub(first.len_utf8());
            while left > 0 {
                let Some(c) = chars.next() else { break };
                pb.extend(c);
                left = left.saturating_sub(c.len_utf8());
            }
        }
        return;
    }
    // The visible result owns the formatting; an empty result keeps the begin's.
    let props = result.runs.iter().find(|(len, _)| *len > 0).map(|(_, props)| props.clone()).unwrap_or(f.props);
    pb.push('\u{FFFC}', props);
    pb.objects.push(InlineObject::Field { instr, result: result.text, locked: false, dirty: false, code: false, form: String::new() });
}

/// `sprmCPicLocation` from a CHPX grpprl: the fc of the picture in the Data stream.
fn pic_location(chpx: &[u8]) -> Option<u32> {
    for prl in sprm::iter(chpx) {
        if prl.op == C_PIC_LOCATION {
            return match prl.operand {
                [b0, b1, b2, b3, ..] => Some(u32::from_le_bytes([*b0, *b1, *b2, *b3])),
                _ => None,
            };
        }
    }
    None
}

/// `sprmCSymbol` from a CHPX grpprl: the font table index and the character code of a symbol
/// character ([MS-DOC] §2.6.1, CSymbolOperand).
fn symbol_operand(chpx: &[u8]) -> Option<(u16, u16)> {
    for prl in sprm::iter(chpx) {
        if prl.op == C_SYMBOL
            && let [f0, f1, c0, c1, ..] = prl.operand
        {
            return Some((u16::from_le_bytes([*f0, *f1]), u16::from_le_bytes([*c0, *c1])));
        }
    }
    None
}

/// `sprmPIlvl` operand: the list level, 0-8.
fn ilvl_operand(operand: &[u8]) -> u8 {
    operand.first().copied().unwrap_or(0).min(8)
}

/// `sprmPIlfo` operand: the signed 16-bit LFO reference that [`num_of`] reads.
fn ilfo_operand(operand: &[u8]) -> i32 {
    match operand {
        [b0, b1, ..] => i16::from_le_bytes([*b0, *b1]) as i32,
        _ => 0,
    }
}

/// `sprmPIlfo` operand (a signed 16-bit value, [MS-DOC] §2.6.2) → the num id, if the
/// paragraph is in a list. 0x0001-0x07FE is a 1-based LFO index; 0xF802-0xFFFF (-2046..=-1)
/// is the negation of one and keeps the paragraph's own indents, which we do by leaving the
/// level's indents out of the paragraph (they never enter `ParaProps` anyway). 0 and 0xF801
/// mean "not in a list".
fn num_of(ilfo: i32) -> Option<u32> {
    match ilfo {
        0x0001..=0x07FE | -0x07FE..=-1 => Some(ilfo.unsigned_abs()),
        _ => None,
    }
}

/// Resolve the character properties for the character at `cp`: the CHPX grpprl of its FC,
/// plus the piece's Prm, over the paragraph style's base character props. Inside a field,
/// the character is recorded into the instruction or the cached result instead of (or as
/// well as) the visible text.
fn push_formatted<'a>(pb: &mut ParaBuild, cp: u32, c: char, ctx: &WalkCtx<'a>, field: Option<&mut OpenField>, st: &mut FmtState<'a>) {
    // The code of a field counted past the nesting cap is never shown.
    if field.as_ref().is_some_and(|f| f.over_hidden > 0) {
        return;
    }
    // A range field's result is ordinary content.
    let field = field.filter(|f| !f.range).map(|f| &mut f.build);
    // The piece and the CHPX run of the previous character usually cover this one too.
    let span = match st.span.filter(|s| s.contains(cp)) {
        Some(s) => Some(s),
        None => {
            st.span = ctx.pieces.span(cp);
            st.span
        }
    };
    let fc = span.and_then(|s| s.fc_at(cp)).unwrap_or(0);
    let run = match st.chpx.filter(|r| r.contains(fc)) {
        Some(r) => r,
        None => {
            let r = ctx.chpx_bins.chpx_run(ctx.word, fc);
            st.chpx = Some(r);
            r
        }
    };
    let grpprl = run.grpprl;
    let prm = span.map_or(0, |s| s.prm());
    let style = pb.props.style.as_deref();
    let hit = st.memo.as_ref().is_some_and(|m| std::ptr::eq(m.chpx, grpprl) && m.prm == prm && m.style.as_deref() == style);
    if !hit {
        let default;
        let base = match style.and_then(|id| ctx.sheet.get(id)) {
            Some(sty) => &sty.chr,
            None => {
                default = CharProps::default();
                &default
            }
        };
        let mut props = char_props(grpprl, ctx.fonts, base, ctx.raw_styles, ctx.style_ids);
        apply_prm(&mut props, ctx.pieces.piece_prm(cp), ctx.fonts, base, ctx.work);
        st.memo = Some(FmtMemo { chpx: grpprl, prm, style: style.map(str::to_owned), props });
        st.generation += 1;
    }
    let Some(m) = st.memo.as_ref() else { return };
    // A symbol character (Insert › Symbol) is stored as the placeholder U+0028 with
    // sprmCSymbol carrying the font and the character code ([MS-DOC] §2.6.1).
    let symbol = if c == '(' { symbol_operand(grpprl) } else { None };
    if field.is_none() && symbol.is_none() {
        pb.push_shared(c, &m.props, st.generation);
        return;
    }
    let mut props = m.props.clone();
    let mut c = c;
    if let Some((ftc, xchar)) = symbol {
        if let Some(font) = ctx.fonts.get(usize::from(ftc)) {
            props.font = Some(font.clone());
        }
        if let Some(sym) = char::from_u32(u32::from(xchar)) {
            c = sym;
        }
    }
    match field {
        Some(f) => match f.sep.as_mut() {
            // Inside a field, characters are recorded for the field object instead of shown;
            // `finish_field` emits the visible text (hyperlink runs or one object anchor).
            Some(result) => result.push(c, props),
            None => f.instr.push(c),
        },
        None => pb.push(c, props),
    }
}

/// What the walk carries from one character to the next while it resolves formatting: the piece
/// and the CHPX run the last character was in (so most characters need no search), and the last
/// formatting resolved.
#[derive(Default)]
struct FmtState<'a> {
    span: Option<piece::PieceSpan<'a>>,
    chpx: Option<fkp::ChpxRun<'a>>,
    memo: Option<FmtMemo<'a>>,
    /// Counts the memo's replacements, so equal values mean the same properties.
    generation: u64,
}

/// The last character formatting resolved, reused while the CHPX, the piece's Prm and the
/// paragraph style stay the same, so a Prm grpprl (up to 64 KB) is interpreted once per
/// formatting change, not once per character (#3692).
struct FmtMemo<'a> {
    /// The CHPX grpprl, compared by address: it is a slice of the WordDocument stream.
    chpx: &'a [u8],
    prm: u16,
    style: Option<String>,
    props: CharProps,
}

/// Apply a piece's Prm (Prm0 short sprm or Prm1 grpprl) to character properties. A Prm1
/// grpprl is charged to `work`; once that is spent the grpprl is skipped.
fn apply_prm(props: &mut CharProps, prm: PrmRef<'_>, fonts: &[String], base: &CharProps, work: &Work) {
    match prm {
        PrmRef::None => {}
        PrmRef::Sprm0 { isprm, val } => {
            if let Some(op) = sprm::from_prm0(isprm) {
                let operand = [val];
                let prl = Prl { op, operand: &operand };
                if prl.sgc() == 2 {
                    fmt::apply_char(props, &prl, base, fonts);
                }
            }
        }
        PrmRef::Grpprl(Some(g)) => {
            if !work.charge(g.len() as u64, Cost::Formatting) {
                return;
            }
            for prl in sprm::iter(g) {
                if prl.sgc() == 2 {
                    fmt::apply_char(props, &prl, base, fonts);
                }
            }
        }
        PrmRef::Grpprl(None) => {}
    }
}

/// Character props from a CHPX grpprl: the direct formatting only. `base` is the paragraph
/// style's character formatting, which toggle operands (0x80 keep, 0x81 invert, [MS-DOC]
/// §2.9.326) resolve against; it is not copied into the result.
fn char_props(grpprl: &[u8], fonts: &[String], base: &CharProps, raw_styles: &[stsh::RawStyle], style_ids: &[String]) -> CharProps {
    let mut p = CharProps::default();
    for prl in sprm::iter(grpprl) {
        if prl.op == fmt::C_ISTD {
            // sprmCIstd names a character style, not direct formatting. Index 10 is
            // Default Paragraph Font and means no character-style override.
            p.style = match prl.operand {
                [lo, hi] => {
                    let istd = u16::from_le_bytes([*lo, *hi]);
                    raw_styles
                        .get(usize::from(istd))
                        .filter(|s| istd != 10 && s.stk == 2)
                        .and_then(|_| style_ids.get(usize::from(istd)))
                        .filter(|id| !id.is_empty())
                        .cloned()
                }
                _ => None,
            };
        } else if prl.sgc() == 2 {
            fmt::apply_char(&mut p, &prl, base, fonts);
        }
    }
    p
}

/// Read a root-level stream fully, or `None` if it is absent.
fn stream<F: Read + Seek>(comp: &mut cfb::CompoundFile<F>, name: &str) -> Option<Vec<u8>> {
    let mut s = comp.open_stream(format!("/{name}")).ok()?;
    let mut v = Vec::new();
    // Read with a cap one byte above the limit so oversized streams are still caught by the
    // caller instead of exhausting memory here.
    s.by_ref().take(MAX_STREAM + 1).read_to_end(&mut v).ok()?;
    Some(v)
}

#[cfg(test)]
// The acceptance tests of P0232 (end of tests.rs) were committed unformatted and must stay
// byte-for-byte as committed; drop this skip once that file is formatted.
#[rustfmt::skip]
mod tests;

#[cfg(test)]
#[path = "tests/table_float_tap.rs"]
mod table_float_tap;

#[cfg(test)]
mod character_style_tests {
    use super::*;

    #[test]
    fn character_style_reference_preserves_paragraph_toggle_base() {
        let styles = vec![stsh::RawStyle { sti: stsh::STI_USER, stk: 2, name: "Emphasis".into(), ..Default::default() }];
        let base = CharProps { bold: Some(true), italic: Some(false), ..Default::default() };
        // Toggle operands keep (0x80) or invert (0x81) the existing paragraph base,
        // regardless of whether the character-style reference precedes them.
        for grpprl in [&[0x30, 0x4A, 0, 0, 0x35, 0x08, 0x81, 0x36, 0x08, 0x80][..], &[0x35, 0x08, 0x81, 0x36, 0x08, 0x80, 0x30, 0x4A, 0, 0][..]] {
            let props = char_props(grpprl, &[], &base, &styles, &fmt::style_ids(&styles));
            assert_eq!(props.style.as_deref(), Some("Emphasis"));
            assert_eq!(props.bold, Some(false));
            assert_eq!(props.italic, Some(false));
        }
    }

    #[test]
    fn character_style_reference_validates_kind_name_and_default_index() {
        let mut styles = vec![stsh::RawStyle { sti: 0x0FFF, stk: 2, name: "custom emphasis".into(), ..Default::default() }; 11];
        for (istd, stk, name) in
            [(0u16, 1, "Normal"), (1, 2, "custom emphasis"), (2, 3, "Table"), (3, 4, "List"), (4, 2, ""), (10, 2, "Default Paragraph Font")]
        {
            // User-made (istiUser): a built-in sti would give Word's id instead of the name's.
            styles[usize::from(istd)] = stsh::RawStyle { sti: stsh::STI_USER, stk, name: name.into(), ..Default::default() };
            // Direct italic precedes the style; direct bold follows it.
            let [lo, hi] = istd.to_le_bytes();
            let grpprl = [0x36, 0x08, 1, 0x30, 0x4A, lo, hi, 0x35, 0x08, 1];
            let props = char_props(&grpprl, &[], &CharProps::default(), &styles, &fmt::style_ids(&styles));
            assert_eq!(props.style, (istd == 1).then(|| fmt::style_id(name)));
            assert_eq!(props.italic, Some(true));
            assert_eq!(props.bold, Some(true));
        }
        for grpprl in [&[0x30, 0x4A, 1][..], &[0x30, 0x4A, 0xFF, 0xFF][..]] {
            assert_eq!(char_props(grpprl, &[], &CharProps::default(), &styles, &fmt::style_ids(&styles)).style, None);
        }
    }
}
