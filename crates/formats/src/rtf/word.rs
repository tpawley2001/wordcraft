use super::reader::apply_section_word;
use super::state::{CellDef, Dest, FmtRevSt, HeaderFooter, HfPages, MAX_OLD_WORDS, MAX_TABS, Pn, PnKind, RevSt, SecFmt, St};
use super::styles::{StyleDef, StyleEntry, is_char_format_word, is_format_word};
use super::{Align, Fmt, Reader, VAlign, model};
use wordcraft_doc::props::{CharProps, EmphasisMark, Highlight, TabAlign, TabLeader, TabStop, Underline};

impl Reader {
    pub(super) fn word(&mut self, w: &str, p: Option<i32>, starred: bool, first_in_group: bool) {
        // Every control word ends a multibyte run: the code page it is read in may change here
        // (`\f`, `\fcharset`, `\cpg`, `\ansicpg`, `\pard`), and `\par` must see the text before it.
        self.finish_multibyte();
        let on = p != Some(0);
        let pv = p.unwrap_or(0);
        if self.skip_destination_word(w, pv, first_in_group) {
            return;
        }
        if self.shape_word(w, p, starred) {
            return;
        }
        if w == "shppict" || self.start_header_footer_word(w) {
            return;
        }
        if self.start_note_word(w) {
            return;
        }
        if let Some(d) = self.destination_for_word(w) {
            self.enter_destination(d);
            return;
        }
        // A nested row's properties (`{\*\nesttableprops \trowd … \nestrow}`) are read: they
        // describe the innermost table's row.
        if w == "nesttableprops" {
            return;
        }
        if self.skip_ignorable_destination(w, starred, first_in_group) {
            return;
        }
        if self.destination_word(w, p, on, pv) || self.page_border_word(w, pv) {
            return;
        }
        if self.underline_word(w, on, pv) || self.char_effect_word(w, on, pv) {
            return;
        }
        match w {
            "par" => self.end_para_mark(),
            "sect" => self.end_section(),
            "cell" => self.end_cell(),
            "nestcell" => self.end_nest_cell(),
            "row" => self.end_outer_row(),
            "nestrow" => self.end_nest_row(),
            "line" | "softline" => self.ch('\n'),
            "tab" => self.ch('\t'),
            "page" | "softpage" => self.ch('\u{000C}'),
            "column" => self.ch('\u{000E}'),
            "emdash" => self.ch('—'),
            "endash" => self.ch('–'),
            "bullet" => self.ch('•'),
            "lquote" => self.ch('‘'),
            "rquote" => self.ch('’'),
            "ldblquote" => self.ch('“'),
            "rdblquote" => self.ch('”'),
            "emspace" => self.ch('\u{2003}'),
            "enspace" => self.ch('\u{2002}'),
            "qmspace" => self.ch('\u{2005}'),
            "zwj" => self.ch('\u{200D}'),
            "zwnj" => self.ch('\u{200C}'),
            "zwbo" => self.ch('\u{200B}'),
            "zwnbo" => self.ch('\u{2060}'),
            "u" => {
                let u = (pv as i64).rem_euclid(65_536) as u16;
                if (0xD800..0xDC00).contains(&u) {
                    // An earlier high surrogate not followed by a low one stands alone.
                    self.flush_high_surrogate();
                    self.pending_high = Some((u, self.st.fmt.clone()));
                } else {
                    match self.pending_high.take() {
                        // A pair takes the formatting of its high half.
                        Some((h, fmt)) if (0xDC00..0xE000).contains(&u) => {
                            let c = char::decode_utf16([h, u]).next().and_then(Result::ok).unwrap_or('\u{FFFD}');
                            self.finish_multibyte();
                            self.with_fmt(fmt, |r| r.ch(c));
                        }
                        held => {
                            self.pending_high = held;
                            self.ch(char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}'));
                        }
                    }
                }
                self.skip_chars = self.st.uc;
            }
            "uc" => self.st.uc = pv.max(0) as usize,
            "plain" => {
                self.st.fmt = Fmt { link: self.st.fmt.link.clone(), link_tip: self.st.fmt.link_tip.clone(), ..Default::default() };
                self.st.mono = None;
                self.st.rev = RevSt::default();
                self.st.chg_c = FmtRevSt::default();
                if self.st.border_side == Some('c') {
                    self.st.border_side = None;
                }
            }
            "pard" => {
                let field = self.st.field;
                // Revision marks are character formatting, which `\pard` leaves alone.
                let (dest, fmt, rev, uc) = (self.st.dest, self.st.fmt.clone(), self.st.rev, self.st.uc);
                let chg_c = std::mem::take(&mut self.st.chg_c);
                let mono = self.st.mono.take();
                self.st = St { dest, fmt, mono, rev, chg_c, uc, field, ..Default::default() };
                self.page_side = None;
            }
            "revised" => self.st.rev.ins = on,
            "deleted" => self.st.rev.del = on,
            "revauth" => self.st.rev.auth = pv,
            "revauthdel" => self.st.rev.authdel = pv,
            // DTTMs are 32-bit patterns, written signed or unsigned.
            "revdttm" => self.st.rev.dttm = pv as u32,
            "revdttmdel" => self.st.rev.dttmdel = pv as u32,
            "crauth" => self.st.chg_c.auth = pv,
            "crdate" => self.st.chg_c.date = pv as u32,
            "prauth" => self.st.chg_p.auth = pv,
            "prdate" => self.st.chg_p.date = pv as u32,
            "b" => self.st.fmt.bold = on,
            "i" => self.st.fmt.italic = on,
            "strike" => self.st.fmt.strike = on,
            "super" => {
                self.st.fmt.sup = on;
                self.st.fmt.sub = false;
            }
            // Raised or lowered by N half-points, at full size (RTF 1.9.1; `w:position`): a
            // position, not superscript or subscript (no N: 6). `\up0`/`\dn0` clear it.
            "up" | "dn" => {
                let n = p.unwrap_or(6);
                let half = if w == "up" { n } else { n.saturating_neg() };
                let pt = (half as f32 / 2.0).clamp(-1584.0, 1584.0);
                self.set_extra(|x| x.position = (pt != 0.0).then_some(pt));
            }
            "sub" => {
                self.st.fmt.sub = on;
                self.st.fmt.sup = false;
            }
            "nosupersub" => {
                self.st.fmt.sup = false;
                self.st.fmt.sub = false;
            }
            "fs" => self.st.fmt.size = if pv > 0 { Some((pv as f32 / 2.0).clamp(1.0, 1638.0)) } else { None },
            "ansicpg" => {
                if let Some(cp) = u16::try_from(pv).ok().filter(|cp| wordcraft_doc::encoding::is_text_code_page(*cp)) {
                    self.ansicpg = cp;
                }
            }
            // The Macintosh and IBM PC character sets: 8-bit text is Mac OS Roman, or DOS code
            // page 437 (`\pc`) or 850 (`\pca`) (#1653).
            "mac" if self.ansicpg == 0 => self.ansicpg = wordcraft_doc::encoding::MAC_ROMAN_CODE_PAGE,
            "pc" if self.ansicpg == 0 => self.ansicpg = 437,
            "pca" if self.ansicpg == 0 => self.ansicpg = 850,
            "deff" => self.deff = pv,
            "f" => {
                self.st.cp = self.font_cps.get(&pv).copied();
                let name = self.fonts.get(&pv).cloned();
                match name {
                    // The default font (`\deff`), monospaced or not, is the document's: text in
                    // it is plain text, as after `\plain`.
                    Some(n) if model::is_mono(&n) && pv == self.deff => {
                        self.st.fmt.code = false;
                        self.st.fmt.font = None;
                        self.st.mono = None;
                    }
                    Some(n) if model::is_mono(&n) => {
                        self.st.fmt.code = true;
                        self.st.fmt.font = None;
                        self.st.mono = Some(n);
                    }
                    Some(n) => {
                        self.st.fmt.code = false;
                        self.st.fmt.font = Some(n);
                        self.st.mono = None;
                    }
                    None => {}
                }
            }
            "cf" => self.st.fmt.color = self.colors.get(pv.max(0) as usize).copied().flatten(),
            // A character border: the border words that follow describe it, until another side
            // starts, the group ends or `\plain`.
            "chbrdr" => {
                self.page_side = None;
                self.cell_side = None;
                self.st.border_side = Some('c');
            }
            "chcbpat" | "cb" => self.st.fmt.background = self.colors.get(pv.max(0) as usize).copied().flatten(),
            "highlight" => self.highlight(pv),
            // Hidden text: kept, as hidden text.
            "v" => self.st.fmt.hidden = on,
            "ql" => self.st.align = Some(Align::Left),
            "qc" => self.st.align = Some(Align::Center),
            "qr" => self.st.align = Some(Align::Right),
            "qj" => self.st.align = Some(Align::Justify),
            "qd" => self.st.align = Some(Align::Distribute),
            "s" => self.st.style = pv,
            // A character style: the document style its stylesheet entry became, if one did.
            "cs" => self.char_style(pv),
            "outlinelevel" => self.st.outline = Some(pv.clamp(0, 9) as u8),
            "pagebb" => self.st.pagebb = on,
            "keepn" => self.st.keepn = on,
            "keep" => self.st.keep = on,
            "noline" => self.st.noline = on,
            "widctlpar" => self.st.widow = Some(true),
            "nowidctlpar" => self.st.widow = Some(false),
            "aspalpha" => self.st.asp_alpha = on,
            "aspnum" => self.st.asp_num = on,
            "contextualspace" => self.st.contextual = Some(true),
            "mirrorindents" => self.st.mirror = Some(true),
            "sbauto" => self.st.sb_auto = Some(on),
            "saauto" => self.st.sa_auto = Some(on),
            "hyphpar" => self.st.no_hyph = Some(!on),
            "nosnaplinegrid" => self.st.snap_grid = Some(false),
            "intbl" => self.st.intbl = true,
            "itap" => {
                let n = pv.clamp(0, model::MAX_DEPTH as i32) as u8;
                self.st.itap = Some(n);
                self.st.intbl = n > 0;
            }
            "ls" => self.st.ls = Some(pv),
            "ilvl" => self.st.ilvl = pv.clamp(0, 8) as u8,
            _ if self.cell_side.is_none() && self.para_border_word(w, pv) => {}
            "trowd" => {
                self.defs.clear();
                self.def_cur = CellDef::default();
                self.row_header = false;
                self.row_height = None;
                self.row_keep = false;
                self.row_style = Default::default();
            }
            // The row's table style: the document style its stylesheet entry became, if one did.
            "ts" => self.row_style.style = self.table_style_id(pv),
            "tbllkhdrrows" | "tbllklastrow" | "tbllkhdrcols" | "tbllklastcol" | "tbllknorowband" | "tbllknocolband" => self.look_word(w, on),
            "trhdr" => self.row_header = true,
            // The row's position, twips; beyond 22 inches clamped.
            "trleft" => self.row_style.left = pv.clamp(-31_680, 31_680),
            "trgaph" => self.row_style.gap_word(pv),
            // The row's default cell padding, by side, and its units.
            "trpaddt" | "trpaddl" | "trpaddb" | "trpaddr" | "trpaddft" | "trpaddfl" | "trpaddfb" | "trpaddfr" => self.row_style.pad_word(w, pv),
            "tblind" => self.row_style.ind = Some(pv.clamp(-31_680, 31_680)),
            "tblindtype" => self.row_style.ind_type = pv,
            "trqc" => self.row_style.align = Some(Align::Center),
            "trqr" => self.row_style.align = Some(Align::Right),
            "trql" => self.row_style.align = None,
            // Twips: positive at least, negative exactly, 0 auto; beyond 22 inches ignored.
            "trrh" => {
                self.row_height = (pv != 0 && pv.unsigned_abs() <= 31680).then(|| (pv.unsigned_abs() as f32 / 20.0, pv < 0));
            }
            "trkeep" => self.row_keep = on,
            "clvmgf" => self.def_cur.vmerge = 1,
            "clvmrg" => self.def_cur.vmerge = 2,
            "clmgf" => self.def_cur.hmerge = 1,
            "clmrg" => self.def_cur.hmerge = 2,
            "clvertalt" => self.def_cur.valign = Some(VAlign::Top),
            "clvertalc" => self.def_cur.valign = Some(VAlign::Center),
            "clvertalb" => self.def_cur.valign = Some(VAlign::Bottom),
            "cltxlrtb" | "cltxlrtbv" => self.def_cur.direction = wordcraft_doc::props::TextDirection::Horizontal,
            "cltxtbrl" | "cltxtbrlv" => self.def_cur.direction = wordcraft_doc::props::TextDirection::Down,
            "cltxbtlr" => self.def_cur.direction = wordcraft_doc::props::TextDirection::Up,
            // A cell's own padding: Word reads `\clpadl` as the top and `\clpadt` as the left.
            "clpadl" => self.def_cur.pad[0] = Some(pv),
            "clpadt" => self.def_cur.pad[1] = Some(pv),
            "clpadb" => self.def_cur.pad[2] = Some(pv),
            "clpadr" => self.def_cur.pad[3] = Some(pv),
            "clpadfl" => self.def_cur.pad_unit[0] = Some(pv),
            "clpadft" => self.def_cur.pad_unit[1] = Some(pv),
            "clpadfb" => self.def_cur.pad_unit[2] = Some(pv),
            "clpadfr" => self.def_cur.pad_unit[3] = Some(pv),
            "clNoWrap" => self.def_cur.no_wrap = true,
            "clcbpat" => self.def_cur.shading = self.colors.get(pv.max(0) as usize).copied().flatten(),
            "sb" => self.st.sb = Some(pv),
            "sa" => self.st.sa = Some(pv),
            "sl" => self.st.sl = pv,
            "slmult" => self.st.slmult = on,
            "li" | "lin" => self.st.li = Some(pv),
            "ri" | "rin" => self.st.ri = Some(pv),
            "fi" => self.st.fi = Some(pv),
            // Indents in hundredths of a character, beside the twips ones.
            "culi" => self.st.culi = Some(pv.clamp(-158400, 158400)),
            "curi" => self.st.curi = Some(pv.clamp(-158400, 158400)),
            "cufi" => self.st.cufi = Some(pv.clamp(-158400, 158400)),
            "tqr" => self.st.tab_align = TabAlign::Right,
            "tqc" => self.st.tab_align = TabAlign::Center,
            "tqdec" => self.st.tab_align = TabAlign::Decimal,
            "tldot" => self.st.tab_leader = TabLeader::Dot,
            "tlmdot" => self.st.tab_leader = TabLeader::MiddleDot,
            "tlhyph" => self.st.tab_leader = TabLeader::Hyphen,
            "tlul" | "tlth" => self.st.tab_leader = TabLeader::Underscore,
            // An equals-sign leader has no model value.
            "tleq" => self.st.tab_leader = TabLeader::None,
            "tx" | "tb" => self.tab_stop(pv, w == "tb"),
            // Character spacing: `\expnd` in quarter points, `\expndtw` in twips; the later wins.
            "expnd" => self.char_spacing(pv as f32 / 4.0),
            "expndtw" => self.char_spacing(pv as f32 / 20.0),
            "clbrdrt" | "clbrdrl" | "clbrdrb" | "clbrdrr" => {
                self.page_side = None;
                self.cell_side = w.chars().last();
            }
            "brdrs" | "brdrth" | "brdrsh" | "brdrdb" | "brdrdot" | "brdrdash" | "brdrdashsm" | "brdrdashd" | "brdrdashdd" | "brdrhair"
            | "brdrtriple" | "brdrthtnsg" | "brdrtnthsg" | "brdrtnthtnsg" | "brdrthtnmg" | "brdrtnthmg" | "brdrtnthtnmg" | "brdrthtnlg"
            | "brdrtnthlg" | "brdrtnthtnlg" | "brdrwavy" | "brdrwavydb" | "brdrnone" | "brdrnil" | "brdrtbl"
                if self.border_open() =>
            {
                use wordcraft_doc::props::{Border, BorderStyle};
                let style = border_style(w).unwrap_or(BorderStyle::Single);
                let b = (style != BorderStyle::None).then_some(Border { style, ..Border::single(0.5) });
                self.each_border_slot(|slot| *slot = b);
            }
            "brdrw" if self.border_open() => {
                let width = (pv as f32 / 20.0).clamp(0.25, 6.0);
                self.each_border_slot(|slot| {
                    if let Some(b) = slot {
                        b.width = width;
                    }
                });
            }
            "brdrcf" if self.border_open() => {
                let color = self.colors.get(pv.max(0) as usize).copied().flatten();
                self.each_border_slot(|slot| {
                    if let Some(b) = slot {
                        b.color = color;
                    }
                });
            }
            "paperw" => self.page.w = Some(pv),
            "paperh" => self.page.h = Some(pv),
            "margl" => self.page.left = Some(pv),
            "margr" => self.page.right = Some(pv),
            "margt" => self.page.top = Some(pv),
            "margb" => self.page.bottom = Some(pv),
            "gutter" => self.page.gutter = Some(pv),
            "rtlgutter" => self.page.rtl_gutter = on,
            "landscape" => self.page.landscape = on,
            _ if self.doc_note_word(w, p) || self.doc_opts.word(w, p) || self.doc_setting_word(w, p) || self.frame_word(w, pv) => {}
            _ if self.section_word(w, p) => {}
            "cellx" => {
                self.cell_side = None;
                let mut d = std::mem::take(&mut self.def_cur);
                d.right = pv;
                if self.defs.len() < 63 {
                    self.defs.push(d);
                }
            }
            "field" => self.start_field(),
            "fldlock" => self.lock_field(),
            "fldrslt" => {
                let instr = self.st.field.and_then(|i| self.fields.get(i));
                let link = instr.and_then(|s| hyperlink(s));
                if link.is_some() {
                    self.st.fmt.link = link.as_deref().and_then(model::kept_link);
                    // The ScreenTip (`\o "text"`) goes with a kept link only; the result group's
                    // end restores the formatting before it, so it never outlives its field.
                    self.st.fmt.link_tip =
                        self.st.fmt.link.as_ref().and(instr).and_then(|s| wordcraft_doc::link::hyperlink_tip(s)).map(std::sync::Arc::from);
                } else if self.field_inline()
                    && let Some(instr) = self.st.field.and_then(|i| self.fields.get(i)).filter(|s| !s.trim().is_empty())
                {
                    // Each result starts its own field, even when it is empty or follows the
                    // same instruction without intervening text.
                    if let Some(i) = self.st.field {
                        self.field_at.entry(i).or_insert(self.para.inlines.len());
                    }
                    self.para.inlines.push(super::Inline::Field {
                        instr: instr.clone(),
                        result: String::new(),
                        fmt: self.st.fmt.clone(),
                        locked: self.st.field.is_some_and(|i| self.locked_fields.contains(&i)),
                        form: self.st.field.and_then(|i| self.forms.get(&i)).map(|ff| ff.to_xml()).unwrap_or_default(),
                    });
                    self.para_has = true;
                }
            }
            _ => {}
        }
    }

    /// Enters destination `d`, setting up what it collects.
    fn enter_destination(&mut self, d: Dest) {
        match d {
            Dest::Bookmark(end) => {
                self.bkmk = Some((self.depth(), String::new()));
                self.bkmk_end = end;
            }
            Dest::Pict => {
                self.pict_depth = Some(self.depth());
                self.pict_hex.clear();
                self.pict_half = None;
                self.pict_kind = 0;
                self.pict_w = None;
                self.pict_h = None;
                self.pict_sx = 0.0;
                self.pict_sy = 0.0;
                self.pict_crop = [0; 4];
                self.pict_nat = (0, 0);
                self.pict_mm = 0;
            }
            Dest::FldInst if self.st.field.is_none() => self.start_field(),
            // Old character formatting holds the properties that changed (Word) or, after a
            // `\plain`, all of them; old paragraph formatting is complete from `\pard`.
            Dest::OldProps(false) => {
                self.st.old_words.clear();
                self.st.old_rev = (None, None);
            }
            Dest::OldProps(true) => self.word("pard", None, false, false),
            // Old section formatting is complete, from Word's defaults (as after `\sectd`).
            Dest::OldSect => self.old_sec = SecFmt::default(),
            _ => {}
        }
        self.st.dest = d;
    }

    fn destination_for_word(&mut self, w: &str) -> Option<Dest> {
        match w {
            "fonttbl" => Some(Dest::FontTbl),
            "colortbl" => Some(Dest::ColorTbl),
            "stylesheet" => Some(Dest::StyleSheet),
            "generator" => {
                self.generator.get_or_insert_with(String::new);
                Some(Dest::Generator)
            }
            "info" => Some(Dest::Skip),
            "title" if self.st.dest == Dest::Skip && self.in_info() => Some(Dest::Info(1)),
            "author" if self.in_info() => Some(Dest::Info(2)),
            "subject" if self.in_info() => Some(Dest::Info(3)),
            "keywords" if self.in_info() => Some(Dest::Info(4)),
            "doccomm" if self.in_info() => Some(Dest::Info(5)),
            "operator" if self.in_info() => Some(Dest::Info(6)),
            "category" if self.in_info() => Some(Dest::Info(7)),
            "manager" if self.in_info() => Some(Dest::Info(11)),
            "company" if self.in_info() => Some(Dest::Info(12)),
            "hlinkbase" if self.in_info() => Some(Dest::Info(13)),
            "creatim" | "revtim" | "printim" if self.in_info() => {
                let which = match w {
                    "creatim" => 0,
                    "revtim" => 1,
                    _ => 2,
                };
                self.in_date = Some((which, self.depth()));
                Some(Dest::Skip)
            }
            "template" if self.st.dest == Dest::Body && self.sub.is_none() && self.hf.is_none() => Some(Dest::Template),
            "userprops" if self.st.dest == Dest::Body && self.sub.is_none() && self.hf.is_none() => Some(Dest::UserProps),
            "propname" if self.st.dest == Dest::UserProps => Some(Dest::Info(8)),
            "staticval" if self.st.dest == Dest::UserProps => Some(Dest::Info(9)),
            "linkval" if self.st.dest == Dest::UserProps => Some(Dest::Info(10)),
            "latentstyles" if self.st.dest == Dest::Body && self.sub.is_none() && self.hf.is_none() => {
                self.latent.begin();
                Some(Dest::LatentStyles)
            }
            "lsdlockedexcept" if self.st.dest == Dest::LatentStyles => Some(Dest::LsdExcept),
            "listtable" => Some(Dest::ListTable),
            "revtbl" => Some(Dest::RevTbl),
            "oldcprops" => Some(Dest::OldProps(false)),
            "oldpprops" => Some(Dest::OldProps(true)),
            "oldsprops" => Some(Dest::OldSect),
            // Note separator and continuation stories (RTF 1.9.1 "Footnotes"), also written
            // without `\*` by older writers: not body text, and no custom separators are kept.
            "ftnsep" | "ftnsepc" | "ftncn" | "aftnsep" | "aftnsepc" | "aftncn" => Some(Dest::Skip),
            "listoverridetable" => Some(Dest::ListOverride),
            "fldinst" => Some(Dest::FldInst),
            "formfield" if self.st.dest == Dest::FldInst => {
                let i = self.st.field?;
                self.forms.entry(i).or_default();
                Some(Dest::FormField)
            }
            _ if self.st.dest == Dest::FormField => {
                let slot = super::form_field::FfSlot::for_word(w)?;
                let open = self.st.field.and_then(|i| self.forms.get_mut(&i)).is_some_and(|f| f.open(slot));
                Some(if open { Dest::FfText(slot) } else { Dest::Skip })
            }
            // Transparent wrapper: read the nested pict without changing the destination.
            "pict" => Some(Dest::Pict),
            // An object's `{\result …}` is what Word shows for it; the rest of the object is
            // control data, skipped below.
            "object" => Some(Dest::Object),
            "result" if self.st.dest == Dest::Object => Some(Dest::Body),
            "bkmkstart" => Some(Dest::Bookmark(false)),
            "bkmkend" => Some(Dest::Bookmark(true)),
            "atrfstart" | "atrfend" | "atnid" | "atnauthor" | "atndate" | "atntime" | "atnref"
                if self.marks_on
                    && self.hf.is_none()
                    && self.st.dest == Dest::Body
                    && self.sub.as_ref().is_none_or(|s| s.comment && !w.starts_with("atrf")) =>
            {
                self.atn_text.clear();
                Some(Dest::Atn(match w {
                    "atrfstart" => 1,
                    "atrfend" => 2,
                    "atnid" => 3,
                    "atnauthor" => 4,
                    "atndate" | "atntime" => 5,
                    _ => 6,
                }))
            }
            "header" | "footer" | "headerl" | "headerr" | "headerf" | "footerl" | "footerr" | "footerf" | "footnote" | "annotation" | "pntext"
            | "listtext" | "nonshppict" | "xe" | "tc" | "txe" | "private" | "atnid" | "atnauthor" | "comment" | "shpinst" | "do"
            | "operator" | "company" | "manager" | "category" | "hlinkbase" | "revtime" | "printim" | "buptim" | "version" | "nofpages"
            | "nofwords" | "nofchars" | "edmins" | "vern" | "themedata" | "colorschememapping" | "latentstyles" | "datastore" | "xmlnstbl"
            | "rsidtbl" | "mmathPr" | "pgdsctbl" | "filetbl" | "listpicture" | "userprops" | "docvar" | "objdata" | "fontemb" | "fontfile"
            | "panose" | "falt" | "mhtmltag" | "htmltag" | "pnseclvl" | "background" | "sp" | "pgptbl"
            // The flat copy of nested tables, for readers without them.
            | "nonesttables" => Some(Dest::Skip),
            // Word 6.0/95 paragraph numbering: read, so the paragraph imports as a list.
            "pn" => {
                self.pn_cur = Pn::default();
                self.pn_depth = Some(self.depth());
                Some(Dest::Pn)
            }
            "pntxtb" | "pntxta" if self.pn_depth.is_some() => {
                self.pn_txt.clear();
                Some(if w == "pntxtb" { Dest::PnTxtB } else { Dest::PnTxtA })
            }
            _ => None,
        }
    }

    /// The words that open a note or comment story of their own (`\footnote`, `\annotation`) and
    /// the two inside a note: `\ftnalt` (an endnote) and `\chftn` (the note's own number).
    fn start_note_word(&mut self, word: &str) -> bool {
        if self.st.dest == Dest::Body {
            if word == "chftn" {
                self.note_mark.saw_chftn();
            }
            match word {
                "ftnalt" => {
                    if let Some(sub) = self.sub.as_mut() {
                        sub.endnote = true;
                    }
                    return true;
                }
                "chftn" if self.sub.as_ref().is_some_and(|s| !s.comment) => {
                    let f = self.st.fmt.clone();
                    self.push_mark('S', 0, f);
                    return true;
                }
                _ => {}
            }
        }
        if !matches!(word, "footnote" | "annotation") || !self.marks_on || self.hf.is_some() || self.sub.is_some() || self.st.dest != Dest::Body {
            return false;
        }
        self.start_sub(word == "annotation");
        true
    }

    fn start_header_footer_word(&mut self, word: &str) -> bool {
        if !matches!(word, "header" | "footer" | "headerl" | "headerr" | "headerf" | "footerl" | "footerr" | "footerf")
            || self.hf.is_some()
            || self.st.dest != Dest::Body
            || self.st.intbl
        {
            return false;
        }
        let footer = word.starts_with("footer");
        let pages = match word.strip_prefix(if footer { "footer" } else { "header" }) {
            Some("f") => HfPages::First,
            Some("l") => HfPages::Even,
            _ => HfPages::Default,
        };
        self.start_header_footer(HeaderFooter { footer, pages });
        true
    }

    fn skip_ignorable_destination(&mut self, word: &str, starred: bool, first_in_group: bool) -> bool {
        // A character style's stylesheet entry is `{\*\csN …}`, a table style's `{\*\tsN …}`.
        let char_style = matches!(word, "cs" | "ts") && self.st.dest == Dest::StyleSheet;
        if !starred || !first_in_group || char_style || matches!(word, "fldinst" | "bkmkstart" | "bkmkend") {
            return false;
        }
        self.st.dest = Dest::Skip;
        true
    }

    fn destination_word(&mut self, word: &str, param: Option<i32>, on: bool, value: i32) -> bool {
        match self.st.dest {
            Dest::ShapeInst | Dest::ShapeName | Dest::ShapeValue => true,
            Dest::Skip | Dest::Object => {
                self.record_date_part(word, value);
                true
            }
            Dest::LatentStyles | Dest::LsdExcept => self.latent_word(word, param),
            Dest::UserProps => {
                if word == "proptype" {
                    self.info_more.set_prop_type(value);
                }
                true
            }
            Dest::FontTbl if !matches!(word, "u" | "uc") => {
                self.font_table_word(word, value);
                true
            }
            Dest::FontTbl => false,
            Dest::ColorTbl => {
                let color = value.clamp(0, 255) as u8;
                match word {
                    "red" => (self.color_cur.0, self.color_cur.3) = (color, true),
                    "green" => (self.color_cur.1, self.color_cur.3) = (color, true),
                    "blue" => (self.color_cur.2, self.color_cur.3) = (color, true),
                    _ => {}
                }
                true
            }
            // Style names take Unicode escapes (with fallback skipping) like the body.
            Dest::StyleSheet if !matches!(word, "u" | "uc") => {
                if self.style_def_cur.entry == StyleEntry::Table && self.table_style_word(word, value) {
                    return true;
                }
                let def = &mut self.style_def_cur;
                match word {
                    "s" | "cs" | "ds" | "ts" => {
                        self.style_cur = if word == "s" { value } else { -1 - value.saturating_abs() };
                        self.style_name.clear();
                        let entry = match word {
                            "s" => StyleEntry::Para,
                            "cs" => StyleEntry::Char,
                            "ts" => StyleEntry::Table,
                            _ => StyleEntry::Other,
                        };
                        *def = StyleDef { entry, ..Default::default() };
                    }
                    "sbasedon" => def.based_on = Some(value),
                    "snext" => def.next = Some(value),
                    "slink" => def.link = Some(value),
                    "sqformat" => def.quick = on,
                    "spriority" => def.priority = u32::try_from(value).ok(),
                    "ssemihidden" | "shidden" => def.hidden |= on,
                    "slocked" => def.locked = on,
                    "b" => def.bold = Some(on),
                    "i" => def.italic = Some(on),
                    "fs" => def.size = (value > 0).then(|| (value as f32 / 2.0).clamp(1.0, 1638.0)),
                    "f" => def.font = Some(value),
                    "cf" => def.color = Some(value),
                    "sb" => def.sb = Some(value),
                    "sa" => def.sa = Some(value),
                    "sl" => def.sl = Some(value),
                    "slmult" => def.slmult = Some(on),
                    "li" | "lin" => def.li = Some(value),
                    "ri" | "rin" => def.ri = Some(value),
                    "fi" => def.fi = Some(value),
                    "keepn" => def.keep_next = Some(on),
                    "keep" => def.keep_lines = Some(on),
                    "ql" => def.align = Some(Align::Left),
                    "qc" => def.align = Some(Align::Center),
                    "qr" => def.align = Some(Align::Right),
                    "qj" => def.align = Some(Align::Justify),
                    "qd" => def.align = Some(Align::Distribute),
                    // Tab stops, character-unit indents and character spacing, as in the body.
                    "tqr" => def.tab_align = TabAlign::Right,
                    "tqc" => def.tab_align = TabAlign::Center,
                    "tqdec" => def.tab_align = TabAlign::Decimal,
                    "tldot" => def.tab_leader = TabLeader::Dot,
                    "tlmdot" => def.tab_leader = TabLeader::MiddleDot,
                    "tlhyph" => def.tab_leader = TabLeader::Hyphen,
                    "tlul" | "tlth" => def.tab_leader = TabLeader::Underscore,
                    "tleq" => def.tab_leader = TabLeader::None,
                    "tx" | "tb" => push_tab_stop(&mut def.tabs, (&mut def.tab_align, &mut def.tab_leader), value, word == "tb"),
                    "cufi" => def.cufi = Some(value.clamp(-158400, 158400)),
                    "culi" => def.culi = Some(value.clamp(-158400, 158400)),
                    "curi" => def.curi = Some(value.clamp(-158400, 158400)),
                    "expnd" => def.spacing = (value != 0).then_some(value as f32 / 4.0),
                    "expndtw" => def.spacing = (value != 0).then_some(value as f32 / 20.0),
                    _ => {}
                }
                true
            }
            Dest::StyleSheet => false,
            Dest::ListTable => {
                self.list_table_word(word, value);
                true
            }
            Dest::Pn => {
                self.pn_word(word, value);
                true
            }
            Dest::PnTxtB | Dest::PnTxtA => !matches!(word, "u" | "uc" | "bullet" | "emdash" | "endash"),
            Dest::ListOverride => {
                match word {
                    "listid" => self.ov_list = value,
                    "ls" => {
                        self.ov_ls = value;
                        if self.overrides.len() < 4096 {
                            self.overrides.insert(value, self.ov_list);
                        }
                    }
                    _ => {}
                }
                true
            }
            Dest::RevTbl if !matches!(word, "u" | "uc") => true,
            Dest::RevTbl => false,
            Dest::Pict => {
                self.picture_word(word, value);
                true
            }
            Dest::Atn(kind) => {
                if kind == 5 {
                    self.record_atn_part(word, value);
                }
                !matches!(word, "u" | "uc")
            }
            Dest::FormField => {
                if let Some(f) = self.st.field.and_then(|i| self.forms.get_mut(&i)) {
                    f.word(word, param);
                }
                true
            }
            Dest::Info(_) | Dest::FldInst | Dest::Bookmark(_) | Dest::Generator | Dest::FfText(_) | Dest::Template if !matches!(word, "u" | "uc") => {
                true
            }
            Dest::Info(_) | Dest::FldInst | Dest::Bookmark(_) | Dest::Generator | Dest::FfText(_) | Dest::Template => false,
            Dest::OldProps(false) if matches!(word, "crauth" | "crdate") => {
                if word == "crauth" {
                    self.st.old_rev.0 = Some(value);
                } else {
                    self.st.old_rev.1 = Some(value as u32);
                }
                true
            }
            Dest::OldSect => {
                apply_section_word(&mut self.old_sec, word, param);
                true
            }
            Dest::OldProps(_) if !is_format_word(word) => true,
            Dest::OldProps(false) if word == "pard" => true,
            Dest::OldProps(_) if self.cell_side.is_some() && word.starts_with("brdr") => true,
            Dest::OldProps(false) => {
                if is_char_format_word(word) && self.st.old_words.len() < MAX_OLD_WORDS {
                    self.st.old_words.push((word.to_string(), param));
                }
                false
            }
            Dest::OldProps(true) | Dest::Body => false,
        }
    }

    fn record_atn_part(&mut self, word: &str, value: i32) {
        let v = value.clamp(0, 9999) as u32;
        let c = self.atn_parts.get_or_insert((0, 1, 1, 0, 0));
        match word {
            "yr" => c.0 = v,
            "mo" => c.1 = v,
            "dy" => c.2 = v,
            "hr" => c.3 = v,
            "min" => c.4 = v,
            _ => {}
        }
    }

    fn font_table_word(&mut self, word: &str, value: i32) {
        match word {
            "f" => {
                self.font_cur = value;
                self.font_name.clear();
                self.font_cp_cur = None;
            }
            "fcharset" => self.font_cp_cur = wordcraft_doc::encoding::charset_code_page(value),
            "cpg" => {
                if let Some(cp) = u16::try_from(value).ok().filter(|cp| wordcraft_doc::encoding::is_text_code_page(*cp)) {
                    self.font_cp_cur = Some(cp);
                }
            }
            _ => {}
        }
    }

    /// A word of a `{\*\pn …}` group (Word 6.0/95 paragraph numbering): the level's kind, its
    /// number format, its start value and the font of its label. The label's own text comes from
    /// the group's `{\pntxtb …}` and `{\pntxta …}`.
    fn pn_word(&mut self, word: &str, value: i32) {
        use wordcraft_doc::section::NumFormat;
        let pn = &mut self.pn_cur;
        match word {
            "pnlvlblt" => pn.kind = PnKind::Bullet,
            "pnlvlbody" => pn.kind = PnKind::Body,
            "pnlvlcont" => pn.kind = PnKind::Cont,
            "pnlvl" => pn.kind = PnKind::Outline(value.clamp(1, 9) as u8),
            "pndec" => pn.format = Some(NumFormat::Decimal),
            "pnucrm" => pn.format = Some(NumFormat::UpperRoman),
            "pnlcrm" => pn.format = Some(NumFormat::LowerRoman),
            "pnucltr" => pn.format = Some(NumFormat::UpperLetter),
            "pnlcltr" => pn.format = Some(NumFormat::LowerLetter),
            "pnstart" => pn.start = Some(value.clamp(0, 32_767) as u32),
            "pnf" => pn.font = Some(value),
            _ => {}
        }
    }

    fn list_table_word(&mut self, word: &str, value: i32) {
        match word {
            "list" => {
                self.list_cur.clear();
                self.list_cur_id = 0;
            }
            "listlevel" if self.list_cur.len() < 9 => self.list_cur.push(true),
            "levelnfc" | "levelnfcn" => {
                if let Some(level) = self.list_cur.last_mut() {
                    *level = !matches!(value, 23 | 255);
                }
            }
            "listid" => {
                self.list_cur_id = value;
                if self.lists.len() < 4096 {
                    self.lists.insert(value, self.list_cur.clone());
                }
            }
            _ => {}
        }
    }

    fn picture_word(&mut self, word: &str, value: i32) {
        match word {
            "pngblip" => self.pict_kind = 1,
            "jpegblip" => self.pict_kind = 2,
            // Windows and Enhanced Metafiles (#4259): drawn by the metafile renderer.
            "wmetafile" => {
                self.pict_kind = 3;
                self.pict_mm = value;
            }
            "emfblip" => self.pict_kind = 4,
            "picw" => self.pict_nat.0 = value,
            "pich" => self.pict_nat.1 = value,
            "picwgoal" => self.pict_w = Some(value.clamp(1, 31_680) as f32 / 20.0),
            "pichgoal" => self.pict_h = Some(value.clamp(1, 31_680) as f32 / 20.0),
            "picscalex" => self.pict_sx = value.clamp(1, 1000) as f32,
            "picscaley" => self.pict_sy = value.clamp(1, 1000) as f32,
            "piccropl" => self.pict_crop[0] = value.clamp(-31_680, 31_680),
            "piccropt" => self.pict_crop[1] = value.clamp(-31_680, 31_680),
            "piccropr" => self.pict_crop[2] = value.clamp(-31_680, 31_680),
            "piccropb" => self.pict_crop[3] = value.clamp(-31_680, 31_680),
            _ => {}
        }
    }

    fn skip_destination_word(&mut self, word: &str, value: i32, first_in_group: bool) -> bool {
        // `{\version N}` is a value, not a destination with text.
        if word == "version" && self.st.dest == Dest::Skip && self.in_info() {
            self.info_more.revision = value.max(0).unsigned_abs();
            return true;
        }
        let kept_info = self.in_info()
            && matches!(
                word,
                "title"
                    | "author"
                    | "subject"
                    | "keywords"
                    | "doccomm"
                    | "operator"
                    | "category"
                    | "creatim"
                    | "revtim"
                    | "printim"
                    | "manager"
                    | "company"
                    | "hlinkbase"
            );
        if !matches!(self.st.dest, Dest::Skip | Dest::Object) || kept_info {
            return false;
        }
        // An object's `{\result …}` is the one thing in it that's read: it's what Word shows.
        if self.st.dest == Dest::Object && first_in_group {
            if word == "result" {
                return false;
            }
            // Other object groups contain metadata or payload, never displayed content.
            // Give them their own skipped destination so a nested result stays hidden.
            self.st.dest = Dest::Skip;
        }
        self.record_date_part(word, value);
        true
    }

    /// A `\yr \mo \dy \hr \min` word inside `\creatim`, `\revtim` or `\printim`.
    fn record_date_part(&mut self, word: &str, value: i32) {
        let Some((which, _)) = self.in_date else { return };
        let Some(date) = self.dates.get_mut(which) else { return };
        let value = value.clamp(0, 9999) as u32;
        let created = date.get_or_insert((0, 1, 1, 0, 0));
        match word {
            "yr" => created.0 = value,
            "mo" => created.1 = value,
            "dy" => created.2 = value,
            "hr" => created.3 = value,
            "min" => created.4 = value,
            _ => {}
        }
    }

    /// `\fldlock`: the field of this `\field` group keeps its stored result (RTF 1.9.1
    /// "Fields"); fields nested in it are not locked by it.
    fn lock_field(&mut self) {
        if self.st.dest == Dest::Body
            && let Some(i) = self.st.field
        {
            self.locked_fields.insert(i);
        }
    }

    fn start_field(&mut self) {
        if self.fields.len() < 100_000 {
            self.fields.push(String::new());
            self.st.field = Some(self.fields.len() - 1);
        }
    }

    /// A tab stop at `twips` from the indent (`\tx`, or a bar tab `\tb`), with the alignment and
    /// leader words read before it. A stop at a position already set replaces it.
    fn tab_stop(&mut self, twips: i32, bar: bool) {
        push_tab_stop(&mut self.st.tabs, (&mut self.st.tab_align, &mut self.st.tab_leader), twips, bar);
    }

    /// A character effect word (Font › Effects, Scale, Kerning) into the run's own properties.
    fn char_effect_word(&mut self, w: &str, on: bool, pv: i32) -> bool {
        match w {
            // `\striked1` is double strikethrough; single strikethrough is `\strike`.
            "striked" => self.set_extra(|x| x.double_strike = on.then_some(true)),
            "caps" => self.set_extra(|x| x.caps = on.then_some(true)),
            "scaps" => self.set_extra(|x| x.small_caps = on.then_some(true)),
            "outl" => self.set_extra(|x| x.outline = on.then_some(true)),
            "shad" => self.set_extra(|x| x.shadow = on.then_some(true)),
            "embo" => self.set_extra(|x| x.emboss = on.then_some(true)),
            "impr" => self.set_extra(|x| x.engrave = on.then_some(true)),
            "noproof" => self.set_extra(|x| x.no_proof = on.then_some(true)),
            "accnone" => self.set_extra(|x| x.emphasis = None),
            "accdot" => self.set_extra(|x| x.emphasis = Some(EmphasisMark::Dot)),
            "acccomma" => self.set_extra(|x| x.emphasis = Some(EmphasisMark::Comma)),
            "acccircle" => self.set_extra(|x| x.emphasis = Some(EmphasisMark::Circle)),
            "accunderdot" => self.set_extra(|x| x.emphasis = Some(EmphasisMark::UnderDot)),
            // Horizontal scale, percent (100 = normal); kerning from a size in half-points (0 = off).
            "charscalex" if (1..=600).contains(&pv) => self.set_extra(|x| x.scale = (pv != 100).then_some(pv as f32)),
            "kerning" if (0..=3276).contains(&pv) => self.set_extra(|x| x.kern = (pv != 0).then_some(pv as f32 / 2.0)),
            _ => return false,
        }
        true
    }

    /// `\highlightN`: colour entry `n` as the text's highlight when it is one of the highlight
    /// colours; any other colour stays character shading, and none (0, auto, missing) clears both.
    fn highlight(&mut self, n: i32) {
        let c = self.colors.get(n.max(0) as usize).copied().flatten();
        let h = c.and_then(|c| Highlight::ALL.into_iter().find(|h| *h != Highlight::None && h.rgb() == Some(c)));
        if h.is_none() {
            self.st.fmt.background = c;
        }
        self.set_extra(|x| x.highlight = h);
    }

    /// `\csN`: the text's character style, when entry `n` became one of the document's own.
    fn char_style(&mut self, n: i32) {
        let id = self.own_char_style(n);
        self.set_extra(|x| x.style = id);
    }

    /// The run's character spacing, points (0 = normal).
    fn char_spacing(&mut self, pt: f32) {
        self.set_extra(|x| x.spacing = (pt != 0.0).then_some(pt));
    }

    /// An underline word: a style (`\ul`, `\uldb`…), `\ulnone`, or the colour `\ulcN`.
    fn underline_word(&mut self, w: &str, on: bool, pv: i32) -> bool {
        match w {
            "ulnone" => self.underline(Underline::None, false),
            "ulc" => {
                // Entry 0 is the auto colour: no underline colour of its own.
                let c = if pv > 0 { self.colors.get(pv as usize).copied().flatten() } else { None };
                self.set_extra(|x| x.underline_color = c);
            }
            _ => match underline_style(w) {
                Some(style) => self.underline(style, on),
                None => return false,
            },
        }
        true
    }

    /// The run's underline turned on in `style`, or off (any underline word with parameter 0).
    fn underline(&mut self, style: Underline, on: bool) {
        self.st.fmt.underline = on;
        self.set_extra(|x| x.underline = on.then_some(style));
    }

    /// Changes the run's character properties beyond the [`Fmt`] fields; none left is `None`.
    pub(super) fn set_extra(&mut self, f: impl FnOnce(&mut CharProps)) {
        let x = self.st.fmt.extra.get_or_insert_with(Default::default);
        f(x);
        if **x == CharProps::default() {
            self.st.fmt.extra = None;
        }
    }

    fn in_info(&self) -> bool {
        self.info_depth.is_some_and(|d| self.depth() > d)
    }
}

/// The border style border word `w` (`\brdrs`, `\brdrdb`…) names; `None` when `w` isn't one.
pub(super) fn border_style(w: &str) -> Option<wordcraft_doc::props::BorderStyle> {
    use wordcraft_doc::props::BorderStyle;
    Some(match w {
        "brdrnone" | "brdrnil" | "brdrtbl" => BorderStyle::None,
        "brdrs" | "brdrsh" | "brdrhair" => BorderStyle::Single,
        "brdrth" => BorderStyle::Thick,
        "brdrdb" | "brdrwavydb" => BorderStyle::Double,
        "brdrtnthsg" => BorderStyle::ThinThickSmallGap,
        "brdrthtnsg" => BorderStyle::ThickThinSmallGap,
        "brdrtnthtnsg" => BorderStyle::ThinThickThinSmallGap,
        "brdrtnthmg" => BorderStyle::ThinThickMediumGap,
        "brdrthtnmg" => BorderStyle::ThickThinMediumGap,
        "brdrtnthtnmg" => BorderStyle::ThinThickThinMediumGap,
        "brdrtnthlg" => BorderStyle::ThinThickLargeGap,
        "brdrthtnlg" => BorderStyle::ThickThinLargeGap,
        "brdrtnthtnlg" => BorderStyle::ThinThickThinLargeGap,
        "brdrdot" => BorderStyle::Dotted,
        "brdrdash" | "brdrdashsm" => BorderStyle::Dashed,
        "brdrdashd" | "brdrdashdd" => BorderStyle::DotDash,
        "brdrtriple" => BorderStyle::Triple,
        "brdrwavy" => BorderStyle::Wave,
        _ => return None,
    })
}

/// The underline an underline control word turns on (RTF 1.9.1); the heavy and long forms
/// are the style they are drawn as, as `Underline::from_ooxml` reads their OOXML names.
fn underline_style(w: &str) -> Option<Underline> {
    Some(match w {
        "ul" => Underline::Single,
        "ulw" => Underline::Words,
        "uldb" => Underline::Double,
        "ulth" => Underline::Thick,
        "uld" | "ulthd" => Underline::Dotted,
        "uldash" | "ulldash" | "ulthdash" | "ulthldash" => Underline::Dash,
        "uldashd" | "ulthdashd" => Underline::DotDash,
        "uldashdd" | "ulthdashdd" => Underline::DotDotDash,
        "ulwave" | "ulhwave" => Underline::Wave,
        "ululdbwave" => Underline::DoubleWave,
        _ => return None,
    })
}

/// The target of a `HYPERLINK` field instruction; `%22` is a quote, as the writer stores one.
pub(super) fn hyperlink(instr: &str) -> Option<String> {
    wordcraft_doc::link::hyperlink_target(instr).map(|t| t.replace("%22", "\""))
}

/// Adds a tab stop at `twips` (`\tx`, or a bar tab `\tb`) to `tabs`, with the pending alignment
/// and leader words read before it, which it then clears. A stop at a position already set
/// replaces it; at most [`MAX_TABS`] are kept.
pub(super) fn push_tab_stop(tabs: &mut Vec<TabStop>, pending: (&mut TabAlign, &mut TabLeader), twips: i32, bar: bool) {
    let (align, leader) = pending;
    let stop = TabStop { pos: twips as f32 / 20.0, align: if bar { TabAlign::Bar } else { *align }, leader: *leader };
    (*align, *leader) = (TabAlign::Left, TabLeader::None);
    if let Some(t) = tabs.iter_mut().find(|t| t.pos == stop.pos) {
        *t = stop;
    } else if tabs.len() < MAX_TABS {
        tabs.push(stop);
    }
}

#[cfg(test)]
mod object_result_tests {
    use super::super::{FBlock, Inline, parse};

    #[test]
    fn result_text_stays_in_place_and_metadata_stays_hidden() {
        let rtf = br"{\rtf1 Before{\object\objemb{\*\objclass Package{\result hidden}}{\objname name{\result hidden}}{\*\objdata 0102{\result hidden}}{\*\unknown{\result hidden}}{\result visible}}After}";
        let flow = parse(rtf).unwrap();
        let text: String = flow
            .blocks
            .iter()
            .filter_map(|block| match block {
                FBlock::Para(p) => Some(&p.inlines),
                FBlock::Table(_) => None,
            })
            .flatten()
            .filter_map(|inline| match inline {
                Inline::Text(text, _) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "BeforevisibleAfter");
    }
}

#[cfg(test)]
mod link_tip_tests {
    use super::super::{export, import};
    use wordcraft_doc::Document;

    /// The (text, link, ScreenTip) of each run in the first paragraph.
    fn runs(d: &Document) -> Vec<(String, Option<String>, Option<String>)> {
        let p = d.body.iter().find_map(|b| b.as_para()).expect("a paragraph");
        p.run_ranges().map(|(r, c)| (p.text[r].to_string(), c.link.clone(), c.link_tip.as_deref().map(str::to_string))).collect()
    }

    #[test]
    fn a_screen_tip_in_other_scripts_round_trips() {
        // The two CJK characters as `\uN?` escapes, N = 26085 and 26412.
        let cjk: String = [26085u32, 26412].iter().map(|n| format!("\\u{n}?")).collect();
        let rtf = format!(
            r#"{{\rtf1\ansi\uc1{{\field{{\*\fldinst{{HYPERLINK "http://example.com" \\o "Say \\"hi\\" a\{{b\}}\\\\c \u233? {cjk}"}}}}{{\fldrslt{{link}}}}}}\par}}"#
        );
        let want = "Say \"hi\" a{b}\\c \u{e9} \u{65e5}\u{672c}";
        let d = import(rtf.as_bytes()).unwrap();
        assert_eq!(runs(&d)[0].2.as_deref(), Some(want));
        let out = export(&d);
        assert!(out.contains(r#"\\o "Say \\"hi\\" a\{b\}\\\\c "#), "{out}");
        assert_eq!(runs(&import(out.as_bytes()).unwrap())[0].2.as_deref(), Some(want));
    }

    #[test]
    fn plain_inside_the_result_keeps_the_screen_tip() {
        let rtf = br#"{\rtf1\ansi{\field{\*\fldinst{HYPERLINK "http://example.com" \\o "tip"}}{\fldrslt{\plain link}}}\par}"#;
        let d = import(rtf).unwrap();
        assert_eq!(runs(&d)[0], ("link".into(), Some("http://example.com".into()), Some("tip".into())));
    }

    #[test]
    fn a_link_written_as_plain_text_has_no_screen_tip() {
        let rtf = br#"{\rtf1\ansi{\field{\*\fldinst{HYPERLINK "javascript:alert(1)" \\o "tip"}}{\fldrslt{link}}}\par}"#;
        let out = export(&import(rtf).unwrap());
        assert!(!out.contains("HYPERLINK") && !out.contains("tip"), "{out}");
    }
}
