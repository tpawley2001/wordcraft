//! Math AutoCorrect: the replacement list and Recognized Functions applied while typing an
//! equation (and, when the user asks for it, ordinary text).
//!
//! The built-in list is the one Microsoft 365 Word (English, US) ships, read from its
//! `OMathAutoCorrect.Entries` on 2026-11-15 (`math_autocorrect_entries.tsv`: name, code points,
//! value). The user's additions, replacements and deletions sit on top of it
//! ([`MathAutoCorrect`]). Rendering and import don't read any of this: only typing does.

use std::sync::OnceLock;

/// Word's built-in entries: one line each, `name<TAB>U+XXXX U+XXXX…<TAB>value`.
const ENTRIES_TSV: &str = include_str!("math_autocorrect_entries.tsv");

/// Word's Recognized Functions, in Word's order.
pub const WORD_FUNCTIONS: &[&str] = &[
    "acos", "acosh", "acot", "acoth", "acsc", "acsch", "arccos", "arccosh", "arccot", "arccoth", "arccsc", "arccsch", "arcsec", "arcsech", "arcsin",
    "arcsinh", "arctan", "arctanh", "arg", "asec", "asech", "asin", "asinh", "atan", "atanh", "cos", "cosh", "cot", "coth", "csc", "csch", "def",
    "deg", "det", "dim", "erf", "exp", "gcd", "hom", "inf", "ker", "lg", "lim", "ln", "log", "max", "min", "pr", "sec", "sech", "sin", "sinh", "sup",
    "tan", "tanh",
];

/// Linear-format keywords that typing turns into their operator although Word's list has no
/// entry for them (they are not shown or counted as entries): `\over` → `/`, `\eqno` → `#`,
/// `\placeholder` → `⬚`, `\quad` → an em space.
pub const KEYWORDS: &[(&str, &str)] = &[("\\over", "/"), ("\\eqno", "#"), ("\\placeholder", "\u{2B1A}"), ("\\quad", "\u{2003}")];

/// Longest entry name, in characters.
pub const MAX_NAME: usize = 31;

/// Longest entry value, in characters.
pub const MAX_VALUE: usize = 255;

/// Longest function name, in characters.
pub const MAX_FUNCTION: usize = 31;

/// The built-in entries, sorted by name: `(name, value)`.
pub fn builtin() -> &'static [(&'static str, String)] {
    static TABLE: OnceLock<Vec<(&'static str, String)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut v: Vec<(&'static str, String)> = ENTRIES_TSV
            .lines()
            .filter_map(|line| {
                let mut cols = line.split('\t');
                let name = cols.next()?;
                // The code points, not the value column: values may be white space.
                let value: String = cols
                    .next()?
                    .split(' ')
                    .filter_map(|u| u.strip_prefix("U+"))
                    .filter_map(|h| u32::from_str_radix(h, 16).ok())
                    .filter_map(char::from_u32)
                    .collect();
                (!name.is_empty() && !value.is_empty()).then_some((name, value))
            })
            .collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        v.dedup_by(|a, b| a.0 == b.0);
        v
    })
}

/// The built-in value of `name`, if Word's list has it.
pub fn builtin_value(name: &str) -> Option<&'static str> {
    let t = builtin();
    t.binary_search_by(|(n, _)| (*n).cmp(name)).ok().and_then(|i| t.get(i)).map(|(_, v)| v.as_str())
}

/// Is `name` one a user entry may have: 1–31 characters, one line, no spaces?
pub fn valid_name(name: &str) -> bool {
    let n = name.chars().count();
    (1..=MAX_NAME).contains(&n) && !name.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Is `value` one a user entry may have: 1–255 characters, one line?
pub fn valid_value(value: &str) -> bool {
    let n = value.chars().count();
    (1..=MAX_VALUE).contains(&n) && !value.contains(['\n', '\r'])
}

/// Is `name` one a recognized function may have: 1–31 ASCII letters?
pub fn valid_function(name: &str) -> bool {
    (1..=MAX_FUNCTION).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphabetic())
}

/// The Math AutoCorrect options in effect: Word's built-in list and functions with the user's
/// changes.
#[derive(Clone, Debug, PartialEq)]
pub struct MathAutoCorrect {
    /// Replace text as you type.
    pub replace_text: bool,
    /// The user's entries: added, or replacing a built-in one.
    pub entries: Vec<(String, String)>,
    /// Built-in entries the user deleted.
    pub removed: Vec<String>,
    /// Function names the user added.
    pub functions_added: Vec<String>,
    /// Built-in function names the user deleted.
    pub functions_removed: Vec<String>,
}

impl Default for MathAutoCorrect {
    fn default() -> Self {
        MathAutoCorrect { replace_text: true, entries: Vec::new(), removed: Vec::new(), functions_added: Vec::new(), functions_removed: Vec::new() }
    }
}

impl MathAutoCorrect {
    /// The value `name` is replaced with (the user's entry, else a built-in one not deleted),
    /// whether or not replacing is on.
    pub fn value(&self, name: &str) -> Option<&str> {
        if let Some((_, v)) = self.entries.iter().find(|(n, _)| n == name) {
            return Some(v);
        }
        if self.removed.iter().any(|r| r == name) {
            return None;
        }
        builtin_value(name)
    }

    /// What typing replaces `name` with: [`Self::value`] or a linear-format keyword, nothing
    /// when replacing is off.
    pub fn replacement(&self, name: &str) -> Option<&str> {
        if !self.replace_text {
            return None;
        }
        self.value(name).or_else(|| KEYWORDS.iter().find(|(k, _)| *k == name).map(|(_, v)| *v))
    }

    /// The longest entry name not starting with `\` that `text` ends with, and its replacement.
    pub fn operator_suffix(&self, text: &str) -> Option<(usize, &str)> {
        if !self.replace_text {
            return None;
        }
        let mut best: Option<(usize, &str)> = None;
        let names = builtin().iter().map(|(n, _)| *n).chain(self.entries.iter().map(|(n, _)| n.as_str()));
        for name in names.filter(|n| !n.starts_with('\\') && text.ends_with(*n)) {
            let len = name.chars().count();
            if best.is_none_or(|(l, _)| len > l)
                && let Some(v) = self.value(name)
            {
                best = Some((len, v));
            }
        }
        best
    }

    /// The entry list as the dialog shows it, sorted by name: `(name, value, built in)`.
    pub fn list(&self) -> Vec<(String, String, bool)> {
        let mut v: Vec<(String, String, bool)> = builtin()
            .iter()
            .filter(|(n, _)| !self.removed.iter().any(|r| r == n) && !self.entries.iter().any(|(e, _)| e == n))
            .map(|(n, v)| (n.to_string(), v.clone(), true))
            .chain(self.entries.iter().map(|(n, v)| (n.clone(), v.clone(), builtin_value(n).is_some())))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// How many entries are in effect.
    pub fn count(&self) -> usize {
        builtin().iter().filter(|(n, _)| !self.removed.iter().any(|r| r == n) && !self.entries.iter().any(|(e, _)| e == n)).count()
            + self.entries.len()
    }

    /// The recognized function names: Word's not deleted, in Word's order, then the user's.
    pub fn functions(&self) -> Vec<&str> {
        WORD_FUNCTIONS
            .iter()
            .copied()
            .filter(|f| !self.functions_removed.iter().any(|r| r == f))
            .chain(self.functions_added.iter().map(String::as_str).filter(|f| !WORD_FUNCTIONS.contains(f)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_list_is_words() {
        assert_eq!(builtin().len(), 780);
        assert_eq!(builtin_value("\\alpha"), Some("\u{3B1}"));
        assert_eq!(builtin_value("\\doubleA"), Some("\u{1D538}"));
        assert_eq!(builtin_value("\\thinsp"), Some("\u{2006}"));
        assert_eq!(builtin_value("\\j"), Some("Jay"));
        assert_eq!(builtin_value("\\lbrace"), Some("\\{"));
        assert_eq!(builtin_value("->"), Some("\u{2192}"));
        assert_eq!(builtin_value("\\over"), None);
        assert_eq!(builtin().iter().filter(|(n, _)| !n.starts_with('\\')).count(), 12);
        assert_eq!(WORD_FUNCTIONS.len(), 55);
    }

    #[test]
    fn user_changes_apply() {
        let mut m = MathAutoCorrect::default();
        assert_eq!(m.count(), 780);
        m.entries.push(("\\alpha".into(), "A".into()));
        m.entries.push(("\\RR".into(), "R".into()));
        m.removed.push("\\beta".into());
        assert_eq!(m.replacement("\\alpha"), Some("A"));
        assert_eq!(m.replacement("\\beta"), None);
        assert_eq!(m.replacement("\\over"), Some("/"));
        assert_eq!(m.count(), 780);
        assert_eq!(m.operator_suffix("x<="), Some((2, "\u{2264}")));
        assert_eq!(m.operator_suffix("x<<"), Some((2, "\u{226A}")));
        m.replace_text = false;
        assert_eq!(m.replacement("\\alpha"), None);
        assert_eq!(m.operator_suffix("x<="), None);
    }

    #[test]
    fn functions_add_and_delete() {
        let m = MathAutoCorrect { functions_added: vec!["foo".into()], functions_removed: vec!["sin".into()], ..MathAutoCorrect::default() };
        let f = m.functions();
        assert_eq!(f.len(), 55);
        assert_eq!(f.last(), Some(&"foo"));
        assert!(!f.contains(&"sin"));
        assert!(valid_function("foo") && !valid_function("f1") && !valid_function(""));
    }
}
