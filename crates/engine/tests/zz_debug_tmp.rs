use serde_json::{Value, json};
use wordcraft_doc::props::NumRef;
use wordcraft_doc::{Document, Path, Pos, StoryRef};
use wordcraft_engine::{Selection, Session};

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.run(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn debug_fixture() {
    let mut s = Session::new(Document::from_text("One\nTwo\nThree\nPlain\n"));
    for i in 0..6 {
        eprintln!("text {i}: {:?}", s.doc.para(StoryRef::Body, &Path::top(i)).map(|p| p.text.clone()));
    }
    s.sel = Selection { anchor: Pos::body(0, 0), focus: Pos::body(2, 0) };
    eprintln!("sel {:?} ordered {:?}", s.sel, s.sel.ordered());
    run(&mut s, "para.numbering", json!({}));
    let num = s.doc.para(StoryRef::Body, &Path::top(0)).unwrap().props.numbering.unwrap().num;
    s.doc.para_mut(StoryRef::Body, &Path::top(1)).unwrap().props.numbering = Some(NumRef { num, level: 1 });
    s.sel = Selection::caret(Pos::body(0, 0));
    for i in 0..5 {
        let p = s.doc.para(StoryRef::Body, &Path::top(i)).unwrap();
        eprintln!("before {i}: {:?} style {:?}", p.props.numbering, p.props.style);
    }
    for i in 0..3 {
        let p = s.doc.para_mut(StoryRef::Body, &Path::top(i)).unwrap();
        p.props.indent_left = Some(100.0);
        p.props.indent_first = Some(-10.0);
    }
    run(&mut s, "list.adjustIndents", json!({"numberPosition": 0.0, "textIndent": 36.0}));
    for i in 0..5 {
        let p = s.doc.para(StoryRef::Body, &Path::top(i)).unwrap();
        eprintln!("after {i}: {:?} {:?} {:?}", p.props.numbering, p.props.indent_left, p.props.indent_first);
    }
}
