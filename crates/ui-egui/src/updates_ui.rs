//! Help › Check for Updates. This build has no self-updater: upstream's craft-update is not a
//! dependency, so the app never contacts an update server and can never replace itself with an
//! upstream build. The command only says so.

use crate::WordApp;

/// What Check for Updates shows.
pub(crate) fn message() -> String {
    crate::i18n::fmt(
        tl!("Automatic updates are turned off. {app} never downloads updates; install new versions from {suite}."),
        &[("app", crate::brand::APP_NAME), ("suite", crate::brand::SUITE_NAME)],
    )
}

#[derive(Default)]
pub(crate) struct Runtime {
    pub(crate) manual_window_open: bool,
}

pub(crate) fn check(app: &mut WordApp) {
    app.updates_rt.manual_window_open = true;
    app.status(message());
}

pub(crate) fn tick(app: &mut WordApp, ctx: &egui::Context) {
    let _ = (app, ctx);
}

pub(crate) fn show(app: &mut WordApp, ctx: &egui::Context) {
    if !app.updates_rt.manual_window_open {
        return;
    }
    let mut open = true;
    let mut close = false;
    egui::Window::new(tl!("Check for Updates"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(message());
            ui.add_space(6.0);
            close = ui.button(tl!("Close")).clicked();
        });
    if !open || close {
        app.updates_rt.manual_window_open = false;
    }
}

#[cfg(test)]
mod tests {
    /// The updater can't come back by accident: no crate in the workspace depends on craft-update.
    #[test]
    fn no_self_updater_in_the_build() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for manifest in ["Cargo.toml", "crates/ui-egui/Cargo.toml", "apps/wordcraft/Cargo.toml"] {
            let text = std::fs::read_to_string(root.join(manifest)).unwrap();
            assert!(!text.contains("craft-update"), "{manifest} depends on craft-update");
        }
        assert!(super::message().contains("never downloads"));
    }
}
