//! Resolve file comment ids against references before building a story's inline objects.
use std::collections::HashMap;

use super::Reader;
use crate::xml::{El, MAX_DEPTH, Node};

#[derive(Default)]
struct Events {
    starts: Vec<usize>,
    ranges: Vec<(usize, usize)>,
    refs: Vec<(usize, u32)>,
}

impl Reader<'_> {
    pub(super) fn prepare_comment_ranges(&mut self, root: &El) -> El {
        let mut root = root.clone();
        let mut events: HashMap<String, Events> = HashMap::new();
        let mut text_at = Vec::new();
        let mut text = 0usize;
        fn scan(r: &mut Reader<'_>, e: &mut El, events: &mut HashMap<String, Events>, text_at: &mut Vec<usize>, text: &mut usize, depth: usize) {
            if depth > MAX_DEPTH {
                return;
            }
            let pos = text_at.len();
            text_at.push(*text);
            let text_run = matches!(e.name.as_str(), "w:t" | "w:delText") && e.kids.iter().any(|n| matches!(n, Node::Text(t) if !t.is_empty()));
            if text_run || matches!(e.name.as_str(), "w:tab" | "w:br" | "w:cr" | "w:drawing" | "w:pict") {
                *text = text.saturating_add(1);
            }
            if matches!(e.name.as_str(), "w:commentRangeStart" | "w:commentRangeEnd" | "w:commentReference") && e.attr("::comment").is_none() {
                let id = e.attr("w:id").unwrap_or("").trim().to_string();
                // This private attribute also prevents a nested story from consuming a reference twice.
                e.attrs.push(("::comment".into(), String::new()));
                if r.comment_map.contains_key(&id) {
                    let ev = events.entry(id.clone()).or_default();
                    match e.name.as_str() {
                        "w:commentRangeStart" => ev.starts.push(pos),
                        "w:commentRangeEnd" => {
                            if let Some(start) = ev.starts.pop() {
                                ev.ranges.push((start, pos));
                            }
                        }
                        _ => {
                            if let Some(cid) = r.comment_map.get_mut(&id).and_then(|q| q.pop_front()) {
                                ev.refs.push((pos, cid));
                            }
                        }
                    }
                }
            }
            for kid in &mut e.kids {
                if let Node::El(k) = kid {
                    scan(r, k, events, text_at, text, depth + 1);
                }
            }
        }
        scan(self, &mut root, &mut events, &mut text_at, &mut text, 0);
        let mut assigned: HashMap<usize, (u32, bool)> = HashMap::new();
        for ev in events.values() {
            let mut candidates: HashMap<usize, Option<(usize, usize)>> = HashMap::new();
            for &(start, end) in &ev.ranges {
                let i = ev.refs.partition_point(|(pos, _)| *pos < start);
                let Some(&(pos, _)) = ev.refs.get(i) else { continue };
                let encloses = pos < end && ev.refs.get(i.saturating_add(1)).is_none_or(|(next, _)| *next > end);
                let adjacent = pos > end && text_at.get(pos) == text_at.get(end);
                if encloses || adjacent {
                    candidates.entry(pos).and_modify(|v| *v = None).or_insert(Some((start, end)));
                }
            }
            // Older producers sometimes use the reference itself as the range's end.
            // Preserve that established import when a single open start has one possible mark.
            if let [start] = ev.starts.as_slice() {
                let i = ev.refs.partition_point(|(pos, _)| pos < start);
                if let Some(&(pos, _)) = ev.refs.get(i)
                    && ev.refs.get(i.saturating_add(1)).is_none()
                {
                    candidates.entry(pos).and_modify(|v| *v = None).or_insert(Some((*start, pos)));
                }
            }
            for &(pos, cid) in &ev.refs {
                let range = candidates.get(&pos).copied().flatten();
                assigned.insert(pos, (cid, range.is_some_and(|(_, end)| end != pos)));
                if let Some((start, end)) = range {
                    assigned.insert(start, (cid, false));
                    assigned.insert(end, (cid, false));
                }
            }
        }
        fn apply(e: &mut El, assigned: &HashMap<usize, (u32, bool)>, pos: &mut usize, depth: usize) {
            if depth > MAX_DEPTH {
                return;
            }
            let here = *pos;
            *pos = pos.saturating_add(1);
            if let Some(&(id, range)) = assigned.get(&here) {
                if let Some((_, value)) = e.attrs.iter_mut().find(|(k, _)| k == "::comment") {
                    *value = id.to_string();
                }
                if range {
                    e.attrs.push(("::range".into(), "1".into()));
                }
            }
            for kid in &mut e.kids {
                if let Node::El(k) = kid {
                    apply(k, assigned, pos, depth + 1);
                }
            }
        }
        apply(&mut root, &assigned, &mut 0, 0);
        root
    }
}
