//! Edit Wrap Points (`arrange.wrapPoints`): reads and edits the outline that Tight and Through
//! wrapping follow around the selected object (`Float::wrap_polygon`, ECMA-376 Part 1
//! §20.4.2.16). Points are in the wrap space: 0..21600 spans the object's width or height.

use serde_json::{Value, json};
use wordcraft_doc::para::{InlineObject, Wrap};
use wordcraft_doc::wrap::{MAX_WRAP_POINTS, WRAP_SPACE, WrapPolygon, shape_outline};

use super::select::{selected, with_float};
use crate::{CmdError, CmdResult, Session, p};

/// Fewest points an outline keeps.
const MIN_POINTS: usize = 3;

/// The outline Tight wrapping follows around `o`: its stored polygon, else one derived from a
/// shape's geometry, else its rectangle. (A picture's opaque pixels are worked out by layout
/// only; here a picture starts from its rectangle.)
fn outline(o: &InlineObject) -> (Vec<[i32; 2]>, bool) {
    if let Some(poly) = o.frame().and_then(|(_, _, f)| f.wrap_polygon.as_ref()) {
        return (poly.points.iter().take(MAX_WRAP_POINTS).copied().collect(), poly.edited);
    }
    let derived = match o {
        InlineObject::Shape { kind, extra, w, h, .. } => {
            shape_outline(*kind, *w, *h, extra.corner).and_then(|pts| WrapPolygon::from_unit(false, &pts))
        }
        _ => None,
    };
    let points = derived.map(|p| p.points).unwrap_or_else(|| vec![[0, 0], [WRAP_SPACE, 0], [WRAP_SPACE, WRAP_SPACE], [0, WRAP_SPACE]]);
    (points, false)
}

/// What `get` (and every edit) returns: the wrapping style and the current outline.
fn report(o: &InlineObject) -> Value {
    let wrap = o.frame().map(|(_, _, f)| f.wrap).unwrap_or_default();
    let (points, edited) = outline(o);
    json!({"wrap": serde_json::to_value(wrap).unwrap_or(Value::Null), "points": points, "edited": edited})
}

fn coord(v: Option<&Value>, what: &str) -> Result<i64, CmdError> {
    v.and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .map(|n| n.round() as i64)
        .ok_or_else(|| CmdError::Params(format!("{what}: a number in the wrap space (0..{WRAP_SPACE}) is required")))
}

fn index(v: &Value, k: &str, len: usize) -> Result<usize, CmdError> {
    let i = p::u64(v, k).ok_or_else(|| CmdError::Params(format!("{k}: a point index is required")))?;
    usize::try_from(i).ok().filter(|i| *i < len).ok_or_else(|| CmdError::Params(format!("{k} {i} is out of range (the outline has {len} points)")))
}

fn point(v: &Value) -> Result<(i64, i64), CmdError> {
    Ok((coord(v.get("x"), "x")?, coord(v.get("y"), "y")?))
}

/// `arrange.wrapPoints`: see the command's params doc.
pub(super) fn wrap_points(s: &mut Session, v: &Value) -> CmdResult {
    let (_, o) = selected(s).ok_or_else(|| CmdError::Disabled("no picture or shape selected".into()))?;
    if o.frame().is_none() {
        return Err(CmdError::Disabled("this object has no wrapping".into()));
    }
    let op = p::str(v, "op").unwrap_or("get");
    let current = || outline(&o).0.into_iter().map(|[x, y]| (i64::from(x), i64::from(y))).collect::<Vec<_>>();
    let points = match op {
        "get" => return Ok(report(&o)),
        "reset" => None,
        "set" => {
            let list = v.get("points").and_then(Value::as_array).ok_or_else(|| CmdError::Params("points: an array of [x, y] is required".into()))?;
            if list.len() > MAX_WRAP_POINTS {
                return Err(CmdError::Params(format!("an outline has at most {MAX_WRAP_POINTS} points")));
            }
            let pts = list
                .iter()
                .map(|pt| {
                    let pair = pt.as_array().filter(|a| a.len() == 2).ok_or_else(|| CmdError::Params("each point is [x, y]".into()))?;
                    Ok((coord(pair.first(), "x")?, coord(pair.get(1), "y")?))
                })
                .collect::<Result<Vec<_>, CmdError>>()?;
            Some(pts)
        }
        "move" => {
            let mut pts = current();
            let i = index(v, "index", pts.len())?;
            let xy = point(v)?;
            if let Some(slot) = pts.get_mut(i) {
                *slot = xy;
            }
            Some(pts)
        }
        "add" => {
            let mut pts = current();
            let i = index(v, "after", pts.len())?;
            if pts.len() >= MAX_WRAP_POINTS {
                return Err(CmdError::Params(format!("an outline has at most {MAX_WRAP_POINTS} points")));
            }
            let xy = point(v)?;
            pts.insert(i.saturating_add(1).min(pts.len()), xy);
            Some(pts)
        }
        "remove" => {
            let mut pts = current();
            let i = index(v, "index", pts.len())?;
            if pts.len() <= MIN_POINTS {
                return Err(CmdError::Params(format!("an outline keeps at least {MIN_POINTS} points")));
            }
            pts.remove(i);
            Some(pts)
        }
        other => return Err(CmdError::Params(format!("unknown op {other:?} (get|set|move|add|remove|reset)"))),
    };
    let poly = match points {
        Some(pts) => Some(std::sync::Arc::new(
            WrapPolygon::new(true, pts).ok_or_else(|| CmdError::Params(format!("an outline keeps at least {MIN_POINTS} points")))?,
        )),
        None => None,
    };
    let reset = poly.is_none();
    with_float(s, |f| {
        // Editing the points of an object that isn't wrapped Tight or Through makes it Tight.
        if !reset && !matches!(f.wrap, Wrap::Tight | Wrap::Through) {
            f.wrap = Wrap::Tight;
        }
        f.wrap_polygon = poly.clone();
    })?;
    let (_, o) = selected(s).ok_or_else(|| CmdError::Failed("object vanished".into()))?;
    Ok(report(&o))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_doc::para::Wrap;

    use super::super::selected;
    use crate::Session;

    fn shape(kind: &str, wrap: &str) -> Session {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.run("insert.shape", &json!({"kind": kind})).unwrap();
        s.run("arrange.wrap", &json!({"wrap": wrap})).unwrap();
        s
    }

    fn wrap(s: &Session) -> (Wrap, f32) {
        let (_, o) = selected(s).unwrap();
        let f = o.frame().unwrap().2;
        (f.wrap, f.dist)
    }

    #[test]
    fn get_derives_an_ellipse_outline_from_its_geometry() {
        let mut s = shape("ellipse", "tight");
        let v = s.run("arrange.wrapPoints", &json!({"op": "get"})).unwrap();
        assert_eq!(v["wrap"], json!("tight"));
        assert!(v["points"].as_array().unwrap().len() > 4, "{v}");
    }

    #[test]
    fn editing_an_inline_object_makes_it_tight_with_the_side_distance() {
        let mut s = shape("rectangle", "inline");
        s.run("arrange.wrapPoints", &json!({"op": "remove", "index": 3})).unwrap();
        let (w, dist) = wrap(&s);
        assert_eq!(w, Wrap::Tight);
        assert_eq!(dist, 9.0);
    }

    #[test]
    fn through_stays_through_and_junk_is_refused() {
        let mut s = shape("rectangle", "through");
        s.run("arrange.wrapPoints", &json!({"op": "move", "index": 1, "x": 20000, "y": 500})).unwrap();
        assert_eq!(wrap(&s).0, Wrap::Through);
        for bad in [
            json!({"op": "nope"}),
            json!({"op": "set", "points": [[0, 0], [1], [2, 2]]}),
            json!({"op": "set", "points": "x"}),
            json!({"op": "move", "index": 0}),
            json!({"op": "add", "after": -1, "x": 0, "y": 0}),
            json!({"op": "set", "points": vec![[0, 0]; 2000]}),
        ] {
            assert!(s.run("arrange.wrapPoints", &bad).is_err(), "{bad}");
        }
        // Far-out coordinates are clamped, not refused.
        s.run("arrange.wrapPoints", &json!({"op": "set", "points": [[0, 0], [1e300, 0], [0, -1e300]]})).unwrap();
    }
}
