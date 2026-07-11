// extract.rs -- HTML content extraction for web_search and fetch_url tools.
// Uses a small, dependency-free HTML cleaner (see `html.rs`) instead of a full
// DOM parsing crate. This avoids the empty-result failures we saw with the
// previous parser and lets us hand the model the full cleaned page.

use crate::utils::html::{clean_html_to_text, extract_embedded_json_prose};
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

/// How much usable content we were able to recover from a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractQuality {
    /// Real body text was present in the static HTML.
    Full,
    /// The body was empty (typically a JavaScript-rendered SPA); we fell back
    /// to the page's `<title>` / `<meta description>` summary instead.
    MetadataOnly,
    /// Nothing usable at all could be recovered.
    Empty,
}

/// Result of extracting content from a fetched HTML page.
pub struct ExtractedPage {
    pub content: String,
    pub quality: ExtractQuality,
}

/// Extract the main textual content from an HTML page for fetch_url.
/// Returns the full page content with the fluff (scripts, styles, comments,
/// navigation, etc.) stripped out -- no truncation beyond what the caller
/// imposes, so the model gets the complete cleaned page.
///
/// Many modern doc/reference sites (ReadMe, etc.) are client-rendered SPAs:
/// the static HTML body is empty and the real content is injected by
/// JavaScript after load. For those pages the cleaned body is empty, so we
/// fall back to the `<title>` and `<meta name="description">` / `og:description`
/// tags, which hold a meaningful summary the model can still use. The
/// `quality` field tells the caller whether that fallback happened, so it can
/// warn the model that the page is not fully readable and it should try an
/// alternate, server-rendered source (e.g. an article or wiki that sends
/// complete HTML rather than an interactive web app).
pub fn extract_html_content(html: &str, _url: &str) -> ExtractedPage {
    let body = clean_html_to_text(html);

    // Client-rendered SPAs often ship an empty `<body>` and embed the real
    // content as JSON inside `<script>` tags. Recover that prose; if it beats
    // the (empty) visible body it is the better source, and it works without a
    // headless browser.
    let embedded = extract_embedded_json_prose(html);
    if embedded.trim().len() >= 200 && embedded.trim().len() > body.trim().len() {
        return ExtractedPage {
            content: embedded,
            quality: ExtractQuality::Full,
        };
    }

    if !body.trim().is_empty() {
        return ExtractedPage {
            content: body,
            quality: ExtractQuality::Full,
        };
    }

    let mut out = String::new();
    if let Some(title) = extract_tag_text(html, "title")
        && !title.is_empty()
    {
        out.push_str(&format!("Title: {}\n", title));
    }
    if let Some(desc) = extract_meta_content(html, "name", "description")
        .or_else(|| extract_meta_content(html, "property", "og:description"))
        && !desc.is_empty()
    {
        out.push_str(&format!("Description: {}\n", desc));
    }

    if out.is_empty() {
        ExtractedPage {
            content: body,
            quality: ExtractQuality::Empty,
        }
    } else {
        ExtractedPage {
            content: out.trim().to_string(),
            quality: ExtractQuality::MetadataOnly,
        }
    }
}

/// Extract the inner text of a simple tag such as `<title>...</title>`.
fn extract_tag_text(html: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let i = html.find(&open)?;
    let gt = html[i..].find('>').map(|g| i + g)?;
    let close = format!("</{}>", tag);
    let ce = html[gt..].find(&close).map(|c| gt + c)?;
    let raw = &html[gt + 1..ce];
    Some(clean_html_to_text(raw))
}

/// Extract the `content` attribute of a `<meta ...>` tag selected by one of its
/// other attributes, e.g. `extract_meta_content(html, "name", "description")`
/// matches `<meta name="description" content="...">`.
fn extract_meta_content(html: &str, attr: &str, val: &str) -> Option<String> {
    let needle = format!("{}=\"{}\"", attr, val);
    let i = html.find(&needle)?;
    let tag_start = html[..i].rfind('<')?;
    let tag = &html[tag_start..];
    let gt = tag.find('>')?;
    let content = grab_attr(&tag[..gt], "content")?;
    // Attribute values may contain HTML entities (e.g. &amp;); decode them.
    Some(clean_html_to_text(&content))
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
        let res = extract_html_content(page, "https://example.com");
        assert_eq!(res.quality, ExtractQuality::Full);
        assert!(res.content.contains("Title"));
        assert!(res.content.contains("Body text here."));
        assert!(!res.content.contains("bad()"));
        assert!(!res.content.contains("menu"));
    }

    #[test]
    fn spa_falls_back_to_head_metadata() {
        // A client-rendered SPA: empty body, but useful <title>/<meta>.
        let page = r#"<!DOCTYPE html><html><head>
            <title>z-ai / glm-5.2</title>
            <meta name="description" content="GLM-5.2 is the latest flagship LLM from Z.ai.">
            <script src="app.js"></script>
        </head><body><div id="root"></div></body></html>"#;
        let res = extract_html_content(
            page,
            "https://docs.api.nvidia.com/nim/reference/z-ai-glm-5.2",
        );
        assert_eq!(res.quality, ExtractQuality::MetadataOnly);
        assert!(res.content.contains("Title: z-ai / glm-5.2"));
        assert!(
            res.content
                .contains("Description: GLM-5.2 is the latest flagship LLM from Z.ai.")
        );
        assert!(!res.content.contains("<div id=\"root\""));
        assert!(!res.content.contains("app.js"));
    }

    #[test]
    fn spa_with_embedded_json_recovers_content() {
        // ReadMe-style SPA: empty body but the real docs live in inline JSON.
        let page = r#"<!DOCTYPE html><html><head><title>z-ai / glm-5.2</title>
            <meta name="description" content="GLM-5.2 is the latest flagship LLM from Z.ai.">
            </head><body><div id="root"></div>
            <script type="application/json">{"data":{
                "summary":"GLM-5.2 is a long-context model with a 1M-token window. It is built for retrieval augmented generation and agentic workflows where the full document history must remain in view.",
                "detail":"It is designed for long-horizon tasks and tool use. The model streams responses and supports structured outputs for production pipelines.",
                "notes":"Use a lower temperature for deterministic results and reserve higher values for creative brainstorming."
            }}</script>
        </body></html>"#;
        let res = extract_html_content(
            page,
            "https://docs.api.nvidia.com/nim/reference/z-ai-glm-5.2",
        );
        assert_eq!(res.quality, ExtractQuality::Full);
        assert!(
            res.content
                .contains("long-context model with a 1M-token window")
        );
        assert!(res.content.contains("long-horizon tasks and tool use"));
        assert!(res.content.contains("deterministic results"));
    }

    #[test]
    fn empty_body_without_metadata_stays_empty() {
        let page = "<html><head></head><body><script>x()</script></body></html>";
        let res = extract_html_content(page, "https://example.com");
        assert_eq!(res.quality, ExtractQuality::Empty);
        assert!(res.content.trim().is_empty());
    }

    #[test]
    fn meta_content_decodes_entities() {
        let page = r#"<html><head><title>A &amp; B</title>
            <meta property="og:description" content="Tom &amp; Jerry &lt;3"></head><body></body></html>"#;
        let res = extract_html_content(page, "https://example.com");
        assert_eq!(res.quality, ExtractQuality::MetadataOnly);
        assert!(res.content.contains("Title: A & B"));
        assert!(res.content.contains("Description: Tom & Jerry <3"));
    }
}
