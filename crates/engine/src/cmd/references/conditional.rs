//! IF comparisons (ECMA-376 Part 1 §17.16.5.27), shared by Update Field and output.
use wordcraft_doc::para::CodeParas;

use super::properties::text_result;

/// Keep unresolved brace-form fields alone; imported nested fields supply their stored text.
pub(super) fn result(instr: &str, code: Option<&CodeParas>) -> Option<String> {
    // Bound token copies and wildcard compilation for imported instructions.
    if instr.len() > 65_536 || instr.contains(['{', '}']) {
        return None;
    }
    let words = super::field_words(instr);
    let args: Vec<_> = words.iter().skip(1).take_while(|(w, q)| *q || !w.starts_with('\\')).collect();
    let op = args.get(1).map(|(w, _)| w.as_str()).unwrap_or("");
    if !matches!(op, "=" | "<>" | "<" | ">" | "<=" | ">=") {
        return Some("Error! Unknown op code for conditional.".into());
    }
    let (left, lq) = args.first().copied()?;
    let (right, rq) = args.get(2).copied()?;
    let numeric = |s: &str| s.replace(',', "").parse::<f64>().ok().filter(|n| n.is_finite());
    let numbers = if !lq && !rq { numeric(left).zip(numeric(right)) } else { None };
    use std::cmp::Ordering::{Greater, Less};
    let ord = numbers.and_then(|(a, b)| a.partial_cmp(&b)).unwrap_or_else(|| left.cmp(right));
    let equal = || numbers.map_or_else(|| wildcard(left, right), |(a, b)| a == b);
    let holds = match op {
        "=" => equal(),
        "<>" => !equal(),
        "<" => ord == Less,
        ">" => ord == Greater,
        "<=" => ord != Greater,
        ">=" => ord != Less,
        _ => false,
    };
    let chosen = if holds { 4 } else { 5 };
    if let Some(code) = code.filter(|c| c.code == wordcraft_doc::para::code_hash(instr)) {
        for nested in &code.nested {
            // Appending a word identifies the token at the offset even inside quoted text.
            let prefix = instr.get(..nested.at)?;
            let index = super::field_words(&format!("{prefix}x")).len().saturating_sub(1);
            if index == chosen
                || (index <= 3 && matches!(wordcraft_layout::fields::field_name(&nested.instr).as_str(), "PAGE" | "NUMPAGES" | "SECTIONPAGES"))
            {
                return None;
            }
        }
    }
    Some(text_result(instr, args.get(chosen.saturating_sub(1)).map(|(w, _)| w.clone()).unwrap_or_default()))
}

/// Match only the right operand's `*` and `?`, with Unicode characters and no recursion.
/// Regex compilation and execution are bounded by the regex crate's existing limits.
fn wildcard(text: &str, pattern: &str) -> bool {
    let mut expression = String::from("\\A(?:");
    for part in pattern.split_inclusive(['*', '?']) {
        match part.chars().next_back() {
            Some(c @ ('*' | '?')) => {
                let literal = part.get(..part.len().saturating_sub(c.len_utf8())).unwrap_or("");
                expression.push_str(&regex::escape(literal));
                expression.push_str(if c == '*' { "(?s:.*)" } else { "(?s:.)" });
            }
            _ => expression.push_str(&regex::escape(part)),
        }
    }
    expression.push_str(")\\z");
    regex::Regex::new(&expression).is_ok_and(|re| re.is_match(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::para::{NestedCode, code_hash};

    #[test]
    fn nested_conditions_use_stored_values_but_pages_and_chosen_branch_fields_stay_cached() {
        let check = |instr: &str, nested_text: &str, nested_instr: &str, want: Option<&str>| {
            let at = instr.find(nested_text).unwrap();
            let code = CodeParas { code: code_hash(instr), nested: vec![NestedCode { at, instr: nested_instr.into() }], ..Default::default() };
            assert_eq!(result(instr, Some(&code)).as_deref(), want, "{instr}");
        };
        check("IF 3 = 2 yes no", "3", "= 1 + 1", Some("no"));
        check(r#"IF x = "x" yes no"#, "x", "DOCVARIABLE Missing", Some("yes"));
        for field in ["PAGE", "NUMPAGES", "SECTIONPAGES"] {
            check("IF 1 = 1 yes no", "1", field, None);
        }
        check("IF 5 = 5 2 no", "2", "= 1 + 1", None);
        check("IF 5 = 4 2 no", "2", "= 1 + 1", Some("no"));
        check(r#"IF 5 = 5 "a 2 b" no"#, "2", "= 1 + 1", None);
        check(r#"IF 5 = 4 "a 2 b" no"#, "2", "= 1 + 1", Some("no"));
        check("IF 5 = 4 yes 2", "2", "= 1 + 1", None);
    }

    #[test]
    fn update_field_uses_the_same_conditional_evaluator() {
        use wordcraft_doc::{CharProps, Document, InlineObject, StoryRef};
        let mut doc = Document::new();
        let path = doc.para_paths(StoryRef::Body).remove(0);
        doc.para_mut(StoryRef::Body, &path)
            .unwrap()
            .insert_object(
                0,
                InlineObject::Field {
                    instr: "IF 10 > 9 yes no".into(),
                    result: "OLD".into(),
                    locked: false,
                    dirty: false,
                    code: false,
                    form: String::new(),
                    code_paras: None,
                },
                &CharProps::default(),
            )
            .unwrap();
        let mut session = crate::Session::new(doc);
        super::super::fields::update_fields(&mut session).unwrap();
        let para = session.doc.para(StoryRef::Body, &path).unwrap();
        assert!(matches!(para.objects.first(), Some(InlineObject::Field { result, .. }) if result == "yes"));
    }
}
