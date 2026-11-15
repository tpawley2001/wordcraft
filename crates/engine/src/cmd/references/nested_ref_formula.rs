//! A formula field (`=`, ECMA-376 Part 1 §17.16.3) whose code holds nested REF fields
//! (§17.16.5.52), worked out again at output: each nested REF's stored text in the code is
//! replaced by its bookmark's current text, and the formula evaluated as Update Field would.
//! Microsoft 365 Word's PDF export prints `= { REF x } +1` as 6 when bookmark `x` holds 5, though
//! both fields stored a stale result; a formula naming the bookmark directly (`= x + 1`) keeps its
//! stored result, as plain formulas do.
use wordcraft_doc::para::{CodeParas, code_hash};
use wordcraft_doc::{Document, Path, StoryRef};

use super::FieldCode;
use super::fields::has_switch;
use super::properties::{bookmark_ends_in_its_paragraph, bookmark_text_at, text_result};

/// The result output shows for the formula `instr` at `path` in the body, or `None` when it keeps
/// its stored result: it holds no nested field (or its code was edited since it was read), a
/// nested field is not a REF or its bookmark is missing or runs on into a later paragraph, a
/// nested REF has a paragraph-number or position switch, or the formula does not evaluate.
/// Unverified in Word: those cases keep the stored result here.
pub(super) fn result(doc: &Document, path: &Path, instr: &str, code: Option<&CodeParas>) -> Option<String> {
    if instr.len() > 65_536 || !instr.trim_start().starts_with('=') {
        return None;
    }
    let code = code.filter(|c| c.code == code_hash(instr) && !c.nested.is_empty())?;
    // Lengths recorded on import, when there is one for each nested field.
    let lens = (code.nested_len.len() == code.nested.len()).then_some(code.nested_len.as_slice());
    let mut fresh = String::with_capacity(instr.len());
    let mut from = 0usize;
    for (i, nested) in code.nested.iter().enumerate() {
        if nested.at < from {
            return None;
        }
        let len = match lens.and_then(|l| l.get(i)) {
            Some(len) => *len,
            None => stored_len(instr.get(nested.at..)?),
        };
        fresh.push_str(instr.get(from..nested.at)?);
        fresh.push_str(&ref_text(doc, &nested.instr)?);
        from = nested.at.checked_add(len)?;
    }
    fresh.push_str(instr.get(from..)?);
    let lang = crate::cmd::doc_lang(doc);
    let evaluated = match path.cell().and_then(|(tp, r, c)| doc.table(StoryRef::Body, &tp).map(|t| (t, r, c))) {
        Some((t, r, c)) => crate::cmd::table_formula::evaluate_field_with(t, Some(doc), lang.as_deref(), r, c, &fresh),
        None => crate::cmd::table_formula::evaluate_text_field(doc, lang.as_deref(), &fresh),
    };
    evaluated.ok()
}

/// The text a nested REF field shows now: its bookmark's text under its `\*` switches. `None` for
/// any other field, a REF with `\p`, `\r`, `\n` or `\w`, or a bookmark that is missing or ends in
/// a later paragraph.
pub(super) fn ref_text(doc: &Document, instr: &str) -> Option<String> {
    if wordcraft_layout::fields::field_name(instr) != "REF" || ['p', 'r', 'n', 'w'].iter().any(|&sw| has_switch(instr, sw)) {
        return None;
    }
    let f = FieldCode::parse(instr);
    let target = f.args.first().map(|s| s.split('\\').next().unwrap_or("").trim_matches('"'))?;
    if target.is_empty() {
        return None;
    }
    let (_, start) = doc.bookmarks().into_iter().find(|(n, _)| n == target)?;
    if !bookmark_ends_in_its_paragraph(doc, target, &start) {
        return None;
    }
    Some(text_result(instr, bookmark_text_at(doc, target, &start)?))
}

/// The length of a nested field's stored text at the start of `code`, for a session saved before
/// the length was recorded: one operand (a sign, then up to a space, an operator or a bracket).
fn stored_len(code: &str) -> usize {
    let sign = code.chars().next().filter(|c| matches!(c, '+' | '-')).map_or(0, char::len_utf8);
    let rest = code.get(sign..).unwrap_or("");
    let operand = rest
        .char_indices()
        .find(|(_, c)| c.is_whitespace() || matches!(c, '+' | '-' | '*' | '/' | '^' | '%' | '=' | '<' | '>' | '(' | ')' | ',' | ';' | '\\'))
        .map_or(rest.len(), |(i, _)| i);
    sign.saturating_add(operand)
}

#[cfg(test)]
mod tests {
    use super::stored_len;

    #[test]
    fn an_unrecorded_length_covers_one_operand() {
        assert_eq!(stored_len("OLD +1"), 3);
        assert_eq!(stored_len("5+1"), 1);
        assert_eq!(stored_len("-2.5*3"), 4);
        assert_eq!(stored_len("+1"), 2);
        assert_eq!(stored_len(""), 0);
        assert_eq!(stored_len("é)"), 2);
    }
}
