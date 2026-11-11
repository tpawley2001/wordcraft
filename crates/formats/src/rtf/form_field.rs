//! Legacy form fields (`FORMCHECKBOX`, `FORMDROPDOWN`, `FORMTEXT`): the `{\*\formfield …}` group
//! of a field instruction (RTF 1.9.1, "Form Fields") read into the field's `w:ffData` XML
//! (ECMA-376 Part 1 §17.16.17), which the document model keeps, and written back from it.

use wordcraft_doc::legacy_form::{MAX_ENTRIES, has_tag, is_on, tag_attr, tag_vals};

/// Longest text kept of one sub-destination (a name, default, format, help text…).
const MAX_TEXT: usize = 4096;

/// `\ffres` meaning "no result of its own: the default applies".
const NO_RESULT: i32 = 25;

/// Text types of a text form field (`\fftypetxtN`), as `w:textInput/w:type` values (§17.16.34).
const TEXT_TYPES: [&str; 6] = ["regular", "number", "date", "currentDate", "currentTime", "calculated"];

/// Which text-valued sub-destination of `\*\formfield` is being read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum FfSlot {
    Name,
    DefText,
    Format,
    HelpText,
    StatText,
    EntryMacro,
    ExitMacro,
    /// A drop-down entry (`{\*\ffl …}`): the text goes to the last entry.
    Entry,
}

impl FfSlot {
    /// The slot a starred sub-destination word opens.
    pub(super) fn for_word(word: &str) -> Option<FfSlot> {
        Some(match word {
            "ffname" => FfSlot::Name,
            "ffdeftext" => FfSlot::DefText,
            "ffformat" => FfSlot::Format,
            "ffhelptext" => FfSlot::HelpText,
            "ffstattext" => FfSlot::StatText,
            "ffentrymcr" => FfSlot::EntryMacro,
            "ffexitmcr" => FfSlot::ExitMacro,
            "ffl" => FfSlot::Entry,
            _ => return None,
        })
    }
}

/// What one `\*\formfield` group says.
#[derive(Clone, Debug, Default)]
pub(super) struct FormField {
    kind: i32,
    res: Option<i32>,
    prot: bool,
    own_help: bool,
    own_stat: bool,
    recalc: Option<bool>,
    type_txt: i32,
    max_len: i32,
    size: i32,
    hps: Option<i32>,
    def_res: Option<i32>,
    name: Option<String>,
    def_text: Option<String>,
    format: Option<String>,
    help: Option<String>,
    stat: Option<String>,
    entry_macro: Option<String>,
    exit_macro: Option<String>,
    entries: Vec<String>,
}

impl FormField {
    /// A control word inside the group (outside its sub-destinations).
    pub(super) fn word(&mut self, word: &str, param: Option<i32>) {
        let value = param.unwrap_or(0);
        let on = param != Some(0);
        match word {
            "fftype" => self.kind = value,
            "ffres" => self.res = Some(value),
            "ffprot" => self.prot = on,
            "ffownhelp" => self.own_help = on,
            "ffownstat" => self.own_stat = on,
            "ffrecalc" => self.recalc = Some(on),
            "fftypetxt" => self.type_txt = value,
            "ffmaxlen" => self.max_len = value,
            "ffsize" => self.size = value,
            "ffhps" => self.hps = Some(value),
            "ffdefres" => self.def_res = Some(value),
            _ => {}
        }
    }

    /// Starts sub-destination `slot`; `false` when it can't be kept (too many entries).
    pub(super) fn open(&mut self, slot: FfSlot) -> bool {
        if slot == FfSlot::Entry {
            if self.entries.len() >= MAX_ENTRIES {
                return false;
            }
            self.entries.push(String::new());
            return true;
        }
        self.slot(slot).get_or_insert_with(String::new);
        true
    }

    /// Text inside sub-destination `slot`.
    pub(super) fn text(&mut self, slot: FfSlot, s: &str) {
        let target = if slot == FfSlot::Entry { self.entries.last_mut() } else { self.slot(slot).as_mut() };
        if let Some(t) = target {
            for c in s.chars() {
                if t.len() >= MAX_TEXT {
                    break;
                }
                t.push(c);
            }
        }
    }

    fn slot(&mut self, slot: FfSlot) -> &mut Option<String> {
        match slot {
            FfSlot::Name | FfSlot::Entry => &mut self.name,
            FfSlot::DefText => &mut self.def_text,
            FfSlot::Format => &mut self.format,
            FfSlot::HelpText => &mut self.help,
            FfSlot::StatText => &mut self.stat,
            FfSlot::EntryMacro => &mut self.entry_macro,
            FfSlot::ExitMacro => &mut self.exit_macro,
        }
    }

    /// The field's `<w:ffData>…</w:ffData>`, children in Word's order.
    pub(super) fn to_xml(&self) -> String {
        let mut x = String::from("<w:ffData>");
        if let Some(n) = &self.name {
            val(&mut x, "w:name", n.trim());
        }
        x.push_str(if self.prot { r#"<w:enabled w:val="0"/>"# } else { "<w:enabled/>" });
        match self.recalc {
            Some(true) => x.push_str("<w:calcOnExit/>"),
            Some(false) => x.push_str(r#"<w:calcOnExit w:val="0"/>"#),
            None => {}
        }
        if let Some(m) = &self.entry_macro {
            val(&mut x, "w:entryMacro", m);
        }
        if let Some(m) = &self.exit_macro {
            val(&mut x, "w:exitMacro", m);
        }
        for (name, text, own) in [("w:helpText", &self.help, self.own_help), ("w:statusText", &self.stat, self.own_stat)] {
            if let Some(t) = text {
                let kind = if own { "text" } else { "autoText" };
                x.push_str(&format!(r#"<{name} w:type="{kind}" w:val="{}"/>"#, xml_attr(t)));
            }
        }
        match self.kind {
            1 => {
                x.push_str("<w:checkBox>");
                match self.hps {
                    Some(h) if self.size == 1 && h > 0 => val(&mut x, "w:size", &h.to_string()),
                    _ => x.push_str("<w:sizeAuto/>"),
                }
                let default = i32::from(self.def_res.is_some_and(|d| d != 0));
                val(&mut x, "w:default", &default.to_string());
                if let Some(r @ (0 | 1)) = self.res {
                    val(&mut x, "w:checked", &r.to_string());
                }
                x.push_str("</w:checkBox>");
            }
            2 => {
                x.push_str("<w:ddList>");
                if let Some(r) = self.res.filter(|r| *r != NO_RESULT && *r >= 0) {
                    val(&mut x, "w:result", &r.to_string());
                }
                if let Some(d) = self.def_res.filter(|d| *d > 0) {
                    val(&mut x, "w:default", &d.to_string());
                }
                for e in &self.entries {
                    val(&mut x, "w:listEntry", e);
                }
                x.push_str("</w:ddList>");
            }
            _ => {
                x.push_str("<w:textInput>");
                if let Some(t) = usize::try_from(self.type_txt).ok().filter(|t| *t > 0).and_then(|t| TEXT_TYPES.get(t)) {
                    val(&mut x, "w:type", t);
                }
                if let Some(d) = &self.def_text {
                    val(&mut x, "w:default", d);
                }
                if self.max_len > 0 {
                    val(&mut x, "w:maxLength", &self.max_len.to_string());
                }
                if let Some(f) = &self.format {
                    val(&mut x, "w:format", f);
                }
                x.push_str("</w:textInput>");
            }
        }
        x.push_str("</w:ffData>");
        x
    }
}

fn val(x: &mut String, name: &str, v: &str) {
    x.push_str(&format!(r#"<{name} w:val="{}"/>"#, xml_attr(v)));
}

fn xml_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // XML 1.0 has no other control characters; tab, line feed and carriage return are
            // written as references so attribute normalisation keeps them.
            '\t' => o.push_str("&#9;"),
            '\n' => o.push_str("&#10;"),
            '\r' => o.push_str("&#13;"),
            c if c.is_control() && (c as u32) < 0x20 => {}
            c => o.push(c),
        }
    }
    o
}

/// The `{\*\formfield …}` group for a field's `w:ffData`, or `None` when it holds no check box,
/// drop-down or text input.
pub(super) fn rtf_group(ff_data: &str) -> Option<String> {
    let kind = if has_tag(ff_data, "w:checkBox") {
        1
    } else if has_tag(ff_data, "w:ddList") {
        2
    } else if has_tag(ff_data, "w:textInput") {
        0
    } else {
        return None;
    };
    let v = |name: &str| tag_attr(ff_data, name, "w:val");
    let num = |name: &str| v(name).and_then(|s| s.trim().parse::<i32>().ok());
    // An on/off element: present without a value means on.
    let flag = |name: &str| has_tag(ff_data, name).then(|| v(name).is_none_or(|s| is_on(&s)));
    let res = match kind {
        1 => flag("w:checked").map(i32::from),
        2 => num("w:result").filter(|r| *r >= 0),
        _ => None,
    };
    let mut g = format!("{{\\*\\formfield{{\\fftype{kind}\\ffres{}", res.unwrap_or(NO_RESULT));
    if flag("w:enabled") == Some(false) {
        g.push_str("\\ffprot");
    }
    let own = |name: &str| has_tag(ff_data, name) && tag_attr(ff_data, name, "w:type").is_none_or(|t| t != "autoText");
    if own("w:helpText") {
        g.push_str("\\ffownhelp");
    }
    if own("w:statusText") {
        g.push_str("\\ffownstat");
    }
    if flag("w:calcOnExit") == Some(true) {
        g.push_str("\\ffrecalc");
    }
    if kind == 2 {
        g.push_str("\\ffhaslistbox");
    }
    let type_txt = if kind == 0 { v("w:type").and_then(|t| TEXT_TYPES.iter().position(|n| *n == t)).unwrap_or(0) } else { 0 };
    g.push_str(&format!("\\fftypetxt{type_txt}"));
    if kind == 0
        && let Some(n) = num("w:maxLength").filter(|n| *n > 0)
    {
        g.push_str(&format!("\\ffmaxlen{n}"));
    }
    match kind {
        1 => {
            match num("w:size").filter(|h| *h > 0) {
                Some(h) => g.push_str(&format!("\\ffsize1\\ffhps{h}")),
                None => g.push_str("\\ffsize0"),
            }
            let default = v("w:default").is_some_and(|d| is_on(&d));
            g.push_str(&format!("\\ffdefres{}", i32::from(default)));
        }
        2 => g.push_str(&format!("\\ffdefres{}", num("w:default").filter(|d| *d >= 0).unwrap_or(0))),
        _ => {}
    }
    let mut dest = |word: &str, text: Option<String>| {
        if let Some(t) = text {
            g.push_str(&format!("{{\\*\\{word} {}}}", super::esc(&t)));
        }
    };
    dest("ffname", v("w:name"));
    dest("ffdeftext", if kind == 0 { v("w:default") } else { None });
    dest("ffformat", if kind == 0 { v("w:format") } else { None });
    dest("ffhelptext", v("w:helpText"));
    dest("ffstattext", v("w:statusText"));
    dest("ffentrymcr", v("w:entryMacro"));
    dest("ffexitmcr", v("w:exitMacro"));
    if kind == 2 {
        for e in tag_vals(ff_data, "w:listEntry") {
            dest("ffl", Some(e));
        }
    }
    g.push_str("}}");
    Some(g)
}

#[cfg(test)]
mod tests {
    use super::super::{export, import};
    use wordcraft_doc::legacy_form::LegacyForm;
    use wordcraft_doc::para::InlineObject;

    fn forms(d: &wordcraft_doc::Document) -> Vec<String> {
        d.body
            .iter()
            .filter_map(|b| b.as_para())
            .flat_map(|p| &p.objects)
            .filter_map(|o| match o {
                InlineObject::Field { form, .. } => Some(form.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn escaped_and_non_ascii_text_round_trips() {
        let rtf = br"{\rtf1\ansi\uc1 {\field{\*\fldinst {FORMDROPDOWN {\*\formfield{\fftype2\ffres0{\*\ffname D}{\*\ffl Caf\u233?}{\*\ffl a\{b\}<&>}}}}}{\fldrslt }}\par}";
        let d = import(rtf).unwrap();
        let form = forms(&d).pop().unwrap_or_default();
        assert!(form.contains(r#"<w:listEntry w:val="a{b}&lt;&amp;&gt;"/>"#), "{form}");
        let want = LegacyForm::DropDown { name: "D".into(), entries: vec!["Café".into(), "a{b}<&>".into()], selected: 0 };
        assert_eq!(LegacyForm::parse(&form), Some(want.clone()));
        let again = forms(&import(export(&d).as_bytes()).unwrap()).pop().unwrap_or_default();
        assert_eq!(again, form);
    }

    #[test]
    fn form_field_group_outside_a_field_and_too_many_entries_are_harmless() {
        let d = import(br"{\rtf1\ansi {\*\formfield{\fftype1{\*\ffname X}}}Body\par}").unwrap();
        assert!(forms(&d).is_empty());
        let text: String = d.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
        assert_eq!(text.trim(), "Body");
        let mut rtf = br"{\rtf1\ansi {\field{\*\fldinst {FORMDROPDOWN {\*\formfield{\fftype2\ffres99999".to_vec();
        for _ in 0..200 {
            rtf.extend_from_slice(br"{\*\ffl e}");
        }
        rtf.extend_from_slice(br"}}}}{\fldrslt }}\par}");
        let form = forms(&import(&rtf).unwrap()).pop().unwrap_or_default();
        assert!(
            matches!(LegacyForm::parse(&form), Some(LegacyForm::DropDown { entries, selected: 0, .. }) if entries.len() == wordcraft_doc::legacy_form::MAX_ENTRIES)
        );
    }

    #[test]
    fn a_result_over_several_paragraphs_stays_text() {
        let rtf = br"{\rtf1\ansi {\field{\*\fldinst TOC \\o}{\fldrslt {One\tab 1\par Two\tab 2}}}\par}";
        let d = import(rtf).unwrap();
        let fields = d.body.iter().filter_map(|b| b.as_para()).flat_map(|p| &p.objects).filter(|o| matches!(o, InlineObject::Field { .. })).count();
        assert_eq!(fields, 0, "a TOC is not split into one field per paragraph");
        let text: Vec<String> = d.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
        assert!(text.iter().any(|t| t.contains("One")) && text.iter().any(|t| t.contains("Two")), "{text:?}");
    }
}
