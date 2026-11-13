//! Output (PDF export, printing, page images) shows FILENAME, USERNAME, USERINITIALS, INFO and
//! DOCVARIABLE fields in headers and footers as they are now, and keeps the body's as stored.
//! Expected results read from Microsoft 365 Word's PDFs of probe documents saved as
//! `stale-hdr.docx` and `stale-ftr.docx`: core title `Real Title`, every field stored as `STALE`.
//! Word showed the file name, the full path, the user's name and initials, `Real Title`, and an
//! empty result for a DOCVARIABLE naming no variable (not the error text Update Field gives).

use std::sync::Arc;

use wordcraft_doc::{Block, CharProps, Document, InlineObject, Paragraph, PartKind, StoryRef};
use wordcraft_doc::DocVar;

use crate::cmd::references::{OutputContext, output_fields, output_fields_in};

fn field(instr: &str, result: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked, dirty: false, code: false, form: String::new(), code_paras: None }
}

/// A paragraph holding `codes` as fields, each stored as `STALE`.
fn para(codes: &[&str], locked: bool) -> Paragraph {
    let mut p = Paragraph::new();
    for code in codes {
        let off = p.len();
        p.insert_object(off, field(code, "STALE", locked), &CharProps::default()).unwrap();
    }
    p
}

/// A new part of `kind` holding one paragraph with `codes`.
fn part(doc: &mut Document, kind: PartKind, codes: &[&str], locked: bool) -> StoryRef {
    StoryRef::Part(doc.add_part(kind, vec![Arc::new(Block::Para(para(codes, locked)))]))
}

/// The results of the fields in `story`, in order.
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

fn probe() -> Document {
    let mut doc = Document::from_text("marked text");
    doc.core.title = "Real Title".into();
    doc
}

fn ctx() -> OutputContext {
    OutputContext {
        file_name: "stale-hdr.docx".into(),
        file_path: "/home/user/docs/stale-hdr.docx".into(),
        user_name: "Ada Lovelace".into(),
        user_initials: "AL".into(),
    }
}

const CODES: [&str; 6] = ["FILENAME", "FILENAME \\p", "USERNAME", "USERINITIALS", "INFO Title", "DOCVARIABLE v"];
const FRESH: [&str; 6] = ["stale-hdr.docx", "/home/user/docs/stale-hdr.docx", "Ada Lovelace", "AL", "Real Title", ""];

/// Word's PDFs of the header probe and of the footer probe.
#[test]
fn output_header_footer_session_fields_refresh() {
    for kind in [PartKind::Header, PartKind::Footer] {
        let mut doc = probe();
        let story = part(&mut doc, kind, &CODES, false);
        let out = output_fields_in(&doc, Some(&ctx())).expect("the stale results change");
        assert_eq!(results(&out, story), FRESH, "{kind:?}");
        // The document itself keeps what it stored.
        assert_eq!(results(&doc, story), ["STALE"; 6]);
    }
}

/// A defined document variable shows its value (unverified in Word: same rule as Update Field).
#[test]
fn output_header_footer_session_defined_doc_variable() {
    let mut doc = probe();
    doc.settings.doc_vars.push(DocVar { name: "Client".into(), value: "Acme Ltd".into() });
    let story = part(&mut doc, PartKind::Header, &["DOCVARIABLE Client", "DOCVARIABLE client"], false);
    let out = output_fields_in(&doc, Some(&ctx())).expect("the variables show");
    assert_eq!(results(&out, story), ["Acme Ltd", "Acme Ltd"]);
}

/// Word's PDF of the body probe: the same fields in the body keep `STALE`.
#[test]
fn output_header_footer_session_body_keeps_stored() {
    let mut doc = probe();
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let body = doc.para_mut(StoryRef::Body, &path).unwrap();
    for code in CODES {
        let off = body.len();
        body.insert_object(off, field(code, "STALE", false), &CharProps::default()).unwrap();
    }
    assert!(output_fields_in(&doc, Some(&ctx())).is_none());
}

/// Locked fields, quoted USERNAME / USERINITIALS texts and other INFO names keep their results;
/// without a context (no session) nothing session-bound changes.
#[test]
fn output_header_footer_session_kept_results() {
    let mut doc = probe();
    let story = part(&mut doc, PartKind::Footer, &CODES, true);
    assert!(output_fields_in(&doc, Some(&ctx())).is_none(), "locked fields keep their results");
    let mut doc = probe();
    let story2 = part(&mut doc, PartKind::Footer, &["USERNAME \"Someone\"", "USERINITIALS \"SO\"", "INFO FileSize"], false);
    assert!(output_fields_in(&doc, Some(&ctx())).is_none(), "{:?}", results(&doc, story2));
    let mut doc = probe();
    let story3 = part(&mut doc, PartKind::Header, &CODES, false);
    // INFO Title is a summary property, which output refreshes without a session too.
    let out = output_fields(&doc).expect("INFO Title changes");
    assert_eq!(results(&out, story3), ["STALE", "STALE", "STALE", "STALE", "Real Title", "STALE"]);
    let out2 = output_fields_in(&doc, None).expect("INFO Title changes");
    assert_eq!(results(&out2, story3), results(&out, story3));
    let _ = story;
}
