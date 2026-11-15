//! Windows language identifiers (LCIDs, [MS-LCID] 2.2), as RTF's `\langN`, `\langfeN`, `\alangN`
//! and `\deflangN` give them, and the language tags DOCX `w:lang` uses.

/// The LCID and language tag pairs this module knows.
const LCIDS: &[(i32, &str)] = &[
    (0x0401, "ar-SA"),
    (0x0402, "bg-BG"),
    (0x0403, "ca-ES"),
    (0x0404, "zh-TW"),
    (0x0405, "cs-CZ"),
    (0x0406, "da-DK"),
    (0x0407, "de-DE"),
    (0x0408, "el-GR"),
    (0x0409, "en-US"),
    (0x040A, "es-ES_tradnl"),
    (0x040B, "fi-FI"),
    (0x040C, "fr-FR"),
    (0x040D, "he-IL"),
    (0x040E, "hu-HU"),
    (0x040F, "is-IS"),
    (0x0410, "it-IT"),
    (0x0411, "ja-JP"),
    (0x0412, "ko-KR"),
    (0x0413, "nl-NL"),
    (0x0414, "nb-NO"),
    (0x0415, "pl-PL"),
    (0x0416, "pt-BR"),
    (0x0418, "ro-RO"),
    (0x0419, "ru-RU"),
    (0x041A, "hr-HR"),
    (0x041B, "sk-SK"),
    (0x041D, "sv-SE"),
    (0x041E, "th-TH"),
    (0x041F, "tr-TR"),
    (0x0420, "ur-PK"),
    (0x0421, "id-ID"),
    (0x0422, "uk-UA"),
    (0x0424, "sl-SI"),
    (0x0425, "et-EE"),
    (0x0426, "lv-LV"),
    (0x0427, "lt-LT"),
    (0x0429, "fa-IR"),
    (0x042A, "vi-VN"),
    (0x0439, "hi-IN"),
    (0x043E, "ms-MY"),
    (0x0804, "zh-CN"),
    (0x0807, "de-CH"),
    (0x0809, "en-GB"),
    (0x080A, "es-MX"),
    (0x080C, "fr-BE"),
    (0x0813, "nl-BE"),
    (0x0816, "pt-PT"),
    (0x0C04, "zh-HK"),
    (0x0C07, "de-AT"),
    (0x0C09, "en-AU"),
    (0x0C0A, "es-ES"),
    (0x0C0C, "fr-CA"),
    (0x1009, "en-CA"),
    (0x100C, "fr-CH"),
    (0x1409, "en-NZ"),
    (0x1809, "en-IE"),
    (0x1C09, "en-ZA"),
    (0x4009, "en-IN"),
];

/// The language tag for LCID `lid`, or `None` for "no language" (0, 0x0400, 0x0800) and any LCID
/// not in the table.
pub(crate) fn lcid_tag(lid: i32) -> Option<&'static str> {
    LCIDS.iter().find(|(n, _)| *n == lid).map(|(_, t)| *t)
}

/// The LCID for language tag `tag` (compared ignoring case), or `None` when the table has none.
pub(crate) fn tag_lcid(tag: &str) -> Option<i32> {
    LCIDS.iter().find(|(_, t)| t.eq_ignore_ascii_case(tag)).map(|(n, _)| *n)
}
