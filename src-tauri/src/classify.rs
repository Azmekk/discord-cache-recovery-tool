//! Cache key URL → kind and metadata.

use percent_encoding::percent_decode_str;
use serde::Serialize;
use url::Url;

use crate::sniff::MediaType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Attachment,
    Gif,
    Emoji,
    Sticker,
    Avatar,
    Server,
    Embed,
    Interface,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub url: Option<String>,
    pub original_url: Option<String>,
    pub host: Option<String>,
    pub kind: Kind,
    pub file_name: String,
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
    pub posted_at: Option<i64>,
    pub group_key: String,
}

const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

/// Snowflake → Unix ms.
pub fn snowflake_ms(id: &str) -> Option<i64> {
    if !is_snowflake(id) {
        return None;
    }
    let v: u64 = id.parse().ok()?;
    crate::http::plausible_ms((v >> 22) as i64 + DISCORD_EPOCH_MS)
}

fn is_snowflake(s: &str) -> bool {
    (15..=21).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_digit())
}

/// Extracts the URL from a cache key: the last whitespace token, if it starts with `http`.
pub fn key_url(key: &str) -> Option<&str> {
    let tok = key.split_whitespace().last()?;
    tok.starts_with("http").then_some(tok)
}

fn decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

fn url_without_query(u: &Url) -> String {
    let mut u = u.clone();
    u.set_query(None);
    u.set_fragment(None);
    u.to_string()
}

fn stem(s: &str) -> &str {
    s.rsplit_once('.').map(|(a, _)| a).unwrap_or(s)
}

const MEDIA_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "jfif", "gif", "webp", "avif", "heic", "heif", "bmp", "ico", "svg",
    "tif", "tiff", "mp4", "m4v", "mov", "webm", "mkv", "ogv", "ogg", "oga", "opus", "mp3", "wav",
    "flac", "m4a", "aac", "weba", "apng",
];

/// Makes sure `name` ends with `.ext`: keeps a matching extension, replaces a different
/// media extension, appends otherwise.
pub fn ensure_ext(name: &str, ext: &str) -> String {
    let name = name.trim();
    if let Some((base, cur)) = name.rsplit_once('.') {
        let cur_l = cur.to_ascii_lowercase();
        let same = cur_l == ext
            || matches!(
                (cur_l.as_str(), ext),
                ("jpeg" | "jfif", "jpg") | ("m4v", "mp4") | ("apng", "png") | ("heif", "heic")
            );
        if same && !base.is_empty() {
            return name.to_string();
        }
        if MEDIA_EXTS.contains(&cur_l.as_str()) && !base.is_empty() {
            return format!("{base}.{ext}");
        }
    }
    if name.is_empty() {
        return format!("file.{ext}");
    }
    format!("{name}.{ext}")
}

/// Classifies an entry. `key` is the raw cache key (None for orphan files),
/// `cache_file` the on-disk file name (used when there is no URL), `orphan_key` a unique
/// group key for entries without a URL.
pub fn classify(
    key: Option<&str>,
    media: &MediaType,
    cache_file: &str,
    orphan_key: &str,
) -> Classified {
    let parsed = key
        .and_then(key_url)
        .and_then(|u| Url::parse(u).ok().map(|p| (u.to_string(), p)));
    let Some((raw_url, u)) = parsed else {
        return Classified {
            url: None,
            original_url: None,
            host: None,
            kind: Kind::Unknown,
            file_name: ensure_ext(cache_file, media.ext),
            channel_id: None,
            guild_id: None,
            posted_at: None,
            group_key: orphan_key.to_string(),
        };
    };
    let host = u.host_str().map(|h| h.to_ascii_lowercase());
    let h = host.as_deref().unwrap_or("");
    let segs: Vec<&str> = u
        .path_segments()
        .map(|s| s.filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    let last = segs.last().map(|s| decode(s)).unwrap_or_default();

    let mut c = Classified {
        url: Some(raw_url),
        original_url: None,
        host: host.clone(),
        kind: Kind::Embed,
        file_name: String::new(),
        channel_id: None,
        guild_id: None,
        posted_at: None,
        group_key: url_without_query(&u),
    };
    let mut name = if last.is_empty() {
        h.to_string()
    } else {
        last.clone()
    };

    let discord_cdn = matches!(
        h,
        "cdn.discordapp.com" | "media.discordapp.net" | "cdn.discord.com" | "media.discord.net"
    );
    let discord_site = h == "discord.com"
        || h.ends_with(".discord.com")
        || h == "discordapp.com"
        || h.ends_with(".discordapp.com");

    if discord_cdn {
        match segs.as_slice() {
            [a, ch, att, file, ..] if matches!(*a, "attachments" | "ephemeral-attachments") => {
                c.kind = Kind::Attachment;
                c.channel_id = is_snowflake(ch).then(|| ch.to_string());
                c.posted_at = snowflake_ms(att);
                c.group_key = format!("att:{att}");
                name = decode(file);
            }
            ["emojis", f, ..] => {
                c.kind = Kind::Emoji;
                c.group_key = format!("emoji:{}", stem(f));
            }
            ["stickers", f, ..] => {
                c.kind = Kind::Sticker;
                c.group_key = format!("sticker:{}", stem(f));
            }
            ["avatars", ..] | ["avatar-decoration-presets", ..] | ["embed", "avatars", ..] => {
                c.kind = Kind::Avatar;
            }
            ["guilds", g, "users", ..] => {
                c.kind = Kind::Avatar;
                c.guild_id = Some(g.to_string());
            }
            [a, g, ..] if matches!(*a, "icons" | "splashes" | "discovery-splashes" | "banners") => {
                c.kind = Kind::Server;
                c.guild_id = is_snowflake(g).then(|| g.to_string());
            }
            ["role-icons", ..] | ["guild-tag-badges", ..] | ["clan-badges", ..] => {
                c.kind = Kind::Server
            }
            ["assets", ..] => c.kind = Kind::Interface,
            _ => {}
        }
    } else if h.starts_with("images-ext-") && h.ends_with(".discordapp.net") {
        c.kind = Kind::Embed;
        if let Some(orig) = images_ext_original(&segs) {
            if let Ok(ou) = Url::parse(&orig) {
                if let Some(l) = ou
                    .path_segments()
                    .and_then(|mut s| s.next_back())
                    .filter(|s| !s.is_empty())
                {
                    name = decode(l);
                } else if let Some(oh) = ou.host_str() {
                    name = oh.to_string();
                }
            }
            c.original_url = Some(orig);
        }
    } else if discord_site && segs.first() == Some(&"assets") {
        c.kind = Kind::Interface;
    } else if h == "tenor.com"
        || h.ends_with(".tenor.com")
        || h == "giphy.com"
        || h.ends_with(".giphy.com")
        || h == "klipy.com"
        || h.ends_with(".klipy.com")
    {
        c.kind = Kind::Gif;
    }

    c.file_name = ensure_ext(&name, media.ext);
    c
}

/// Reconstructs the source URL from an images-ext proxy path:
/// `/external/<sig>/[%3F<query>/]<scheme>/<host>/<path…>`.
fn images_ext_original(segs: &[&str]) -> Option<String> {
    let rest = match segs {
        ["external", _sig, rest @ ..] => rest,
        _ => return None,
    };
    let (query, rest) = match rest.first() {
        Some(q) if q.len() >= 3 && q[..3].eq_ignore_ascii_case("%3F") => {
            (Some(decode(q)), &rest[1..])
        }
        _ => (None, rest),
    };
    let [scheme, host, path @ ..] = rest else {
        return None;
    };
    if !matches!(*scheme, "http" | "https") || host.is_empty() {
        return None;
    }
    let mut out = format!("{scheme}://{host}/{}", path.join("/"));
    if let Some(q) = query {
        out.push_str(&q);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> MediaType {
        crate::sniff::sniff(crate::cache::simple::tests::PNG).unwrap()
    }

    fn cl(key: &str) -> Classified {
        classify(Some(key), &png(), "f_000001", "orphan:1")
    }

    #[test]
    fn snowflake_math() {
        // 175928847299117063 is Discord's documented example: 2016-04-30T11:18:25.796Z.
        assert_eq!(snowflake_ms("175928847299117063"), Some(1_462_015_105_796));
        assert_eq!(snowflake_ms("not a number"), None);
        assert_eq!(snowflake_ms("1"), None);
    }

    #[test]
    fn attachment_with_prefixed_key() {
        let c = cl("1/0/_dk_https://discord.com https://discord.com https://cdn.discordapp.com/attachments/111111111111111111/175928847299117063/My%20Photo.png?ex=1&is=2&hm=3");
        assert_eq!(c.kind, Kind::Attachment);
        assert_eq!(c.file_name, "My Photo.png");
        assert_eq!(c.channel_id.as_deref(), Some("111111111111111111"));
        assert_eq!(c.posted_at, Some(1_462_015_105_796));
        assert_eq!(c.group_key, "att:175928847299117063");
        assert_eq!(c.host.as_deref(), Some("cdn.discordapp.com"));
        assert!(c
            .url
            .unwrap()
            .starts_with("https://cdn.discordapp.com/attachments/"));
    }

    #[test]
    fn media_proxy_attachment_shares_group() {
        let a = cl("https://media.discordapp.net/attachments/111111111111111111/175928847299117063/My%20Photo.png?width=400&height=300");
        assert_eq!(a.kind, Kind::Attachment);
        assert_eq!(a.group_key, "att:175928847299117063");
        // Body is a webp but URL says png → extension follows the body.
        let webp = MediaType {
            mime: "image/webp",
            ext: "webp",
        };
        let b = classify(Some("https://media.discordapp.net/attachments/1/175928847299117063/clip.png?format=webp"), &webp, "x", "o");
        assert_eq!(b.file_name, "clip.webp");
        assert_eq!(b.channel_id, None);
    }

    #[test]
    fn url_table() {
        let cases: &[(&str, Kind, &str)] = &[
            ("https://cdn.discordapp.com/emojis/123456789012345678.webp?size=48&quality=lossless", Kind::Emoji, "emoji:123456789012345678"),
            ("https://media.discordapp.net/stickers/987654321098765432.png?size=160", Kind::Sticker, "sticker:987654321098765432"),
            ("https://cdn.discordapp.com/avatars/111111111111111111/a_abcdef.png?size=128", Kind::Avatar, "https://cdn.discordapp.com/avatars/111111111111111111/a_abcdef.png"),
            ("https://cdn.discordapp.com/guilds/222222222222222222/users/111111111111111111/avatars/abc.png", Kind::Avatar, "https://cdn.discordapp.com/guilds/222222222222222222/users/111111111111111111/avatars/abc.png"),
            ("https://cdn.discordapp.com/avatar-decoration-presets/a_1.png", Kind::Avatar, "https://cdn.discordapp.com/avatar-decoration-presets/a_1.png"),
            ("https://cdn.discordapp.com/embed/avatars/0.png", Kind::Avatar, "https://cdn.discordapp.com/embed/avatars/0.png"),
            ("https://cdn.discordapp.com/icons/333333333333333333/abc.webp?size=96", Kind::Server, "https://cdn.discordapp.com/icons/333333333333333333/abc.webp"),
            ("https://cdn.discordapp.com/role-icons/444444444444444444/abc.png", Kind::Server, "https://cdn.discordapp.com/role-icons/444444444444444444/abc.png"),
            ("https://media.tenor.com/AbCd/tenor.gif", Kind::Gif, "https://media.tenor.com/AbCd/tenor.gif"),
            ("https://media2.giphy.com/media/xyz/giphy.gif?cid=1", Kind::Gif, "https://media2.giphy.com/media/xyz/giphy.gif"),
            ("https://discord.com/assets/abc123.svg", Kind::Interface, "https://discord.com/assets/abc123.svg"),
            ("https://cdn.discordapp.com/assets/collectibles/x.png", Kind::Interface, "https://cdn.discordapp.com/assets/collectibles/x.png"),
            ("https://example.org/pic.png", Kind::Embed, "https://example.org/pic.png"),
        ];
        for (url, kind, group) in cases {
            let c = cl(url);
            assert_eq!(c.kind, *kind, "{url}");
            assert_eq!(c.group_key, *group, "{url}");
        }
        assert_eq!(
            cl(cases[6].0).guild_id.as_deref(),
            Some("333333333333333333")
        );
        assert_eq!(
            cl(cases[3].0).guild_id.as_deref(),
            Some("222222222222222222")
        );
        assert_eq!(cl(cases[0].0).file_name, "123456789012345678.png");
    }

    #[test]
    fn images_ext_original_url() {
        let c = cl("https://images-ext-1.discordapp.net/external/AbC-sig_123/%3Fv%3D2%26s%3D1/https/i.example.com/path/to/Cool%20Pic.jpg?width=400");
        assert_eq!(c.kind, Kind::Embed);
        assert_eq!(
            c.original_url.as_deref(),
            Some("https://i.example.com/path/to/Cool%20Pic.jpg?v=2&s=1")
        );
        assert_eq!(c.file_name, "Cool Pic.png");
        let c = cl(
            "https://images-ext-2.discordapp.net/external/sig/https/pbs.example.net/media/x.png",
        );
        assert_eq!(
            c.original_url.as_deref(),
            Some("https://pbs.example.net/media/x.png")
        );
        assert_eq!(c.file_name, "x.png");
    }

    #[test]
    fn orphans_and_ext() {
        let c = classify(None, &png(), "f_00002a", "orphan:7");
        assert_eq!(c.kind, Kind::Unknown);
        assert_eq!(c.file_name, "f_00002a.png");
        assert_eq!(c.group_key, "orphan:7");
        let c = classify(Some("not a url at all"), &png(), "f_1", "orphan:8");
        assert_eq!(c.kind, Kind::Unknown);
        assert_eq!(ensure_ext("photo.JPEG", "jpg"), "photo.JPEG");
        assert_eq!(ensure_ext("report.final", "png"), "report.final.png");
        assert_eq!(ensure_ext("noext", "gif"), "noext.gif");
        assert_eq!(ensure_ext("", "gif"), "file.gif");
    }
}
