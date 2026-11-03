//! Shapes with their own text inside a text box. Word never writes a `w:txbxContent` inside
//! another (ECMA-376 Part 1 §17.17.1.1) and can't open a file that does, so such a shape is
//! written without one: WordArt as a VML `v:shape` with a `v:textpath` (ECMA-376 Part 4), the form
//! Word 2007 wrote it in; any other shape without its text box, its text following as paragraphs.

use super::{CharProps, InlineObject, PartRels, ShapeKind, TextColor, W, Wrap, Writer};
use crate::xml::esc;

/// The longest WordArt text written (what the reader keeps).
const MAX_WORDART_CHARS: usize = 200;

impl Writer<'_> {
    /// The text box story of shape `o`, if it has one.
    fn story_of(&self, o: &InlineObject) -> Option<u32> {
        let InlineObject::Shape { story, .. } = o else { return None };
        (*story).filter(|id| self.doc.parts.get(id).is_some_and(|p| p.kind == wordcraft_doc::PartKind::TextBox))
    }

    /// Whether shape `o` has text of its own (written by [`Self::write_nested_story_shape`]
    /// inside a text box).
    pub(super) fn nested_story(&self, o: &InlineObject) -> bool {
        self.story_of(o).is_some()
    }

    /// Shape `o`, inside a text box, with its own text: WordArt as VML, a text box as its text
    /// (after this paragraph), any other shape without its text (which follows likewise).
    pub(super) fn write_nested_story_shape(&mut self, w: &mut W, o: &InlineObject, props: &CharProps, rels: &mut PartRels, depth: usize) {
        let Some(id) = self.story_of(o) else { return };
        let InlineObject::Shape { kind, extra, .. } = o else { return };
        if *kind == ShapeKind::TextBox {
            if extra.warp.is_some()
                && let Some(pict) = self.vml_wordart(o, id)
            {
                self.rev_open(w, props);
                w.open("w:r", &[]);
                self.rpr(w, props);
                w.raw(&pict);
                w.close("w:r");
                self.rev_close(w, props);
            } else {
                self.flattened.push(id);
            }
        } else {
            self.write_shape(w, o, props, rels, depth);
        }
    }

    /// Whether shape `o` is WordArt whose letters cast a Word 2007 (VML) shadow that DrawingML
    /// can't hold (a second copy or a transform): it is written back as VML.
    pub(super) fn vml_shadowed(&self, o: &InlineObject) -> bool {
        let InlineObject::Shape { kind: ShapeKind::TextBox, .. } = o else { return false };
        let Some(part) = self.story_of(o).and_then(|id| self.doc.parts.get(&id)) else { return false };
        first_props(part).and_then(|p| p.text_effects.as_deref()).is_some_and(|fx| fx.shadow.is_some() && fx.shadow_extra.is_some())
    }

    /// WordArt shape `o` as VML (see [`Self::vml_shadowed`]); any other shape as usual.
    pub(super) fn write_vml_wordart(&mut self, w: &mut W, o: &InlineObject, props: &CharProps, rels: &mut PartRels, depth: usize) {
        match self.story_of(o).and_then(|id| self.vml_wordart(o, id)) {
            Some(pict) => {
                self.rev_open(w, props);
                w.open("w:r", &[]);
                self.rpr(w, props);
                w.raw(&pict);
                w.close("w:r");
                self.rev_close(w, props);
            }
            None => self.write_shape(w, o, props, rels, depth),
        }
    }

    /// The stories of shapes inside a text box met since the last call, as plain paragraphs.
    pub(super) fn write_flattened(&mut self, w: &mut W, rels: &mut PartRels, depth: usize) {
        for id in std::mem::take(&mut self.flattened) {
            // Each story once: a story showing itself can't loop.
            if !self.written_boxes.insert(id) {
                continue;
            }
            let Some(blocks) = self.doc.parts.get(&id).map(|p| p.blocks.clone()) else { continue };
            self.blocks(w, &blocks, rels, false, depth.saturating_add(1));
        }
    }

    /// A `w:pict` holding WordArt shape `o` with story `id`: its text, size, place, colour and
    /// font. `None` when the story has no text.
    fn vml_wordart(&mut self, o: &InlineObject, id: u32) -> Option<String> {
        let InlineObject::Shape { w: sw, h: sh, float, extra, .. } = o else { return None };
        let part = self.doc.parts.get(&id)?;
        let mut text = String::new();
        let mut first: Option<&CharProps> = None;
        for b in &part.blocks {
            let wordcraft_doc::Block::Para(p) = &**b else { continue };
            if !text.is_empty() {
                text.push('\n');
            }
            text.extend(p.text.chars().filter(|c| *c != wordcraft_doc::para::OBJ && !c.is_control()));
            if first.is_none() && !p.text.is_empty() {
                first = p.runs.first().map(|r| &r.props);
            }
        }
        let text: String = text.chars().take(MAX_WORDART_CHARS).collect();
        if text.trim().is_empty() {
            return None;
        }
        // The shape type: WordArt's `o:spt` 136 (plain) to 175, from the warp.
        let spt = extra.warp.as_ref().and_then(|t| crate::read::VML_TEXT_WARPS.iter().position(|p| *p == t.preset)).map_or(136, |i| 136 + i);
        // Plain text stretched between the box's top and bottom edges (as the watermark is).
        let path = if spt == 136 { r#" path="m0,0l21600,0m0,21600l21600,21600e""# } else { "" };
        let base = if float.wrap == Wrap::Inline { "" } else { "position:absolute" };
        let style = super::super::embed::vml_style_with(base, *sw, *sh, float);
        let font = first.and_then(|p| p.font.as_deref()).map(super::super::watermark::css_font).filter(|f| !f.trim().is_empty());
        let size = first.and_then(|p| p.size).filter(|s| s.is_finite()).map(|s| s.clamp(1.0, 1584.0));
        let mut tstyle = Vec::new();
        if let Some(f) = font {
            tstyle.push(format!("font-family:\"{f}\""));
        }
        if let Some(s) = size {
            tstyle.push(format!("font-size:{}pt", crate::units::n(s.round() as i64)));
        }
        let color = match first.and_then(|p| p.color) {
            Some(TextColor::Rgb(c)) => c.hex(),
            _ => "000000".to_string(),
        };
        let (look, children) =
            first.and_then(|p| p.text_effects.as_deref()).map(vml_look).unwrap_or_else(|| (r#" stroked="f""#.to_string(), String::new()));
        let docpr = self.next_docpr();
        Some(format!(
            concat!(
                r#"<w:pict><v:shapetype id="_x0000_t{spt}" coordsize="21600,21600" o:spt="{spt}"{path}>"#,
                r#"<v:path textpathok="t"/><v:textpath on="t" fitshape="t"/></v:shapetype>"#,
                r##"<v:shape id="WordArt{docpr}" type="#_x0000_t{spt}" style="{style}" fillcolor="#{color}"{look}>{children}"##,
                r#"<v:textpath style="{tstyle}" string="{text}"/></v:shape></w:pict>"#
            ),
            spt = spt,
            path = path,
            docpr = docpr,
            style = esc(&style),
            color = color,
            look = look,
            children = children,
            tstyle = esc(&tstyle.join(";")),
            text = esc(&text),
        ))
    }
}

/// The first run's properties of the first paragraph with text in `part`.
fn first_props(part: &wordcraft_doc::Part) -> Option<&CharProps> {
    part.blocks.iter().find_map(|b| match &**b {
        wordcraft_doc::Block::Para(p) if !p.text.is_empty() => p.runs.first().map(|r| &r.props),
        _ => None,
    })
}

/// A VML opacity, 0..1, from a transparency in percent.
fn opacity(transparency: f32) -> String {
    let o = (1.0 - wordcraft_geom::finite(transparency).clamp(0.0, 100.0) / 100.0).clamp(0.0, 1.0);
    format!("{}f", (o * 65_536.0).round() as i64)
}

/// The VML attributes (after `fillcolor`) and children that draw WordArt letters with effects
/// `fx` (ECMA-376 Part 4: `v:fill` §19.1.2.5, `v:stroke` §19.1.2.21, `v:shadow` §19.1.2.18), as
/// the reader (`read::story::vml_text_effects`) maps them back.
fn vml_look(fx: &wordcraft_doc::wordart::TextEffects) -> (String, String) {
    use super::super::embed::pt;
    use wordcraft_doc::wordart::TextFill;
    let fx = fx.sanitized();
    let mut attrs = String::new();
    let mut children = String::new();
    match &fx.fill {
        Some(TextFill::None) => attrs.push_str(r#" filled="f""#),
        Some(TextFill::Solid { transparency, .. }) if *transparency > 0.0 => {
            children.push_str(&format!(r#"<v:fill opacity="{}"/>"#, opacity(*transparency)));
        }
        _ => {}
    }
    match fx.outline {
        Some(wordcraft_doc::wordart::TextOutline { color: Some(c), width, transparency }) => {
            attrs.push_str(&format!(r##" strokecolor="#{}" strokeweight="{}pt""##, c.hex(), pt(width)));
            if transparency > 0.0 {
                children.push_str(&format!(r#"<v:stroke opacity="{}"/>"#, opacity(transparency)));
            }
        }
        _ => attrs.push_str(r#" stroked="f""#),
    }
    if let Some(s) = fx.shadow {
        let (dx, dy) = s.offset();
        let extra = fx.shadow_extra.unwrap_or_default();
        let mut more = String::new();
        let kind = if let Some(t) = extra.transform {
            let [sxx, sxy, syx, syy] = t.matrix.map(|v| format!("{}f", (v * 65_536.0).round() as i64));
            let [ox, oy] = t.origin.map(|v| format!("{}f", (v * 65_536.0).round() as i64));
            more = format!(r#" origin="{ox},{oy}" matrix="{sxx},{sxy},{syx},{syy},0,0""#);
            "perspective"
        } else if let Some(c) = extra.second {
            more = format!(r##" color2="#{}" offset2="{}pt,{}pt""##, c.color.hex(), pt(c.dx), pt(c.dy));
            "double"
        } else {
            "single"
        };
        children.push_str(&format!(
            r##"<v:shadow on="t" type="{kind}" color="#{}" opacity="{}" offset="{}pt,{}pt"{more}/>"##,
            s.color.hex(),
            opacity(s.transparency),
            pt(dx),
            pt(dy)
        ));
    }
    (attrs, children)
}
