//! The EQ field (ECMA-376 Part 1 §17.16.5.20), parsed by [`parse_eq`] into a tree of arrays,
//! fractions, scripts, brackets, lists, radicals, boxes, displacements, integrals and overstrikes.
//!
//! The overstrike switch (ECMA-376 Part 1 §17.16.5.20, `\o`): its arguments are drawn
//! on top of one another, centred (`\ac`, the default), left-aligned (`\al`) or right-aligned
//! (`\ar`). Home › Font › Enclose Characters stores a character in a circle, square, triangle or
//! diamond as one: ` eq \o\ac(○,字)`.
//!
//! Which of the two enclosure styles a field has is kept in the document model as a trailing
//! `\* enlarge` (WordCraft's own; nothing is written for the default "shrink text" style). In a
//! `.docx` the style is Word's: the font sizes of the field code's runs (the text smaller than the
//! symbol, or the symbol larger than the text), so the marker never reaches a file.

/// How the overstruck arguments line up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// An `EQ \o(…)` field: the arguments' text (nested switches flattened to their text).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overstrike {
    pub align: OAlign,
    pub args: Vec<String>,
}

/// Most arguments read from one field (the rest are ignored).
const MAX_ARGS: usize = 16;

/// Parse an `EQ \o` field code; `None` for any other field (or EQ switch).
pub fn parse_overstrike(instr: &str) -> Option<Overstrike> {
    let t = instr.trim_start();
    let name_end = t.find(|c: char| c.is_whitespace() || c == '\\').unwrap_or(t.len());
    if !t.get(..name_end)?.eq_ignore_ascii_case("eq") {
        return None;
    }
    let mut rest = t.get(name_end..)?.trim_start();
    // `\o`, then its alignment options, then the argument list.
    rest = rest.strip_prefix('\\')?;
    let o = rest.chars().next()?;
    if !o.eq_ignore_ascii_case(&'o') {
        return None;
    }
    rest = rest.get(1..)?;
    let mut align = OAlign::Center;
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix('(') {
            rest = r;
            break;
        }
        let r = rest.strip_prefix('\\')?;
        let sw = r.get(..2)?;
        align = match sw.to_ascii_lowercase().as_str() {
            "al" => OAlign::Left,
            "ac" => OAlign::Center,
            "ar" => OAlign::Right,
            _ => return None,
        };
        rest = r.get(2..)?;
    }
    // Arguments up to the matching `)`, split at top-level `,` or `;` (the list separator).
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut chars = rest.chars();
    let mut closed = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                cur.push(c);
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            '(' => {
                depth = depth.saturating_add(1);
                cur.push(c);
            }
            ')' if depth == 0 => {
                closed = true;
                break;
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' | ';' if depth == 0 => {
                if args.len() < MAX_ARGS {
                    args.push(arg_text(&cur));
                }
                cur.clear();
            }
            c => cur.push(c),
        }
    }
    if !closed {
        return None;
    }
    if args.len() < MAX_ARGS {
        args.push(arg_text(&cur));
    }
    Some(Overstrike { align, args })
}

/// An argument's text: escapes (`\,` `\(` `\)` `\\`) become the character, nested switches
/// (`\s\up4(…)`) and their brackets are dropped, leaving what they show.
fn arg_text(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.peek().copied() {
                Some(n @ (',' | '(' | ')' | '\\' | ';')) => {
                    out.push(n);
                    it.next();
                }
                // A switch: its letters, then an optional (signed) number.
                Some(_) => {
                    while it.next_if(char::is_ascii_alphabetic).is_some() {}
                    while it.next_if(|n| *n == ' ').is_some() {}
                    while it.next_if(|n| n.is_ascii_digit() || *n == '-' || *n == '.').is_some() {}
                }
                None => {}
            },
            '(' | ')' => {}
            c => out.push(c),
        }
    }
    out
}

/// One part of an EQ field's display (ECMA-376 Part 1 §17.16.5.20), parsed from its code.
#[derive(Clone, Debug, PartialEq)]
pub enum EqNode {
    /// Literal text.
    Text(String),
    /// Parts side by side.
    Seq(Vec<EqNode>),
    /// `\a`: items in `cols` columns, filled row by row; `vs` / `hs` points between rows /
    /// columns.
    Array { align: OAlign, cols: u16, vs: f32, hs: f32, items: Vec<EqNode> },
    /// `\f(num,den)`.
    Fraction(Box<EqNode>, Box<EqNode>),
    /// `\s`: raised `up` points (negative lowers), `ai` / `di` points added above / below the
    /// line; several items stack.
    Script { up: f32, ai: f32, di: f32, items: Vec<EqNode> },
    /// `\b`: the argument between brackets (`None`: no bracket on that side).
    Brackets { left: Option<char>, right: Option<char>, body: Box<EqNode> },
    /// `\l(…)`: the items with list separators between them.
    List(Vec<EqNode>),
    /// `\r(e)` / `\r(n,e)`.
    Radical { index: Option<Box<EqNode>>, base: Box<EqNode> },
    /// `\x`: borders on the chosen sides.
    Boxed { top: bool, bottom: bool, left: bool, right: bool, body: Box<EqNode> },
    /// `\d()`: a gap of `by` points (negative moves back), underlined with `line`.
    Displace { by: f32, line: bool },
    /// `\i(lo,hi,e)`: an integral (or sum, product, any character).
    Integral { symbol: char, variable: bool, inline: bool, lo: Box<EqNode>, hi: Box<EqNode>, body: Box<EqNode> },
    /// `\o(…)`: the items drawn on top of one another.
    Overstrike { align: OAlign, items: Vec<EqNode> },
}

impl EqNode {
    /// The text the part shows, flattened.
    pub fn text(&self) -> String {
        let join = |v: &[EqNode], sep: &str| v.iter().map(EqNode::text).collect::<Vec<_>>().join(sep);
        match self {
            EqNode::Text(t) => t.clone(),
            EqNode::Seq(v) | EqNode::Array { items: v, .. } | EqNode::Script { items: v, .. } | EqNode::Overstrike { items: v, .. } => join(v, ""),
            EqNode::List(v) => join(v, ","),
            EqNode::Fraction(a, b) => format!("{}{}", a.text(), b.text()),
            EqNode::Brackets { left, right, body } => {
                format!("{}{}{}", left.map(String::from).unwrap_or_default(), body.text(), right.map(String::from).unwrap_or_default())
            }
            EqNode::Radical { index, base } => format!("{}\u{221A}{}", index.as_ref().map(|i| i.text()).unwrap_or_default(), base.text()),
            EqNode::Boxed { body, .. } => body.text(),
            EqNode::Displace { .. } => String::new(),
            EqNode::Integral { symbol, lo, hi, body, .. } => format!("{symbol}{}{}{}", lo.text(), hi.text(), body.text()),
        }
    }
}

/// Deepest nesting of switches read.
const MAX_DEPTH: usize = 24;
/// Most items in one switch's argument list, and most parts in one field.
const MAX_ITEMS: usize = 256;
const MAX_NODES: usize = 4096;
/// Largest distance (points) an option may give.
const MAX_PT: f32 = 1584.0;

/// Parse an `EQ` field code into its display tree; `None` for any other field, an unknown
/// switch or option, or a malformed code (unbalanced brackets).
pub fn parse_eq(instr: &str) -> Option<EqNode> {
    let t = instr.trim_start();
    let name_end = t.find(|c: char| c.is_whitespace() || c == '\\').unwrap_or(t.len());
    if !t.get(..name_end)?.eq_ignore_ascii_case("eq") {
        return None;
    }
    let mut p = EqParser { s: t.get(name_end..)?.chars().collect(), i: 0, nodes: 0 };
    let mut parts = Vec::new();
    let mut text = String::new();
    while let Some(c) = p.peek() {
        if c == '\\' {
            match p.peek_at(1) {
                // A general switch (`\* MERGEFORMAT`, the enclosure marker) ends the equation.
                Some('*') => break,
                Some(n @ (',' | '(' | ')' | '\\' | ';')) => {
                    text.push(n);
                    p.i += 2;
                }
                _ => {
                    flush(&mut text, &mut parts, true);
                    parts.push(p.switch(0)?);
                }
            }
        } else {
            text.push(c);
            p.i += 1;
        }
        if parts.len() > MAX_ITEMS {
            return None;
        }
    }
    flush(&mut text, &mut parts, true);
    match parts.len() {
        0 => None,
        1 => parts.pop(),
        _ => Some(EqNode::Seq(parts)),
    }
}

/// Move pending text into `parts` (at the top level, space between switches is dropped).
fn flush(text: &mut String, parts: &mut Vec<EqNode>, top: bool) {
    if !text.is_empty() && !(top && text.trim().is_empty()) {
        let t = if top { text.trim().to_string() } else { text.clone() };
        parts.push(EqNode::Text(t));
    }
    text.clear();
}

struct EqParser {
    s: Vec<char>,
    i: usize,
    nodes: usize,
}

impl EqParser {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn peek_at(&self, k: usize) -> Option<char> {
        self.s.get(self.i.saturating_add(k)).copied()
    }
    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }
    /// A run of ASCII letters, lowercased.
    fn letters(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek().filter(char::is_ascii_alphabetic) {
            out.push(c.to_ascii_lowercase());
            self.i += 1;
        }
        out
    }
    /// An optional signed number after an option (spaces allowed before it).
    fn number(&mut self) -> Option<f32> {
        let save = self.i;
        self.skip_space();
        let mut t = String::new();
        if let Some(c @ ('-' | '+')) = self.peek() {
            t.push(c);
            self.i += 1;
        }
        while let Some(c) = self.peek().filter(|c| c.is_ascii_digit() || *c == '.') {
            t.push(c);
            self.i += 1;
        }
        match t.parse::<f32>() {
            Ok(v) if v.is_finite() => Some(v.clamp(-MAX_PT, MAX_PT)),
            _ => {
                self.i = save;
                None
            }
        }
    }
    /// The character after `\lc`, `\fc`…: itself, or the one after a backslash.
    fn option_char(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += 1;
        if c == '\\' {
            let n = self.peek()?;
            self.i += 1;
            return Some(n);
        }
        Some(c)
    }

    /// A switch at `\`: its name, options and arguments.
    fn switch(&mut self, depth: usize) -> Option<EqNode> {
        if depth >= MAX_DEPTH {
            return None;
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_NODES || self.peek() != Some('\\') {
            return None;
        }
        self.i += 1;
        let name = self.letters();
        let mut opts: Vec<(String, Option<f32>, Option<char>)> = Vec::new();
        loop {
            self.skip_space();
            match self.peek()? {
                '(' => break,
                '\\' => {
                    self.i += 1;
                    let o = self.letters();
                    if o.is_empty() {
                        return None;
                    }
                    let ch = if matches!(o.as_str(), "lc" | "rc" | "bc" | "fc" | "vc") { Some(self.option_char()?) } else { None };
                    let n = if ch.is_none() { self.number() } else { None };
                    opts.push((o, n, ch));
                    if opts.len() > 16 {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        self.i += 1;
        let mut args = self.args(depth)?;
        let num = |o: &(String, Option<f32>, Option<char>), d: f32| o.1.unwrap_or(d);
        let align_of = |o: &str| match o {
            "al" => Some(OAlign::Left),
            "ac" => Some(OAlign::Center),
            "ar" => Some(OAlign::Right),
            _ => None,
        };
        let one = |args: &mut Vec<EqNode>| if args.len() == 1 { args.pop() } else { Some(EqNode::Seq(std::mem::take(args))) };
        Some(match name.as_str() {
            "a" => {
                let (mut align, mut cols, mut vs, mut hs) = (OAlign::Center, 1u16, 0.0, 0.0);
                for o in &opts {
                    match o.0.as_str() {
                        "co" => cols = num(o, 1.0).clamp(1.0, 64.0) as u16,
                        "vs" => vs = num(o, 0.0),
                        "hs" => hs = num(o, 0.0),
                        a => align = align_of(a)?,
                    }
                }
                EqNode::Array { align, cols, vs, hs, items: args }
            }
            "f" => {
                if !opts.is_empty() || args.len() != 2 {
                    return None;
                }
                let den = args.pop()?;
                let num = args.pop()?;
                EqNode::Fraction(Box::new(num), Box::new(den))
            }
            "s" => {
                let (mut up, mut ai, mut di) = (0.0, 0.0, 0.0);
                for o in &opts {
                    match o.0.as_str() {
                        "up" => up = num(o, 2.0),
                        "do" => up = -num(o, 2.0),
                        "ai" => ai = num(o, 0.0),
                        "di" => di = num(o, 0.0),
                        _ => return None,
                    }
                }
                EqNode::Script { up, ai, di, items: args }
            }
            "b" => {
                let (mut left, mut right) = (Some('('), Some(')'));
                let mut set = false;
                for o in &opts {
                    if !set {
                        (left, right) = (None, None);
                        set = true;
                    }
                    match (o.0.as_str(), o.2) {
                        ("lc", Some(c)) => left = Some(c),
                        ("rc", Some(c)) => right = Some(c),
                        ("bc", Some(c)) => (left, right) = (Some(c), Some(mirror(c))),
                        _ => return None,
                    }
                }
                EqNode::Brackets { left, right, body: Box::new(one(&mut args)?) }
            }
            "l" => {
                if !opts.is_empty() {
                    return None;
                }
                EqNode::List(args)
            }
            "r" => {
                if !opts.is_empty() || args.len() > 2 {
                    return None;
                }
                let base = args.pop()?;
                let index = args.pop().map(Box::new);
                EqNode::Radical { index, base: Box::new(base) }
            }
            "x" => {
                let (mut top, mut bottom, mut left, mut right) = (false, false, false, false);
                for o in &opts {
                    match o.0.as_str() {
                        "to" => top = true,
                        "bo" => bottom = true,
                        "le" => left = true,
                        "ri" => right = true,
                        _ => return None,
                    }
                }
                if opts.is_empty() {
                    (top, bottom, left, right) = (true, true, true, true);
                }
                EqNode::Boxed { top, bottom, left, right, body: Box::new(one(&mut args)?) }
            }
            "d" => {
                if args.iter().any(|a| !a.text().trim().is_empty()) {
                    return None;
                }
                let (mut by, mut line) = (0.0f32, false);
                for o in &opts {
                    match o.0.as_str() {
                        "fo" => by += num(o, 0.0),
                        "ba" => by -= num(o, 0.0),
                        "li" => line = true,
                        _ => return None,
                    }
                }
                EqNode::Displace { by, line }
            }
            "i" => {
                let (mut symbol, mut variable, mut inline) = ('\u{222B}', true, false);
                for o in &opts {
                    match (o.0.as_str(), o.2) {
                        ("su", _) => symbol = '\u{2211}',
                        ("pr", _) => symbol = '\u{220F}',
                        ("in", _) => inline = true,
                        ("fc", Some(c)) => (symbol, variable) = (c, false),
                        ("vc", Some(c)) => (symbol, variable) = (c, true),
                        _ => return None,
                    }
                }
                if args.len() != 3 {
                    return None;
                }
                let body = args.pop()?;
                let hi = args.pop()?;
                let lo = args.pop()?;
                EqNode::Integral { symbol, variable, inline, lo: Box::new(lo), hi: Box::new(hi), body: Box::new(body) }
            }
            "o" => {
                let mut align = OAlign::Center;
                for o in &opts {
                    align = align_of(&o.0)?;
                }
                EqNode::Overstrike { align, items: args }
            }
            _ => return None,
        })
    }

    /// Arguments after `(` up to the matching `)`, split at `,` or `;`.
    fn args(&mut self, depth: usize) -> Option<Vec<EqNode>> {
        let mut args = Vec::new();
        let mut parts = Vec::new();
        let mut text = String::new();
        // Bare brackets in an argument are text.
        let mut parens = 0usize;
        loop {
            let c = self.peek()?;
            match c {
                '\\' => match self.peek_at(1)? {
                    n @ (',' | '(' | ')' | '\\' | ';') => {
                        text.push(n);
                        self.i += 2;
                    }
                    n if n.is_ascii_alphabetic() => {
                        flush(&mut text, &mut parts, false);
                        parts.push(self.switch(depth.saturating_add(1))?);
                    }
                    n => {
                        text.push(n);
                        self.i += 2;
                    }
                },
                '(' => {
                    parens = parens.saturating_add(1);
                    text.push(c);
                    self.i += 1;
                }
                ')' if parens > 0 => {
                    parens -= 1;
                    text.push(c);
                    self.i += 1;
                }
                ')' | ',' | ';' if parens == 0 => {
                    self.i += 1;
                    flush(&mut text, &mut parts, false);
                    let arg = if parts.len() == 1 { parts.pop()? } else { EqNode::Seq(std::mem::take(&mut parts)) };
                    args.push(arg);
                    if args.len() > MAX_ITEMS {
                        return None;
                    }
                    if c == ')' {
                        return Some(args);
                    }
                }
                c => {
                    text.push(c);
                    self.i += 1;
                }
            }
        }
    }
}

/// The closing bracket for an opening one (`\bc`).
fn mirror(c: char) -> char {
    match c {
        '(' => ')',
        '{' => '}',
        '[' => ']',
        '<' => '>',
        '⟨' => '⟩',
        c => c,
    }
}

/// Escape text for an EQ argument.
fn escape_arg(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, ',' | '(' | ')' | '\\' | ';') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The enclosure drawn around enclosed characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EncloseShape {
    Circle,
    Square,
    Triangle,
    Diamond,
}

impl EncloseShape {
    pub const ALL: [EncloseShape; 4] = [EncloseShape::Circle, EncloseShape::Square, EncloseShape::Triangle, EncloseShape::Diamond];

    /// The symbol in the field code.
    pub fn symbol(self) -> char {
        match self {
            EncloseShape::Circle => '○',
            EncloseShape::Square => '□',
            EncloseShape::Triangle => '△',
            EncloseShape::Diamond => '◇',
        }
    }
    /// The shape a field-code symbol stands for (the usual symbols and their large or small
    /// variants).
    pub fn from_symbol(s: &str) -> Option<EncloseShape> {
        let mut it = s.trim().chars();
        let c = it.next()?;
        if it.next().is_some() {
            return None;
        }
        Some(match c {
            '○' | '◯' | '⭘' | '◦' | '〇' => EncloseShape::Circle,
            '□' | '◻' | '⬜' | '▢' | '◽' => EncloseShape::Square,
            '△' | '▵' | '⊿' => EncloseShape::Triangle,
            '◇' | '◊' | '⬦' | '⋄' => EncloseShape::Diamond,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            EncloseShape::Circle => "circle",
            EncloseShape::Square => "square",
            EncloseShape::Triangle => "triangle",
            EncloseShape::Diamond => "diamond",
        }
    }
    pub fn parse(s: &str) -> Option<EncloseShape> {
        EncloseShape::ALL.into_iter().find(|k| k.name().eq_ignore_ascii_case(s.trim()))
    }
}

/// Shrink the text to fit the shape, or enlarge the shape around the text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EncloseStyle {
    #[default]
    Shrink,
    Enlarge,
}

impl EncloseStyle {
    pub fn name(self) -> &'static str {
        match self {
            EncloseStyle::Shrink => "shrink",
            EncloseStyle::Enlarge => "enlarge",
        }
    }
    pub fn parse(s: &str) -> Option<EncloseStyle> {
        match s.trim().to_ascii_lowercase().as_str() {
            "shrink" => Some(EncloseStyle::Shrink),
            "enlarge" => Some(EncloseStyle::Enlarge),
            _ => None,
        }
    }
}

/// Enclosed characters: an `EQ \o` field with an enclosure symbol and the text.
#[derive(Clone, Debug, PartialEq)]
pub struct Enclosure {
    pub shape: EncloseShape,
    pub style: EncloseStyle,
    pub text: String,
}

/// The model's marker for the "enlarge symbol" style (see the module docs).
const ENLARGE: &str = "\\* enlarge";

impl Enclosure {
    /// Enclosed characters from a field code: `\o` with exactly two arguments, one of them an
    /// enclosure symbol and the other non-empty text.
    pub fn parse(instr: &str) -> Option<Enclosure> {
        let o = parse_overstrike(instr)?;
        let [a, b] = o.args.as_slice() else { return None };
        let (shape, text) = match (EncloseShape::from_symbol(a), EncloseShape::from_symbol(b)) {
            (Some(s), _) => (s, b),
            (None, Some(s)) => (s, a),
            _ => return None,
        };
        if text.trim().is_empty() {
            return None;
        }
        let close = instr.rfind(')')?;
        let tail = instr.get(close + 1..).unwrap_or("").trim();
        let style = if tail.eq_ignore_ascii_case(ENLARGE) { EncloseStyle::Enlarge } else { EncloseStyle::Shrink };
        Some(Enclosure { shape, style, text: text.clone() })
    }

    /// The field code in the model (with the style marker; see [`Enclosure::word_instr`]).
    pub fn instr(&self) -> String {
        match self.style {
            EncloseStyle::Shrink => self.word_instr(),
            EncloseStyle::Enlarge => format!("{} {ENLARGE}", self.word_instr()),
        }
    }

    /// The field code as Word writes it: `eq \o\ac(○,字)`.
    pub fn word_instr(&self) -> String {
        format!("eq \\o\\ac({},{})", self.shape.symbol(), escape_arg(&self.text))
    }
}

/// Characters Word counts as full width (East Asian wide or fullwidth); two half-width characters
/// or one full-width character fit in an enclosure.
pub fn is_full_width(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD)
}

/// Whether `text` fits in an enclosure: one full-width character or one or two half-width ones.
pub fn fits_enclosure(text: &str) -> bool {
    let n = text.chars().count();
    let wide = text.chars().filter(|c| is_full_width(*c)).count();
    !text.chars().any(|c| c.is_control() || c == crate::para::OBJ) && (n == 1 || (n == 2 && wide == 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_word_enclosures_and_general_overstrike() {
        let e = Enclosure::parse(" eq \\o\\ac(○,字)").unwrap();
        assert_eq!((e.shape, e.style, e.text.as_str()), (EncloseShape::Circle, EncloseStyle::Shrink, "字"));
        // Nested switches flatten to their text; the list separator may be `;`.
        let e = Enclosure::parse("EQ \\o\\ac(□;\\s\\up 1(12))").unwrap();
        assert_eq!((e.shape, e.text.as_str()), (EncloseShape::Square, "12"));
        let e = Enclosure { shape: EncloseShape::Diamond, style: EncloseStyle::Enlarge, text: "a,".into() };
        assert_eq!(Enclosure::parse(&e.instr()), Some(e.clone()));
        assert_eq!(e.word_instr(), "eq \\o\\ac(◇,a\\,)");
        let o = parse_overstrike("eq \\o \\al(=,/)").unwrap();
        assert_eq!((o.align, o.args), (OAlign::Left, vec!["=".to_string(), "/".to_string()]));
        assert!(parse_overstrike("eq \\f(1,2)").is_none());
        assert!(parse_overstrike("PAGE").is_none());
        assert!(parse_overstrike("eq \\o(unclosed").is_none());
        assert!(Enclosure::parse("eq \\o(a,b)").is_none());
        assert!(fits_enclosure("字") && fits_enclosure("12") && !fits_enclosure("字字") && !fits_enclosure("123") && !fits_enclosure(""));
    }

    fn t(s: &str) -> EqNode {
        EqNode::Text(s.into())
    }

    #[test]
    fn parses_arrays_fractions_and_scripts() {
        let a = parse_eq(" EQ \\a \\al (Milk,Eggs,Detergent,Veggies) ").unwrap();
        assert_eq!(
            a,
            EqNode::Array { align: OAlign::Left, cols: 1, vs: 0.0, hs: 0.0, items: vec![t("Milk"), t("Eggs"), t("Detergent"), t("Veggies")] }
        );
        let a = parse_eq("eq \\A \\AR \\co2 \\vs 6 \\hs3(a;b;c;d)").unwrap();
        assert_eq!(a, EqNode::Array { align: OAlign::Right, cols: 2, vs: 6.0, hs: 3.0, items: vec![t("a"), t("b"), t("c"), t("d")] });
        // Switches nest inside arguments.
        let f = parse_eq("EQ \\f(\\s\\up4(2),3)").unwrap();
        let up = EqNode::Script { up: 4.0, ai: 0.0, di: 0.0, items: vec![t("2")] };
        assert_eq!(f, EqNode::Fraction(Box::new(up), Box::new(t("3"))));
        let s = parse_eq("EQ \\s\\do3\\ai2\\di1(i,j)").unwrap();
        assert_eq!(s, EqNode::Script { up: -3.0, ai: 2.0, di: 1.0, items: vec![t("i"), t("j")] });
        assert!(parse_eq("EQ \\f(1)").is_none(), "a fraction has two arguments");
    }

    #[test]
    fn parses_brackets_lists_radicals_boxes_displacements_integrals() {
        assert_eq!(parse_eq("EQ \\b(a)").unwrap(), EqNode::Brackets { left: Some('('), right: Some(')'), body: Box::new(t("a")) });
        assert_eq!(parse_eq("EQ \\b \\bc\\{ (a)").unwrap(), EqNode::Brackets { left: Some('{'), right: Some('}'), body: Box::new(t("a")) });
        assert_eq!(parse_eq("EQ \\b\\lc\\[(a)").unwrap(), EqNode::Brackets { left: Some('['), right: None, body: Box::new(t("a")) });
        assert_eq!(parse_eq("EQ \\b\\rc|(a)").unwrap(), EqNode::Brackets { left: None, right: Some('|'), body: Box::new(t("a")) });
        assert_eq!(parse_eq("EQ \\l(1,2,3)").unwrap(), EqNode::List(vec![t("1"), t("2"), t("3")]));
        assert_eq!(parse_eq("EQ \\r(x)").unwrap(), EqNode::Radical { index: None, base: Box::new(t("x")) });
        assert_eq!(parse_eq("EQ \\r(3,x)").unwrap(), EqNode::Radical { index: Some(Box::new(t("3"))), base: Box::new(t("x")) });
        let all = EqNode::Boxed { top: true, bottom: true, left: true, right: true, body: Box::new(t("x")) };
        assert_eq!(parse_eq("EQ \\x(x)").unwrap(), all);
        assert_eq!(
            parse_eq("EQ \\x\\to\\le(x)").unwrap(),
            EqNode::Boxed { top: true, bottom: false, left: true, right: false, body: Box::new(t("x")) }
        );
        assert_eq!(parse_eq("EQ \\d \\fo10 \\li()").unwrap(), EqNode::Displace { by: 10.0, line: true });
        assert_eq!(parse_eq("EQ \\d\\ba4()").unwrap(), EqNode::Displace { by: -4.0, line: false });
        let i = parse_eq("EQ \\i \\su(1,5,3)").unwrap();
        assert_eq!(
            i,
            EqNode::Integral { symbol: '∑', variable: true, inline: false, lo: Box::new(t("1")), hi: Box::new(t("5")), body: Box::new(t("3")) }
        );
        let EqNode::Integral { symbol, inline, .. } = parse_eq("EQ \\i\\in\\pr(a,b,c)").unwrap() else { panic!() };
        assert_eq!((symbol, inline), ('∏', true));
        let EqNode::Integral { symbol, variable, .. } = parse_eq("EQ \\i\\fc\\$(a,b,c)").unwrap() else { panic!() };
        assert_eq!((symbol, variable), ('$', false));
        assert_eq!(parse_eq("EQ \\i(a,b,c)").unwrap().text(), "∫abc");
        assert_eq!(parse_eq("EQ \\o\\al(=,/)").unwrap(), EqNode::Overstrike { align: OAlign::Left, items: vec![t("="), t("/")] });
    }

    #[test]
    fn eq_escapes_text_and_malformed_codes() {
        assert_eq!(parse_eq(r"EQ \l(a\,b,c\(d\)\\\;)").unwrap(), EqNode::List(vec![t("a,b"), t("c(d)\\;")]));
        // Bare brackets in an argument are text; text sits beside switches.
        assert_eq!(parse_eq("EQ \\a(f(x,y),z)").unwrap().text(), "f(x,y)z");
        assert_eq!(
            parse_eq("EQ x\\s\\up4(2) \\* MERGEFORMAT").unwrap(),
            EqNode::Seq(vec![t("x"), EqNode::Script { up: 4.0, ai: 0.0, di: 0.0, items: vec![t("2")] }])
        );
        for bad in ["PAGE", "EQ", "EQ \\q(1)", "EQ \\a\\zz(1)", "EQ \\f(1,2", "EQ \\a", "EQ \\d(x)", "EQ \\b\\lc"] {
            assert!(parse_eq(bad).is_none(), "{bad}");
        }
        // Nesting is bounded.
        let deep = format!("EQ {}x{}", "\\s(".repeat(100), ")".repeat(100));
        assert!(parse_eq(&deep).is_none());
        let ok = format!("EQ {}x{}", "\\s(".repeat(10), ")".repeat(10));
        assert_eq!(parse_eq(&ok).unwrap().text(), "x");
    }
}
