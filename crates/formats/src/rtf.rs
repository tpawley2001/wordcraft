//! Rich Text Format.
//!
//! Export: font and colour tables, a stylesheet (headings with outline levels, Title, Quote,
//! Code, then the document's own paragraph and character styles from number 10 with their
//! based-on, next and linked styles and flags; `\sN` and `\csN` on the text that uses them), list tables (`\listtable` / `\listoverridetable` with `\listtext` labels for
//! older readers), document info, page setup, paragraph formatting (`\pard`, alignment, page
//! break before), character formatting (`\b \i \ul \strike \super \sub \f \fs \cf \chcbpat`),
//! hyperlinks as `HYPERLINK` fields, bookmarks, pictures (`\pngblip` / `\jpegblip`), tables
//! (`\trowd … \cellx … \cell … \row`, header rows, vertical merges, shading), footnotes and
//! endnotes (`{\super\chftn}{\footnote[\ftnalt] …}`) and comments (`{\*\atrfstart n}` …
//! `{\*\atrfend n}` around the commented text, then `{\*\atnid initials}{\*\atnauthor name}`
//! `\chatn{\*\annotation{\*\atnref n}{\*\atndate …}` text `}`).
//!
//! Import: a tolerant reader for groups and destinations (unknown `\*` destinations are skipped;
//! objects show their result; headers, footers, shape text, notes and comments are read into
//! stories of their own), `\par \line \tab \page`, `\uN` with `\ucN`
//! fallback skipping, `\'hh` and 8-bit text in the document's code page (`\ansicpg`, `\mac`, `\pc`, `\pca`) or
//! the current font's (`\fcharset`, `\cpg`: Cyrillic, Central European, Greek, Mac OS Roman, the
//! East Asian multibyte pages 932/936/949/950…; Windows-1252 when none is named). A multibyte
//! code page's bytes stream through one decoder, so a lead byte and its trail byte decode
//! together and a run that looks like a UTF-8 BOM is not mistaken for one; the formatting words
//! above, stylesheet-based
//! headings and `\outlinelevel`, the document's own paragraph (`\sN`) and character
//! (`\*\csN`) styles with `\sbasedon`, `\snext`, `\slink`, `\sqformat`, `\spriority`,
//! `\ssemihidden`/`\shidden` and `\slocked`, on the paragraphs and runs that use them, lists through `\ls`/`\ilvl`, fields, bookmarks, pictures and
//! tables, footnotes, endnotes (`\footnote` with `\ftnalt`, or any `\footnote` when the file has
//! no `\ftnbj`/`\ftntj` and `\fet` is absent or 0, as Word reads it) and comments (author, initials, date,
//! text, the commented range). Group nesting, picture sizes and the number of notes and comments
//! are capped. Not kept: custom note marks, comment replies (`\atnparent`), notes inside
//! headers, footers and other notes, tables inside note and comment texts (flattened).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use wordcraft_doc::props::VAlign;
use wordcraft_doc::{Align, Block, Blocks, Document, InlineObject, Paragraph, Rgb};

use crate::model::{self, Cell, FBlock, FTable, Flow, Fmt, Inline, Kind, ListInfo, Meta, Para, RevMark, make_img};

mod date;
mod export;
mod form_field;
mod frames;
mod note_marks;
mod parse;
mod reader;
mod state;
mod styles;
mod text;
mod word;

pub use date::{dttm_to_iso, iso_to_dttm};
#[cfg(test)]
use export::{Tables, Writer};
pub use export::{esc, export};
pub use parse::{import, parse};
use reader::Reader;
#[cfg(test)]
use state::MAX_SECTIONS;
#[cfg(test)]
use word::hyperlink;

// The RTF tests keep their acceptance files exactly as committed (long fixture lines included),
// so rustfmt leaves them as written.
#[cfg(test)]
#[rustfmt::skip]
mod tests;

#[cfg(test)]
mod char_border_edges;
#[cfg(test)]
mod gutter_edges;
#[cfg(test)]
mod info_edges;
#[cfg(test)]
mod own_style_edges;
#[cfg(test)]
mod shape_regressions;
mod shapes;
