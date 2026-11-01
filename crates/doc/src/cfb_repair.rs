//! Repairs, in a copy, compound-file defects Word tolerates before the `cfb` crate reads the
//! file ([MS-CFB] §2.3, §2.6.1). Stub: to be implemented.

use std::borrow::Cow;

/// Repair stale FAT entries past the end of the file and junk in unused directory entries.
/// Returns the bytes unchanged (borrowed) when nothing needs repair.
pub fn repair(bytes: &[u8]) -> Cow<'_, [u8]> {
    Cow::Borrowed(bytes)
}
