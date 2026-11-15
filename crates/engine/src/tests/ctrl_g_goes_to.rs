//! Ctrl+G runs Go To on Linux and Windows, as Word for Windows binds it (F5 and Ctrl+Alt+G too);
//! Find Next keeps Shift+F4 and Ctrl+Alt+Y there. macOS keeps Cmd+G for Find Next.

use wordcraft_doc::Document;

use crate::keymap::normalize;
use crate::{Session, cmd};

fn id(k: &str) -> Option<&'static str> {
    cmd::registry().by_shortcut(k).map(|c| c.id)
}

fn norm(k: &str) -> String {
    normalize(k).unwrap_or_else(|| panic!("{k} is not a key"))
}

#[test]
fn ctrl_g_is_go_to_on_linux_and_windows() {
    if cfg!(target_os = "macos") {
        assert_eq!(id("Mod+G"), Some("edit.findNext"));
        assert_eq!(id("Mod+Alt+G"), Some("edit.goto"));
        assert_eq!(id("F5"), Some("edit.goto"));
        return;
    }
    for k in ["Mod+G", "F5", "Mod+Alt+G"] {
        assert_eq!(id(k), Some("edit.goto"), "{k}");
    }
    for k in ["Shift+F4", "Mod+Alt+Y"] {
        assert_eq!(id(k), Some("edit.findNext"), "{k}");
    }
}

#[test]
fn go_to_lists_ctrl_g_first_and_find_next_no_longer_has_it() {
    let s = Session::new(Document::new());
    let goto = s.keymap.keys_for(&s.registry, "edit.goto");
    let next = s.keymap.keys_for(&s.registry, "edit.findNext");
    if cfg!(target_os = "macos") {
        assert!(next.contains(&norm("Mod+G")), "{next:?}");
        assert!(!goto.contains(&norm("Mod+G")), "{goto:?}");
    } else {
        assert_eq!(goto.first(), Some(&norm("Mod+G")), "{goto:?}");
        assert!(goto.contains(&norm("F5")), "{goto:?}");
        assert!(!next.contains(&norm("Mod+G")), "{next:?}");
        assert!(next.contains(&norm("Shift+F4")), "{next:?}");
    }
}
