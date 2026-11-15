//! `word/settings.xml` merged into the file's original settings part.
//!
//! The model owns only some settings (`super::settings_xml`). Regenerating the part from the
//! model alone drops everything else Word stores there: compatibility options, rsids, document
//! variables, proofing state, the colour scheme mapping, "remove personal information on save"…
//! So when a file brought settings the model doesn't hold ([`has_unowned`]; the reader keeps its
//! bytes under `SETTINGS_ORIG`), the writer merges instead ([`merge`]): the original root start
//! tag (namespace declarations, `mc:Ignorable`) and every child the model doesn't own are kept
//! byte for byte; owned children come from the generated part (or go, when the model turned
//! them off); owned children the original lacks are inserted at their CT_Settings position; in
//! `w:compat` only the `compatibilityMode` setting and the `w:balanceSingleByteDoubleByteWidth`,
//! `w:suppressTopSpacingWP`, `w:wrapTrailSpaces`, `w:footnoteLayoutLikeWW8` and
//! `w:useWord97LineBreakRules` switches are replaced. Kept children that refer to a
//! relationship (`r:id`, e.g. a mail merge data source) are dropped: their relationships aren't
//! carried. Anything unexpected returns `None`, and the generated part is written as before.

/// Children of `w:settings` the model writes (`super::settings_xml`).
const OWNED: &[&str] = &[
    "w:writeProtection",
    "w:removePersonalInformation",
    "w:removeDateAndTime",
    "w:displayBackgroundShape",
    "w:printFormsData",
    "w:mirrorMargins",
    "w:gutterAtTop",
    "w:attachedTemplate",
    "w:linkStyles",
    "w:stylePaneFormatFilter",
    "w:stylePaneSortMethod",
    "w:trackRevisions",
    "w:doNotTrackMoves",
    "w:documentProtection",
    "w:defaultTabStop",
    "w:autoHyphenation",
    "w:consecutiveHyphenLimit",
    "w:hyphenationZone",
    "w:doNotHyphenateCaps",
    "w:evenAndOddHeaders",
    "w:drawingGridHorizontalSpacing",
    "w:drawingGridVerticalSpacing",
    "w:displayHorizontalDrawingGridEvery",
    "w:displayVerticalDrawingGridEvery",
    "w:doNotUseMarginsForDrawingGridOrigin",
    "w:drawingGridHorizontalOrigin",
    "w:drawingGridVerticalOrigin",
    "w:characterSpacingControl",
    "w:footnotePr",
    "w:endnotePr",
    "w:docVars",
    "m:mathPr",
    "w:captions",
];

/// Owned children whose original, when there is one, is kept as is.
const KEEP_ORIGINAL: &[&str] = &["m:mathPr", "w:displayBackgroundShape"];

/// CT_Settings' sequence (ECMA-376 §17.15.1.78), for placing inserted children.
const ORDER: &[&str] = &[
    "w:writeProtection",
    "w:view",
    "w:zoom",
    "w:removePersonalInformation",
    "w:removeDateAndTime",
    "w:doNotDisplayPageBoundaries",
    "w:displayBackgroundShape",
    "w:printPostScriptOverText",
    "w:printFractionalCharacterWidth",
    "w:printFormsData",
    "w:embedTrueTypeFonts",
    "w:embedSystemFonts",
    "w:saveSubsetFonts",
    "w:saveFormsData",
    "w:mirrorMargins",
    "w:alignBordersAndEdges",
    "w:bordersDoNotSurroundHeader",
    "w:bordersDoNotSurroundFooter",
    "w:gutterAtTop",
    "w:hideSpellingErrors",
    "w:hideGrammaticalErrors",
    "w:activeWritingStyle",
    "w:proofState",
    "w:formsDesign",
    "w:attachedTemplate",
    "w:linkStyles",
    "w:stylePaneFormatFilter",
    "w:stylePaneSortMethod",
    "w:documentType",
    "w:mailMerge",
    "w:revisionView",
    "w:trackRevisions",
    "w:doNotTrackMoves",
    "w:doNotTrackFormatting",
    "w:documentProtection",
    "w:autoFormatOverride",
    "w:styleLockTheme",
    "w:styleLockQFSet",
    "w:defaultTabStop",
    "w:autoHyphenation",
    "w:consecutiveHyphenLimit",
    "w:hyphenationZone",
    "w:doNotHyphenateCaps",
    "w:showEnvelope",
    "w:summaryLength",
    "w:clickAndTypeStyle",
    "w:defaultTableStyle",
    "w:evenAndOddHeaders",
    "w:bookFoldRevPrinting",
    "w:bookFoldPrinting",
    "w:bookFoldPrintingSheets",
    "w:drawingGridHorizontalSpacing",
    "w:drawingGridVerticalSpacing",
    "w:displayHorizontalDrawingGridEvery",
    "w:displayVerticalDrawingGridEvery",
    "w:doNotUseMarginsForDrawingGridOrigin",
    "w:drawingGridHorizontalOrigin",
    "w:drawingGridVerticalOrigin",
    "w:doNotShadeFormData",
    "w:noPunctuationKerning",
    "w:characterSpacingControl",
    "w:printTwoOnOne",
    "w:strictFirstAndLastChars",
    "w:noLineBreaksAfter",
    "w:noLineBreaksBefore",
    "w:savePreviewPicture",
    "w:doNotValidateAgainstSchema",
    "w:saveInvalidXml",
    "w:ignoreMixedContent",
    "w:alwaysShowPlaceholderText",
    "w:doNotDemarcateInvalidXml",
    "w:saveXmlDataOnly",
    "w:useXSLTWhenSaving",
    "w:saveThroughXslt",
    "w:showXMLTags",
    "w:alwaysMergeEmptyNamespace",
    "w:updateFields",
    "w:hdrShapeDefaults",
    "w:footnotePr",
    "w:endnotePr",
    "w:compat",
    "w:docVars",
    "w:rsids",
    "m:mathPr",
    "w:attachedSchema",
    "w:themeFontLang",
    "w:clrSchemeMapping",
    "w:doNotIncludeSubdocsInStats",
    "w:doNotAutoCompressPictures",
    "w:forceUpgrade",
    "w:captions",
    "w:readModeInkLockDown",
    "w:smartTagType",
    "sl:schemaLibrary",
    "w:shapeDefaults",
    "w:doNotEmbedSmartTags",
    "w:decimalSymbol",
    "w:listSeparator",
];

fn rank(name: &str) -> usize {
    ORDER.iter().position(|n| *n == name).unwrap_or(ORDER.len())
}

/// A child element of the root: its qualified name and byte span.
struct Child {
    name: String,
    start: usize,
    end: usize,
}

/// Byte index of the `>` that ends the tag at the start of `s`, skipping quoted attribute values.
fn tag_end(s: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// The root start tag's span end and the root's direct children, or `None` for anything the
/// scanner doesn't expect (CDATA, DOCTYPE, a different root).
fn children(xml: &str, root: &str) -> Option<(usize, usize, Vec<Child>)> {
    let open = xml.find(&format!("<{root}"))?;
    let open_end = open + tag_end(xml.get(open..)?)? + 1;
    if xml.get(open_end - 2..open_end)? == "/>" {
        return None;
    }
    let close = xml.rfind(&format!("</{root}>"))?;
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = open_end;
    let mut cur: Option<(String, usize)> = None;
    while i < close {
        let rest = xml.get(i..close)?;
        let Some(lt) = rest.find('<') else { break };
        let at = i + lt;
        let tail = xml.get(at..close)?;
        if tail.starts_with("<!--") {
            i = at + tail.find("-->")? + 3;
            continue;
        }
        if tail.starts_with("<![CDATA[") || tail.starts_with("<!") {
            return None;
        }
        if tail.starts_with("<?") {
            i = at + tail.find("?>")? + 2;
            continue;
        }
        let gt = at + tag_end(tail)? + 1;
        let tag = xml.get(at..gt)?;
        if tag.starts_with("</") {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                let (name, s) = cur.take()?;
                out.push(Child { name, start: s, end: gt });
            }
        } else {
            let name: String = tag.get(1..)?.chars().take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/').collect();
            let selfclose = tag.ends_with("/>");
            if depth == 0 {
                if selfclose {
                    out.push(Child { name, start: at, end: gt });
                } else {
                    cur = Some((name, at));
                    depth = 1;
                }
            } else if !selfclose {
                depth += 1;
            }
        }
        i = gt;
    }
    if depth != 0 {
        return None;
    }
    Some((open_end, close, out))
}

/// CT_Compat children that come after `w:suppressTopSpacingWP` (ECMA-376 Part 1, §17.15.3, wml.xsd
/// `CT_Compat`), in order, for placing an inserted switch.
const AFTER_SUPPRESS_TOP_SPACING_WP: &[&str] = &[
    "w:suppressSpBfAfterPgBrk",
    "w:swapBordersFacingPages",
    "w:convMailMergeEsc",
    "w:truncateFontHeightsLikeWP6",
    "w:mwSmallCaps",
    "w:usePrinterMetrics",
    "w:doNotSuppressParagraphBorders",
    "w:wrapTrailSpaces",
    "w:footnoteLayoutLikeWW8",
    "w:shapeLayoutLikeWW8",
    "w:alignTablesRowByRow",
    "w:forgetLastTabAlignment",
    "w:adjustLineHeightInTable",
    "w:autoSpaceLikeWord95",
    "w:noSpaceRaiseLower",
    "w:doNotUseHTMLParagraphAutoSpacing",
    "w:layoutRawTableWidth",
    "w:layoutTableRowsApart",
    "w:useWord97LineBreakRules",
    "w:doNotBreakWrappedTables",
    "w:doNotSnapToGridInCell",
    "w:selectFldWithFirstOrLastChar",
    "w:applyBreakingRules",
    "w:doNotWrapTextWithPunct",
    "w:doNotUseEastAsianBreakRules",
    "w:useWord2002TableStyleRules",
    "w:growAutofit",
    "w:useFELayout",
    "w:useNormalStyleForList",
    "w:doNotUseIndentAsNumberingTabStop",
    "w:useAltKinsokuLineBreakRules",
    "w:allowSpaceOfSameStyleInTable",
    "w:doNotSuppressIndentation",
    "w:doNotAutofitConstrainedTables",
    "w:autofitToFirstFixedWidthCell",
    "w:underlineTabInNumList",
    "w:displayHangulFixedWidth",
    "w:splitPgBreakAndParaMark",
    "w:doNotVertAlignCellWithSp",
    "w:doNotBreakConstrainedForcedTable",
    "w:doNotVertAlignInTxbx",
    "w:useAnsiKerningForTwoByteTabs",
    "w:cachedColBalance",
    "w:compatSetting",
];

/// CT_Compat children between `w:balanceSingleByteDoubleByteWidth` and `w:suppressTopSpacingWP`
/// (ECMA-376 Part 1, §17.15.3, wml.xsd `CT_Compat`), in order.
const AFTER_BALANCE_SBCS_DBCS: &[&str] = &[
    "w:noExtraLineSpacing",
    "w:doNotLeaveBackslashAlone",
    "w:ulTrailSpace",
    "w:doNotExpandShiftReturn",
    "w:spacingInWholePoints",
    "w:lineWrapLikeWord6",
    "w:printBodyTextBeforeHeader",
    "w:printColBlack",
    "w:wpSpaceWidth",
    "w:showBreaksInFrames",
    "w:subFontBySize",
    "w:suppressBottomSpacing",
    "w:suppressTopSpacing",
    "w:suppressSpacingAtTopOfPage",
    "w:suppressTopSpacingWP",
];

const SUPPRESS_TOP_SPACING_WP: &str = "w:suppressTopSpacingWP";
const BALANCE_SBCS_DBCS: &str = "w:balanceSingleByteDoubleByteWidth";
const WORD97_LINE_BREAK_RULES: &str = "w:useWord97LineBreakRules";
const WRAP_TRAIL_SPACES: &str = "w:wrapTrailSpaces";
const FOOTNOTE_LAYOUT_LIKE_WW8: &str = "w:footnoteLayoutLikeWW8";

/// `compat` (a `w:compat` element) with a self-closing `<w:compat/>` written as start and end tags.
fn expand_compat(compat: &str) -> Option<String> {
    let open = compat.find("<w:compat")?;
    let end = open + tag_end(compat.get(open..)?)? + 1;
    let head = compat.get(..end)?;
    match head.strip_suffix("/>") {
        Some(start) => Some(format!("{}></w:compat>{}", start.trim_end(), compat.get(end..)?)),
        None => Some(compat.to_string()),
    }
}

/// `orig` (a `w:compat` element) with its `name` switches replaced by the generated one, if any,
/// placed before the first child that CT_Compat puts after `name` (one of `later`).
fn merge_switch(orig: &str, generated: &str, name: &str, later: &[&str]) -> Option<String> {
    let generated = expand_compat(generated)?;
    let (_, _, g_kids) = children(&generated, "w:compat")?;
    let wanted = match g_kids.iter().find(|k| k.name == name) {
        Some(k) => Some(generated.get(k.start..k.end)?),
        None => None,
    };
    let mut s = expand_compat(orig)?;
    let (_, _, kids) = children(&s, "w:compat")?;
    for k in kids.iter().rev().filter(|k| k.name == name) {
        s = format!("{}{}", s.get(..k.start)?, s.get(k.end..)?);
    }
    let Some(wanted) = wanted else { return Some(s) };
    let (_, close, kids) = children(&s, "w:compat")?;
    let at = kids.iter().find(|k| later.contains(&k.name.as_str())).map_or(close, |k| k.start);
    Some(format!("{}{wanted}{}", s.get(..at)?, s.get(at..)?))
}

/// The CT_Compat children that come after `name` (one of [`AFTER_SUPPRESS_TOP_SPACING_WP`]).
fn after(name: &str) -> Option<&'static [&'static str]> {
    let i = AFTER_SUPPRESS_TOP_SPACING_WP.iter().position(|n| *n == name)?;
    AFTER_SUPPRESS_TOP_SPACING_WP.get(i + 1..)
}

/// The original `w:compat` with its `compatibilityMode` and the switches the model owns
/// (`w:balanceSingleByteDoubleByteWidth`, `w:suppressTopSpacingWP`, `w:wrapTrailSpaces`,
/// `w:footnoteLayoutLikeWW8`, `w:useWord97LineBreakRules`) taken from the
/// generated one.
fn merge_compat(orig: &str, generated: &str) -> Option<String> {
    let mut s = merge_compat_mode(orig, generated)?;
    s = merge_switch(&s, generated, SUPPRESS_TOP_SPACING_WP, AFTER_SUPPRESS_TOP_SPACING_WP)?;
    let later_than_balance: Vec<&str> = AFTER_BALANCE_SBCS_DBCS.iter().chain(AFTER_SUPPRESS_TOP_SPACING_WP).copied().collect();
    s = merge_switch(&s, generated, BALANCE_SBCS_DBCS, &later_than_balance)?;
    s = merge_switch(&s, generated, WRAP_TRAIL_SPACES, after(WRAP_TRAIL_SPACES)?)?;
    s = merge_switch(&s, generated, FOOTNOTE_LAYOUT_LIKE_WW8, after(FOOTNOTE_LAYOUT_LIKE_WW8)?)?;
    merge_switch(&s, generated, WORD97_LINE_BREAK_RULES, after(WORD97_LINE_BREAK_RULES)?)
}

/// The original `w:compat` with its `compatibilityMode` taken from the generated one.
fn merge_compat_mode(orig: &str, generated: &str) -> Option<String> {
    let mode = |s: &str| -> Option<(usize, usize)> {
        let at = s.find("w:name=\"compatibilityMode\"")?;
        let tag_start = s.get(..at)?.rfind('<')?;
        let tag_end = tag_start + s.get(tag_start..)?.find('>')? + 1;
        Some((tag_start, tag_end))
    };
    match (mode(orig), mode(generated)) {
        (Some((a, b)), Some((c, d))) => Some(format!("{}{}{}", orig.get(..a)?, generated.get(c..d)?, orig.get(b..)?)),
        (None, Some((c, d))) => {
            // No mode in the original: add the generated one before `</w:compat>`, or expand a self-closing `<w:compat/>`.
            if let Some(close) = orig.rfind("</w:compat>") {
                Some(format!("{}{}{}", orig.get(..close)?, generated.get(c..d)?, orig.get(close..)?))
            } else if let Some(slash) = orig.rfind("/>") {
                let open = orig.get(..slash)?.trim_end();
                Some(format!("{open}>{}</w:compat>", generated.get(c..d)?))
            } else {
                None
            }
        }
        _ => Some(orig.to_string()),
    }
}

/// Does `original` (a settings part) hold anything the model doesn't write? Then the reader keeps
/// it for [`merge`]. False for parts WordCraft wrote itself, and for anything it can't scan.
pub fn has_unowned(original: &[u8]) -> bool {
    let Some(xml) = std::str::from_utf8(original).ok() else { return false };
    let Some((_, _, kids)) = children(xml, "w:settings") else { return false };
    kids.iter().any(|k| match k.name.as_str() {
        "w:zoom" => false,
        // Anything in `w:compat` beyond its own tag and the compatibility mode.
        "w:compat" => xml.get(k.start..k.end).is_some_and(|c| {
            let mode = usize::from(c.contains("w:name=\"compatibilityMode\""));
            c.matches("<w:").count().saturating_sub(1 + mode) > 0
        }),
        n => !OWNED.contains(&n),
    })
}

/// The merged settings part, or `None` to fall back to `generated`.
pub fn merge(original: &[u8], generated: &[u8]) -> Option<Vec<u8>> {
    let orig = std::str::from_utf8(original).ok()?;
    let gen_s = std::str::from_utf8(generated).ok()?;
    let (o_open, o_close, o_kids) = children(orig, "w:settings")?;
    let (_, _, g_kids) = children(gen_s, "w:settings")?;
    // Prefixes the generated children use must be declared by the original root.
    let root_tag = orig.get(..o_open)?;
    for p in ["w", "m", "r"] {
        if g_kids.iter().any(|k| k.name.starts_with(&format!("{p}:"))) && !root_tag.contains(&format!("xmlns:{p}=")) {
            return None;
        }
    }
    let g_text = |k: &Child| gen_s.get(k.start..k.end).map(str::to_string);
    let mut used = vec![false; g_kids.len()];
    let mark = |name: &str, used: &mut Vec<bool>| -> Option<usize> {
        let gi = g_kids.iter().enumerate().position(|(i, g)| g.name == name && !used.get(i).copied().unwrap_or(true))?;
        if let Some(u) = used.get_mut(gi) {
            *u = true;
        }
        Some(gi)
    };
    let mut out: Vec<(String, String)> = Vec::new();
    for k in &o_kids {
        let text = orig.get(k.start..k.end)?;
        if KEEP_ORIGINAL.contains(&k.name.as_str()) {
            // The model reads these only partly (math settings) or writes them only in one case
            // (the background shape flag, with a page colour): the original wins.
            mark(&k.name, &mut used);
            out.push((k.name.clone(), text.to_string()));
        } else if OWNED.contains(&k.name.as_str()) {
            if let Some(gi) = mark(&k.name, &mut used) {
                out.push((k.name.clone(), g_text(g_kids.get(gi)?)?));
            }
        } else if k.name == "w:compat" {
            let merged = match mark("w:compat", &mut used) {
                Some(gi) => merge_compat(text, &g_text(g_kids.get(gi)?)?)?,
                None => text.to_string(),
            };
            out.push((k.name.clone(), merged));
        } else if !text.contains("r:id=") {
            out.push((k.name.clone(), text.to_string()));
        }
    }
    for (gi, g) in g_kids.iter().enumerate() {
        let owned = OWNED.contains(&g.name.as_str()) || g.name == "w:compat";
        if used.get(gi).copied().unwrap_or(true) || !owned {
            continue;
        }
        let r = rank(&g.name);
        let at = out.iter().position(|(n, _)| rank(n) > r).unwrap_or(out.len());
        out.insert(at, (g.name.clone(), g_text(g)?));
    }
    let mut s = String::with_capacity(orig.len() + 256);
    s.push_str(root_tag);
    for (_, t) in &out {
        s.push_str(t);
    }
    s.push_str(orig.get(o_close..)?);
    Some(s.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEN: &str = r#"<w:settings xmlns:w="w" xmlns:m="m" xmlns:r="r"><w:zoom w:percent="100"/><w:defaultTabStop w:val="720"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="u" w:val="15"/></w:compat></w:settings>"#;

    #[test]
    fn odd_or_hostile_originals_fall_back_or_merge_without_panicking() {
        let odd = [
            "",
            "<w:settings",
            "<w:settings/>",
            "<x:other/>",
            r#"<w:settings xmlns:w="w"><w:a><w:b></w:a></w:settings>"#,
            r#"<w:settings xmlns:w="w"><![CDATA[x]]></w:settings>"#,
            r#"<w:settings xmlns:w="w"></w:x></w:settings>"#,
            "<w:settings xmlns:w=\"w\">\u{1F600}<w:rsids/></w:settings>",
            r#"<w:settings xmlns:w="w"><!-- c --><?pi x?><w:rsids/></w:settings>"#,
        ];
        for o in odd {
            let _ = merge(o.as_bytes(), GEN.as_bytes());
            let _ = has_unowned(o.as_bytes());
        }
        assert!(merge(&[0xff, 0xfe], GEN.as_bytes()).is_none());
        // Missing declarations for a prefix the generated part uses: fall back.
        assert!(merge(br#"<w:settings xmlns:x="x"><w:rsids/></w:settings>"#, GEN.as_bytes()).is_none());
    }

    #[test]
    fn a_gt_inside_an_attribute_value_does_not_hide_the_kept_settings() {
        for v in [r#"w:val="x>y""#, "w:val='x>y'"] {
            let orig = format!(
                r#"<w:settings xmlns:w="w" xmlns:m="m" xmlns:r="r"><w:docVar w:name="a" {v}/><w:docVar w:name="b" {v}></w:docVar></w:settings>"#
            );
            assert!(has_unowned(orig.as_bytes()), "{v}");
            let out = String::from_utf8(merge(orig.as_bytes(), GEN.as_bytes()).unwrap()).unwrap();
            assert!(out.contains(r#"w:name="a""#) && out.contains(r#"w:name="b""#), "{v}");
        }
    }

    #[test]
    fn a_compat_block_without_a_mode_gets_the_generated_one() {
        let orig = r#"<w:settings xmlns:w="w" xmlns:m="m" xmlns:r="r"><w:compat><w:useFELayout/></w:compat></w:settings>"#;
        let out = String::from_utf8(merge(orig.as_bytes(), GEN.as_bytes()).unwrap()).unwrap();
        assert!(out.contains("<w:useFELayout/>") && out.contains("compatibilityMode"), "{out}");
        assert!(has_unowned(orig.as_bytes()));
        assert!(!has_unowned(GEN.as_bytes()));
    }

    #[test]
    fn a_self_closing_compat_block_gets_the_generated_mode() {
        let orig = r#"<w:settings xmlns:w="w" xmlns:m="m" xmlns:r="r"><w:compat/><w:rsids><w:rsidRoot w:val="00112233"/></w:rsids></w:settings>"#;
        let out = String::from_utf8(merge(orig.as_bytes(), GEN.as_bytes()).unwrap()).unwrap();
        assert!(out.contains("<w:compat><w:compatSetting") && out.contains("</w:compat>"), "{out}");
        assert!(out.contains("<w:rsidRoot w:val=\"00112233\"/>"), "{out}");
    }

    fn merged(orig_compat: &str, gen_compat: &str) -> String {
        let orig = format!(r#"<w:settings xmlns:w="w" xmlns:m="m" xmlns:r="r">{orig_compat}<w:rsids/></w:settings>"#);
        let gen_s = format!(r#"<w:settings xmlns:w="w" xmlns:m="m" xmlns:r="r">{gen_compat}</w:settings>"#);
        String::from_utf8(merge(orig.as_bytes(), gen_s.as_bytes()).unwrap()).unwrap()
    }

    #[test]
    fn a_self_closing_compat_block_gets_the_generated_wrap_trail_spaces() {
        let out = merged("<w:compat/>", "<w:compat><w:wrapTrailSpaces/></w:compat>");
        assert!(out.contains("<w:compat><w:wrapTrailSpaces/></w:compat>"), "{out}");
        let out = merged("<w:compat/>", r#"<w:compat><w:wrapTrailSpaces/><w:compatSetting w:name="compatibilityMode" w:val="15"/></w:compat>"#);
        assert!(out.contains(r#"<w:compat><w:wrapTrailSpaces/><w:compatSetting w:name="compatibilityMode""#), "{out}");
        assert_eq!(out.matches("<w:wrapTrailSpaces").count(), 1, "{out}");
    }

    #[test]
    fn a_longer_name_with_the_same_prefix_is_kept() {
        let out = merged("<w:compat><w:wrapTrailSpacesX/><w:useFELayout/></w:compat>", "<w:compat/>");
        assert!(out.contains("<w:compat><w:wrapTrailSpacesX/><w:useFELayout/></w:compat>"), "{out}");
        let out = merged("<w:compat><w:wrapTrailSpacesX/><w:useFELayout/></w:compat>", "<w:compat><w:wrapTrailSpaces/></w:compat>");
        assert!(out.contains("<w:compat><w:wrapTrailSpacesX/><w:wrapTrailSpaces/><w:useFELayout/></w:compat>"), "{out}");
    }

    #[test]
    fn the_open_close_form_is_removed() {
        let out = merged(r#"<w:compat><w:wrapTrailSpaces w:val="1"></w:wrapTrailSpaces><w:useFELayout/></w:compat>"#, "<w:compat></w:compat>");
        assert!(!out.contains("wrapTrailSpaces"), "{out}");
        assert!(out.contains("<w:compat><w:useFELayout/></w:compat>"), "{out}");
    }

    #[test]
    fn suppress_top_spacing_wp_comes_from_the_model_once_in_ct_compat_order() {
        let orig = "<w:compat><w:noLeading/><w:suppressTopSpacingWP/><w:wrapTrailSpaces/></w:compat>";
        let out = merged(orig, "<w:compat><w:noLeading/><w:wrapTrailSpaces/></w:compat>");
        assert!(!out.contains("suppressTopSpacingWP"), "turned off in the model: {out}");
        let on = "<w:compat><w:noLeading/><w:suppressTopSpacingWP/><w:wrapTrailSpaces/></w:compat>";
        assert_eq!(merged(orig, on).matches("<w:suppressTopSpacingWP").count(), 1);
        let out = merged("<w:compat><w:noLeading/><w:usePrinterMetrics/><w:wrapTrailSpaces/></w:compat>", on);
        assert!(out.contains("<w:noLeading/><w:suppressTopSpacingWP/><w:usePrinterMetrics/><w:wrapTrailSpaces/>"), "{out}");
    }

    #[test]
    fn the_sbcs_dbcs_and_word97_pair_comes_from_the_model_once_in_ct_compat_order() {
        let orig = "<w:compat><w:noLeading/><w:balanceSingleByteDoubleByteWidth/><w:doNotExpandShiftReturn/><w:layoutTableRowsApart/><w:useWord97LineBreakRules/><w:useFELayout/></w:compat>";
        let out = merged(orig, "<w:compat/>");
        assert!(!out.contains("balanceSingle") && !out.contains("useWord97"), "turned off in the model: {out}");
        let on = "<w:compat><w:balanceSingleByteDoubleByteWidth/><w:useWord97LineBreakRules/></w:compat>";
        let out = merged(orig, on);
        assert_eq!(out.matches("<w:balanceSingleByteDoubleByteWidth").count(), 1, "{out}");
        assert_eq!(out.matches("<w:useWord97LineBreakRules").count(), 1, "{out}");
        let bare = "<w:compat><w:noLeading/><w:doNotExpandShiftReturn/><w:layoutTableRowsApart/><w:useFELayout/></w:compat>";
        let out = merged(bare, on);
        assert!(
            out.contains("<w:noLeading/><w:balanceSingleByteDoubleByteWidth/><w:doNotExpandShiftReturn/><w:layoutTableRowsApart/><w:useWord97LineBreakRules/><w:useFELayout/>"),
            "{out}"
        );
    }

    #[test]
    fn footnote_layout_like_ww8_comes_from_the_model_once_in_ct_compat_order() {
        let orig = "<w:compat><w:wrapTrailSpaces/><w:footnoteLayoutLikeWW8/><w:doNotUseHTMLParagraphAutoSpacing/></w:compat>";
        let out = merged(orig, "<w:compat><w:wrapTrailSpaces/></w:compat>");
        assert!(!out.contains("footnoteLayoutLikeWW8"), "turned off in the model: {out}");
        let out = merged(orig, "<w:compat><w:wrapTrailSpaces/><w:footnoteLayoutLikeWW8/></w:compat>");
        assert_eq!(out.matches("<w:footnoteLayoutLikeWW8").count(), 1, "{out}");
        let out = merged(
            "<w:compat><w:wrapTrailSpaces/><w:doNotUseHTMLParagraphAutoSpacing/></w:compat>",
            "<w:compat><w:wrapTrailSpaces/><w:footnoteLayoutLikeWW8/></w:compat>",
        );
        assert!(out.contains("<w:wrapTrailSpaces/><w:footnoteLayoutLikeWW8/><w:doNotUseHTMLParagraphAutoSpacing/>"), "{out}");
    }
}
