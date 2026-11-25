//! Reading what was typed or pasted into Open Network Stream (S4.11.1, S4.11.4): links one per
//! line or several on a line, cleaned of what pasting drags along (surrounding quotes or angle
//! brackets, trailing commas), checked to be media links, and de-duplicated.
//!
//! A media link names a scheme libVLC streams from ([`crate::app::media::scheme`]).
//! `javascript:`, `data:` and any other scheme are refused, and so are magnet links: libVLC
//! can't play torrents.

use crate::app::media::scheme::is_media_link;

/// What pasting drags along around a link. Brackets aren't among them: a link may end in one.
const WRAPPERS: &[char] = &['"', '\'', '“', '”', '‘', '’', '<', '>'];
/// What trails a link copied from a list.
const TRAILERS: &[char] = &[',', ';'];

/// What stands for a password in a link shown on screen.
const PASSWORD_MASK: &str = "•••";

/// The links in a text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Links {
    /// The media links, in order, each once.
    pub urls: Vec<String>,
    /// How many repeats of a link were left out.
    pub duplicates: usize,
    /// What was left out for not being a media link, as written.
    pub refused: Vec<String>,
}

/// The links in `text`, separated by line breaks or spaces. A line holding no link is refused
/// whole, as one entry; on a line with links, each other word is.
pub fn read(text: &str) -> Links {
    let mut links = Links::default();
    let mut seen = Vec::new();
    for line in text.lines() {
        let words: Vec<&str> =
            line.split_whitespace().map(clean).filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            continue;
        }
        if !words.iter().any(|word| is_media_link(word)) {
            links.refused.push(line.trim().to_owned());
            continue;
        }
        for word in words {
            links.add(word, &mut seen);
        }
    }
    links
}

impl Links {
    /// Adds `word` as a link, or counts it as a repeat or refuses it.
    fn add(&mut self, word: &str, seen: &mut Vec<String>) {
        if !is_media_link(word) {
            self.refused.push(word.to_owned());
            return;
        }
        let key = comparable(word);
        if seen.contains(&key) {
            self.duplicates += 1;
        } else {
            seen.push(key);
            self.urls.push(word.to_owned());
        }
    }
}

/// `word` without the quotes, brackets and trailing commas around it.
fn clean(word: &str) -> &str {
    word.trim_matches(WRAPPERS).trim_end_matches(TRAILERS).trim_matches(WRAPPERS)
}

/// `link` as compared for repeats: schemes and hosts ignore case.
fn comparable(link: &str) -> String {
    let (scheme, rest) = link.split_once("://").unwrap_or(("", link));
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    format!(
        "{}://{}{}",
        scheme.to_ascii_lowercase(),
        rest[..host_end].to_ascii_lowercase(),
        &rest[host_end..]
    )
}

/// Schemes where what comes before `@` is an address, not an account: `udp://@:1234` listens,
/// `udp://source@group:1234` takes one source's multicast.
const ADDRESS_BEFORE_AT: &[&str] = &["udp", "rtp"];

/// The user name and password part of `link` (`user:pass` in `https://user:pass@host/`), and
/// where the host starts.
fn user_info(link: &str) -> Option<(&str, usize)> {
    let (scheme, rest) = link.split_once("://")?;
    if ADDRESS_BEFORE_AT.iter().any(|known| known.eq_ignore_ascii_case(scheme)) {
        return None;
    }
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let at = authority.rfind('@')?;
    Some((&authority[..at], scheme.len() + 3 + at + 1))
}

/// Whether `link` carries a password or a user name.
pub fn has_credentials(link: &str) -> bool {
    user_info(link).is_some_and(|(info, _)| !info.is_empty())
}

/// `link` without its user name and password, as it may be written to disk (S9.7.3): a
/// password belongs in the keychain, never in the history, the session or a log.
pub fn without_user_info(link: &str) -> String {
    match user_info(link) {
        Some((info, host)) if !info.is_empty() => {
            let scheme_end = link.find("://").map_or(0, |at| at + 3);
            format!("{}{}", &link[..scheme_end], &link[host..])
        }
        _ => link.to_owned(),
    }
}

/// `link` as shown on screen: a password in it masked.
pub fn masked(link: &str) -> String {
    let Some((info, host)) = user_info(link) else { return link.to_owned() };
    let Some((user, _)) = info.split_once(':') else { return link.to_owned() };
    let scheme_end = link.find("://").map_or(0, |at| at + 3);
    format!("{}{user}:{PASSWORD_MASK}@{}", &link[..scheme_end], &link[host..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_links_are_cleaned_and_split() {
        let text = "  \"https://a.example/live.m3u8\",\n<rtsp://cam/1> 'udp://@:1234';\n\n";
        let links = read(text);
        assert_eq!(links.urls, ["https://a.example/live.m3u8", "rtsp://cam/1", "udp://@:1234"]);
        assert_eq!(links.refused, Vec::<String>::new());
    }

    #[test]
    fn only_media_schemes_are_links() {
        let links = read(
            "javascript:alert(1) data:text/html,x magnet:?xt=urn:btih:1 HTTP://X/y\n\
             www.example.com chrome://settings ftp://\n\
             https://en.wikipedia.org/wiki/Foo_(bar)",
        );
        assert_eq!(links.urls, ["HTTP://X/y", "https://en.wikipedia.org/wiki/Foo_(bar)"]);
        let refused = [
            "javascript:alert(1)",
            "data:text/html,x",
            "magnet:?xt=urn:btih:1",
            "www.example.com chrome://settings ftp://",
        ];
        assert_eq!(links.refused, refused);
    }

    #[test]
    fn repeats_are_left_out_and_counted() {
        let links = read("https://Host/a\nhttps://host/a https://host/A");
        assert_eq!(links.urls, ["https://Host/a", "https://host/A"]);
        assert_eq!(links.duplicates, 1);
    }

    #[test]
    fn passwords_are_masked() {
        assert!(has_credentials("https://me:secret@host/a@b"));
        assert!(has_credentials("ftp://me@host/"));
        assert!(!has_credentials("udp://@:1234"));
        assert!(!has_credentials("https://host/a@b"));
        assert_eq!(masked("https://me:secret@host/x"), "https://me:•••@host/x");
        assert_eq!(masked("ftp://me@host/"), "ftp://me@host/");
        assert_eq!(masked("https://host/a"), "https://host/a");
    }

    #[test]
    fn user_info_is_left_out_of_what_is_stored() {
        assert_eq!(without_user_info("https://me:secret@host/a@b"), "https://host/a@b");
        assert_eq!(without_user_info("rtsp://token@cam:554/1"), "rtsp://cam:554/1");
        assert_eq!(without_user_info("udp://@:1234"), "udp://@:1234");
        assert_eq!(
            without_user_info("udp://10.0.0.1@239.0.0.1:1234"),
            "udp://10.0.0.1@239.0.0.1:1234"
        );
        assert_eq!(without_user_info("ftp://@host/"), "ftp://@host/");
        assert_eq!(without_user_info("https://host/a@b"), "https://host/a@b");
    }
}
