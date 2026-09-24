//! Where the bytes of a photograph live.
//!
//! **A seam with a working default, which is D39's shape rather than a
//! preference.** *"The system is complete on its own. Every external system is
//! one implementation of a seam that has a working default behind it."* A
//! deployment running no object store must still be able to photograph a
//! carton, so the default is a directory and an S3 or R2 implementation is the
//! same trait later.
//!
//! # Content addressing
//!
//! A file is named by the SHA-256 of its own bytes, fanned out two levels so no
//! directory holds a hundred thousand entries. Three things follow, and none of
//! them had to be built:
//!
//! **Writing is idempotent.** Photographing an unchanged carton twice produces
//! one file. A retake that changes nothing costs nothing, and a request retried
//! after a timeout cannot produce a duplicate.
//!
//! **The name is a checksum.** A file that has rotted or been half-written is
//! detectable by reading it, without a second column to compare against.
//!
//! **Deletion is a reference question rather than a file question**, which is
//! the shape D30 already answers for the ledger: bytes are removable when no
//! row addresses them, and that is a query rather than a guess.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::fs;

use crate::error::ApiError;

/// What the store will accept. A photograph of a carton is not a video and not
/// a scan of a document; the list is short on purpose and widening it is a
/// decision rather than a header.
pub const ACCEPTED: [&str; 3] = ["image/jpeg", "image/png", "image/webp"];

/// The sides of a box, plus the one that is not a side.
///
/// `label` is the seventh and the most useful: it is the carrier label, the
/// barcode and the lot code, which is what D28's unresolvable-scan question
/// eventually wants a picture of.
pub const FACES: [&str; 7] = ["front", "back", "left", "right", "top", "bottom", "label"];

/// **A photograph that is not a side of the object.** A crushed corner, a torn
/// seal, whatever a record is being asked to believe.
///
/// Deliberately outside `FACES` rather than an eighth member of it. D141
/// resolves the picker's recognition picture from `front`, and evidence filed
/// among the sides would eventually be picked up by a resolution that walks
/// them — so keeping it out is the structural half of the rule, and migration
/// 84 is the other. D140.
pub const DETAIL: &str = "detail";

pub fn is_face(s: &str) -> bool {
    FACES.contains(&s) || s == DETAIL
}

/// Above this an upload is refused rather than truncated.
///
/// A handheld camera at full resolution is a few megabytes; twelve is generous
/// for one face of a box and small enough that a phone on warehouse wifi is not
/// uploading for a minute. The number is a limit on the *transfer*, not a
/// judgement about the picture.
pub const MAX_BYTES: usize = 12 * 1024 * 1024;

/// Where the files go. Overridable so `cargo run` and the tests do not write
/// into an image's directory.
pub fn directory() -> PathBuf {
    std::env::var("SPORK_IMAGE_DIR")
        .unwrap_or_else(|_| "/var/lib/spork/images".to_string())
        .into()
}

/// The address of these bytes.
pub fn digest_of(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// `<root>/ab/cd/abcd…` — two levels of fan-out, so a tenant with a hundred
/// thousand photographs does not have a hundred thousand entries in one
/// directory, which is where several filesystems stop being fast.
fn path_for(root: &Path, digest: &str) -> PathBuf {
    root.join(&digest[0..2]).join(&digest[2..4]).join(digest)
}

/// Write bytes and return their address.
///
/// **Written to a temporary name and renamed into place**, because `rename` is
/// atomic within a filesystem and a plain write is not: a process that dies
/// mid-write otherwise leaves a short file under a name that promises a
/// checksum, and every later reader believes it.
///
/// An existing file at the address is left alone. It is the same bytes — that
/// is what the address means.
pub async fn put(root: &Path, bytes: &[u8]) -> Result<String, ApiError> {
    let digest = digest_of(bytes);
    let target = path_for(root, &digest);

    if fs::metadata(&target).await.is_ok() {
        return Ok(digest);
    }

    let parent = target
        .parent()
        .ok_or_else(|| ApiError::Rejected("image path has no parent".into()))?;
    fs::create_dir_all(parent)
        .await
        .map_err(|e| ApiError::Rejected(format!("could not open the image store: {e}")))?;

    let temporary = parent.join(format!(".{digest}.part"));
    fs::write(&temporary, bytes)
        .await
        .map_err(|e| ApiError::Rejected(format!("could not write the image: {e}")))?;
    fs::rename(&temporary, &target)
        .await
        .map_err(|e| ApiError::Rejected(format!("could not place the image: {e}")))?;

    Ok(digest)
}

/// Read bytes back by address. `None` when the row survives and the file does
/// not, which is a real state worth reporting rather than a 500: the database
/// and the volume are two things and can be restored separately.
pub async fn get(root: &Path, digest: &str) -> Result<Option<Vec<u8>>, ApiError> {
    if !is_digest(digest) {
        return Err(ApiError::Rejected("that is not a content address".into()));
    }
    match fs::read(path_for(root, digest)).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ApiError::Rejected(format!("could not read the image: {e}"))),
    }
}

/// **Checked before it reaches the filesystem.** A digest arrives in a URL, and
/// a path built from unvalidated input is how `..` becomes an arbitrary read.
/// Sixty-four lower-case hex characters cannot traverse anything.
pub fn is_digest(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What the bytes are, read from the bytes rather than from the request.
///
/// A client states a content type and a client can be wrong or lying. These are
/// the magic numbers of the three formats the store accepts, and disagreement
/// with the declared type is the request being refused — a file stored as one
/// thing and served as another is how an image endpoint becomes an XSS.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Pixel dimensions, where the header says so plainly.
///
/// Only the two formats whose size sits at a fixed offset. WebP has several
/// container variants and parsing them properly is a decoder — a photograph
/// whose dimensions we did not read is still a photograph, so the columns are
/// nullable and this returns `None` rather than guessing.
/// A size, or nothing, so that a failed parse never reads as a real dimension.
fn positive(w: i32, h: i32) -> Option<(i32, i32)> {
    (w > 0 && h > 0).then_some((w, h))
}

pub fn dimensions(bytes: &[u8]) -> Option<(i32, i32)> {
    // PNG: IHDR width and height are big-endian u32 at 16 and 20.
    if bytes.len() > 24 && bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        // **Zero is not a size, it is a parse that failed.** Bytes that begin
        // like a PNG and carry no IHDR read as 0 x 0, and `observation_image`
        // requires both to be positive when either is present — so believing
        // this answer turns a truncated upload into a constraint violation and
        // a 500. Unknown is a state the column already has.
        return positive(i32::try_from(w).ok()?, i32::try_from(h).ok()?);
    }

    // JPEG: walk the segments to the first start-of-frame, which carries the
    // size. Anything else is a marker with a length to skip.
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2usize;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            // SOF0..SOF15, excluding the four that are not frame headers.
            if (0xC0..=0xCF).contains(&marker)
                && marker != 0xC4
                && marker != 0xC8
                && marker != 0xCC
            {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]);
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]);
                return Some((i32::from(w), i32::from(h)));
            }
            let length = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            if length < 2 {
                return None;
            }
            i += 2 + length;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_digest_is_sixty_four_lower_case_hex_and_nothing_else() {
        assert!(is_digest(&"a".repeat(64)));
        assert!(!is_digest(&"A".repeat(64)), "upper case is not our encoding");
        assert!(!is_digest("../../etc/passwd"), "and this is the reason");
        assert!(!is_digest(&"a".repeat(63)));
        assert!(!is_digest(&format!("{}/x", "a".repeat(62))));
    }

    #[test]
    fn the_type_is_read_from_the_bytes_rather_than_the_request() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(
            sniff(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some("image/png")
        );
        assert_eq!(sniff(b"<svg>this is not a photograph</svg>"), None);
    }

    #[test]
    fn a_png_states_its_size_at_a_fixed_offset() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0, 0, 0, 13]);
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&1280u32.to_be_bytes());
        png.extend_from_slice(&960u32.to_be_bytes());
        png.extend_from_slice(&[8, 2, 0, 0, 0]);
        assert_eq!(dimensions(&png), Some((1280, 960)));
    }

    #[test]
    fn a_face_is_one_of_seven_sides_or_a_detail() {
        assert!(is_face("front"));
        assert!(is_face("label"), "the one that is not a side");
        assert!(is_face(DETAIL), "evidence, which is not a side either");
        assert!(!is_face("sideways"));
        assert!(!is_face("FRONT"), "the check constraint is lower case");

        // **`detail` is not in `FACES`, and that is the structural half of the
        // rule.** D141 walks the sides looking for `front`; an eighth member
        // here would put evidence in reach of a resolution that means to find a
        // product photograph. Migration 84 says the same thing to the database.
        assert!(!FACES.contains(&DETAIL));
    }

    #[test]
    fn the_address_is_the_content() {
        // The empty string's SHA-256, which is the one everybody can check.
        assert_eq!(
            digest_of(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
