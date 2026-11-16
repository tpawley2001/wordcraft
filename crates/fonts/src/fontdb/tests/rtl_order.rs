//! Right-to-left fallback does not depend on what loaded before it (#4300): Hebrew and Arabic
//! text gets the same fallback font whether or not a font outside the ranked right-to-left
//! families that covers it loaded first (Liberation Sans, which a document asking for Arial
//! loads), per character and per run, and a rescan that finds a higher-ranked family changes the
//! choice. Skipped where the fonts aren't installed.
//!
//! Each database drops the bundled faces covering Hebrew or Arabic (the craft-fonts Noto faces),
//! so the choice is among the folder's fonts in every build, as in a build without craft-fonts.
use std::path::{Path, PathBuf};

use crate::fontdb::fallback_data::RTL_FALLBACKS;
use crate::fontdb::*;

const DEJAVU: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
const LIBERATION: &str = "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf";
/// Installed fonts outside [`RTL_FALLBACKS`] that may cover Hebrew or Arabic: Liberation Sans 2
/// has Hebrew (the older Liberation Sans has neither), FreeSerif and FreeSans have both or one.
const UNRANKED: &[&str] = &[
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    LIBERATION,
    "/usr/share/fonts/truetype/freefont/FreeSerif.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
];

/// A fresh folder named for `tag` holding copies of the installed files `files`; `None` (with a
/// message) where one isn't installed.
fn folder(tag: &str, files: &[&str]) -> Option<PathBuf> {
    if files.iter().any(|p| !Path::new(p).exists()) {
        eprintln!("skipped: DejaVu Sans and Liberation Sans are not installed");
        return None;
    }
    let dir = std::env::temp_dir().join(format!("dc-rtl-order-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for p in files {
        std::fs::copy(p, dir.join(Path::new(p).file_name().unwrap())).unwrap();
    }
    Some(dir)
}

/// A database over `dir` without the bundled faces covering Hebrew or Arabic.
fn db_over(dir: &Path) -> FontDb {
    let db = FontDb::with_font_dirs(vec![dir.to_path_buf()]);
    db.faces.write().unwrap().retain(|f| !f.covers('ש') && !f.covers('ب'));
    db
}

fn ranked(f: &FontFace) -> bool {
    RTL_FALLBACKS.iter().any(|n| f.family.eq_ignore_ascii_case(n))
}

/// The fallback family `lookup` gives on a fresh database over `dir`. With `unranked_first`, a
/// Latin lookup in Arial runs first (as a bulleted list does), then the installed fonts outside
/// the ranked families load, so one covering `c` is there before the lookup; `None` (with a
/// message) where none covers it. A build with craft-fonts resolves Arial to its own Latin-only
/// Liberation Sans, so the installed files are loaded as well.
fn chosen(dir: &Path, c: char, unranked_first: bool, lookup: &dyn Fn(&FontDb, u32) -> Option<Arc<FontFace>>) -> Option<Option<String>> {
    let db = db_over(dir);
    let latin = db.face(FALLBACK_FAMILY, "Regular");
    if unranked_first {
        assert!(db.face("Arial", "Regular").covers('A'));
        for p in UNRANKED.iter().filter(|p| Path::new(p).exists()) {
            db.add_font(std::fs::read(p).unwrap());
        }
        if !db.read_faces().iter().any(|f| !ranked(f) && f.covers(c)) {
            eprintln!("skipped: no installed font outside the ranked families covers {c}");
            return None;
        }
    }
    Some(lookup(&db, latin.id()).map(|f| f.family.clone()))
}

fn same_either_order(tag: &str, c: char, lookup: &dyn Fn(&FontDb, u32) -> Option<Arc<FontFace>>) {
    let Some(dir) = folder(tag, &[DEJAVU, LIBERATION]) else { return };
    let Some(a) = chosen(&dir, c, false, lookup) else { return };
    let Some(b) = chosen(&dir, c, true, lookup) else { return };
    assert_eq!(a.as_deref(), Some("DejaVu Sans"), "{tag}: the ranked right-to-left family");
    assert_eq!(a, b, "{tag}: the same family whether or not an unranked font covering it loaded first");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn hebrew_fallback_is_the_same_in_either_order() {
    same_either_order("hebrew", 'ש', &|db, ex| db.fallback_for('ש', ex));
}

#[test]
fn arabic_fallback_is_the_same_in_either_order() {
    same_either_order("arabic", 'ب', &|db, ex| db.fallback_for('ب', ex));
}

#[test]
fn hebrew_run_fallback_is_the_same_in_either_order() {
    same_either_order("run", 'ש', &|db, ex| db.fallback_for_run(&['ש', 'ל', 'ו', 'ם'], ex, false, false));
}

/// A higher-ranked right-to-left family installed after the first lookup is chosen after a
/// rescan, as a fresh database over the folder chooses it.
#[test]
fn a_rescan_brings_in_a_higher_ranked_rtl_family() {
    let Some(dir) = folder("rescan", &[LIBERATION]) else { return };
    let db = db_over(&dir);
    let latin = db.face(FALLBACK_FAMILY, "Regular");
    let lookup = |db: &FontDb| db.fallback_for('ש', latin.id()).map(|f| f.family.clone());
    // Twice, so the second answer is a cached one.
    let before = lookup(&db);
    assert_eq!(lookup(&db), before);
    assert_ne!(before.as_deref(), Some("DejaVu Sans"), "not installed yet");
    std::fs::copy(DEJAVU, dir.join("DejaVuSans.ttf")).unwrap();
    db.load_system_fonts();
    assert_eq!(lookup(&db).as_deref(), Some("DejaVu Sans"), "the family installed since (was {before:?})");
    let _ = std::fs::remove_dir_all(dir);
}
