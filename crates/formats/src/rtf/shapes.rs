//! RTF 1.9.1 shape instructions. Only the text story and its placement are interpreted;
//! unknown instructions remain outside the document flow.
use super::model::BoxLook;
use super::state::{Dest, Output};
use super::{Inline, Reader};
use wordcraft_doc::para::{Anchor, Float, ShapeKind, Wrap};

pub(super) struct Shape {
    depth: usize,
    text_depth: Option<usize>,
    parent: Option<Box<Output>>,
    blocks: Option<Vec<super::FBlock>>,
    rect: [i32; 4],
    float: Float,
    behind: bool,
    kind: ShapeKind,
    look: BoxLook,
    name: String,
    value: String,
}

impl Reader {
    pub(super) fn shape_word(&mut self, w: &str, p: Option<i32>, starred: bool) -> bool {
        if w == "shpinst" && self.st.dest == Dest::Body {
            if self.shapes.len() >= 32 {
                self.st.dest = Dest::Skip;
                return true;
            }
            self.shapes.push(Shape {
                // The enclosing `shp` owns both instructions and the fallback result.
                depth: self.depth().saturating_sub(1),
                text_depth: None,
                parent: None,
                blocks: None,
                rect: [0; 4],
                float: Float { wrap: Wrap::InFrontOfText, v_rel: Anchor::Paragraph, ..Default::default() },
                behind: false,
                kind: ShapeKind::TextBox,
                look: BoxLook { inset: Some([7.2, 3.6, 7.2, 3.6]), ..Default::default() },
                name: String::new(),
                value: String::new(),
            });
            self.st.dest = Dest::ShapeInst;
            return true;
        }
        if w == "shprslt" && self.shapes.last().is_some_and(|s| s.blocks.is_some()) {
            self.st.dest = Dest::Skip;
            return true;
        }
        if matches!(self.st.dest, Dest::ShapeName | Dest::ShapeValue) {
            return true;
        }
        if self.st.dest != Dest::ShapeInst {
            return false;
        }
        if w == "shptxt" {
            let parent = self.swap_output(Output::default());
            let depth = self.depth();
            if let Some(s) = self.shapes.last_mut() {
                s.parent = Some(Box::new(parent));
                s.text_depth = Some(depth);
            }
            self.st.dest = Dest::Body;
            self.st.intbl = false;
            self.st.field = None;
            return true;
        }
        let Some(s) = self.shapes.last_mut() else { return true };
        let v = p.unwrap_or(0);
        match w {
            "shpleft" => s.rect[0] = v,
            "shptop" => s.rect[1] = v,
            "shpright" => s.rect[2] = v,
            "shpbottom" => s.rect[3] = v,
            "shpbxpage" => s.float.h_rel = Anchor::Page,
            "shpbxmargin" => s.float.h_rel = Anchor::Margin,
            "shpbxcolumn" => s.float.h_rel = Anchor::Column,
            "shpbypage" => s.float.v_rel = Anchor::Page,
            "shpbymargin" => s.float.v_rel = Anchor::Margin,
            "shpbypara" => s.float.v_rel = Anchor::Paragraph,
            "shpfblwtxt" => s.behind = v != 0,
            "shpz" => s.float.z = v.saturating_add(1).max(0) as u32,
            "shpwr" => {
                s.float.wrap = match v {
                    1 => Wrap::TopAndBottom,
                    2 => Wrap::Square,
                    4 => Wrap::Tight,
                    5 => Wrap::Through,
                    _ => Wrap::InFrontOfText,
                }
            }
            "sp" => {
                s.name.clear();
                s.value.clear();
            }
            "sn" => self.st.dest = Dest::ShapeName,
            "sv" => self.st.dest = Dest::ShapeValue,
            _ => {
                if starred {
                    self.st.dest = Dest::Skip;
                }
            }
        }
        true
    }

    pub(super) fn shape_property_text(&mut self, text: &str) {
        if let Some(s) = self.shapes.last_mut() {
            let target = if self.st.dest == Dest::ShapeName { &mut s.name } else { &mut s.value };
            if target.len().saturating_add(text.len()) <= 256 {
                target.push_str(text);
            }
        }
    }

    /// Whether a shape's text is being read (its group is still open).
    pub(super) fn in_shape_text(&self) -> bool {
        self.shapes.iter().any(|s| s.parent.is_some())
    }

    pub(super) fn close_shape_group(&mut self) {
        let depth = self.depth();
        if self.st.dest == Dest::ShapeValue
            && let Some(s) = self.shapes.last_mut()
            && let Ok(v) = s.value.trim().parse::<i32>()
        {
            match s.name.trim() {
                "shapeType" => s.kind = if v == 202 { ShapeKind::TextBox } else { ShapeKind::Rectangle },
                "fFilled" if v == 0 => s.look.fill = None,
                "fLine" if v == 0 => s.look.line = None,
                name => {
                    let index = match name {
                        "dxTextLeft" => Some(0),
                        "dyTextTop" => Some(1),
                        "dxTextRight" => Some(2),
                        "dyTextBottom" => Some(3),
                        _ => None,
                    };
                    if let Some(slot) = index.and_then(|i| s.look.inset.as_mut().and_then(|a| a.get_mut(i))) {
                        *slot = (v as f32 / 12700.0).clamp(0.0, 1584.0);
                    }
                }
            }
        }
        if self.shapes.last().is_some_and(|s| s.text_depth == Some(depth)) {
            if self.para_has || !self.para.inlines.is_empty() {
                self.end_para(false);
            }
            self.close_table();
            let parent = self.shapes.last_mut().and_then(|s| {
                s.text_depth = None;
                s.parent.take()
            });
            if let Some(parent) = parent {
                let output = self.swap_output(*parent);
                if let Some(s) = self.shapes.last_mut() {
                    s.blocks = Some(output.body);
                }
            }
        }
        if self.shapes.last().is_some_and(|s| s.depth == depth)
            && let Some(mut s) = self.shapes.pop()
            && let Some(blocks) = s.blocks
        {
            let [left, top, right, bottom] = s.rect;
            s.float.x = left as f32 / 20.0;
            s.float.y = top as f32 / 20.0;
            if s.behind && s.float.wrap == Wrap::InFrontOfText {
                s.float.wrap = Wrap::BehindText;
            }
            self.para.inlines.push(Inline::Shape {
                kind: s.kind,
                w: (i64::from(right) - i64::from(left)).clamp(0, 31680) as f32 / 20.0,
                h: (i64::from(bottom) - i64::from(top)).clamp(0, 31680) as f32 / 20.0,
                float: Some(Box::new(s.float)),
                blocks,
                look: s.look,
                path: None,
            });
            self.para_has = true;
        }
    }
}
