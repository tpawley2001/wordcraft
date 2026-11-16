//! The installed font files a .docx save embeds when File › Options › Save › Embed fonts in the
//! file is on (#6329). Found through the font database layout draws with, then handed to the
//! docx writer as data ([`wordcraft_docx::FontFile`]), so the writer looks nothing up itself.

use std::collections::HashSet;

use wordcraft_doc::{CharProps, Document, StoryRef};
use wordcraft_docx::{FaceStyle, FontFile};

/// Most font families considered for one save (a hostile file can name thousands).
const MAX_FAMILIES: usize = 1024;
/// Longest family name considered, in characters.
const MAX_NAME_CHARS: usize = 256;

/// The font files a .docx save of `doc` embeds: none unless its Embed fonts in the file setting
/// is on. Only installed fonts (not one shown through a substitute), only TrueType files that
/// allow embedding, and without the common system fonts unless the document asks for them.
pub fn fonts_to_embed(doc: &Document) -> Vec<FontFile> {
    let s = &doc.settings;
    if !s.embed_true_type_fonts {
        return Vec::new();
    }
    let mut out = Vec::new();
    for family in families(doc) {
        out.extend(select(&family, installed_faces(&family), !s.embed_system_fonts));
    }
    out
}

/// The font families `doc` names: in its runs (every story), its styles and document defaults,
/// and its theme's major and minor fonts. Trimmed, in the order found, each once (ignoring case).
pub fn families(doc: &Document) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    let mut add = |name: &str| {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || out.len() >= MAX_FAMILIES {
            return;
        }
        if seen.insert(name.to_lowercase()) {
            out.push(name.to_string());
        }
    };
    let add_props = |c: &CharProps, add: &mut dyn FnMut(&str)| {
        for f in [&c.font, &c.font_cs, &c.font_ea].into_iter().flatten() {
            add(f);
        }
    };
    add(&doc.settings.minor_font);
    add(&doc.settings.major_font);
    add_props(&doc.styles.default_chr, &mut add);
    for st in &doc.styles.styles {
        add_props(&st.chr, &mut add);
    }
    let stories = std::iter::once(StoryRef::Body).chain(doc.parts.keys().map(|id| StoryRef::Part(*id)));
    for story in stories {
        for path in doc.para_paths(story) {
            let Some(p) = doc.para(story, &path) else { continue };
            for r in &p.runs {
                add_props(&r.props, &mut add);
            }
        }
    }
    out
}

/// The installed faces of `family`, as font files: its Regular face, and its Bold, Italic and
/// Bold Italic faces where they are files of their own. Empty when the family isn't installed
/// (text in it is drawn in a substitute).
fn installed_faces(family: &str) -> Vec<(FaceStyle, Vec<u8>)> {
    let mut out: Vec<(FaceStyle, Vec<u8>)> = Vec::new();
    let mut files: Vec<*const u8> = Vec::new();
    for style in FaceStyle::ALL {
        let r = wordcraft_fonts::word::resolve(family, style.bold(), style.italic());
        if r.substituted || r.synth_bold || r.synth_italic {
            continue;
        }
        let face = r.face.get();
        let data = face.data();
        // A face of another family (a fallback), of a collection, or a file already taken (a
        // variable font's instances share one) isn't a face of its own.
        if !face.named(family) || face.index() != 0 || files.contains(&data.as_ptr()) {
            continue;
        }
        if style == FaceStyle::Regular && (face.weight >= 600.0 || face.italic) {
            continue;
        }
        if data.len() > wordcraft_docx::MAX_EMBED_FILE_BYTES {
            continue;
        }
        files.push(data.as_ptr());
        out.push((style, data.to_vec()));
    }
    // Without its Regular face a family isn't embedded (Word embeds the faces of a font it has).
    if !out.iter().any(|(s, _)| *s == FaceStyle::Regular) {
        out.clear();
    }
    out
}

/// The faces of `family` that may be embedded: none when `skip_system_fonts` and it is a common
/// system font; else those [`wordcraft_docx::embeddable`] accepts.
pub(crate) fn select(family: &str, faces: Vec<(FaceStyle, Vec<u8>)>, skip_system_fonts: bool) -> Vec<FontFile> {
    if skip_system_fonts && wordcraft_docx::is_common_system_font(family) {
        return Vec::new();
    }
    faces
        .into_iter()
        .filter(|(_, data)| wordcraft_docx::embeddable(data).is_ok())
        .map(|(style, data)| FontFile { family: family.to_string(), style, data })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jetbrains() -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts/JetBrainsMono-Regular.ttf")).unwrap()
    }

    /// `data` with its OS/2 `fsType` set to `fs_type`.
    fn with_fs_type(mut data: Vec<u8>, fs_type: u16) -> Vec<u8> {
        let tables = u16::from_be_bytes([data[4], data[5]]) as usize;
        let rec = (0..tables).map(|i| 12 + 16 * i).find(|&r| &data[r..r + 4] == b"OS/2").unwrap();
        let at = u32::from_be_bytes(data[rec + 8..rec + 12].try_into().unwrap()) as usize;
        data[at + 8..at + 10].copy_from_slice(&fs_type.to_be_bytes());
        data
    }

    #[test]
    fn the_installed_bundled_font_is_found_with_its_regular_face_only() {
        let faces = installed_faces("JetBrains Mono");
        assert_eq!(faces.len(), 1, "{:?}", faces.iter().map(|f| f.0).collect::<Vec<_>>());
        assert_eq!(faces[0].0, FaceStyle::Regular);
        assert!(faces[0].1 == jetbrains());
        assert!(installed_faces("Wordcraft Nowhere Face").is_empty(), "a missing font is not its substitute");
    }

    #[test]
    fn common_system_fonts_are_skipped_unless_asked_for() {
        let faces = || vec![(FaceStyle::Regular, jetbrains())];
        assert!(select("Times New Roman", faces(), true).is_empty());
        assert!(select("segoe ui", faces(), true).is_empty());
        assert_eq!(select("Times New Roman", faces(), false).len(), 1);
        assert_eq!(select("JetBrains Mono", faces(), true).len(), 1);
    }

    #[test]
    fn restricted_and_bitmap_only_fonts_are_not_embedded() {
        for (fs_type, ok) in [(0x0000, true), (0x0004, true), (0x0008, true), (0x0002, false), (0x0200, false), (0x0208, false)] {
            let faces = vec![(FaceStyle::Regular, with_fs_type(jetbrains(), fs_type))];
            assert_eq!(select("JetBrains Mono", faces, true).len(), usize::from(ok), "fsType {fs_type:#06x}");
        }
    }

    #[test]
    fn families_come_from_runs_styles_defaults_and_the_theme_once_each() {
        let mut s = crate::Session::new(Document::from_text("Some text"));
        s.run("select.all", &serde_json::json!({})).unwrap();
        s.run("format.font", &serde_json::json!({"name": "Run Face"})).unwrap();
        s.doc.settings.minor_font = "Minor Face".into();
        s.doc.settings.major_font = "Major Face".into();
        std::sync::Arc::make_mut(&mut s.doc.styles).default_chr.font = Some(" minor face ".into());
        let fams = families(&s.doc);
        assert_eq!(fams.iter().filter(|f| f.eq_ignore_ascii_case("minor face")).count(), 1, "{fams:?}");
        for f in ["Major Face", "Run Face"] {
            assert!(fams.iter().any(|x| x == f), "{f}: {fams:?}");
        }
    }

    #[test]
    fn nothing_is_gathered_while_the_setting_is_off() {
        let mut doc = Document::from_text("x");
        std::sync::Arc::make_mut(&mut doc.styles).default_chr.font = Some("JetBrains Mono".into());
        assert!(fonts_to_embed(&doc).is_empty());
        doc.settings.embed_true_type_fonts = true;
        assert_eq!(fonts_to_embed(&doc).len(), 1);
    }
}
