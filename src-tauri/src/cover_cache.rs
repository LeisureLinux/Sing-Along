//! Album cover extraction and caching.
//!
//! Covers are pulled out of the audio files themselves — either an embedded
//! picture frame or a `cover`/`folder` image sitting next to them — and written
//! once into the app data directory. The library row keeps only the cached file
//! name, so browsing albums does not re-read audio files or decode embedded
//! art on every scroll.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;

/// File names that conventionally hold the album art for a folder, in the order
/// they are preferred. Lowercase comparison, so `Cover.JPG` matches too.
const SIDECAR_NAMES: [&str; 4] = ["cover", "folder", "front", "albumart"];
const IMAGE_EXTS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

/// Read the embedded front cover of an audio file, if it has one.
///
/// Falls back to any picture frame when no explicit front cover is tagged:
/// rippers do not always set the type, and a back cover beats a blank album.
pub fn embedded_cover(path: &Path) -> Option<Vec<u8>> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag
        .get_picture_type(PictureType::CoverFront)
        .or_else(|| tag.pictures().first())?;
    Some(picture.data().to_vec())
}

/// Find an image file sitting beside the track.
fn sidecar_cover(dir: &Path) -> Option<Vec<u8>> {
    let entries = fs::read_dir(dir).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase());
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase());
            match (stem, ext) {
                (Some(stem), Some(ext)) => {
                    SIDECAR_NAMES.contains(&stem.as_str()) && IMAGE_EXTS.contains(&ext.as_str())
                }
                _ => false,
            }
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }

    // Preference order is SIDECAR_NAMES, then extension order. Sorting by the
    // index in those arrays keeps `cover.jpg` ahead of `folder.png`.
    candidates.sort_by_key(|path| {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        (
            SIDECAR_NAMES.iter().position(|n| *n == stem).unwrap_or(usize::MAX),
            IMAGE_EXTS.iter().position(|e| *e == ext).unwrap_or(usize::MAX),
        )
    });

    fs::read(&candidates[0]).ok()
}

/// Cover for a track: embedded art first, then a folder image.
pub fn cover_for_track(track_path: &Path) -> Option<Vec<u8>> {
    embedded_cover(track_path).or_else(|| {
        track_path
            .parent()
            .and_then(|dir| sidecar_cover(dir))
    })
}

/// Write cover bytes into the cache directory and return the stored file name.
///
/// The name is derived from the album key so a rescan overwrites rather than
/// accumulating files, and so two albums that happen to share a cover still get
/// separate entries. Returns `None` when there is nothing to write.
pub fn store_cover(cache_dir: &Path, key: &str, bytes: &[u8]) -> Option<String> {
    let ext = match bytes {
        b if b.starts_with(&[0xFF, 0xD8]) => "jpg",
        b if b.starts_with(&[0x89, b'P', b'N', b'G']) => "png",
        b if b.len() > 12 && &b[8..12] == b"WEBP" => "webp",
        // Unknown format: a renamed file browsers cannot decode would be worse
        // than no cover, and the album just keeps its placeholder.
        _ => return None,
    };

    let file_name = format!("{key}.{ext}");
    let target = cache_dir.join(&file_name);

    // Skip the write when the bytes are unchanged; a full rescan touches every
    // album and rewriting identical files is pure churn.
    if let Ok(existing) = fs::read(&target) {
        if existing == bytes {
            return Some(file_name);
        }
    }

    fs::create_dir_all(cache_dir).ok()?;
    fs::write(&target, bytes).ok()?;
    Some(file_name)
}

/// Read a cached cover and return it as a `data:` URL.
///
/// The asset protocol is the usual way to serve a local image, but with
/// `useHttpsScheme` the webview runs on an `https://` origin and WebKitGTK
/// refuses to decode an `asset:` source from there — `fetch` succeeds while
/// `<img>` fires `error`. A data URL sidesteps the scheme entirely.
///
/// Covers are small (tens of KB) and only one album view is open at a time, so
/// the base64 overhead is not worth optimising away.
pub fn cached_cover_data_url(cache_dir: &Path, file_name: &str) -> Result<String> {
    let path = cache_dir.join(file_name);
    let bytes = fs::read(&path)
        .with_context(|| format!("cover not readable: {}", path.display()))?;

    let mime = match bytes.as_slice() {
        b if b.starts_with(&[0xFF, 0xD8]) => "image/jpeg",
        b if b.starts_with(&[0x89, b'P', b'N', b'G']) => "image/png",
        b if b.len() > 12 && &b[8..12] == b"WEBP" => "image/webp",
        _ => "application/octet-stream",
    };

    Ok(format!(
        "data:{mime};base64,{}",
        base64_encode(&bytes)
    ))
}

/// Minimal base64 encoder, so the crate does not gain a dependency for this one
/// call site.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);

    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;

        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_image_formats() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4];
        let png = [0x89, b'P', b'N', b'G', 1, 2, 3, 4];
        let mut webp = vec![0u8; 16];
        webp[8..12].copy_from_slice(b"WEBP");

        let dir = std::env::temp_dir().join("sing-along-cover-test");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(
            store_cover(&dir, "a", &jpeg).as_deref(),
            Some("a.jpg")
        );
        assert_eq!(store_cover(&dir, "b", &png).as_deref(), Some("b.png"));
        assert_eq!(
            store_cover(&dir, "c", &webp).as_deref(),
            Some("c.webp")
        );
        assert_eq!(store_cover(&dir, "d", &[0u8; 4]), None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn encodes_base64_with_the_expected_padding() {
        // Known vectors: the padding rules are the part that goes wrong.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn builds_a_data_url_with_the_right_mime() {
        let dir = std::env::temp_dir().join("sing-along-cover-dataurl");
        let _ = fs::remove_dir_all(&dir);

        let png = [0x89, b'P', b'N', b'G', 1, 2, 3, 4];
        store_cover(&dir, "t", &png).unwrap();
        let url = cached_cover_data_url(&dir, "t.png").unwrap();
        assert!(url.starts_with("data:image/png;base64,"), "{url}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reuses_the_stored_file_when_bytes_are_identical() {
        let dir = std::env::temp_dir().join("sing-along-cover-idempotent");
        let _ = fs::remove_dir_all(&dir);

        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 9, 9, 9, 9];
        let first = store_cover(&dir, "same", &jpeg);
        let written = fs::read(dir.join("same.jpg")).unwrap();
        let second = store_cover(&dir, "same", &jpeg);

        assert_eq!(first, second);
        assert_eq!(written, jpeg);

        let _ = fs::remove_dir_all(&dir);
    }
}
