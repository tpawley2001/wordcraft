//! Bookmark ranges and locked fields, RTF 1.9.1: `{\*\bkmkstart name}` … `{\*\bkmkend name}`
//! cover the text between them (also across paragraph marks), and `\fldlock` keeps a field's
//! stored result. Both are read into the document and written back.

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::para::{InlineObject, OBJ};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

/// (paragraph index, byte offset) of every start and end mark of bookmark `name` in the body.
fn marks(d: &Document, name: &str) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
    let (mut starts, mut ends) = (Vec::new(), Vec::new());
    for (i, b) in d.body.iter().enumerate() {
        let Some(p) = b.as_para() else { continue };
        for (off, o) in p.objects_with_offsets() {
            match o {
                InlineObject::BookmarkStart { name: n } if n == name => starts.push((i, off)),
                InlineObject::BookmarkEnd { name: n } if n == name => ends.push((i, off)),
                _ => {}
            }
        }
    }
    (starts, ends)
}

/// The text bookmark `name` covers, paragraphs joined by `¶`, markers left out; `None` when the
/// bookmark doesn't have exactly one start and one end.
fn covered(d: &Document, name: &str) -> Option<String> {
    let (starts, ends) = marks(d, name);
    let (&[(sp, so)], &[(ep, eo)]) = (starts.as_slice(), ends.as_slice()) else { return None };
    let mut out = String::new();
    for i in sp..=ep {
        let p = d.body[i].as_para()?;
        let from = if i == sp { so } else { 0 };
        let to = if i == ep { eo } else { p.text.len() };
        out.extend(p.text.get(from..to)?.chars().filter(|c| *c != OBJ));
        if i != ep {
            out.push('¶');
        }
    }
    Some(out)
}

/// (instruction, result, locked) of every simple field in the body, in order.
fn fields(d: &Document) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    for b in &d.body {
        let Some(p) = b.as_para() else { continue };
        for (_, o) in p.objects_with_offsets() {
            if let InlineObject::Field { instr, result, locked, .. } = o {
                out.push((instr.trim().to_string(), result.clone(), *locked));
            }
        }
    }
    out
}

#[test]
fn a_bookmark_covers_the_text_between_its_marks() {
    let d = open(r"\pard\plain\f0\fs24 A {\*\bkmkstart bm1}bk{\*\bkmkend bm1} B\par");
    assert_eq!(covered(&d, "bm1").as_deref(), Some("bk"));
}

#[test]
fn a_bookmark_covers_formatted_runs() {
    let d = open(r"\pard\plain\f0\fs24 {\*\bkmkstart bm1}{\b bold} and plain{\*\bkmkend bm1} tail\par");
    assert_eq!(covered(&d, "bm1").as_deref(), Some("bold and plain"));
}

#[test]
fn a_bookmark_spans_a_paragraph_mark() {
    let d = open(r"\pard\plain\f0\fs24 {\*\bkmkstart bm1}one\par two{\*\bkmkend bm1} B\par");
    let (starts, ends) = marks(&d, "bm1");
    assert_eq!(starts.iter().map(|m| m.0).collect::<Vec<_>>(), [0]);
    assert_eq!(ends.iter().map(|m| m.0).collect::<Vec<_>>(), [1]);
    assert_eq!(covered(&d, "bm1").as_deref(), Some("one¶two"));
}

#[test]
fn an_empty_bookmark_stays_a_point() {
    let d = open(r"\pard\plain\f0\fs24 x{\*\bkmkstart p}{\*\bkmkend p}y\par");
    assert_eq!(covered(&d, "p").as_deref(), Some(""));
    let text: String = d.body[0].as_para().unwrap().text.chars().filter(|c| *c != OBJ).collect();
    assert_eq!(text, "xy");
}

#[test]
fn overlapping_bookmarks_each_keep_their_range() {
    let d = open(r"\pard\plain\f0\fs24 {\*\bkmkstart a}one {\*\bkmkstart b}two{\*\bkmkend a} three{\*\bkmkend b} four\par");
    assert_eq!(covered(&d, "a").as_deref(), Some("one two"));
    assert_eq!(covered(&d, "b").as_deref(), Some("two three"));
}

#[test]
fn column_keywords_are_not_part_of_the_name() {
    let d = open(r"\pard\plain\f0\fs24 {\*\bkmkstart bm1\bkmkcolf1\bkmkcoll3}bk{\*\bkmkend bm1} B\par");
    assert_eq!(covered(&d, "bm1").as_deref(), Some("bk"));
}

#[test]
fn a_stray_end_makes_no_bookmark_and_an_unended_start_is_a_point() {
    let d = open(r"\pard\plain\f0\fs24 x{\*\bkmkend ghost}y{\*\bkmkstart open}z\par");
    assert_eq!(marks(&d, "ghost"), (vec![], vec![]));
    assert_eq!(covered(&d, "open").as_deref(), Some(""));
    let text: String = d.body[0].as_para().unwrap().text.chars().filter(|c| *c != OBJ).collect();
    assert_eq!(text, "xyz");
}

#[test]
fn export_writes_the_end_after_the_covered_text() {
    let d = open(r"\pard\plain\f0\fs24 A {\*\bkmkstart bm1}WORD{\*\bkmkend bm1} B\par");
    let rtf = export(&d);
    let start = rtf.find(r"{\*\bkmkstart bm1}").expect("start written");
    let end = rtf.find(r"{\*\bkmkend bm1}").expect("end written");
    let text = rtf.find("WORD").expect("covered text written");
    assert!(start < text && text < end, "start {start}, text {text}, end {end} in {}", &rtf[start..]);
    assert_eq!(rtf.matches(r"\bkmkstart bm1").count(), 1);
    assert_eq!(rtf.matches(r"\bkmkend bm1").count(), 1);
}

#[test]
fn ranges_survive_export_and_import() {
    let d = open(
        r"\pard\plain\f0\fs24 {\*\bkmkstart span}one\par two{\*\bkmkend span} {\*\bkmkstart a}three {\*\bkmkstart b}four{\*\bkmkend a} five{\*\bkmkend b} x{\*\bkmkstart p}{\*\bkmkend p}y\par",
    );
    let back = import(export(&d).as_bytes()).unwrap();
    assert_eq!(covered(&back, "span").as_deref(), Some("one¶two"));
    assert_eq!(covered(&back, "a").as_deref(), Some("three four"));
    assert_eq!(covered(&back, "b").as_deref(), Some("four five"));
    assert_eq!(covered(&back, "p").as_deref(), Some(""));
}

#[test]
fn a_point_bookmark_is_written_with_both_marks_together() {
    let d = open(r"\pard\plain\f0\fs24 x{\*\bkmkstart p}{\*\bkmkend p}y\par");
    assert!(export(&d).contains(r"{\*\bkmkstart p}{\*\bkmkend p}"));
}

#[test]
fn fldlock_is_read_and_keeps_the_stored_result() {
    let d = open(
        r#"\pard\plain\f0\fs24 {\field\fldlock{\*\fldinst DATE \\@ "yyyy"}{\fldrslt 2020}} and {\field{\*\fldinst AUTHOR}{\fldrslt Ann}}\par"#,
    );
    let f = fields(&d);
    assert_eq!(f.len(), 2, "{f:?}");
    assert_eq!(f[0], (r#"DATE \@ "yyyy""#.to_string(), "2020".to_string(), true));
    assert_eq!((f[1].0.as_str(), f[1].2), ("AUTHOR", false));
}

#[test]
fn the_lock_ends_with_its_field() {
    let d = open(
        r"\pard\plain\f0\fs24 {\field\fldlock{\*\fldinst AUTHOR}{\fldrslt Ann}}{\field{\*\fldinst TITLE}{\fldrslt T}}\par {\field{\*\fldinst SUBJECT}{\fldrslt S}}\par",
    );
    assert_eq!(fields(&d).iter().map(|f| f.2).collect::<Vec<_>>(), [true, false, false]);
}

#[test]
fn fldlock_is_written_for_locked_fields_only() {
    let d = open(r"\pard\plain\f0\fs24 {\field\fldlock{\*\fldinst AUTHOR}{\fldrslt Ann}} {\field{\*\fldinst TITLE}{\fldrslt T}}\par");
    let rtf = export(&d);
    assert_eq!(rtf.matches(r"\fldlock").count(), 1, "{rtf}");
    let lock = rtf.find(r"{\field\fldlock{\*\fldinst").expect("the lock directly after \\field");
    assert!(rtf[lock..].find("AUTHOR").unwrap() < rtf[lock..].find("TITLE").unwrap());
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!(fields(&back).iter().map(|f| (f.0.clone(), f.2)).collect::<Vec<_>>(), [("AUTHOR".to_string(), true), ("TITLE".to_string(), false)]);
}
