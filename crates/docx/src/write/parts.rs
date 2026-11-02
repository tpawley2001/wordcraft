use super::bibliography::{bibliography_props_xml, bibliography_xml};
use super::settings::{
    ANONYMOUS_INITIALS, annotation_author, app_xml, core_xml, note_id_base, notes_part, settings_xml, theme_xml, write_comment_ids,
};
use super::styles::{numbering_xml, styles_xml};
use super::{
    BTreeMap, Blocks, CT_WML, Document, DocxError, Flavor, HashMap, MAX_VBA_RELATED, PartRels, VBA_PROJECT_PART, VBA_RELATED, W, Writer, embed,
    glossary, rt, settings_merge, watermark, xml,
};

/// The parts of the package [`write_as`] zips, in order, `[Content_Types].xml` first: (part name
/// without the leading `/`, bytes).
pub(crate) fn write_parts(doc: &Document, flavor: Flavor) -> Result<Vec<(String, Vec<u8>)>, DocxError> {
    // A range field must be whole in the file: drop markers that lost their partner in editing.
    let balanced;
    let doc = if doc.has_unbalanced_field_ranges() {
        let mut d = doc.clone();
        d.balance_field_ranges();
        balanced = d;
        &balanced
    } else {
        doc
    };
    let mut wr = Writer {
        eq_number: 0,
        comment_refs: Default::default(),
        doc,
        media_files: BTreeMap::new(),
        bookmarks: HashMap::new(),
        next_rev_id: 0,
        open_moves: Vec::new(),
        // Fresh drawing ids start past the shapes' own (connectors refer to those).
        docpr: doc.next_shape_id().saturating_sub(1),
        shape_ids: std::collections::HashSet::new(),
        z: 251_658_240,
        footnotes: Vec::new(),
        endnotes: Vec::new(),
        hf: Vec::new(),
        linked_headers: Default::default(),
        wm_header: (0..=u32::MAX).rev().find(|id| !doc.parts.contains_key(id)).unwrap_or(u32::MAX),
        pending_watermark: None,
        para_ids: HashMap::new(),
        pending_mark: None,
        current_note: None,
        toc_hold_end: false,
        toc_field: None,
        toc_begin_here: false,
        toc_end_here: false,
        inline_controls: Vec::new(),
        used_media: Default::default(),
        boxes: wordcraft_doc::BoxBudget::default(),
        written_boxes: std::collections::HashSet::new(),
        in_txbx: 0,
        flattened: Vec::new(),
        embeds: embed::EmbedWriter::new(doc),
        txbx_links: doc
            .text_box_chains()
            .iter()
            .enumerate()
            .flat_map(|(n, c)| c.iter().enumerate().map(move |(seq, id)| (*id, (n as u32 + 1, seq as u32))))
            .collect(),
    };
    wr.assign_media();
    wr.embeds.reserve_media(wr.media_files.values());
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut overrides: Vec<(String, String)> = Vec::new();
    let mut rels = PartRels::default();
    let mut root = PartRels::default();
    let ns = xml::body_ns();
    let ns_refs: Vec<(&str, &str)> = ns.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    // Comments get paragraph ids before any story is written.
    let mut comment_paras: Vec<(u32, String)> = Vec::new();
    for (i, (cid, c)) in doc.comments.iter().enumerate() {
        if let Some(part) = doc.parts.get(&c.part)
            && let Some(last) = part.blocks.iter().rev().find_map(|b| b.as_para())
        {
            let pid = format!("{:08X}", 0x1000_0000u32 + i as u32 + 1);
            wr.para_ids.insert(last as *const _ as usize, pid.clone());
            comment_paras.push((*cid, pid));
        }
    }

    let (body, body_anchor) = write_document_body(doc, &mut wr, &mut rels, &ns_refs);

    // Headers and footers (discovered while writing sections).
    let mut done = 0;
    while let Some((id, file, footer)) = wr.hf.get(done).cloned() {
        done += 1;
        let mut prels = PartRels::default();
        let mut w = W::new();
        let tag = if footer { "w:ftr" } else { "w:hdr" };
        w.open(tag, &ns_refs);
        let blocks: Blocks = doc.parts.get(&id).map(|p| p.blocks.clone()).unwrap_or_default();
        // The watermark goes in every header, at the start of its first paragraph.
        wr.pending_watermark = match &doc.settings.watermark {
            Some(wm) if !footer => watermark::run(wm, done, &doc.last_section),
            _ => None,
        };
        wr.blocks(&mut w, &blocks, &mut prels, false, 0);
        // Always taken: the story ends with a top-level paragraph.
        wr.pending_watermark = None;
        w.close(tag);
        push_part(
            &mut entries,
            &mut overrides,
            &format!("word/{file}"),
            w.into_bytes(),
            &format!("{CT_WML}{}+xml", if footer { "footer" } else { "header" }),
            prels,
        );
    }

    // The glossary: kept verbatim while its entries are as read and its parts were kept, else
    // written from the model (before the notes: references its entries hold are collected too).
    // When the privacy settings ask for changes, its kept parts are written without the names and
    // dates of its tracked changes and comments (#3692), or from the model if they can't be.
    let (names, dates) = (doc.settings.remove_personal_info, doc.settings.remove_date_time);
    if glossary::edited(doc)
        || (!doc.glossary.is_empty() && !wr.embeds.kept_glossary())
        || ((names || dates) && !wr.embeds.scrub_glossary(names, dates))
    {
        wr.embeds.drop_glossary = true;
        wr.glossary(&mut entries, &mut overrides, &mut rels);
    }

    write_notes(doc, &ns_refs, &mut wr, &mut entries, &mut overrides, &mut rels);

    write_comments(doc, &comment_paras, &ns_refs, &mut wr, &mut entries, &mut overrides, &mut rels);

    rels.add(rt::STYLES, "styles.xml", false);
    push_part(&mut entries, &mut overrides, "word/styles.xml", styles_xml(doc), &format!("{CT_WML}styles+xml"), PartRels::default());
    if !doc.numbering.nums.is_empty() || !doc.numbering.abstracts.is_empty() {
        rels.add(rt::NUMBERING, "numbering.xml", false);
        push_part(&mut entries, &mut overrides, "word/numbering.xml", numbering_xml(doc), &format!("{CT_WML}numbering+xml"), PartRels::default());
    }
    rels.add(rt::SETTINGS, "settings.xml", false);
    // The attached template is an external relationship of the settings part (§17.15.1.6).
    let mut settings_rels = PartRels::default();
    let template = doc.settings.attached_template.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let template_rel = template.map(|t| settings_rels.add(rt::ATTACHED_TEMPLATE, t, true));
    push_part(
        &mut entries,
        &mut overrides,
        "word/settings.xml",
        {
            let generated = settings_xml(doc, notes_part(doc, &wr, true), notes_part(doc, &wr, false), template_rel.as_deref());
            match doc.passthrough.get(crate::package::SETTINGS_ORIG) {
                Some(orig) => settings_merge::merge(&crate::strict::to_transitional(orig), &generated).unwrap_or(generated),
                None => generated,
            }
        },
        &format!("{CT_WML}settings+xml"),
        settings_rels,
    );
    rels.add(rt::THEME, "theme/theme1.xml", false);
    push_part(
        &mut entries,
        &mut overrides,
        "word/theme/theme1.xml",
        theme_xml(doc),
        "application/vnd.openxmlformats-officedocument.theme+xml",
        PartRels::default(),
    );

    wr.embeds.font_table(&mut rels);

    // Media referenced by some drawing (orphan parts would make the package suspect).
    let mut media_types: BTreeMap<String, &str> = BTreeMap::new();
    for (key, file) in &wr.media_files {
        if !wr.used_media.contains(key) {
            continue;
        }
        if let Some(bytes) = doc.media.get(key) {
            let ext = file.rsplit('.').next().unwrap_or("bin").to_ascii_lowercase();
            media_types.insert(ext, content_type_for(file));
            entries.push((format!("word/media/{file}"), bytes.to_vec()));
        }
    }

    wr.embeds.custom_xml_rels("D", &mut rels);
    wr.embeds.custom_xml_rels("P", &mut root);
    wr.embeds.unmodelled_rels("D", &mut rels);
    wr.embeds.unmodelled_rels("P", &mut root);
    // Parts of embedded objects and custom XML stores kept from the file.
    for (name, bytes, ct, prels) in std::mem::take(&mut wr.embeds.parts) {
        push_part(&mut entries, &mut overrides, &name, bytes, &ct, prels);
    }
    push_web_settings(doc, &mut rels, &mut entries, &mut overrides);

    // The macro project and the parts it relates to (VBA data, signatures…), verbatim. A
    // macro-free package can't hold them (Word drops them too).
    if let Some(vba) = doc.passthrough.get(VBA_PROJECT_PART).filter(|b| !b.is_empty()) {
        if flavor.macros() {
            let mut vba_rels = PartRels::default();
            let manifest = doc.passthrough.get(VBA_RELATED).map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default();
            let mut written: Vec<String> = Vec::new();
            for line in manifest.lines().take(MAX_VBA_RELATED) {
                let mut f = line.split('\t');
                let (Some(kind), Some(path), Some(ct), None) = (f.next(), f.next(), f.next(), f.next()) else { continue };
                let Some(target) = path.strip_prefix("word/").filter(|t| !t.is_empty()) else { continue };
                let Some(bytes) = doc.passthrough.get(path) else { continue };
                let ours = written.iter().any(|w| w.eq_ignore_ascii_case(path));
                // Never shadow a part this writer produces (a hostile relationship may point at one).
                let clash = path.eq_ignore_ascii_case("word/document.xml")
                    || path.eq_ignore_ascii_case(VBA_PROJECT_PART)
                    || path.to_ascii_lowercase().ends_with(".rels")
                    || entries.iter().any(|(n, _)| n.eq_ignore_ascii_case(path));
                if clash && !ours {
                    continue;
                }
                // A relative reference whose first segment has a colon would read as a URI scheme.
                let target = if target.split('/').next().is_some_and(|seg| seg.contains(':')) { format!("./{target}") } else { target.to_string() };
                vba_rels.add(kind, &target, false);
                if !ours {
                    push_part(&mut entries, &mut overrides, path, bytes.to_vec(), ct, PartRels::default());
                    written.push(path.to_string());
                }
            }
            rels.add(rt::VBA_PROJECT, "vbaProject.bin", false);
            push_part(&mut entries, &mut overrides, VBA_PROJECT_PART, vba.to_vec(), "application/vnd.ms-office.vbaProject", vba_rels);
        } else {
            log::warn!("docx: {flavor:?} can't hold macros; the VBA project is left out");
        }
    }

    // Main part goes first in the zip after content types.
    entries.insert(0, ("word/document.xml".into(), body));
    overrides.insert(0, ("/word/document.xml".into(), flavor.main_content_type().into()));
    entries.insert(1, ("word/_rels/document.xml.rels".into(), rels.xml()));

    // Package-level parts.
    root.add(rt::OFFICE_DOC, "word/document.xml", false);
    root.add(rt::CORE, "docProps/core.xml", false);
    root.add(rt::APP, "docProps/app.xml", false);
    entries.push(("docProps/core.xml".into(), core_xml(doc)));
    overrides.push(("/docProps/core.xml".into(), "application/vnd.openxmlformats-package.core-properties+xml".into()));
    entries.push(("docProps/app.xml".into(), app_xml(doc)));
    overrides.push(("/docProps/app.xml".into(), "application/vnd.openxmlformats-officedocument.extended-properties+xml".into()));
    if !doc.custom_props.is_empty() {
        root.add(rt::CUSTOM, "docProps/custom.xml", false);
        entries.push(("docProps/custom.xml".into(), crate::custom::write(&doc.custom_props)));
        overrides.push(("/docProps/custom.xml".into(), "application/vnd.openxmlformats-officedocument.custom-properties+xml".into()));
    }
    if !doc.sources.is_empty() {
        root.add(rt::CUSTOM_XML, "customXml/item1.xml", false);
        let mut item_rels = PartRels::default();
        item_rels.add(rt::CUSTOM_XML_PROPS, "itemProps1.xml", false);
        push_part(
            &mut entries,
            &mut overrides,
            "customXml/item1.xml",
            bibliography_xml(doc),
            "application/vnd.openxmlformats-officedocument.customXml+xml",
            item_rels,
        );
        push_part(
            &mut entries,
            &mut overrides,
            "customXml/itemProps1.xml",
            bibliography_props_xml(),
            "application/vnd.openxmlformats-officedocument.customXmlProperties+xml",
            PartRels::default(),
        );
    }
    entries.insert(0, ("_rels/.rels".into(), root.xml()));

    // Content types.
    let mut w = W::new();
    w.open("Types", &[("xmlns", "http://schemas.openxmlformats.org/package/2006/content-types")]);
    w.empty("Default", &[("Extension", "rels"), ("ContentType", "application/vnd.openxmlformats-package.relationships+xml")]);
    w.empty("Default", &[("Extension", "xml"), ("ContentType", "application/xml")]);
    for (ext, ct) in &media_types {
        if ext != "xml" && ext != "rels" {
            w.empty("Default", &[("Extension", ext), ("ContentType", ct)]);
        }
    }
    for (part, ct) in &overrides {
        w.empty("Override", &[("PartName", part), ("ContentType", ct)]);
    }
    w.close("Types");
    entries.insert(0, ("[Content_Types].xml".into(), w.into_bytes()));
    // Comments without a surviving anchor still need one reference in a saved story.
    let missing: Vec<_> = doc.comments.keys().filter(|id| !wr.comment_refs.contains(id)).copied().collect();
    if !missing.is_empty()
        && let Some((_, bytes)) = entries.iter_mut().find(|(name, _)| name == "word/document.xml")
    {
        let mut refs = W::new();
        refs.s.clear();
        refs.open("w:p", &[]);
        for id in missing {
            wr.comment_reference(&mut refs, id);
        }
        refs.close("w:p");
        bytes.splice(body_anchor..body_anchor, refs.into_bytes());
    }
    for (name, bytes) in &mut entries {
        if name.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("xml"))
            && let Some(rewritten) = modernize_beta_w15(name, bytes)
        {
            *bytes = rewritten;
        }
    }
    Ok(entries)
}

const W15_BETA: &[u8] = b"http://schemas.microsoft.com/office/word/2010/11/wordml";
const W15_FINAL: &[u8] = b"http://schemas.microsoft.com/office/word/2012/wordml";

/// Upgrade declarations of the Word 2013 preview namespace in kept XML. The people part also
/// loses its preview-only contact attributes; its other markup stays byte for byte as read.
fn modernize_beta_w15(name: &str, bytes: &[u8]) -> Option<Vec<u8>> {
    if !bytes.windows(W15_BETA.len()).any(|w| w == W15_BETA) {
        return None;
    }
    let without_contacts;
    let bytes = if name.eq_ignore_ascii_case("word/people.xml") {
        without_contacts = beta_people_without_contacts(bytes).unwrap_or_else(|| bytes.to_vec());
        without_contacts.as_slice()
    } else {
        bytes
    };
    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while let Some(at) = rest.windows(W15_BETA.len()).position(|w| w == W15_BETA) {
        let (head, tail) = rest.split_at(at);
        out.extend_from_slice(head);
        if beta_namespace_declaration(head, tail.get(W15_BETA.len()..).unwrap_or_default()) {
            out.extend_from_slice(W15_FINAL);
        } else {
            out.extend_from_slice(W15_BETA);
        }
        rest = tail.get(W15_BETA.len()..).unwrap_or_default();
    }
    out.extend_from_slice(rest);
    Some(out)
}

/// Is the preview URI at the start of `bytes` the whole value of an `xmlns` declaration?
fn beta_namespace_declaration(head: &[u8], tail: &[u8]) -> bool {
    let Some((&quote, head)) = head.split_last() else { return false };
    if !matches!(quote, b'\'' | b'"') || tail.first() != Some(&quote) {
        return false;
    }
    let mut at = head.len();
    while at > 0 && head.get(at - 1).is_some_and(|b| b.is_ascii_whitespace()) {
        at -= 1;
    }
    if at == 0 || head.get(at - 1) != Some(&b'=') {
        return false;
    }
    at -= 1;
    while at > 0 && head.get(at - 1).is_some_and(|b| b.is_ascii_whitespace()) {
        at -= 1;
    }
    let name_end = at;
    while at > 0 && head.get(at - 1).is_some_and(|b| !b.is_ascii_whitespace() && *b != b'<') {
        at -= 1;
    }
    let name = head.get(at..name_end).unwrap_or_default();
    name == b"xmlns" || (name.starts_with(b"xmlns:") && name.len() > b"xmlns:".len())
}

/// Remove `contact` attributes bound to the preview namespace from its `person` elements.
fn beta_people_without_contacts(bytes: &[u8]) -> Option<Vec<u8>> {
    use quick_xml::events::Event;
    use quick_xml::name::{QName, ResolveResult};
    use quick_xml::reader::NsReader;

    let mut r = NsReader::from_reader(bytes);
    r.config_mut().check_end_names = false;
    let mut out = Vec::with_capacity(bytes.len());
    let mut copied = 0usize;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let (res, ev) = r.read_resolved_event_into(&mut buf).ok()?;
        let is_person = matches!(res, ResolveResult::Bound(ns) if ns.as_ref() == W15_BETA)
            && matches!(&ev, Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"person");
        let end = usize::try_from(r.buffer_position()).ok()?;
        match ev {
            Event::Eof => break,
            Event::Start(ref e) | Event::Empty(ref e) if is_person => {
                let closing = if matches!(ev, Event::Empty(_)) { 3 } else { 2 };
                let start = end.checked_sub(e.as_ref().len().checked_add(closing)?)?;
                if bytes.get(start) != Some(&b'<') || bytes.get(start + 1..end - closing + 1)? != e.as_ref() {
                    return None;
                }
                out.extend_from_slice(bytes.get(copied..start)?);
                let content = e.as_ref();
                let name_end = content.iter().position(|b| b.is_ascii_whitespace() || *b == b'/').unwrap_or(content.len());
                let mut attr = name_end;
                out.push(b'<');
                out.extend_from_slice(content.get(..attr)?);
                let mut kept = attr;
                while attr < content.len() {
                    while content.get(attr).is_some_and(|b| b.is_ascii_whitespace()) {
                        attr += 1;
                    }
                    if content.get(attr).is_none_or(|b| *b == b'/') {
                        break;
                    }
                    let name_start = attr;
                    while content.get(attr).is_some_and(|b| !b.is_ascii_whitespace() && *b != b'=' && *b != b'/') {
                        attr += 1;
                    }
                    let key = content.get(name_start..attr)?;
                    while content.get(attr).is_some_and(|b| b.is_ascii_whitespace()) {
                        attr += 1;
                    }
                    if content.get(attr) != Some(&b'=') {
                        return None;
                    }
                    attr += 1;
                    while content.get(attr).is_some_and(|b| b.is_ascii_whitespace()) {
                        attr += 1;
                    }
                    let Some(&quote @ (b'\'' | b'"')) = content.get(attr) else { return None };
                    attr += 1;
                    let value_end = content.get(attr..)?.iter().position(|b| *b == quote)?.checked_add(attr)?;
                    attr = value_end.checked_add(1)?;
                    let (ares, local) = r.resolver_mut().resolve_attribute(QName(key));
                    if matches!(ares, ResolveResult::Bound(ns) if ns.as_ref() == W15_BETA) && local.as_ref() == b"contact" {
                        let mut remove = name_start;
                        while remove > kept && content.get(remove - 1).is_some_and(|b| b.is_ascii_whitespace()) {
                            remove -= 1;
                        }
                        out.extend_from_slice(content.get(kept..remove)?);
                        kept = attr;
                    }
                }
                out.extend_from_slice(content.get(kept..)?);
                out.extend_from_slice(if closing == 3 { b"/>" } else { b">" });
                copied = end;
            }
            _ => {}
        }
    }
    out.extend_from_slice(bytes.get(copied..)?);
    Some(out)
}

fn write_document_body(doc: &Document, wr: &mut Writer<'_>, rels: &mut PartRels, ns_refs: &[(&str, &str)]) -> (Vec<u8>, usize) {
    let mut w = W::new();
    w.open("w:document", ns_refs);
    let pic = doc.settings.page_picture.as_ref().filter(|p| wr.media_files.contains_key(&p.media) && doc.media.contains_key(&p.media));
    if pic.is_some() || doc.settings.page_color.is_some() {
        let color = doc.settings.page_color.map(|c| c.hex());
        let attrs: Vec<(&str, &str)> = color.iter().map(|c| ("w:color", c.as_str())).collect();
        w.open("w:background", &attrs);
        if let Some(p) = pic
            && let Some(file) = wr.media_files.get(&p.media)
        {
            let rid = rels.add(rt::IMAGE, &format!("media/{file}"), false);
            wr.used_media.insert(p.media.clone());
            w.open("v:background", &[("id", "_x0000_s1025"), ("o:bwmode", "white")]);
            let mut fa: Vec<(&str, &str)> = vec![("r:id", &rid)];
            if !p.fill_type.is_empty() {
                fa.push(("type", &p.fill_type));
            }
            if !p.title.is_empty() {
                fa.push(("o:title", &p.title));
            }
            w.empty("v:fill", &fa);
            w.close("v:background");
        }
        w.close("w:background");
    }
    w.open("w:body", &[]);
    wr.blocks(&mut w, &doc.body, rels, true, 0);
    let anchor = w.output_len();
    let last = doc.last_section.clone();
    wr.sectpr(&mut w, &last, rels);
    w.close("w:body");
    w.close("w:document");
    (w.into_bytes(), anchor)
}

fn write_notes(
    doc: &Document,
    ns_refs: &[(&str, &str)],
    wr: &mut Writer<'_>,
    entries: &mut Vec<(String, Vec<u8>)>,
    overrides: &mut Vec<(String, String)>,
    rels: &mut PartRels,
) {
    for foot in [true, false] {
        let seps = if foot { &doc.footnote_separators } else { &doc.endnote_separators };
        if !notes_part(doc, wr, foot) {
            continue;
        }
        let (root, tag, file) = if foot { ("w:footnotes", "w:footnote", "footnotes.xml") } else { ("w:endnotes", "w:endnote", "endnotes.xml") };
        let mut prels = PartRels::default();
        let mut w = W::new();
        w.open(root, ns_refs);
        // The separators and continuation notice: the document's own, else the defaults (no
        // notice). Word numbers them -1, 0 and 1.
        for (id, kind, own) in [
            ("-1", "separator", &seps.separator),
            ("0", "continuationSeparator", &seps.continuation_separator),
            ("1", "continuationNotice", &seps.continuation_notice),
        ] {
            match own {
                Some(blocks) => {
                    w.open(tag, &[("w:type", kind), ("w:id", id)]);
                    if blocks.is_empty() {
                        w.empty("w:p", &[]);
                    } else {
                        wr.blocks(&mut w, blocks, &mut prels, false, 0);
                    }
                    w.close(tag);
                }
                None if kind != "continuationNotice" => {
                    w.open(tag, &[("w:type", kind), ("w:id", id)]);
                    w.open("w:p", &[]);
                    w.open("w:pPr", &[]);
                    w.empty("w:spacing", &[("w:after", "0"), ("w:line", "240"), ("w:lineRule", "auto")]);
                    w.close("w:pPr");
                    w.open("w:r", &[]);
                    w.empty(if kind == "separator" { "w:separator" } else { "w:continuationSeparator" }, &[]);
                    w.close("w:r");
                    w.close("w:p");
                    w.close(tag);
                }
                None => {}
            }
        }
        let base = note_id_base(doc, foot);
        let mut k = 0;
        while let Some(pid) = (if foot { &wr.footnotes } else { &wr.endnotes }).get(k).copied() {
            let nid = (k + base).to_string();
            k += 1;
            w.open(tag, &[("w:id", &nid)]);
            let blocks: Blocks = doc.parts.get(&pid).map(|p| p.blocks.clone()).unwrap_or_default();
            wr.note_blocks(&mut w, &blocks, &mut prels, foot, pid);
            w.close(tag);
        }
        w.close(root);
        let kind = if foot { rt::FOOTNOTES } else { rt::ENDNOTES };
        rels.add(kind, file, false);
        push_part(
            entries,
            overrides,
            &format!("word/{file}"),
            w.into_bytes(),
            &format!("{CT_WML}{}+xml", if foot { "footnotes" } else { "endnotes" }),
            prels,
        );
    }
}

fn write_comments(
    doc: &Document,
    comment_paras: &[(u32, String)],
    ns_refs: &[(&str, &str)],
    wr: &mut Writer<'_>,
    entries: &mut Vec<(String, Vec<u8>)>,
    overrides: &mut Vec<(String, String)>,
    rels: &mut PartRels,
) {
    if !doc.comments.is_empty() {
        let mut prels = PartRels::default();
        let mut w = W::new();
        w.open("w:comments", ns_refs);
        for (cid, c) in &doc.comments {
            let id = cid.to_string();
            let author = annotation_author(doc, &c.author);
            let mut a = vec![("w:id", id.as_str()), ("w:author", author)];
            if !c.date.is_empty() && !doc.settings.remove_date_time {
                a.push(("w:date", c.date.as_str()));
            }
            if doc.settings.remove_personal_info {
                a.push(("w:initials", ANONYMOUS_INITIALS));
            } else if !c.initials.is_empty() {
                a.push(("w:initials", c.initials.as_str()));
            }
            w.open("w:comment", &a);
            let blocks: Blocks = doc.parts.get(&c.part).map(|p| p.blocks.clone()).unwrap_or_default();
            wr.blocks(&mut w, &blocks, &mut prels, false, 0);
            w.close("w:comment");
        }
        w.close("w:comments");
        rels.add(rt::COMMENTS, "comments.xml", false);
        push_part(entries, overrides, "word/comments.xml", w.into_bytes(), &format!("{CT_WML}comments+xml"), prels);

        let mut w = W::new();
        w.open(
            "w15:commentsEx",
            &[
                ("xmlns:w15", "http://schemas.microsoft.com/office/word/2012/wordml"),
                ("xmlns:mc", "http://schemas.openxmlformats.org/markup-compatibility/2006"),
            ],
        );
        // Assigned in BTreeMap comment-ID order above, including gaps for missing parts.
        let pid_of =
            |cid: u32| comment_paras.binary_search_by_key(&cid, |(c, _)| *c).ok().and_then(|i| comment_paras.get(i)).map(|(_, p)| p.as_str());
        for (cid, c) in &doc.comments {
            let Some(pid) = pid_of(*cid) else { continue };
            let done = if c.resolved { "1" } else { "0" };
            match c.parent.and_then(pid_of) {
                Some(parent) => w.empty("w15:commentEx", &[("w15:paraId", pid), ("w15:paraIdParent", parent), ("w15:done", done)]),
                None => w.empty("w15:commentEx", &[("w15:paraId", pid), ("w15:done", done)]),
            }
        }
        w.close("w15:commentsEx");
        rels.add(rt::COMMENTS_EX, "commentsExtended.xml", false);
        push_part(entries, overrides, "word/commentsExtended.xml", w.into_bytes(), &format!("{CT_WML}commentsExtended+xml"), PartRels::default());
        write_comment_ids(doc, comment_paras, entries, overrides, rels);
    }
}

pub(super) fn push_part(
    entries: &mut Vec<(String, Vec<u8>)>,
    overrides: &mut Vec<(String, String)>,
    path: &str,
    bytes: Vec<u8>,
    ct: &str,
    rels: PartRels,
) {
    if !rels.is_empty() {
        let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
        entries.push((format!("{dir}/_rels/{file}.rels"), rels.xml()));
    }
    entries.push((path.to_string(), bytes));
    overrides.push((format!("/{path}"), ct.to_string()));
}

/// Content type by file extension (or sniffed name).
pub(super) fn content_type_for(file: &str) -> &'static str {
    match file.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "tif" | "tiff" => "image/tiff",
        "svg" => "image/svg+xml",
        "emf" => "image/x-emf",
        "wmf" => "image/x-wmf",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// Guess an image extension from magic bytes.
pub(super) fn sniff(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else if bytes.starts_with(b"BM") {
        "bmp"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        "tiff"
    } else if bytes.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
        "wmf"
    } else if bytes.get(40..44) == Some(b" EMF") {
        "emf"
    } else if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
        "svg"
    } else {
        "bin"
    }
}

/// HTML divs: a web settings part kept from the file is the one they were read from, so it goes
/// back as it was; otherwise one is written for them.
fn push_web_settings(doc: &Document, rels: &mut PartRels, entries: &mut Vec<(String, Vec<u8>)>, overrides: &mut Vec<(String, String)>) {
    if doc.settings.web_divs.is_empty() || rels.has_kind(rt::WEB_SETTINGS) {
        return;
    }
    let taken = |name: &str| entries.iter().any(|(e, _)| e.eq_ignore_ascii_case(name));
    if let Some(file) = ["webSettings.xml", "webSettings1.xml", "webSettings2.xml"].into_iter().find(|f| !taken(&format!("word/{f}"))) {
        rels.add(rt::WEB_SETTINGS, file, false);
        let bytes = web_settings_xml(&doc.settings.web_divs);
        push_part(entries, overrides, &format!("word/{file}"), bytes, &format!("{CT_WML}webSettings+xml"), PartRels::default());
    }
}

/// A web settings part (`w:webSettings`) holding `divs` (`w:divs`, ECMA-376 §17.15.2.8).
fn web_settings_xml(divs: &[wordcraft_doc::WebDiv]) -> Vec<u8> {
    fn div(w: &mut W, d: &wordcraft_doc::WebDiv, depth: usize) {
        w.open("w:div", &[("w:id", &d.id)]);
        if d.body_div {
            w.val("w:bodyDiv", "1");
        }
        for (name, v) in [("w:marLeft", d.mar_left), ("w:marRight", d.mar_right), ("w:marTop", d.mar_top), ("w:marBottom", d.mar_bottom)] {
            w.val(name, &crate::units::twips(if v.is_finite() { v } else { 0.0 }));
        }
        if let Some(b) = &d.border {
            super::props::borders(w, "w:divBdr", b, None, &[]);
        }
        if !d.children.is_empty() && depth < wordcraft_doc::MAX_DIV_DEPTH {
            w.open("w:divsChild", &[]);
            for c in &d.children {
                div(w, c, depth + 1);
            }
            w.close("w:divsChild");
        }
        w.close("w:div");
    }
    let mut w = W::new();
    w.open("w:webSettings", &[("xmlns:w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main")]);
    w.open("w:divs", &[]);
    for d in divs {
        div(&mut w, d, 0);
    }
    w.close("w:divs");
    w.close("w:webSettings");
    w.into_bytes()
}

#[cfg(test)]
mod beta_w15_tests {
    use super::*;

    #[test]
    fn preview_declarations_change_without_touching_other_bytes() {
        let original = format!(
            r#"<?xml version="1.0"?><w15:people xmlns:mc="urn:test" xmlns:w15 = '{B}' mc:note="{B}"><w15:person w15:author="Ann" w15:contact="ann@example.com" other = 'x' w15:contact="duplicate"/><w15:person other="y"/></w15:people>"#,
            B = String::from_utf8_lossy(W15_BETA)
        );
        let final_ns = String::from_utf8_lossy(W15_FINAL);
        let expected = format!(
            r#"<?xml version="1.0"?><w15:people xmlns:mc="urn:test" xmlns:w15 = '{final_ns}' mc:note="{B}"><w15:person w15:author="Ann" other = 'x'/><w15:person other="y"/></w15:people>"#,
            B = String::from_utf8_lossy(W15_BETA)
        );
        let declaration = format!(r#"<x xmlns:w15="{B}"/>"#, B = String::from_utf8_lossy(W15_BETA));
        let split = declaration.find(std::str::from_utf8(W15_BETA).unwrap()).unwrap();
        assert!(beta_namespace_declaration(&declaration.as_bytes()[..split], declaration.as_bytes()[split..].get(W15_BETA.len()..).unwrap()));
        let result = modernize_beta_w15("word/people.xml", original.as_bytes()).unwrap();
        assert_eq!(result, expected.into_bytes());
    }
}
