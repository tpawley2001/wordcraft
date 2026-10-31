//! The app's name, suite, About text and links, in one place (Jolly Roger Office rebrand).
//!
//! Every user-visible product name comes from here, so renaming the app is a one-file change.
//! Packaging files that can't read Rust constants (desktop entry, metainfo, Windows installer)
//! carry the same values and say so. Flavour text (the pirate voice) sits behind the
//! `pirate_speak` setting; menus, commands, dialogs and errors stay plain English.

/// The word processor's name, as window titles, the About box and messages show it.
pub const APP_NAME: &str = "Captain's Log";
/// The About box's title (kept equal to "About " + [`APP_NAME`] by a test).
pub const ABOUT_TITLE: &str = "About Captain's Log";
/// The office suite the app belongs to.
pub const SUITE_NAME: &str = "Jolly Roger Office";
/// The freedesktop application id: desktop entry, icon name and Wayland app id.
pub const APP_ID: &str = "com.hillyard_tech.CaptainsLog";
/// The user name for comments and tracked changes until one is set in File › Options.
pub const DEFAULT_AUTHOR: &str = "Captain";
/// One line under the name in the About box.
pub const TAGLINE: &str = "A free, open-source word processor written in Rust.";
/// The upstream credit the licences ask us to keep visible.
pub const UPSTREAM_CREDIT: &str = "Based on WordCraft by the ArtCraft team (MIT OR Apache-2.0).";
/// Where WordCraft's source lives, for the credit.
pub const UPSTREAM_SOURCE: &str = "https://github.com/storytold/wordcraft";
/// Copyright line for the About box.
pub const COPYRIGHT: &str = "MIT OR Apache-2.0. Copyright (c) 2026 Hillyard Tech, the ArtCraft Team and the WordCraft contributors.";

/// Interface text written with the upstream name, with the app's name in its place. Applied after
/// the translation lookup (`brand::named(tl!("…WordCraft…"))`), so the catalogs' keys still match
/// and translated text is renamed too.
pub fn named(text: &str) -> String {
    text.replace("WordCraft", APP_NAME)
}

/// The Start screen's greeting for the hour of the day (0–23): plain, or in the pirate voice.
pub fn greeting(hour: u32, pirate_speak: bool) -> &'static str {
    match (hour, pirate_speak) {
        (0..=11, false) => "Good morning",
        (12..=17, false) => "Good afternoon",
        (_, false) => "Good evening",
        (0..=11, true) => "Mornin', Captain",
        (12..=17, true) => "Ahoy, Captain",
        (_, true) => "Evenin', Captain",
    }
}

/// The Start screen's empty Recent list.
pub fn no_recent(pirate_speak: bool) -> &'static str {
    if pirate_speak {
        "No voyages logged yet. Open a document, or start a blank page and make yer mark."
    } else {
        "Documents you open will show up here."
    }
}

/// A second line for the About box, in the pirate voice only.
pub fn about_flavour(pirate_speak: bool) -> Option<&'static str> {
    pirate_speak.then_some("Keep a true log, Captain: the sea forgets, but the page remembers.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_puts_the_app_name_in() {
        assert_eq!(named("Show the Start screen when WordCraft starts"), format!("Show the Start screen when {APP_NAME} starts"));
        assert_eq!(named("plain"), "plain");
    }

    #[test]
    fn plain_greetings_follow_the_hour() {
        assert_eq!(greeting(8, false), "Good morning");
        assert_eq!(greeting(13, false), "Good afternoon");
        assert_eq!(greeting(21, false), "Good evening");
        assert_eq!(greeting(99, false), "Good evening", "junk hours never panic");
    }

    #[test]
    fn pirate_speak_only_changes_flavour_text() {
        assert_eq!(greeting(13, true), "Ahoy, Captain");
        assert_ne!(no_recent(true), no_recent(false));
        assert!(about_flavour(false).is_none());
        assert!(about_flavour(true).is_some());
    }

    /// The upstream credit and licence stay in the About box text.
    #[test]
    fn credit_names_upstream_and_licence() {
        assert!(UPSTREAM_CREDIT.contains("WordCraft") && UPSTREAM_CREDIT.contains("MIT OR Apache-2.0"));
        assert!(!APP_NAME.contains("Craft"));
        assert_eq!(ABOUT_TITLE, format!("About {APP_NAME}"));
    }
}
