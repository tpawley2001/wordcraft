use super::{DocLayout, Pos, WordApp};

/// The simple field at or just before `pos` when it shows its code (View › Field Codes, or its
/// own toggle): where it is.
pub(super) fn field_showing_code(app: &WordApp, pos: &wordcraft_doc::Pos) -> Option<wordcraft_doc::Pos> {
    let para = app.session.doc.para_at(pos)?;
    let width = wordcraft_doc::para::OBJ.len_utf8();
    [Some(pos.off), pos.off.checked_sub(width)].into_iter().flatten().find_map(|off| match para.object_at(off) {
        Some(wordcraft_doc::para::InlineObject::Field { code, .. }) if *code != app.session.view.field_codes => {
            Some(wordcraft_doc::Pos { off, ..pos.clone() })
        }
        _ => None,
    })
}

/// The hyperlink under the pointer at (x, y) on `page`, where `pos` is the caret position the
/// point hits. The hit snaps to the nearest text, so the link only counts when the point is on
/// the linked character itself, not beside, above or below it (issue #413).
pub(super) fn link_under(app: &WordApp, layout: &DocLayout, page: usize, x: f32, y: f32, pos: &Pos) -> Option<String> {
    link_and_tip_under(app, layout, page, x, y, pos).map(|(link, _)| link)
}

/// [`link_under`] with the link's ScreenTip, when it has one.
pub(super) fn link_and_tip_under(app: &WordApp, layout: &DocLayout, page: usize, x: f32, y: f32, pos: &Pos) -> Option<(String, Option<String>)> {
    let doc = &app.session.doc;
    let para = doc.para_at(pos)?;
    let at = wordcraft_geom::Point::new(x, y);
    // The caret falls between characters: the one under the point is on either side of it.
    [(pos.off, para.next_boundary(pos.off)), (para.prev_boundary(pos.off), pos.off)].into_iter().filter(|(a, b)| a < b).find_map(|(a, b)| {
        let props = para.props_of_char(a);
        let link = match props.link.clone().filter(|l| !l.is_empty()) {
            Some(link) => (link, props.link_tip.as_deref().filter(|t| !t.is_empty()).map(str::to_string)),
            None => (field_link(para.object_at(a)?)?, None),
        };
        let (from, to) = (Pos { off: a, ..pos.clone() }, Pos { off: b, ..pos.clone() });
        layout.selection_rects(doc, &from, &to, page).into_iter().any(|(pi, r)| pi == page && r.contains(at)).then_some(link)
    })
}

/// The link a cross-reference field makes: `REF name \h` and `PAGEREF name \h` lead to the
/// bookmark `name` (`#name`), as in Word; without `\h` the result is plain text (#2945).
pub(super) fn field_link(obj: &wordcraft_doc::InlineObject) -> Option<String> {
    let wordcraft_doc::InlineObject::Field { instr, .. } = obj else { return None };
    let code = wordcraft_engine::cmd::references::FieldCode::parse(instr);
    let name = code.args.first().filter(|n| !n.is_empty())?;
    (matches!(code.name.as_str(), "REF" | "PAGEREF") && code.has('h')).then(|| format!("#{name}"))
}

/// The tip shown while the pointer rests on a link: its ScreenTip when it has one, else where it
/// goes (`target`), then the `follow` line ("Ctrl+Click to follow link").
pub(super) fn link_hover_text(target: &str, screen_tip: Option<&str>, follow: &str) -> String {
    let first = screen_tip.filter(|t| !t.is_empty()).unwrap_or(target);
    format!("{first}\n{follow}")
}

/// The hover tip's last line: how to follow the link with Use Ctrl+Click to follow hyperlink on
/// (`"Ctrl+Click to follow link"`, ⌘ on macOS) or off (`"Click to follow link"`).
pub(crate) fn follow_hint(ctrl_click_follows: bool) -> String {
    if !ctrl_click_follows {
        return tl!("Click to follow link").to_string();
    }
    let key = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };
    crate::i18n::fmt(tl!("{key}+Click to follow link"), &[("key", key)])
}

/// A link to a hidden bookmark (`#_Toc…`, `#_Ref…`): a name the program made, which nobody
/// chose or sees, so the hover tip says where it goes instead of showing it (#2177).
pub(super) fn is_generated_bookmark_link(link: &str) -> bool {
    link.strip_prefix("#_").is_some()
}

/// Ctrl/⌘+click on `link` (in the text or on a picture): a place in this document takes the caret
/// to its bookmark; a web or email address opens in the browser; any other is not followed.
pub(crate) fn follow_link(app: &mut WordApp, link: String) {
    if let Some(name) = link.strip_prefix('#') {
        // The caret goes to the bookmark's start, as in Word (Go To selects its text).
        if app.run("edit.goto", serde_json::json!({"bookmark": name})).is_ok() {
            let _ = app.run("select.collapse", serde_json::json!({}));
        }
    } else if is_followable_link(&link) {
        app.canvas.open_url = Some(link);
    } else {
        app.status(tl!("Only web (http, https) and email (mailto) links can be followed"));
    }
}

/// Whether Ctrl/⌘+click may hand a document's hyperlink to the system: web and email links only,
/// so a document can't launch `file:` paths, programs or custom-scheme handlers.
pub(super) fn is_followable_link(url: &str) -> bool {
    wordcraft_doc::link::is_web_or_mail_link(url)
}

#[cfg(test)]
mod hover_tests {
    use super::link_hover_text;

    #[test]
    fn a_links_screen_tip_shows_in_place_of_its_address() {
        let follow = "Ctrl+Click to follow link";
        assert_eq!(link_hover_text("https://example.com", Some("More about it"), follow), "More about it\nCtrl+Click to follow link");
        assert_eq!(link_hover_text("https://example.com", None, follow), "https://example.com\nCtrl+Click to follow link");
        assert_eq!(link_hover_text("https://example.com", Some(""), follow), "https://example.com\nCtrl+Click to follow link");
    }
}
