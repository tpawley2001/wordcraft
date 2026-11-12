//! A password-protected package is a compound file ([MS-CFB]) with `EncryptionInfo` and
//! `EncryptedPackage` streams. Some writers leave the FAT entries past the last sector of the
//! file marked ENDOFCHAIN instead of FREESECT; Word still asks for the password and opens the
//! document, so the encrypted-package sniff and the decryption must tolerate it the way the
//! `.doc` reader does (`wordcraft_doc::cfb_repair`). Every fixture is built here from bytes.

use std::io::{Cursor, Read, Write};

use wordcraft_doc::{Document, StoryRef};
use wordcraft_docx::{DocxError, decrypt, encrypt_standard_for_tests, encrypt_with_spin_count, is_encrypted, read, read_with_password};

const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
const FREE: u32 = 0xFFFF_FFFF;
const PASSWORD: &str = "open sesame";
const TEXT: &str = "Quarterly figures, not for release";

fn package() -> Vec<u8> {
    wordcraft_docx::write(&Document::from_text(TEXT)).unwrap()
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// The two streams of an encrypted package.
fn streams(file: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut comp = cfb::CompoundFile::open(Cursor::new(file)).unwrap();
    let mut get = |path: &str| {
        let mut v = Vec::new();
        comp.open_stream(path).unwrap().read_to_end(&mut v).unwrap();
        v
    };
    (get("/EncryptionInfo"), get("/EncryptedPackage"))
}

/// A compound file of the given version holding the two streams under the given names.
fn repack(file: &[u8], version: cfb::Version, info_name: &str, package_name: &str) -> Vec<u8> {
    let (info, package) = streams(file);
    let mut comp = cfb::CompoundFile::create_with_version(version, Cursor::new(Vec::new())).unwrap();
    comp.create_stream(info_name).unwrap().write_all(&info).unwrap();
    comp.create_stream(package_name).unwrap().write_all(&package).unwrap();
    comp.flush().unwrap();
    comp.into_inner().into_inner()
}

/// Mark every FAT entry that lies past the last sector of the file ENDOFCHAIN, as the corpus
/// files have them ([MS-CFB] §2.3 wants FREESECT there). Returns how many entries changed.
fn stale_end_markers(file: &mut [u8]) -> usize {
    let sector_len = 1usize << u16::from_le_bytes([file[0x1E], file[0x1F]]);
    let sectors = file.len().div_ceil(sector_len) - 1;
    let entries = sector_len / 4;
    let mut changed = 0;
    for ordinal in 0..109 {
        let fat = u32_at(file, 0x4C + ordinal * 4);
        if fat == FREE {
            break;
        }
        let at = (fat as usize + 1) * sector_len;
        for slot in 0..entries {
            let offset = at + slot * 4;
            if ordinal * entries + slot >= sectors && u32_at(file, offset) == FREE {
                file[offset..offset + 4].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
                changed += 1;
            }
        }
    }
    changed
}

/// Asks for the password, opens with it, refuses a wrong one.
fn assert_opens_with_its_password(file: &[u8], what: &str) {
    assert!(is_encrypted(file), "{what}: recognised as password-protected");
    assert_eq!(read(file).unwrap_err(), DocxError::PasswordRequired, "{what}: read asks for the password");
    assert_eq!(read_with_password(file, None).unwrap_err(), DocxError::PasswordRequired, "{what}: no password");
    assert_eq!(decrypt(file, Some("Open sesame")).unwrap_err(), DocxError::WrongPassword, "{what}: wrong password");
    assert_eq!(decrypt(file, Some(PASSWORD)).unwrap(), package(), "{what}: the package comes back whole");
    let doc = read_with_password(file, Some(PASSWORD)).unwrap();
    assert_eq!(doc.plain_text(StoryRef::Body).trim_end(), TEXT, "{what}: the text");
}

fn agile() -> Vec<u8> {
    encrypt_with_spin_count(&package(), PASSWORD, 16).unwrap()
}

#[test]
fn the_fixture_helpers_build_what_they_say() {
    let healthy = repack(&agile(), cfb::Version::V3, "/EncryptionInfo", "/EncryptedPackage");
    assert_eq!(u16::from_le_bytes([healthy[0x1E], healthy[0x1F]]), 9, "512-byte sectors");
    assert_opens_with_its_password(&healthy, "healthy 512-byte-sector file");
    let mut stale = healthy.clone();
    let changed = stale_end_markers(&mut stale);
    assert!(changed > 0, "the fixture has FAT entries past the end of the file");
    assert_eq!(stale.len(), healthy.len());
    // The only difference is those FAT entries.
    let differing = healthy.iter().zip(&stale).filter(|(a, b)| a != b).count();
    assert_eq!(differing, changed, "one byte per entry: FF FF FF FF -> FE FF FF FF");
}

#[test]
fn stale_end_markers_past_the_end_of_a_512_byte_sector_file_still_ask_for_the_password() {
    let mut file = repack(&agile(), cfb::Version::V3, "/EncryptionInfo", "/EncryptedPackage");
    assert!(stale_end_markers(&mut file) > 0);
    assert_opens_with_its_password(&file, "agile, 512-byte sectors, stale FAT end markers");
}

#[test]
fn stale_end_markers_past_the_end_of_a_4096_byte_sector_file_still_ask_for_the_password() {
    let mut file = repack(&agile(), cfb::Version::V4, "/EncryptionInfo", "/EncryptedPackage");
    assert_eq!(u16::from_le_bytes([file[0x1E], file[0x1F]]), 12, "4096-byte sectors");
    assert!(stale_end_markers(&mut file) > 0);
    assert_opens_with_its_password(&file, "agile, 4096-byte sectors, stale FAT end markers");
}

#[test]
fn stale_end_markers_in_a_file_as_encrypt_writes_it_still_ask_for_the_password() {
    // The whole file as `encrypt` writes it (with its data-space storages), not re-packed.
    let mut file = agile();
    assert!(stale_end_markers(&mut file) > 0);
    assert_opens_with_its_password(&file, "agile with data spaces, stale FAT end markers");
}

#[test]
fn stale_end_markers_in_a_standard_encrypted_file_still_ask_for_the_password() {
    let standard = encrypt_standard_for_tests(&package(), PASSWORD, 128).unwrap();
    let mut file = repack(&standard, cfb::Version::V3, "/EncryptionInfo", "/EncryptedPackage");
    assert!(stale_end_markers(&mut file) > 0);
    assert_opens_with_its_password(&file, "standard encryption, stale FAT end markers");
}

#[test]
fn upper_case_stream_names_with_stale_end_markers_still_ask_for_the_password() {
    // Compound-file names compare case-insensitively ([MS-CFB] §2.6.4).
    let mut file = repack(&agile(), cfb::Version::V3, "/ENCRYPTIONINFO", "/ENCRYPTEDPACKAGE");
    assert_opens_with_its_password(&file, "upper-case stream names");
    assert!(stale_end_markers(&mut file) > 0);
    assert_opens_with_its_password(&file, "upper-case stream names, stale FAT end markers");
}

#[test]
fn what_is_not_an_encrypted_package_stays_refused() {
    // A plain package.
    assert!(!is_encrypted(&package()));
    // A compound file without the two streams, healthy and with stale markers.
    let mut other = {
        let mut comp = cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new())).unwrap();
        comp.create_stream("/WordDocument").unwrap().write_all(&[0u8; 600]).unwrap();
        comp.flush().unwrap();
        comp.into_inner().into_inner()
    };
    assert!(!is_encrypted(&other));
    assert!(stale_end_markers(&mut other) > 0);
    assert!(!is_encrypted(&other));
    // Only one of the two streams.
    let (info, _) = streams(&agile());
    let mut half = {
        let mut comp = cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new())).unwrap();
        comp.create_stream("/EncryptionInfo").unwrap().write_all(&info).unwrap();
        comp.flush().unwrap();
        comp.into_inner().into_inner()
    };
    assert!(stale_end_markers(&mut half) > 0);
    assert!(!is_encrypted(&half));
    // The hostile DIFAT of #3642 is still refused before `cfb` builds its FAT.
    let hostile = wordcraft_doc::cfb_guard::repeating_difat_sample();
    assert!(!is_encrypted(&hostile));
    assert!(matches!(decrypt(&hostile, None), Err(e) if e.to_string().contains("FAT sectors in a file of")));
}
