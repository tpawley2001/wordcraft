//! The latent-style table (RTF 1.9.1, "Style and Formatting Restrictions": `{\*\latentstyles …}`):
//! for each built-in style a file does not define, whether it shows in the Styles gallery, its
//! sort priority and whether it is hidden until used. The model keeps the table as the DOCX
//! `w:latentStyles` element (ECMA-376 Part 1 §17.7.4.5, `w:lsdException` §17.7.4.8), as XML; the
//! reader builds that XML from the RTF words and the writer turns it back into them.

use super::state::{MAX_LATENT_EXCEPTIONS, truncate_chars};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

/// Longest exception name kept.
const MAX_NAME: usize = 255;

/// The table's own words and their `w:latentStyles` attributes, in the order Word writes them.
const TABLE_WORDS: [(&str, &str, bool); 6] = [
    ("lsdstimax", "w:count", false),
    ("lsdlockeddef", "w:defLockedState", true),
    ("lsdsemihiddendef", "w:defSemiHidden", true),
    ("lsdunhideuseddef", "w:defUnhideWhenUsed", true),
    ("lsdqformatdef", "w:defQFormat", true),
    ("lsdprioritydef", "w:defUIPriority", false),
];

/// An exception's words and their `w:lsdException` attributes, in the order Word writes them
/// (`\lsdlocked`, always written, comes last). The flag marks an on/off word.
const EXCEPTION_WORDS: [(&str, &str, bool); 4] = [
    ("lsdsemihidden", "w:semiHidden", true),
    ("lsdunhideused", "w:unhideWhenUsed", true),
    ("lsdqformat", "w:qFormat", true),
    ("lsdpriority", "w:uiPriority", false),
];

type Attrs = Vec<(&'static str, String)>;

/// The attribute value of word `(…, attr, on_off)` with parameter `p`: none for a missing or
/// negative value; `0`/`1` for an on/off word, else the number.
fn value(on_off: bool, p: Option<i32>) -> Option<String> {
    let n = p.filter(|n| *n >= 0)?;
    Some(if on_off { String::from(if n == 0 { "0" } else { "1" }) } else { n.to_string() })
}

fn set(attrs: &mut Attrs, key: &'static str, v: String) {
    if let Some(slot) = attrs.iter_mut().find(|(k, _)| *k == key) {
        slot.1 = v;
    } else {
        attrs.push((key, v));
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn push_attrs(out: &mut String, attrs: &[(&'static str, String)]) {
    for (k, v) in attrs {
        out.push_str(&format!(" {k}=\"{}\"", xml_escape(v)));
    }
}

#[derive(Debug, Default)]
struct Table {
    root: Attrs,
    exceptions: Vec<(String, Attrs)>,
    /// The exception being read: its words so far and its name.
    cur: Attrs,
    name: String,
}

impl Table {
    fn commit(&mut self) {
        let attrs = std::mem::take(&mut self.cur);
        let mut name = std::mem::take(&mut self.name).trim().to_string();
        truncate_chars(&mut name, MAX_NAME);
        if !name.is_empty() && self.exceptions.len() < MAX_LATENT_EXCEPTIONS {
            self.exceptions.push((name, attrs));
        }
    }

    fn to_xml(&self) -> String {
        let mut out = String::from("<w:latentStyles");
        push_attrs(&mut out, &self.root);
        if self.exceptions.is_empty() {
            out.push_str("/>");
            return out;
        }
        out.push('>');
        for (name, attrs) in &self.exceptions {
            out.push_str("<w:lsdException");
            push_attrs(&mut out, &[("w:name", name.clone())]);
            push_attrs(&mut out, attrs);
            out.push_str("/>");
        }
        out.push_str("</w:latentStyles>");
        out
    }
}

/// The reader's latent-style table: the one being read and the last one read.
#[derive(Debug, Default)]
pub(super) struct Latent {
    reading: Option<Table>,
    kept: Option<String>,
}

impl Latent {
    /// `{\*\latentstyles`: a new table, which replaces any earlier one when it ends.
    pub(super) fn begin(&mut self) {
        self.reading = Some(Table::default());
    }

    /// A word on the table itself.
    pub(super) fn table_word(&mut self, word: &str, p: Option<i32>) {
        let Some(t) = self.reading.as_mut() else { return };
        if let Some((_, key, on_off)) = TABLE_WORDS.iter().find(|(w, _, _)| *w == word)
            && let Some(v) = value(*on_off, p)
        {
            set(&mut t.root, key, v);
        }
    }

    /// A word inside `{\lsdlockedexcept …}`: one of the current exception's.
    pub(super) fn exception_word(&mut self, word: &str, p: Option<i32>) {
        let Some(t) = self.reading.as_mut() else { return };
        let found = EXCEPTION_WORDS.iter().find(|(w, _, _)| *w == word).map(|(_, k, o)| (*k, *o));
        let found = found.or((word == "lsdlocked").then_some(("w:locked", true)));
        if let Some((key, on_off)) = found
            && let Some(v) = value(on_off, p)
        {
            set(&mut t.cur, key, v);
        }
    }

    /// Text inside `{\lsdlockedexcept …}`: exception names, each ended by `;`.
    pub(super) fn exception_text(&mut self, s: &str) {
        let Some(t) = self.reading.as_mut() else { return };
        for c in s.chars() {
            match c {
                ';' => t.commit(),
                '\r' | '\n' => {}
                // A little past the cap, so trimming still leaves a full name.
                c if t.name.len() < MAX_NAME * 4 + 64 => t.name.push(c),
                _ => {}
            }
        }
    }

    /// The end of a `{\lsdlockedexcept …}` group: an exception without its `;` is dropped.
    pub(super) fn end_exceptions(&mut self) {
        if let Some(t) = self.reading.as_mut() {
            t.cur.clear();
            t.name.clear();
        }
    }

    /// The end of the `{\*\latentstyles …}` group: the table is kept as `w:latentStyles` XML.
    pub(super) fn end(&mut self) {
        if let Some(t) = self.reading.take() {
            self.kept = Some(t.to_xml());
        }
    }

    /// The last table read, as `w:latentStyles` XML.
    pub(super) fn take(&mut self) -> Option<String> {
        self.kept.take()
    }
}

// ---------------------------------------------------------------------------------------------
// Export

/// The attributes of a start tag, values unescaped; none when one does not decode.
fn attrs_of(e: &BytesStart<'_>) -> Option<Vec<(String, String)>> {
    let mut v = Vec::new();
    for a in e.attributes().take(64) {
        let a = a.ok()?;
        let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let val = a.normalized_value(XmlVersion::Explicit1_0).ok()?.into_owned();
        v.push((k, val));
    }
    Some(v)
}

fn get<'a>(a: &'a [(String, String)], k: &str) -> Option<&'a str> {
    a.iter().find(|(n, _)| n == k).map(|(_, v)| v.trim())
}

/// The RTF value of attribute `k`: `0`/`1` for an on/off one, else a non-negative integer.
fn rtf_value(a: &[(String, String)], k: &str, on_off: bool) -> Option<u32> {
    let v = get(a, k)?;
    if on_off {
        match v {
            "1" | "true" | "on" => Some(1),
            "0" | "false" | "off" => Some(0),
            _ => None,
        }
    } else {
        v.parse::<u32>().ok()
    }
}

/// One exception as Word writes it: its words, `\lsdlockedN`, its name and `;`.
fn exception(a: &[(String, String)], def_locked: u32, out: &mut String) {
    let Some(name) = get(a, "w:name").filter(|n| !n.is_empty() && !n.contains(';')) else { return };
    for (word, key, on_off) in EXCEPTION_WORDS {
        if let Some(n) = rtf_value(a, key, on_off) {
            out.push_str(&format!("\\{word}{n} "));
        }
    }
    let locked = rtf_value(a, "w:locked", true).unwrap_or(def_locked);
    out.push_str(&format!("\\lsdlocked{locked} {};\n", super::export::esc(&name.replace(char::is_control, ""))));
}

/// The `{\*\latentstyles …}` group for the model's `w:latentStyles` XML, or nothing when the XML
/// is not one well-formed table.
pub(super) fn group(xml: &str) -> String {
    write_group(xml).unwrap_or_default()
}

fn write_group(xml: &str) -> Option<String> {
    let mut r = quick_xml::Reader::from_str(xml);
    let mut out = String::new();
    let mut exceptions = String::new();
    let mut count = 0usize;
    let mut def_locked = 0;
    let mut in_root = false;
    // Elements open inside the root.
    let mut depth = 0usize;
    loop {
        let ev = r.read_event().ok()?;
        let (start, empty) = match &ev {
            Event::Start(e) => (Some(e), false),
            Event::Empty(e) => (Some(e), true),
            _ => (None, false),
        };
        if !in_root {
            match (start, &ev) {
                (Some(e), _) if e.name().as_ref() == b"w:latentStyles" => {
                    let a = attrs_of(e)?;
                    out.push_str("{\\*\\latentstyles");
                    for (word, key, on_off) in TABLE_WORDS {
                        if let Some(n) = rtf_value(&a, key, on_off) {
                            out.push_str(&format!("\\{word}{n}"));
                        }
                    }
                    def_locked = rtf_value(&a, "w:defLockedState", true).unwrap_or(0);
                    if empty {
                        break;
                    }
                    in_root = true;
                }
                // Anything else before the root: not a latent-style table.
                (Some(_), _) | (None, Event::End(_) | Event::Eof) => return None,
                _ => {}
            }
            continue;
        }
        match (start, &ev) {
            (Some(e), _) => {
                if depth == 0 && e.name().as_ref() == b"w:lsdException" && count < MAX_LATENT_EXCEPTIONS {
                    count += 1;
                    exception(&attrs_of(e)?, def_locked, &mut exceptions);
                }
                if !empty {
                    depth = depth.saturating_add(1);
                }
            }
            (None, Event::End(_)) if depth > 0 => depth -= 1,
            (None, Event::End(_)) => break,
            // The root never closes.
            (None, Event::Eof) => return None,
            _ => {}
        }
    }
    if !exceptions.is_empty() {
        out.push_str("{\\lsdlockedexcept ");
        out.push_str(&exceptions);
        out.push('}');
    }
    out.push_str("}\n");
    Some(out)
}
