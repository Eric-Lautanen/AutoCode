// extract.rs -- HTML content extraction for web_search and fetch_url tools.
// Uses a small, dependency-free HTML cleaner (see `html.rs`) instead of a full
// DOM parsing crate. This avoids the empty-result failures we saw with the
// previous parser and lets us hand the model the full cleaned page.

use crate::utils::html::clean_html_to_text;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

static SEARCH_CACHE: LazyLock<Mutex<HashMap<String, (Instant, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const CACHE_TTL_SECS: u64 = 120;

pub fn search_cache_get(key: &str) -> Option<String> {
    let mut cache = SEARCH_CACHE.lock().ok()?;
    // Clean expired entries on read
    cache.retain(|_, (expiry, _)| Instant::now() < *expiry);
    cache.get(key).map(|(_, v)| v.clone())
}

const CACHE_MAX_ENTRIES: usize = 500;

pub fn search_cache_set(key: &str, value: &str) {
    if let Ok(mut cache) = SEARCH_CACHE.lock() {
        if cache.len() >= CACHE_MAX_ENTRIES
            && let Some(k) = cache.keys().next().cloned()
        {
            cache.remove(&k);
        }
        let expiry = Instant::now() + std::time::Duration::from_secs(CACHE_TTL_SECS);
        cache.insert(key.to_string(), (expiry, value.to_string()));
    }
}

/// Domains to exclude from search results.  Social media and low-quality
/// content farms that waste tokens with noise.
const DOMAIN_BLACKLIST: &[&str] = &[
    // Social media
    "reddit.com",
    "youtube.com",
    "youtu.be",
    "facebook.com",
    "twitter.com",
    "x.com",
    "instagram.com",
    "tiktok.com",
    "linkedin.com",
    "discord.com",
    "discord.gg",
    "t.me",
    "telegram.org",
    "whatsapp.com",
    "pinterest.com",
    "tumblr.com",
    "threads.net",
    "bsky.app",
    "mastodon.social",
    // Content farms / clickbait / low-quality tutorials
    "medium.com",
    "towardsdatascience.com",
    "betterprogramming.pub",
    "dev.to",
    "hashnode.com",
    "quora.com",
    "answers.com",
    "w3schools.com",
    "geeksforgeeks.org",
    "tutorialspoint.com",
    "javatpoint.com",
    "educba.com",
    "guru99.com",
    "yahoo.com",
    "msn.com",
    "cnn.com",
];

fn domain_is_blacklisted(url: &str) -> bool {
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .to_lowercase();
    for bad in DOMAIN_BLACKLIST {
        if host == *bad || host.ends_with(&format!(".{}", bad)) {
            return true;
        }
    }
    false
}

/// Extract search results from DuckDuckGo HTML. Returns formatted results string.
///
/// This is a lightweight string scan (no DOM parser): it locates each
/// `class="result__a"` anchor, pulls the real URL out of DDG's redirect
/// (`uddg=` param), grabs the accompanying snippet, and formats the result.
pub fn extract_ddg_results(html: &str, max_results: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    let mut rest = html;

    while count < max_results {
        let Some(marker) = rest.find("class=\"result__a\"") else {
            break;
        };

        // Find the start of the enclosing <a ...> tag.
        let tag_start = rest[..marker].rfind('<').unwrap_or(0);
        let Some(gt) = rest[tag_start..].find('>') else {
            break;
        };
        let tag = &rest[tag_start..tag_start + gt + 1];
        let after_tag = tag_start + gt + 1;

        let Some(href) = grab_attr(tag, "href") else {
            rest = &rest[after_tag..];
            continue;
        };

        let (title, title_end) = grab_tag_text(rest, after_tag);
        let snippet = rest[title_end..]
            .find("class=\"result__snippet\"")
            .map(|si| {
                let base = title_end + si;
                let tstart = rest[..base].rfind('<').unwrap_or(base);
                let g2 = rest[tstart..].find('>').unwrap_or(0);
                grab_tag_text(rest, tstart + g2 + 1).0
            })
            .unwrap_or_default();

        // Decode the real URL from DDG's redirect wrapper: /l/?uddg=ENCODED&...
        let url = if let Some(idx) = href.find("uddg=") {
            let enc = &href[idx + 5..];
            let enc = enc.split('&').next().unwrap_or(enc).trim();
            url_decode(enc)
        } else {
            url_decode(href.trim())
        };

        if domain_is_blacklisted(&url) {
            rest = &rest[title_end..];
            continue;
        }

        if !url.is_empty() {
            count += 1;
            out.push_str(&format!("{}. [{}]\n", count, url));
            if !title.is_empty() {
                out.push_str(&format!("   {}\n", title));
            }
            if !snippet.is_empty() {
                out.push_str(&format!("   {}\n", snippet));
            }
            out.push('\n');
        }

        rest = &rest[title_end..];
    }

    if count > 0 {
        format!("Search results ({}):\n\n{}", count, out)
    } else {
        String::new()
    }
}

/// Return the value of an attribute (e.g. `href`) found inside a tag string.
fn grab_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{}=\"", name);
    let i = tag.find(&needle)?;
    let start = i + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

/// Extract the text between `from` and the next `</a>`, cleaned of tags/entities.
fn grab_tag_text(html: &str, from: usize) -> (String, usize) {
    let end = html[from..]
        .find("</a>")
        .map(|e| from + e)
        .unwrap_or(html.len());
    let raw = &html[from..end];
    (clean_html_to_text(raw), end)
}

fn url_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '%' => {
                let hex: String = chars.by_ref().take(2).collect();
                if let Ok(b) = u8::from_str_radix(&hex, 16) {
                    bytes.push(b);
                } else {
                    bytes.push(b'%');
                    bytes.extend_from_slice(hex.as_bytes());
                }
            }
            '+' => bytes.push(b' '),
            _ => {
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

/// Extract the main textual content from an HTML page for fetch_url.
/// Returns the full page content with the fluff (scripts, styles, comments,
/// navigation, etc.) stripped out -- no truncation beyond what the caller
/// imposes, so the model gets the complete cleaned page.
pub fn extract_html_content(html: &str, _url: &str) -> String {
    clean_html_to_text(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A realistic (abbreviated) DuckDuckGo HTML results page.
    const DDG_FIXTURE: &str = r#"
        <div id="links">
          <div class="result__body">
            <a rel="nofollow" class="result__a" href="/l/?uddg=https%3A%2F%2Fwww.nvidia.com%2Fen-us%2Ftechnologies%2Fnim%2F&amp;rut=123">NVIDIA NIM | Generative AI Microservices</a>
            <a class="result__snippet">NVIDIA NIM is a set of microservices to deploy AI models like GLM across GPUs.</a>
          </div>
          <div class="result__body">
            <a rel="nofollow" class="result__a" href="/l/?uddg=https%3A%2F%2Fwww.reddit.com%2Fr%2Flocalllama%2F&amp;rut=456">Discussion on Reddit</a>
            <a class="result__snippet">People talking about NIM and GLM models.</a>
          </div>
          <div class="result__body">
            <a rel="nofollow" class="result__a" href="/l/?uddg=https%3A%2F%2Fhuggingface.co%2Fspaces%2F&amp;rut=789">GLM on Hugging Face</a>
            <a class="result__snippet">Try GLM 5.2 directly in your browser.</a>
          </div>
        </div>
    "#;

    #[test]
    fn extract_ddg_results_basic() {
        let out = extract_ddg_results(DDG_FIXTURE, 10);
        // Reddit is blacklisted, so only 2 of 3 should appear.
        assert!(out.contains("Search results (2):"));
        assert!(out.contains("https://www.nvidia.com/en-us/technologies/nim/"));
        assert!(out.contains("https://huggingface.co/spaces/"));
        assert!(out.contains("NVIDIA NIM"));
        assert!(out.contains("GLM on Hugging Face"));
        // Blacklisted domain must be excluded.
        assert!(!out.contains("reddit.com"));
    }

    #[test]
    fn extract_ddg_respects_max_results() {
        let out = extract_ddg_results(DDG_FIXTURE, 1);
        assert!(out.contains("Search results (1):"));
        // Only the first non-blacklisted result (nvidia) is present.
        assert!(out.contains("nvidia.com"));
        assert!(!out.contains("huggingface.co"));
    }

    #[test]
    fn extract_ddg_decodes_uddg_url() {
        let out = extract_ddg_results(DDG_FIXTURE, 10);
        // The uddg param is percent-encoded; verify it decodes to a real URL.
        assert!(out.contains("https://www.nvidia.com/en-us/technologies/nim/"));
        assert!(!out.contains("uddg="));
    }

    #[test]
    fn extract_ddg_empty_input() {
        assert_eq!(extract_ddg_results("", 5), "");
        assert_eq!(
            extract_ddg_results("<html><body>no results</body></html>", 5),
            ""
        );
    }

    #[test]
    fn domain_blacklist_checks() {
        assert!(domain_is_blacklisted("https://reddit.com/x"));
        assert!(domain_is_blacklisted("http://www.youtube.com/watch"));
        assert!(domain_is_blacklisted("https://old.reddit.com/r/foo"));
        assert!(!domain_is_blacklisted("https://nvidia.com/nim"));
        assert!(!domain_is_blacklisted("https://example.com"));
    }

    #[test]
    fn url_decode_checks() {
        assert_eq!(
            url_decode("https%3A%2F%2Fexample.com"),
            "https://example.com"
        );
        assert_eq!(url_decode("a+b"), "a b");
        assert_eq!(url_decode("foo%20bar"), "foo bar");
    }

    #[test]
    fn html_content_cleaned() {
        let page = "<html><head><style>x{}</style></head><body>\
            <script>bad()</script>\
            <nav>menu</nav>\
            <main><h1>Title</h1><p>Body text here.</p></main>\
            </body></html>";
        let out = extract_html_content(page, "https://example.com");
        assert!(out.contains("Title"));
        assert!(out.contains("Body text here."));
        assert!(!out.contains("bad()"));
        assert!(!out.contains("menu"));
    }
}
