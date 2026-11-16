//! Promote only properties touched by a formatting command into an automatic paragraph style.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use wordcraft_doc::{CharProps, ParaProps, Pos, StyleKind};

use crate::{CmdError, Session};

#[derive(Default)]
pub(super) struct Patch(Map<String, Value>);

fn map<T: Serialize>(props: &T) -> Result<Map<String, Value>, CmdError> {
    match serde_json::to_value(props).map_err(|e| CmdError::Failed(e.to_string()))? {
        Value::Object(map) => Ok(map),
        _ => Err(CmdError::Failed("formatting properties must be an object".into())),
    }
}

fn decode<T: DeserializeOwned>(map: Map<String, Value>) -> Result<T, CmdError> {
    serde_json::from_value(Value::Object(map)).map_err(|e| CmdError::Failed(e.to_string()))
}

impl Patch {
    fn record<T: Serialize + Default + Clone>(&mut self, before: &T, f: &dyn Fn(&mut T)) -> Result<(), CmdError> {
        let mut after = before.clone();
        f(&mut after);
        let mut probe = T::default();
        f(&mut probe);
        let (before, after, probe) = (map(before)?, map(&after)?, map(&probe)?);
        // The probe also catches setting a property to the value already present directly.
        // Style references, links and revision metadata must never migrate into a definition.
        for key in before.keys().chain(after.keys()).chain(probe.keys()) {
            if ["style", "link", "linkTip", "ins", "del", "fmtChange", "numChange", "symFont"].contains(&key.as_str()) {
                continue;
            }
            if before.get(key) != after.get(key) || probe.contains_key(key) {
                self.0.insert(key.clone(), after.get(key).cloned().unwrap_or(Value::Null));
            }
        }
        Ok(())
    }

    pub(super) fn clear<T: Serialize + DeserializeOwned>(&self, props: &mut T) -> Result<(), CmdError> {
        let mut out = map(props)?;
        out.retain(|key, _| !self.0.contains_key(key));
        *props = decode(out)?;
        Ok(())
    }

    fn merge_into<T: Serialize + DeserializeOwned>(&self, props: &mut T) -> Result<(), CmdError> {
        let mut out = map(props)?;
        for (key, value) in &self.0 {
            if value.is_null() {
                out.remove(key);
            } else {
                out.insert(key.clone(), value.clone());
            }
        }
        *props = decode(out)?;
        Ok(())
    }

    pub(super) fn extend(&mut self, patch: Self) {
        self.0.extend(patch.0);
    }

    pub(super) fn character(props: &CharProps, f: &dyn Fn(&mut CharProps)) -> Result<Self, CmdError> {
        let mut patch = Self::default();
        patch.record(props, f)?;
        Ok(patch)
    }

    pub(super) fn paragraph(props: &ParaProps, f: &dyn Fn(&mut ParaProps)) -> Result<Self, CmdError> {
        let mut patch = Self::default();
        patch.record(props, f)?;
        Ok(patch)
    }
}

pub(super) fn style_id(s: &Session, props: &ParaProps) -> Option<String> {
    if s.doc.settings.track_changes {
        return None;
    }
    let id = props.style.as_deref().unwrap_or("Normal");
    s.doc.styles.get(id).filter(|st| st.kind == StyleKind::Paragraph && st.auto_redefine).map(|st| st.id.clone())
}

pub(super) fn redefine(s: &mut Session, patches: BTreeMap<String, Patch>, character: bool) -> Result<(), CmdError> {
    if patches.is_empty() {
        return Ok(());
    }
    for (id, patch) in patches {
        if let Some(st) = Arc::make_mut(&mut s.doc.styles).get_mut(&id) {
            if character {
                patch.merge_into(&mut st.chr)?;
            } else {
                patch.merge_into(&mut st.para)?;
            }
        }
    }
    super::para::touch_all(s);
    Ok(())
}

/// Keep partial paragraphs direct, even when another selected paragraph redefines their style.
pub(super) fn format_range(s: &mut Session, a: &Pos, b: &Pos, f: &dyn Fn(&mut CharProps)) -> Result<(), CmdError> {
    let mut patches: BTreeMap<String, Patch> = BTreeMap::new();
    let mut full = Vec::new();
    for path in s.doc.para_paths_in(a, b) {
        let Some(para) = s.doc.para(a.story, &path) else { continue };
        let from = if path == a.path { a.off } else { 0 };
        let to = if path == b.path { b.off } else { para.len() };
        if from != 0 || to != para.len() {
            continue;
        }
        let Some(id) = style_id(s, &para.props) else { continue };
        let mut patch = Patch::default();
        for run in &para.runs {
            patch.record(&run.props, f)?;
        }
        patch.record(&para.mark, f)?;
        patches.entry(id).or_default().extend(Patch(patch.0.clone()));
        full.push((path, patch));
    }
    s.doc.format_range(a, b, f)?;
    for (path, patch) in full {
        let para = s.doc.para_mut(a.story, &path)?;
        for run in &mut para.runs {
            patch.clear(&mut run.props)?;
        }
        patch.clear(&mut para.mark)?;
        para.touch();
    }
    redefine(s, patches, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Selection;
    use serde_json::json;
    use wordcraft_doc::{Align, Document, StoryRef, Style};

    fn session() -> Session {
        let mut doc = Document::from_text("First paragraph\nSecond paragraph\nNormal paragraph");
        Arc::make_mut(&mut doc.styles).upsert(Style { id: "Automatic".into(), name: "Automatic".into(), auto_redefine: true, ..Default::default() });
        for i in 0..2 {
            doc.para_mut(StoryRef::Body, &Pos::body(i, 0).path).unwrap().props.style = Some("Automatic".into());
        }
        Session::new(doc)
    }

    #[test]
    fn mixed_selection_keeps_other_properties_and_undoes_together() {
        let mut s = session();
        s.doc.para_mut(StoryRef::Body, &Pos::body(0, 0).path).unwrap().props.space_after = Some(17.0);
        s.sel = Selection { anchor: Pos::body(0, 0), focus: Pos::body(2, 6) };
        s.run("para.alignCenter", &json!({})).unwrap();
        let st = s.doc.styles.get("Automatic").unwrap();
        assert_eq!(st.para.align, Some(Align::Center));
        assert_eq!(st.para.space_after, None);
        assert_eq!(s.doc.para_at(&Pos::body(0, 0)).unwrap().props.space_after, Some(17.0));
        for i in 0..2 {
            assert_eq!(s.doc.para_at(&Pos::body(i, 0)).unwrap().props.align, None);
        }
        assert_eq!(s.doc.para_at(&Pos::body(2, 0)).unwrap().props.align, Some(Align::Center));
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc.styles.get("Automatic").unwrap().para.align, None);
        assert_eq!(s.doc.para_at(&Pos::body(2, 0)).unwrap().props.align, None);
    }

    #[test]
    fn whole_and_partial_paragraphs_keep_unrelated_run_formatting() {
        let mut s = session();
        s.doc.para_mut(StoryRef::Body, &Pos::body(0, 0).path).unwrap().format(0, 5, &|c| c.italic = Some(true)).unwrap();
        s.sel = Selection { anchor: Pos::body(0, 0), focus: Pos::body(1, 6) };
        s.run("format.bold", &json!({"value": true})).unwrap();
        let st = s.doc.styles.get("Automatic").unwrap();
        assert_eq!(st.chr.bold, Some(true));
        assert_eq!(st.chr.italic, None);
        let first = s.doc.para_at(&Pos::body(0, 0)).unwrap();
        assert_eq!(first.props_of_char(0).italic, Some(true));
        assert!(first.runs.iter().all(|r| r.props.bold.is_none()));
        assert_eq!(first.mark.bold, None);
        assert_eq!(s.doc.para_at(&Pos::body(1, 0)).unwrap().props_of_char(0).bold, Some(true));
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc.styles.get("Automatic").unwrap().chr.bold, None);
        assert_eq!(s.doc.para_at(&Pos::body(0, 0)).unwrap().props_of_char(0).italic, Some(true));
    }

    #[test]
    fn tracking_disables_both_promotions() {
        let mut s = session();
        s.doc.settings.track_changes = true;
        s.run("para.alignCenter", &json!({})).unwrap();
        s.sel = Selection { anchor: Pos::body(0, 0), focus: Pos::body(0, 15) };
        s.run("format.bold", &json!({"value": true})).unwrap();
        let st = s.doc.styles.get("Automatic").unwrap();
        assert_eq!(st.para.align, None);
        assert_eq!(st.chr.bold, None);
        let p = s.doc.para_at(&Pos::body(0, 0)).unwrap();
        assert_eq!(p.props.align, Some(Align::Center));
        assert_eq!(p.props_of_char(0).bold, Some(true));
    }

    #[test]
    fn setting_existing_direct_value_still_promotes_it() {
        let mut s = session();
        s.doc.para_mut(StoryRef::Body, &Pos::body(0, 0).path).unwrap().props.align = Some(Align::Center);
        s.run("para.align", &json!({"value": "center"})).unwrap();
        assert_eq!(s.doc.styles.get("Automatic").unwrap().para.align, Some(Align::Center));
        assert_eq!(s.doc.para_at(&Pos::body(0, 0)).unwrap().props.align, None);
    }

    #[test]
    fn empty_paragraph_promotes_mark_and_pending_formatting() {
        let mut s = session();
        s.doc.para_mut(StoryRef::Body, &Pos::body(0, 0).path).unwrap().text.clear();
        s.doc.para_mut(StoryRef::Body, &Pos::body(0, 0).path).unwrap().runs.clear();
        s.sel = Selection::caret(Pos::body(0, 0));
        s.run("format.bold", &json!({"value": true})).unwrap();
        assert_eq!(s.doc.styles.get("Automatic").unwrap().chr.bold, Some(true));
        assert_eq!(s.doc.para_at(&Pos::body(0, 0)).unwrap().mark.bold, None);
        assert_eq!(s.pending.as_ref().and_then(|p| p.bold), None);
    }

    #[test]
    fn different_automatic_styles_each_take_the_change() {
        let mut s = session();
        let mut other = s.doc.styles.get("Automatic").unwrap().clone();
        other.id = "Other".into();
        Arc::make_mut(&mut s.doc.styles).upsert(other);
        s.doc.para_mut(StoryRef::Body, &Pos::body(1, 0).path).unwrap().props.style = Some("Other".into());
        s.sel = Selection { anchor: Pos::body(0, 0), focus: Pos::body(2, 0) };
        s.run("format.bold", &json!({"value": false})).unwrap();
        for id in ["Automatic", "Other"] {
            assert_eq!(s.doc.styles.get(id).unwrap().chr.bold, Some(false));
        }
        assert_eq!(s.doc.para_at(&Pos::body(2, 0)).unwrap().mark.bold, None);
    }
}
