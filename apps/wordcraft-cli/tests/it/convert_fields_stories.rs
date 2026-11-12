//! `convert --update-fields` refreshes the fields of every story (headers, footers, footnotes and
//! endnotes), not only the body's, as Word's own export shows them; locked fields and protected
//! documents keep their stored results.
use std::process::Command;
use std::sync::Arc;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::{Block, CharProps, Document, Paragraph, PartKind, Path, Pos, SectionProps, StoryRef};

fn author_field(locked: bool) -> InlineObject {
    InlineObject::Field { instr: "AUTHOR".into(), result: "stale".into(), locked, dirty: false, code: false, form: String::new() }
}

/// A new part of `kind` holding one paragraph with an `AUTHOR` field per entry of `locked`.
fn part_with_fields(doc: &mut Document, kind: PartKind, locked: &[bool]) -> u32 {
    let mut p = Paragraph::new();
    for l in locked {
        let off = p.len();
        p.insert_object(off, author_field(*l), &CharProps::default()).unwrap();
    }
    doc.add_part(kind, vec![Arc::new(Block::Para(p))])
}

/// Two sections, author "Ada", every field stored as `stale`:
/// body `AUTHOR`; section 1 default header (`AUTHOR`, locked `AUTHOR`), default footer,
/// first-page header, even-page footer; section 2 default header; a footnote and an endnote.
fn document() -> Document {
    let mut doc = Document::from_text("First section\nSecond section");
    doc.core.creator = "Ada".into();
    doc.settings.even_odd_headers = true;
    let mut first = SectionProps { title_page: true, ..SectionProps::default() };
    first.headers.default = Some(part_with_fields(&mut doc, PartKind::Header, &[false, true]));
    first.footers.default = Some(part_with_fields(&mut doc, PartKind::Footer, &[false]));
    first.headers.first = Some(part_with_fields(&mut doc, PartKind::Header, &[false]));
    first.footers.even = Some(part_with_fields(&mut doc, PartKind::Footer, &[false]));
    doc.para_mut(StoryRef::Body, &Path::top(0)).unwrap().section = Some(Box::new(first));
    doc.last_section.headers.default = Some(part_with_fields(&mut doc, PartKind::Header, &[false]));
    let footnote = part_with_fields(&mut doc, PartKind::Footnote, &[false]);
    let endnote = part_with_fields(&mut doc, PartKind::Endnote, &[false]);
    let chr = CharProps::default();
    doc.insert_object(&Pos::body(1, 0), InlineObject::NoteRef { kind: NoteKind::Endnote, id: endnote, custom: String::new() }, &chr).unwrap();
    doc.insert_object(&Pos::body(0, 0), InlineObject::NoteRef { kind: NoteKind::Footnote, id: footnote, custom: String::new() }, &chr).unwrap();
    doc.insert_object(&Pos::body(0, 0), author_field(false), &chr).unwrap();
    doc
}

/// The stored results of a story's fields, in document order.
fn results(doc: &Document, story: StoryRef) -> Vec<String> {
    let mut out = Vec::new();
    for path in doc.para_paths(story) {
        for o in &doc.para(story, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                out.push(result.clone());
            }
        }
    }
    out
}

/// The results of every part of `kind`, in part order.
fn kind_results(doc: &Document, kind: PartKind) -> Vec<String> {
    doc.parts.iter().filter(|(_, p)| p.kind == kind).flat_map(|(id, _)| results(doc, StoryRef::Part(*id))).collect()
}

/// (story name, results) for each story of the test document as it reads after a conversion.
fn stories(doc: &Document) -> Vec<(&'static str, Vec<String>)> {
    let sections = doc.sections();
    assert_eq!(sections.len(), 2, "the two sections survive the conversion");
    let (first, second) = (sections[0].1, sections[1].1);
    let part = |id: Option<u32>, what: &str| results(doc, StoryRef::Part(id.unwrap_or_else(|| panic!("{what} is missing"))));
    vec![
        ("body", results(doc, StoryRef::Body)),
        ("section 1 header", part(first.headers.default, "section 1 header")),
        ("section 1 footer", part(first.footers.default, "section 1 footer")),
        ("section 1 first-page header", part(first.headers.first, "section 1 first-page header")),
        ("section 1 even-page footer", part(first.footers.even, "section 1 even-page footer")),
        ("section 2 header", part(second.headers.default, "section 2 header")),
        ("footnote", kind_results(doc, PartKind::Footnote)),
        ("endnote", kind_results(doc, PartKind::Endnote)),
    ]
}

fn expected(fresh: &str) -> Vec<(&'static str, Vec<String>)> {
    let one = || vec![fresh.to_string()];
    vec![
        ("body", one()),
        // The second field is locked: it keeps its stored result.
        ("section 1 header", vec![fresh.to_string(), "stale".to_string()]),
        ("section 1 footer", one()),
        ("section 1 first-page header", one()),
        ("section 1 even-page footer", one()),
        ("section 2 header", one()),
        ("footnote", one()),
        ("endnote", one()),
    ]
}

/// Saves `doc` as .docx with the CLI, converts that to JSON (with `--update-fields` when
/// `refresh`), and returns the converted document and the conversion's stderr.
fn convert(doc: &Document, name: &str, refresh: bool) -> (Document, String) {
    let dir = std::env::temp_dir().join(format!("wc-convert-fields-stories-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.json");
    std::fs::write(&input, serde_json::to_vec(doc).unwrap()).unwrap();
    let docx = dir.join("in.docx");
    let make = Command::new(env!("CARGO_BIN_EXE_wordcraft-cli")).arg("convert").arg(&input).arg(&docx).output().unwrap();
    assert!(make.status.success(), "{}", String::from_utf8_lossy(&make.stderr));
    let output = dir.join("out.json");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_wordcraft-cli"));
    cmd.arg("convert").arg(&docx).arg(&output);
    if refresh {
        cmd.arg("--update-fields");
    }
    let run = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(run.status.success(), "{stderr}");
    let out: Document = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    (out, stderr)
}

/// Ordinary conversion keeps every story's cached results (the fixture itself survives .docx).
#[test]
fn convert_without_the_flag_keeps_every_story_cached() {
    let (out, _) = convert(&document(), "cached", false);
    assert_eq!(stories(&out), expected("stale"));
}

/// Headers and footers: default, first-page and even-page, in both sections; a locked field stays.
#[test]
fn update_fields_refreshes_headers_and_footers() {
    let (out, _) = convert(&document(), "hf", true);
    let (got, want) = (stories(&out), expected("Ada"));
    assert_eq!(got[..6], want[..6]);
}

/// Footnote and endnote text.
#[test]
fn update_fields_refreshes_footnotes_and_endnotes() {
    let (out, _) = convert(&document(), "notes", true);
    let (got, want) = (stories(&out), expected("Ada"));
    assert_eq!(got[6..], want[6..]);
}

/// A protected document: nothing is updated in any story, with the usual warning.
#[test]
fn update_fields_leaves_every_story_of_a_protected_document() {
    let mut doc = document();
    doc.settings.protection = Some("readOnly".into());
    let (out, stderr) = convert(&doc, "protected", true);
    assert!(stderr.contains("warning:"), "{stderr}");
    assert_eq!(stories(&out), expected("stale"));
}
