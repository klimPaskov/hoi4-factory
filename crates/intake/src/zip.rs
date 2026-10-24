//! A small, strict ZIP central-directory reader.
//!
//! Intake needs only a list of entries and a way to read one entry's bytes. A hand-written
//! reader keeps the parsed surface small and every decision visible:
//!
//! * the end-of-central-directory record must end exactly at the end of the file, so a
//!   comment that contains a second record cannot fool the parser
//! * the entry count and directory size are bounded **before** the directory is read, so a
//!   header that claims millions of entries costs nothing
//! * only stored and deflate entries are read. Encryption, AES, and every other method
//!   are refused
//! * a local header must repeat the central directory name, and entry data must lie before
//!   the central directory
//! * sizes come from the central directory only, and the extractor enforces them while
//!   streaming, so a lying header cannot expand past what the limits allow
//!
//! No part of the archive is ever extracted by this module. See [`crate::extract`].

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use crate::error::{IntakeError, shorten};

const SIG_EOCD: u32 = 0x0605_4b50;
const SIG_ZIP64_LOCATOR: u32 = 0x0706_4b50;
const SIG_ZIP64_EOCD: u32 = 0x0606_4b50;
const SIG_CENTRAL: u32 = 0x0201_4b50;
const SIG_LOCAL: u32 = 0x0403_4b50;
const EOCD_LEN: usize = 22;
const CENTRAL_FIXED_LEN: usize = 46;
const LOCAL_FIXED_LEN: usize = 30;
const MAX_COMMENT: usize = 65_535;
/// Upper bound on the central directory read into memory.
pub(crate) const MAX_CENTRAL_DIRECTORY: u64 = 64 * 1024 * 1024;

pub(crate) const METHOD_STORED: u16 = 0;
pub(crate) const METHOD_DEFLATE: u16 = 8;
const METHOD_AES: u16 = 99;

/// What the external attributes say the entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredKind {
    File,
    Directory,
    Symlink,
    /// Device, FIFO or socket.
    Device,
    /// A second name for an existing file. ZIP cannot express it. Folder uploads that
    /// come from the file system can.
    Hardlink,
}

#[derive(Debug, Clone)]
pub(crate) struct ZipEntry {
    pub name: String,
    pub name_raw: Vec<u8>,
    pub method: u16,
    pub crc32: u32,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub local_offset: u64,
    pub kind: DeclaredKind,
    /// Unix permission bits when the entry carries them (`0o7777` mask), else `None`.
    pub unix_mode: Option<u32>,
}

#[derive(Debug)]
pub(crate) struct ZipIndex {
    pub entries: Vec<ZipEntry>,
    pub central_offset: u64,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(raw)
}

fn malformed(reason: &str) -> IntakeError {
    IntakeError::ArchiveMalformed(reason.to_owned())
}

fn read_at(file: &mut File, offset: u64, len: usize) -> Result<Vec<u8>, IntakeError> {
    file.seek(SeekFrom::Start(offset))
        .map_err(IntakeError::io("seek archive"))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf).map_err(|err| {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            malformed("archive is truncated")
        } else {
            IntakeError::Io {
                operation: "read archive",
                source: err,
            }
        }
    })?;
    Ok(buf)
}

/// Where the central directory is and how many entries it claims.
struct DirectoryLocation {
    entries: u64,
    size: u64,
    offset: u64,
}

fn locate_directory(file: &mut File, file_len: u64) -> Result<DirectoryLocation, IntakeError> {
    if file_len < EOCD_LEN as u64 {
        return Err(IntakeError::NotAZipArchive);
    }
    let tail_len = file_len.min((EOCD_LEN + MAX_COMMENT) as u64);
    let tail_start = file_len - tail_len;
    let tail = read_at(file, tail_start, tail_len as usize)?;

    // The rightmost record whose comment length reaches exactly the end of the file.
    let mut found = None;
    for pos in (0..=tail.len() - EOCD_LEN).rev() {
        if u32_at(&tail, pos) == SIG_EOCD {
            let comment = u16_at(&tail, pos + 20) as usize;
            if pos + EOCD_LEN + comment == tail.len() {
                found = Some(pos);
                break;
            }
        }
    }
    let pos = found.ok_or(IntakeError::NotAZipArchive)?;
    let eocd_abs = tail_start + pos as u64;
    let disk = u16_at(&tail, pos + 4);
    let cd_disk = u16_at(&tail, pos + 6);
    if disk != 0 || cd_disk != 0 {
        return Err(malformed("multi-disk archives are not supported"));
    }
    let entries = u64::from(u16_at(&tail, pos + 10));
    let size = u64::from(u32_at(&tail, pos + 12));
    let offset = u64::from(u32_at(&tail, pos + 16));
    let needs_zip64 = entries == 0xFFFF || size == 0xFFFF_FFFF || offset == 0xFFFF_FFFF;
    let location = if needs_zip64 {
        if eocd_abs < 20 {
            return Err(malformed("zip64 record is missing"));
        }
        let locator = read_at(file, eocd_abs - 20, 20)?;
        if u32_at(&locator, 0) != SIG_ZIP64_LOCATOR {
            return Err(malformed("zip64 locator is missing"));
        }
        if u32_at(&locator, 4) != 0 || u32_at(&locator, 16) != 1 {
            return Err(malformed("multi-disk archives are not supported"));
        }
        let record_offset = u64_at(&locator, 8);
        if record_offset
            .checked_add(56)
            .is_none_or(|end| end > eocd_abs - 20)
        {
            return Err(malformed("zip64 record lies outside the archive"));
        }
        let record = read_at(file, record_offset, 56)?;
        if u32_at(&record, 0) != SIG_ZIP64_EOCD {
            return Err(malformed("zip64 record signature is wrong"));
        }
        if u32_at(&record, 16) != 0 || u32_at(&record, 20) != 0 {
            return Err(malformed("multi-disk archives are not supported"));
        }
        DirectoryLocation {
            entries: u64_at(&record, 32),
            size: u64_at(&record, 40),
            offset: u64_at(&record, 48),
        }
    } else {
        DirectoryLocation {
            entries,
            size,
            offset,
        }
    };
    if location
        .offset
        .checked_add(location.size)
        .is_none_or(|end| end > eocd_abs)
    {
        return Err(malformed("central directory lies outside the archive"));
    }
    Ok(location)
}

/// Parse the central directory of the archive in `file`.
pub(crate) fn read_index(file: &mut File, max_entries: u64) -> Result<ZipIndex, IntakeError> {
    let file_len = file
        .metadata()
        .map_err(IntakeError::io("inspect archive"))?
        .len();
    let location = locate_directory(file, file_len)?;
    if location.entries > max_entries {
        return Err(IntakeError::TooManyEntries { limit: max_entries });
    }
    if location.size > MAX_CENTRAL_DIRECTORY
        || location.entries.saturating_mul(CENTRAL_FIXED_LEN as u64) > location.size
    {
        return Err(malformed("central directory size is implausible"));
    }
    if location.entries > 0 {
        let first = read_at(file, 0, 4)?;
        if u32_at(&first, 0) != SIG_LOCAL {
            return Err(malformed("data precedes the first entry"));
        }
    }
    let directory = read_at(file, location.offset, location.size as usize)?;
    let mut entries = Vec::with_capacity(location.entries as usize);
    let mut at = 0usize;
    for _ in 0..location.entries {
        if directory.len() < at + CENTRAL_FIXED_LEN || u32_at(&directory, at) != SIG_CENTRAL {
            return Err(malformed("central directory record is damaged"));
        }
        let version_made_by = u16_at(&directory, at + 4);
        let flags = u16_at(&directory, at + 8);
        let method = u16_at(&directory, at + 10);
        let crc32 = u32_at(&directory, at + 16);
        let mut compressed = u64::from(u32_at(&directory, at + 20));
        let mut uncompressed = u64::from(u32_at(&directory, at + 24));
        let name_len = u16_at(&directory, at + 28) as usize;
        let extra_len = u16_at(&directory, at + 30) as usize;
        let comment_len = u16_at(&directory, at + 32) as usize;
        let mut disk_start = u32::from(u16_at(&directory, at + 34));
        let external = u32_at(&directory, at + 38);
        let mut local_offset = u64::from(u32_at(&directory, at + 42));
        let record_end = at + CENTRAL_FIXED_LEN + name_len + extra_len + comment_len;
        if record_end > directory.len() {
            return Err(malformed("central directory record is truncated"));
        }
        let name_raw =
            directory[at + CENTRAL_FIXED_LEN..at + CENTRAL_FIXED_LEN + name_len].to_vec();
        let extra = &directory
            [at + CENTRAL_FIXED_LEN + name_len..at + CENTRAL_FIXED_LEN + name_len + extra_len];

        // Zip64 extended information: values appear only for fields set to the sentinel.
        let mut cursor = 0usize;
        while cursor + 4 <= extra.len() {
            let id = u16_at(extra, cursor);
            let size = u16_at(extra, cursor + 2) as usize;
            let body_end = cursor + 4 + size;
            if body_end > extra.len() {
                return Err(malformed("extra field is truncated"));
            }
            if id == 0x0001 {
                let body = &extra[cursor + 4..body_end];
                let mut used = 0usize;
                let take64 = |used: &mut usize| -> Result<u64, IntakeError> {
                    if *used + 8 > body.len() {
                        return Err(malformed("zip64 extra field is too short"));
                    }
                    let value = u64_at(body, *used);
                    *used += 8;
                    Ok(value)
                };
                if uncompressed == 0xFFFF_FFFF {
                    uncompressed = take64(&mut used)?;
                }
                if compressed == 0xFFFF_FFFF {
                    compressed = take64(&mut used)?;
                }
                if local_offset == 0xFFFF_FFFF {
                    local_offset = take64(&mut used)?;
                }
                if disk_start == 0xFFFF {
                    if used + 4 > body.len() {
                        return Err(malformed("zip64 extra field is too short"));
                    }
                    disk_start = u32_at(body, used);
                }
            }
            cursor = body_end;
        }
        if disk_start != 0 {
            return Err(malformed("multi-disk archives are not supported"));
        }

        let name_text = match std::str::from_utf8(&name_raw) {
            Ok(text) => text.to_owned(),
            Err(_) => {
                return Err(IntakeError::PathRejected {
                    path: shorten(&String::from_utf8_lossy(&name_raw)),
                    reason: "entry name is not valid UTF-8".into(),
                });
            }
        };
        let encrypted = flags & 0x0001 != 0 || flags & 0x0040 != 0 || method == METHOD_AES;
        if encrypted {
            return Err(IntakeError::EncryptedEntry {
                path: shorten(&name_text),
            });
        }
        let (kind, unix_mode) = classify(&name_text, external, version_made_by);
        entries.push(ZipEntry {
            name: name_text,
            name_raw,
            method,
            crc32,
            compressed_size: compressed,
            uncompressed_size: uncompressed,
            local_offset,
            kind,
            unix_mode,
        });
        at = record_end;
    }
    if at != directory.len() {
        return Err(malformed(
            "unexpected data after the last central directory record",
        ));
    }
    Ok(ZipIndex {
        entries,
        central_offset: location.offset,
    })
}

/// Entry kind from the name and external attributes. The high 16 bits are read as Unix
/// mode bits whatever the creating system claims, because tools disagree about that field
/// and the stricter reading is the safe one.
fn classify(name: &str, external: u32, _version_made_by: u16) -> (DeclaredKind, Option<u32>) {
    let mode = external >> 16;
    let dos_directory = external & 0x10 != 0;
    let kind = match mode & 0o170000 {
        0o120000 => DeclaredKind::Symlink,
        0o140000 | 0o060000 | 0o020000 | 0o010000 => DeclaredKind::Device,
        0o040000 => DeclaredKind::Directory,
        0o100000 => DeclaredKind::File,
        _ if name.ends_with('/') || dos_directory => DeclaredKind::Directory,
        _ => DeclaredKind::File,
    };
    let perms = if mode & 0o170000 != 0 {
        Some(mode & 0o7777)
    } else {
        None
    };
    (kind, perms)
}

/// Open the data of one entry. The reader yields the entry's decompressed bytes and
/// nothing else. The caller enforces the size and checks the CRC-32.
pub(crate) fn open_entry<'a>(
    file: &'a mut File,
    index: &ZipIndex,
    entry: &ZipEntry,
) -> Result<Box<dyn Read + 'a>, IntakeError> {
    let header = read_at(file, entry.local_offset, LOCAL_FIXED_LEN)?;
    if u32_at(&header, 0) != SIG_LOCAL {
        return Err(malformed("local header signature is wrong"));
    }
    let name_len = u16_at(&header, 26) as usize;
    let extra_len = u16_at(&header, 28) as u64;
    if name_len != entry.name_raw.len() {
        return Err(malformed("local header name differs from the directory"));
    }
    let local_name = read_at(file, entry.local_offset + LOCAL_FIXED_LEN as u64, name_len)?;
    if local_name != entry.name_raw {
        return Err(malformed("local header name differs from the directory"));
    }
    let data_start = entry.local_offset + LOCAL_FIXED_LEN as u64 + name_len as u64 + extra_len;
    if data_start
        .checked_add(entry.compressed_size)
        .is_none_or(|end| end > index.central_offset)
    {
        return Err(malformed("entry data lies outside the archive"));
    }
    file.seek(SeekFrom::Start(data_start))
        .map_err(IntakeError::io("seek archive"))?;
    let raw = file.take(entry.compressed_size);
    match entry.method {
        METHOD_STORED => {
            if entry.compressed_size != entry.uncompressed_size {
                return Err(malformed("stored entry sizes differ"));
            }
            Ok(Box::new(raw))
        }
        METHOD_DEFLATE => Ok(Box::new(flate2::read::DeflateDecoder::new(raw))),
        _ => Err(IntakeError::UnsupportedCompression {
            path: shorten(&entry.name),
        }),
    }
}
