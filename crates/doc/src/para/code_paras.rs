//! Paragraph marks inside a field's code. Word lets a complex field's code run over several
//! paragraphs (ECMA-376 Part 1, §17.16.18, §17.16.23): the marks are hidden with the code, so the
//! paragraphs from the field's begin to its separator show as one. WordCraft reads them as one
//! [`super::Paragraph`] and keeps on the field what is needed to save the paragraphs as they were.

use serde::{Deserialize, Serialize};

use crate::props::{CharProps, ParaProps};

/// The paragraph marks inside a field's code.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodeParas {
    /// [`code_hash`] of the field's code: a code edited since it was read no longer matches, and
    /// the field is saved in one paragraph.
    pub code: u64,
    /// The first paragraph's own space after (the joined paragraph has the last one's).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_after: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_after_auto: Option<bool>,
    pub marks: Vec<CodeMark>,
    /// Nested fields flattened to stored text on import. Retained to avoid evaluating page
    /// conditions or replacing branch fields when preparing output.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nested: Vec<NestedCode>,
}

/// A paragraph mark inside a field's code: the paragraph that starts after it.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodeMark {
    /// Byte offset in the field's code where the next paragraph's part starts.
    pub at: usize,
    /// The next paragraph's own properties.
    pub props: ParaProps,
    /// The formatting of the next paragraph's mark.
    pub mark: CharProps,
}

/// A stable hash of a field code (64-bit FNV-1a).
pub fn code_hash(code: &str) -> u64 {
    code.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
}

impl CodeParas {
    /// The parts of `code` between the marks, when the marks still fit it (in order, on
    /// character boundaries, within the code).
    pub fn parts<'a>(&self, code: &'a str) -> Option<Vec<&'a str>> {
        if self.marks.is_empty() || code_hash(code) != self.code {
            return None;
        }
        let mut parts = Vec::with_capacity(self.marks.len() + 1);
        let mut from = 0usize;
        for m in &self.marks {
            parts.push(code.get(from..m.at)?);
            from = m.at;
        }
        parts.push(code.get(from..)?);
        Some(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(code: &str, at: &[usize]) -> CodeParas {
        CodeParas { code: code_hash(code), marks: at.iter().map(|&at| CodeMark { at, ..Default::default() }).collect(), ..Default::default() }
    }

    #[test]
    fn parts_follow_the_marks() {
        let s = split("QUOTE \"x y z\"", &[9, 11]);
        assert_eq!(s.parts("QUOTE \"x y z\""), Some(vec!["QUOTE \"x ", "y ", "z\""]));
    }

    #[test]
    fn an_edited_or_mismatched_code_has_no_parts() {
        let s = split("QUOTE \"x y z\"", &[9, 11]);
        assert_eq!(s.parts("QUOTE \"x y w\""), None);
        assert_eq!(split("ab", &[3]).parts("ab"), None);
        assert_eq!(split("ab", &[2, 1]).parts("ab"), None);
        assert_eq!(split("é", &[1]).parts("é"), None);
        assert_eq!(split("ab", &[]).parts("ab"), None);
    }
}

/// A nested field's instruction and the byte offset of its stored text in the outer code.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NestedCode {
    pub at: usize,
    pub instr: String,
}
