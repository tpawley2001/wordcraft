//! Word for Windows' alternate default keys: Ctrl+Shift+B/I/U (bold, italic, underline),
//! Ctrl+3/4/6 (lock, unlock, unlink fields), Alt+Shift+U (update fields) and Alt+Shift+Backspace
//! (redo). Measured in Microsoft 365 Word: each runs the same command as the primary key.

use wordcraft_doc::Document;

use crate::keymap::normalize;
use crate::{Session, cmd};

/// (alternate key, command, primary key that must stay first)
const ROWS: &[(&str, &str, &str)] = &[
    ("Mod+Shift+B", "format.bold", "Mod+B"),
    ("Mod+Shift+I", "format.italic", "Mod+I"),
    ("Mod+Shift+U", "format.underline", "Mod+U"),
    ("Mod+3", "fields.lock", "Mod+F11"),
    ("Mod+4", "fields.unlock", "Mod+Shift+F11"),
    ("Mod+6", "fields.unlink", "Mod+Shift+F9"),
    ("Alt+Shift+U", "references.updateFields", "F9"),
    ("Alt+Shift+Backspace", "edit.redo", "Mod+Y"),
];

fn id(k: &str) -> Option<&'static str> {
    cmd::registry().by_shortcut(k).map(|c| c.id)
}

fn keys(s: &Session, id: &str) -> Vec<String> {
    s.keymap.keys_for(&s.registry, id)
}

fn norm(k: &str) -> String {
    normalize(k).unwrap_or_else(|| panic!("{k} is not a key"))
}

#[test]
fn alternate_keys_run_words_commands_on_linux_and_windows() {
    for &(alt, want, _) in ROWS {
        if cfg!(target_os = "macos") {
            assert_ne!(id(alt), Some(want), "{alt} stays unbound on macOS");
        } else {
            assert_eq!(id(alt), Some(want), "{alt}");
        }
    }
}

#[test]
fn primary_keys_still_resolve_and_are_listed_first() {
    let s = Session::new(Document::new());
    for &(alt, cmd_id, primary) in ROWS {
        if cfg!(target_os = "macos") && primary == "F9" {
            continue;
        }
        if !cfg!(target_os = "macos") {
            assert_eq!(id(primary), Some(cmd_id), "{primary}");
        }
        let listed = keys(&s, cmd_id);
        if !cfg!(target_os = "macos") {
            assert_eq!(listed.first(), Some(&norm(primary)), "{cmd_id}: {listed:?}");
            assert!(listed.contains(&norm(alt)), "{cmd_id} lists {alt}: {listed:?}");
        } else {
            assert!(!listed.contains(&norm(alt)), "{cmd_id} on macOS: {listed:?}");
        }
    }
}

#[test]
fn alternate_keys_resolve_through_the_customizable_key_map() {
    if cfg!(target_os = "macos") {
        return;
    }
    let s = Session::new(Document::new());
    for &(alt, want, _) in ROWS {
        let got = s.keymap.resolve(&s.registry, &norm(alt)).map(|c| c.id);
        assert_eq!(got, Some(want), "{alt}");
    }
}
