//! Magic-byte sniffing of media bodies.

/// Detected media type: MIME type and canonical file extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaType {
    pub mime: &'static str,
    pub ext: &'static str,
}

const fn mt(mime: &'static str, ext: &'static str) -> MediaType {
    MediaType { mime, ext }
}

/// Number of leading body bytes the scanner reads for sniffing.
pub const SNIFF_LEN: usize = 64;

/// Sniffs a media type from the first bytes of a body. Only image/video/audio types.
pub fn sniff(b: &[u8]) -> Option<MediaType> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(mt("image/png", "png"));
    }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(mt("image/jpeg", "jpg"));
    }
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        return Some(mt("image/gif", "gif"));
    }
    if b.len() >= 12 && &b[0..4] == b"RIFF" {
        match &b[8..12] {
            b"WEBP" => return Some(mt("image/webp", "webp")),
            b"WAVE" => return Some(mt("audio/wav", "wav")),
            b"AVI " => return Some(mt("video/x-msvideo", "avi")),
            _ => {}
        }
    }
    if b.len() >= 12 && &b[4..8] == b"ftyp" {
        return sniff_ftyp(b);
    }
    if b.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        if contains(b, b"webm") {
            return Some(mt("video/webm", "webm"));
        }
        return Some(mt("video/x-matroska", "mkv"));
    }
    if b.starts_with(b"OggS") {
        return Some(mt("audio/ogg", "ogg"));
    }
    if b.starts_with(b"fLaC") {
        return Some(mt("audio/flac", "flac"));
    }
    if b.starts_with(b"ID3") || (b.len() >= 2 && b[0] == 0xFF && matches!(b[1], 0xFB | 0xF3 | 0xF2))
    {
        return Some(mt("audio/mpeg", "mp3"));
    }
    if is_svg(b) {
        return Some(mt("image/svg+xml", "svg"));
    }
    None
}

fn sniff_ftyp(b: &[u8]) -> Option<MediaType> {
    let brand = &b[8..12];
    // Compatible brands follow the major brand + minor version (box offset 16).
    let compat = |name: &[u8]| {
        let end = b
            .len()
            .min(u32::from_be_bytes(b[0..4].try_into().unwrap()) as usize);
        (16..end.saturating_sub(3))
            .step_by(4)
            .any(|i| &b[i..i + 4] == name)
    };
    Some(match brand {
        b"avif" | b"avis" => mt("image/avif", "avif"),
        b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" => mt("image/heic", "heic"),
        b"mif1" | b"msf1" => {
            if compat(b"avif") || compat(b"avis") {
                mt("image/avif", "avif")
            } else {
                mt("image/heic", "heic")
            }
        }
        b"qt  " => mt("video/quicktime", "mov"),
        b"M4A " | b"M4B " | b"M4P " => mt("audio/mp4", "m4a"),
        b"crx " => return None,
        _ => mt("video/mp4", "mp4"),
    })
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn is_svg(b: &[u8]) -> bool {
    let b = b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(b);
    let start = b
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(b.len());
    let t = &b[start..];
    if t.len() >= 4 && t[..4].eq_ignore_ascii_case(b"<svg") {
        return true;
    }
    t.starts_with(b"<?xml") && contains(t, b"<svg")
}

/// Maps an HTTP Content-Type to a media type when it is image/video/audio.
pub fn from_content_type(ct: &str) -> Option<MediaType> {
    let essence = ct.split(';').next()?.trim().to_ascii_lowercase();
    let known: &[(&str, &str, &str)] = &[
        ("image/png", "image/png", "png"),
        ("image/apng", "image/apng", "png"),
        ("image/jpeg", "image/jpeg", "jpg"),
        ("image/jpg", "image/jpeg", "jpg"),
        ("image/gif", "image/gif", "gif"),
        ("image/webp", "image/webp", "webp"),
        ("image/avif", "image/avif", "avif"),
        ("image/heic", "image/heic", "heic"),
        ("image/heif", "image/heif", "heif"),
        ("image/bmp", "image/bmp", "bmp"),
        ("image/x-icon", "image/x-icon", "ico"),
        ("image/vnd.microsoft.icon", "image/x-icon", "ico"),
        ("image/svg+xml", "image/svg+xml", "svg"),
        ("image/tiff", "image/tiff", "tiff"),
        ("video/mp4", "video/mp4", "mp4"),
        ("video/webm", "video/webm", "webm"),
        ("video/quicktime", "video/quicktime", "mov"),
        ("video/x-matroska", "video/x-matroska", "mkv"),
        ("video/ogg", "video/ogg", "ogv"),
        ("audio/mpeg", "audio/mpeg", "mp3"),
        ("audio/mp3", "audio/mpeg", "mp3"),
        ("audio/ogg", "audio/ogg", "ogg"),
        ("audio/opus", "audio/ogg", "opus"),
        ("audio/wav", "audio/wav", "wav"),
        ("audio/x-wav", "audio/wav", "wav"),
        ("audio/wave", "audio/wav", "wav"),
        ("audio/flac", "audio/flac", "flac"),
        ("audio/x-flac", "audio/flac", "flac"),
        ("audio/mp4", "audio/mp4", "m4a"),
        ("audio/aac", "audio/aac", "aac"),
        ("audio/webm", "audio/webm", "weba"),
    ];
    known
        .iter()
        .find(|(k, _, _)| *k == essence)
        .map(|(_, m, e)| mt(m, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext(b: &[u8]) -> Option<&'static str> {
        sniff(b).map(|m| m.ext)
    }

    #[test]
    fn sniffs_common_media() {
        assert_eq!(ext(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"), Some("png"));
        assert_eq!(ext(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]), Some("jpg"));
        assert_eq!(ext(b"GIF89a\x01\0\x01\0"), Some("gif"));
        assert_eq!(ext(b"RIFF\x24\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(ext(b"RIFF\x24\0\0\0WAVEfmt "), Some("wav"));
        assert_eq!(ext(b"\0\0\0\x1cftypavif\0\0\0\0avifmif1"), Some("avif"));
        assert_eq!(ext(b"\0\0\0\x1cftypmif1\0\0\0\0mif1avif"), Some("avif"));
        assert_eq!(ext(b"\0\0\0\x18ftypheic\0\0\0\0mif1heic"), Some("heic"));
        assert_eq!(
            ext(b"\0\0\0\x20ftypisom\0\0\x02\0isomiso2avc1mp41"),
            Some("mp4")
        );
        assert_eq!(ext(b"\0\0\0\x14ftypqt  \0\0\0\0qt  "), Some("mov"));
        assert_eq!(ext(b"\0\0\0\x1cftypM4A \0\0\0\0M4A mp42"), Some("m4a"));
        assert_eq!(
            ext(b"\x1a\x45\xdf\xa3\x9f\x42\x86\x81\x01\x42\x82\x84webm"),
            Some("webm")
        );
        assert_eq!(
            ext(b"\x1a\x45\xdf\xa3\x9f\x42\x82\x88matroska"),
            Some("mkv")
        );
        assert_eq!(ext(b"OggS\0\x02"), Some("ogg"));
        assert_eq!(ext(b"ID3\x04\0"), Some("mp3"));
        assert_eq!(ext(&[0xFF, 0xFB, 0x90, 0x64]), Some("mp3"));
        assert_eq!(ext(b"fLaC\0\0\0\x22"), Some("flac"));
        assert_eq!(
            ext(b"  <svg xmlns=\"http://www.w3.org/2000/svg\">"),
            Some("svg")
        );
        assert_eq!(
            ext(b"<?xml version=\"1.0\"?><svg viewBox=\"0 0 1 1\">"),
            Some("svg")
        );
    }

    #[test]
    fn rejects_non_media() {
        assert_eq!(ext(b"<!DOCTYPE html><html>"), None);
        assert_eq!(ext(b"{\"a\":1}"), None);
        assert_eq!(ext(b"<?xml version=\"1.0\"?><rss>"), None);
        assert_eq!(ext(b""), None);
    }

    #[test]
    fn content_type_fallback() {
        assert_eq!(
            from_content_type("image/png; charset=binary").unwrap().ext,
            "png"
        );
        assert_eq!(from_content_type("Video/MP4").unwrap().mime, "video/mp4");
        assert!(from_content_type("text/html").is_none());
        assert!(from_content_type("application/json").is_none());
    }
}
