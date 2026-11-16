//! Fonts written into a saved package: File › Options › Save › Embed fonts in the file (#6329).
//!
//! The caller (the engine) finds the installed font files the document uses and hands them in as
//! [`FontFile`]s; this module checks that each may be embedded ([`embeddable`]), obfuscates it as
//! ECMA-376 Part 1 §17.8.1 describes and writes it as a `word/fonts/fontN.odttf` part related
//! from the font table, which names it with a `w:embedRegular` / `w:embedBold` / `w:embedItalic`
//! / `w:embedBoldItalic` element (§17.8.3.3–§17.8.3.6). Whole files only: no subsetting.

use std::collections::HashSet;

use super::PartRels;
use super::embed::EmbedWriter;
use crate::package::relative;
use crate::xml::{self, El, Node};

/// Relationship type of an embedded font part (§17.8.3.3).
pub(crate) const REL_FONT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/font";
/// Content type of an obfuscated font part (§17.8.1).
pub(crate) const CT_OBFUSCATED_FONT: &str = "application/vnd.openxmlformats-officedocument.obfuscatedFont";

/// Most bytes of one font file embedded: a larger file is left out.
pub const MAX_EMBED_FILE_BYTES: usize = 32 * 1024 * 1024;
/// Most font faces embedded by one save.
pub const MAX_EMBED_FACES: usize = 64;
/// Most bytes of font files embedded by one save.
pub const MAX_EMBED_BYTES: usize = 64 * 1024 * 1024;

/// The fonts "Do not embed common system fonts" leaves out. Word doesn't publish its list; this
/// is the core set of fonts Windows installs, which is the most likely meaning (unverified).
/// Compared without regard to case.
pub const COMMON_SYSTEM_FONTS: &[&str] = &[
    "Arial",
    "Arial Black",
    "Calibri",
    "Calibri Light",
    "Cambria",
    "Cambria Math",
    "Candara",
    "Comic Sans MS",
    "Consolas",
    "Constantia",
    "Corbel",
    "Courier New",
    "Ebrima",
    "Franklin Gothic Medium",
    "Gabriola",
    "Gadugi",
    "Georgia",
    "Impact",
    "Lucida Console",
    "Lucida Sans Unicode",
    "Malgun Gothic",
    "Marlett",
    "Microsoft Himalaya",
    "Microsoft JhengHei",
    "Microsoft New Tai Lue",
    "Microsoft PhagsPa",
    "Microsoft Sans Serif",
    "Microsoft Tai Le",
    "Microsoft YaHei",
    "Microsoft Yi Baiti",
    "MingLiU-ExtB",
    "Mongolian Baiti",
    "MS Gothic",
    "MV Boli",
    "Myanmar Text",
    "Nirmala UI",
    "Palatino Linotype",
    "Segoe Print",
    "Segoe Script",
    "Segoe UI",
    "Segoe UI Emoji",
    "Segoe UI Historic",
    "Segoe UI Symbol",
    "SimSun",
    "Sitka",
    "Sylfaen",
    "Symbol",
    "Tahoma",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
    "Webdings",
    "Wingdings",
    "Yu Gothic",
    "Aptos",
    "Aptos Display",
];

/// Is `family` one of the [`COMMON_SYSTEM_FONTS`]?
pub fn is_common_system_font(family: &str) -> bool {
    let family = family.trim();
    COMMON_SYSTEM_FONTS.iter().any(|f| f.eq_ignore_ascii_case(family))
}

/// Which face of a family a font file is: the font table element it is embedded under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FaceStyle {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl FaceStyle {
    /// Every style, in the font table's order.
    pub const ALL: [FaceStyle; 4] = [FaceStyle::Regular, FaceStyle::Bold, FaceStyle::Italic, FaceStyle::BoldItalic];

    /// The style's `w:embed*` element.
    pub fn element(self) -> &'static str {
        match self {
            FaceStyle::Regular => "w:embedRegular",
            FaceStyle::Bold => "w:embedBold",
            FaceStyle::Italic => "w:embedItalic",
            FaceStyle::BoldItalic => "w:embedBoldItalic",
        }
    }

    /// Is it a bold face?
    pub fn bold(self) -> bool {
        matches!(self, FaceStyle::Bold | FaceStyle::BoldItalic)
    }

    /// Is it an italic face?
    pub fn italic(self) -> bool {
        matches!(self, FaceStyle::Italic | FaceStyle::BoldItalic)
    }
}

/// An installed font file to embed: the family the document names, which face it is, and the
/// file as installed.
#[derive(Clone, Debug, PartialEq)]
pub struct FontFile {
    pub family: String,
    pub style: FaceStyle,
    pub data: Vec<u8>,
}

/// Why a font file can't be embedded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotEmbeddable {
    /// Over [`MAX_EMBED_FILE_BYTES`].
    TooLarge,
    /// Not a single TrueType-outline font (`glyf` table): a CFF font, a collection, or not a font.
    NotTrueType,
    /// OS/2 `fsType` bit 1: Restricted License embedding.
    Restricted,
    /// OS/2 `fsType` bit 9: Bitmap embedding only.
    BitmapOnly,
}

/// The table `tag` of the single font file `data`, if it has one within the file.
fn sfnt_table<'a>(data: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let be16 = |at: usize| data.get(at..at.checked_add(2)?).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let be32 = |at: usize| data.get(at..at.checked_add(4)?).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let tables = usize::from(be16(4)?);
    for i in 0..tables {
        let rec = 12usize.checked_add(i.checked_mul(16)?)?;
        if data.get(rec..rec.checked_add(4)?)? != tag {
            continue;
        }
        let offset = usize::try_from(be32(rec.checked_add(8)?)?).ok()?;
        let len = usize::try_from(be32(rec.checked_add(12)?)?).ok()?;
        return data.get(offset..offset.checked_add(len)?);
    }
    None
}

/// Whether the font file `data` may be embedded: a TrueType-outline font of at most
/// [`MAX_EMBED_FILE_BYTES`] whose OS/2 `fsType` allows embedding (neither Restricted License,
/// bit 1, nor Bitmap embedding only, bit 9). A font without an OS/2 table has no restriction.
pub fn embeddable(data: &[u8]) -> Result<(), NotEmbeddable> {
    if data.len() > MAX_EMBED_FILE_BYTES {
        return Err(NotEmbeddable::TooLarge);
    }
    // 0x00010000 or 'true': TrueType outlines; 'OTTO' (CFF) and 'ttcf' (collection) are not.
    let version = data.get(..4).ok_or(NotEmbeddable::NotTrueType)?;
    if version != [0, 1, 0, 0] && version != b"true" {
        return Err(NotEmbeddable::NotTrueType);
    }
    if sfnt_table(data, b"glyf").is_none() {
        return Err(NotEmbeddable::NotTrueType);
    }
    if let Some(fs_type) = sfnt_table(data, b"OS/2").and_then(|t| t.get(8..10)).map(|b| u16::from_be_bytes([b[0], b[1]])) {
        if fs_type & 0x0002 != 0 {
            return Err(NotEmbeddable::Restricted);
        }
        if fs_type & 0x0200 != 0 {
            return Err(NotEmbeddable::BitmapOnly);
        }
    }
    Ok(())
}

/// A fresh random GUID in the `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` form of `w:fontKey`.
fn fresh_guid() -> String {
    let mut b = [0u8; 16];
    if getrandom::fill(&mut b).is_err() {
        // No system randomness: the key only obfuscates, it protects nothing.
        use std::hash::{BuildHasher, Hasher};
        static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        for (i, chunk) in b.chunks_mut(8).enumerate() {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(n);
            h.write_usize(i);
            for (d, s) in chunk.iter_mut().zip(h.finish().to_le_bytes()) {
                *d = s;
            }
        }
    }
    let hex: String = b.iter().map(|x| format!("{x:02X}")).collect();
    let part = |r: std::ops::Range<usize>| hex.get(r).unwrap_or("");
    format!("{{{}-{}-{}-{}-{}}}", part(0..8), part(8..12), part(12..16), part(16..20), part(20..32))
}

/// `data` obfuscated with the key of `guid` (§17.8.1): its first 32 bytes XORed with the key, the
/// inverse of what [`crate::embedded_fonts`] does on reading.
fn obfuscate(mut data: Vec<u8>, guid: &str) -> Option<Vec<u8>> {
    let key = crate::fonts::font_key(guid)?;
    for (i, b) in data.iter_mut().take(32).enumerate() {
        *b ^= key.get(i % 16).copied().unwrap_or(0);
    }
    Some(data)
}

/// The `w:embed*` elements to add to one family's `w:font`.
pub(crate) type FamilyEmbeds = (String, Vec<El>);

impl EmbedWriter<'_> {
    /// Write `fonts` as font parts related from the font table `table` (through `rels`), leaving
    /// out families in `skip` (lower case: those the table already embeds), within the caps.
    /// Returns the `w:embed*` elements per family, in the order first given.
    pub(crate) fn embed_fonts(&mut self, table: &str, fonts: &[FontFile], skip: &HashSet<String>, rels: &mut PartRels) -> Vec<FamilyEmbeds> {
        let mut out: Vec<FamilyEmbeds> = Vec::new();
        let mut done: HashSet<(String, FaceStyle)> = HashSet::new();
        let (mut faces, mut bytes, mut capped) = (0usize, 0usize, 0usize);
        for f in fonts {
            let family = f.family.trim();
            let key = family.to_lowercase();
            if family.is_empty() || skip.contains(&key) || done.contains(&(key.clone(), f.style)) || embeddable(&f.data).is_err() {
                continue;
            }
            let total = bytes.saturating_add(f.data.len());
            if faces >= MAX_EMBED_FACES || total > MAX_EMBED_BYTES {
                capped += 1;
                continue;
            }
            let guid = fresh_guid();
            let Some(data) = obfuscate(f.data.clone(), &guid) else { continue };
            done.insert((key.clone(), f.style));
            faces += 1;
            bytes = total;
            let name = self.fresh("word/fonts/font", ".odttf");
            let id = rels.add_unique(REL_FONT, &relative(table, &name));
            self.parts.push((name, data, CT_OBFUSCATED_FONT.to_string(), PartRels::default()));
            let el = El {
                name: f.style.element().to_string(),
                attrs: vec![("r:id".into(), id), ("w:fontKey".into(), guid), ("w:subsetted".into(), "0".into())],
                kids: Vec::new(),
            };
            match out.iter_mut().find(|(n, _)| n.to_lowercase() == key) {
                Some((_, els)) => els.push(el),
                None => out.push((family.to_string(), vec![el])),
            }
        }
        if capped > 0 {
            log::warn!("docx: past {MAX_EMBED_FACES} font faces or {MAX_EMBED_BYTES} bytes of fonts; {capped} more are not embedded");
        }
        // The font table lists a family's faces Regular, Bold, Italic, Bold Italic (§17.8.3.10).
        for (_, els) in &mut out {
            els.sort_by_key(|e| FaceStyle::ALL.iter().position(|s| s.element() == e.name));
        }
        out
    }
}

/// The families a font table (`w:fonts`) already embeds a face of, in lower case.
pub(crate) fn embedded_families(table: &El) -> HashSet<String> {
    table
        .children("w:font")
        .filter(|f| f.els().any(|e| FaceStyle::ALL.iter().any(|s| s.element() == e.name)))
        .filter_map(|f| f.attr("w:name"))
        .map(|n| n.trim().to_lowercase())
        .collect()
}

/// The kept font table `table` with `embeds` added: to the family's `w:font` when it has one,
/// else in a new `w:font` at the end. Written with every namespace the writer knows declared.
pub(crate) fn add_embeds(mut table: El, embeds: Vec<FamilyEmbeds>) -> Vec<u8> {
    for (family, els) in embeds {
        let at = table.kids.iter_mut().find_map(|k| match k {
            Node::El(e) if e.name == "w:font" && e.attr("w:name").is_some_and(|n| n.trim().eq_ignore_ascii_case(&family)) => Some(e),
            _ => None,
        });
        match at {
            Some(font) => font.kids.extend(els.into_iter().map(Node::El)),
            None => table.kids.push(Node::El(El {
                name: "w:font".into(),
                attrs: vec![("w:name".into(), family)],
                kids: els.into_iter().map(Node::El).collect(),
            })),
        }
    }
    let mut x = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    x.push_str("<w:fonts");
    for (k, v) in xml::body_ns() {
        x.push_str(&format!(" {k}=\"{}\"", xml::esc(&v)));
    }
    x.push('>');
    for k in &table.kids {
        match k {
            Node::El(e) => x.push_str(&e.to_xml()),
            Node::Text(t) => x.push_str(&xml::esc(t)),
        }
    }
    x.push_str("</w:fonts>");
    x.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled JetBrains Mono (TrueType, fsType 0).
    fn font() -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts/JetBrainsMono-Regular.ttf")).unwrap()
    }

    /// `data` with its OS/2 fsType set to `fs_type`.
    fn with_fs_type(mut data: Vec<u8>, fs_type: u16) -> Vec<u8> {
        let os2 = sfnt_table(&data, b"OS/2").unwrap().as_ptr() as usize - data.as_ptr() as usize;
        data[os2 + 8..os2 + 10].copy_from_slice(&fs_type.to_be_bytes());
        data
    }

    #[test]
    fn installable_truetype_is_embeddable_and_restricted_or_bitmap_only_is_not() {
        let f = font();
        assert_eq!(embeddable(&f), Ok(()));
        assert_eq!(embeddable(&with_fs_type(f.clone(), 0x0008)), Ok(()), "editable embedding is fine");
        assert_eq!(embeddable(&with_fs_type(f.clone(), 0x0002)), Err(NotEmbeddable::Restricted));
        assert_eq!(embeddable(&with_fs_type(f.clone(), 0x0200)), Err(NotEmbeddable::BitmapOnly));
        assert_eq!(embeddable(&with_fs_type(f, 0x0204)), Err(NotEmbeddable::BitmapOnly));
    }

    #[test]
    fn cff_collections_and_junk_are_not_truetype() {
        let mut otto = font();
        otto[..4].copy_from_slice(b"OTTO");
        assert_eq!(embeddable(&otto), Err(NotEmbeddable::NotTrueType));
        let mut ttc = font();
        ttc[..4].copy_from_slice(b"ttcf");
        assert_eq!(embeddable(&ttc), Err(NotEmbeddable::NotTrueType));
        for junk in [&[][..], &[0, 1, 0, 0][..], &[0, 1, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 1][..]] {
            assert_eq!(embeddable(junk), Err(NotEmbeddable::NotTrueType));
        }
        assert_eq!(embeddable(&vec![0u8; MAX_EMBED_FILE_BYTES + 1]), Err(NotEmbeddable::TooLarge));
    }

    #[test]
    fn system_fonts_are_listed_without_regard_to_case() {
        assert!(is_common_system_font("Times New Roman"));
        assert!(is_common_system_font(" calibri "));
        assert!(is_common_system_font("APTOS DISPLAY"));
        assert!(!is_common_system_font("JetBrains Mono"));
    }

    #[test]
    fn a_guid_is_fresh_and_its_obfuscation_reads_back() {
        let (a, b) = (fresh_guid(), fresh_guid());
        assert_ne!(a, b);
        assert_eq!(a.len(), 38);
        assert!(a.starts_with('{') && a.ends_with('}'));
        assert!(a.chars().all(|c| matches!(c, '{' | '}' | '-' | '0'..='9' | 'A'..='F')), "{a}");
        let f = font();
        let o = obfuscate(f.clone(), &a).unwrap();
        assert_ne!(o[..32], f[..32]);
        assert_eq!(obfuscate(o, &a).unwrap(), f, "XOR twice gives the file back");
    }

    #[test]
    fn at_most_64_faces_are_embedded() {
        let doc = wordcraft_doc::Document::default();
        let mut w = EmbedWriter::new(&doc);
        let f = font();
        let fonts: Vec<FontFile> = (0..70).map(|i| FontFile { family: format!("Family {i}"), style: FaceStyle::Regular, data: f.clone() }).collect();
        let mut rels = PartRels::default();
        let embeds = w.embed_fonts("word/fontTable.xml", &fonts, &HashSet::new(), &mut rels);
        assert_eq!(embeds.len(), MAX_EMBED_FACES);
        assert_eq!(w.parts.len(), MAX_EMBED_FACES);
        let names: HashSet<&str> = w.parts.iter().map(|p| p.0.as_str()).collect();
        assert_eq!(names.len(), MAX_EMBED_FACES, "every part has its own name");
        assert!(names.contains("word/fonts/font1.odttf") && names.contains("word/fonts/font64.odttf"));
    }

    #[test]
    fn a_family_the_table_embeds_or_a_restricted_face_is_left_out() {
        let doc = wordcraft_doc::Document::default();
        let mut w = EmbedWriter::new(&doc);
        let f = font();
        let fonts = vec![
            FontFile { family: "Kept".into(), style: FaceStyle::Regular, data: f.clone() },
            FontFile { family: "Locked".into(), style: FaceStyle::Regular, data: with_fs_type(f.clone(), 0x0002) },
            FontFile { family: "New".into(), style: FaceStyle::Bold, data: f.clone() },
            FontFile { family: "New".into(), style: FaceStyle::Regular, data: f.clone() },
            FontFile { family: "new".into(), style: FaceStyle::Regular, data: f },
        ];
        let skip: HashSet<String> = ["kept".to_string()].into();
        let embeds = w.embed_fonts("word/fontTable.xml", &fonts, &skip, &mut PartRels::default());
        assert_eq!(embeds.len(), 1);
        let (family, els) = &embeds[0];
        assert_eq!(family, "New");
        let order: Vec<&str> = els.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(order, ["w:embedRegular", "w:embedBold"], "one of each face, in schema order");
    }

    /// A package with `settings` (inside `w:settings`) and a font table holding `fonts` (inside
    /// `w:fonts`).
    fn package(settings: &str, fonts: &str) -> Vec<u8> {
        use std::io::Write;
        const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
        let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
        let parts = [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/></Types>"#.to_string(),
            ),
            ("_rels/.rels", format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{rel}/officeDocument" Target="word/document.xml"/></Relationships>"#)),
            ("word/document.xml", format!(r#"<w:document {NS}><w:body><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:body></w:document>"#)),
            (
                "word/_rels/document.xml.rels",
                format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{rel}/settings" Target="settings.xml"/><Relationship Id="rId2" Type="{rel}/fontTable" Target="fontTable.xml"/></Relationships>"#),
            ),
            ("word/settings.xml", format!("<w:settings {NS}>{settings}</w:settings>")),
            ("word/fontTable.xml", format!("<w:fonts {NS}>{fonts}</w:fonts>")),
        ];
        let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, xml) in parts {
            zw.start_file(name, opts).unwrap();
            zw.write_all(xml.as_bytes()).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    fn part(bytes: &[u8], name: &str) -> String {
        use std::io::Read;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut out = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn val_0_reads_as_off_and_a_bare_element_as_on() {
        let doc = crate::read(&package(r#"<w:embedTrueTypeFonts w:val="0"/><w:embedSystemFonts w:val="false"/><w:saveSubsetFonts/>"#, "")).unwrap();
        assert!(!doc.settings.embed_true_type_fonts);
        assert!(!doc.settings.embed_system_fonts);
        assert!(doc.settings.save_subset_fonts);
        let doc = crate::read(&package(r#"<w:embedTrueTypeFonts w:val="on"/><w:embedSystemFonts/>"#, "")).unwrap();
        assert!(doc.settings.embed_true_type_fonts && doc.settings.embed_system_fonts && !doc.settings.save_subset_fonts);
    }

    #[test]
    fn merged_settings_keep_schema_order_and_the_model_wins() {
        // `w:proofState` isn't modelled: the original part is merged into.
        let orig = r#"<w:zoom w:percent="100"/><w:embedTrueTypeFonts w:val="0"/><w:mirrorMargins/><w:proofState w:spelling="clean"/>"#;
        let mut doc = crate::read(&package(orig, "")).unwrap();
        doc.settings.embed_true_type_fonts = true;
        doc.settings.save_subset_fonts = true;
        doc.settings.embed_system_fonts = true;
        let xml = part(&crate::write(&doc).unwrap(), "word/settings.xml");
        let at = |el: &str| xml.find(el).unwrap_or_else(|| panic!("{el} in {xml}"));
        assert!(!xml.contains(r#"w:embedTrueTypeFonts w:val="0""#), "{xml}");
        assert!(at("<w:zoom") < at("<w:embedTrueTypeFonts"));
        assert!(at("<w:embedTrueTypeFonts") < at("<w:embedSystemFonts"));
        assert!(at("<w:embedSystemFonts") < at("<w:saveSubsetFonts"));
        assert!(at("<w:saveSubsetFonts") < at("<w:mirrorMargins"));
        assert!(at("<w:mirrorMargins") < at("<w:proofState"));
    }

    #[test]
    fn a_kept_font_table_keeps_its_entries_and_gains_the_new_embeds() {
        let table = r#"<w:font w:name="Kept Face"><w:family w:val="swiss"/></w:font><w:font w:name="Other"><w:family w:val="roman"/></w:font>"#;
        let mut doc = crate::read(&package("<w:proofState/>", table)).unwrap();
        doc.settings.embed_true_type_fonts = true;
        let f = font();
        let fonts = [
            FontFile { family: "kept face".into(), style: FaceStyle::Regular, data: f.clone() },
            FontFile { family: "Brand New".into(), style: FaceStyle::Regular, data: f.clone() },
        ];
        let bytes = crate::write_as_with_fonts(&doc, crate::Flavor::Document, &fonts).unwrap();
        let xml = part(&bytes, "word/fontTable.xml");
        assert_eq!(xml.matches("<w:font ").count(), 3, "{xml}");
        assert!(xml.contains(r#"<w:font w:name="Other"><w:family w:val="roman"/></w:font>"#), "{xml}");
        assert!(xml.contains(r#"<w:font w:name="Kept Face"><w:family w:val="swiss"/><w:embedRegular"#), "{xml}");
        assert!(xml.contains(r#"<w:font w:name="Brand New"><w:embedRegular"#), "{xml}");
        let rels = part(&bytes, "word/_rels/fontTable.xml.rels");
        assert_eq!(rels.matches(REL_FONT).count(), 2, "{rels}");
        let back = crate::read(&bytes).unwrap();
        let embedded = crate::embedded_fonts(&back);
        assert_eq!(embedded.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Kept Face", "Brand New"]);
        assert!(embedded.iter().all(|e| e.data == f));
        // Saved again with the same fonts: the table already embeds both, nothing is added.
        let again = crate::write_as_with_fonts(&back, crate::Flavor::Document, &fonts).unwrap();
        let back = crate::read(&again).unwrap();
        assert_eq!(crate::embedded_fonts(&back).len(), 2);
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&again[..])).unwrap();
        let odttf = (0..zip.len()).filter(|&i| zip.by_index(i).unwrap().name().ends_with(".odttf")).count();
        assert_eq!(odttf, 2);
    }
}
