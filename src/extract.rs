// extract.rs -- HTML content extraction for web_search and fetch_url tools.
// Uses scraper (html5ever + CSS selectors) for robust extraction.

use scraper::{Html, Selector};
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

pub fn search_cache_set(key: &str, value: &str) {
    if let Ok(mut cache) = SEARCH_CACHE.lock() {
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
pub fn extract_ddg_results(html: &str, max_results: usize) -> String {
    let doc = Html::parse_document(html);
    let mut out = String::new();
    let mut count = 0;

    // DDG result containers
    let Ok(result_sel) = Selector::parse(".result__body") else {
        return String::new();
    };
    let Ok(url_sel) = Selector::parse(".result__a") else {
        return String::new();
    };
    let Ok(snippet_sel) = Selector::parse(".result__snippet") else {
        return String::new();
    };

    for result in doc.select(&result_sel) {
        if count >= max_results {
            break;
        }

        // Extract actual URL from DDG's redirect link: //duckduckgo.com/l/?uddg=ENCODED_URL
        let raw_url = result
            .select(&url_sel)
            .next()
            .and_then(|a| a.value().attr("href"))
            .unwrap_or("");

        // Decode the uddg parameter from the redirect URL
        let url = if raw_url.contains("uddg=") {
            raw_url
                .split("uddg=")
                .nth(1)
                .unwrap_or(raw_url)
                .split('&')
                .next()
                .unwrap_or(raw_url)
                .trim()
        } else {
            raw_url.trim()
        };
        // URL-decode the extracted value
        let decoded = url_decode(url);

        // Skip blacklisted domains
        if domain_is_blacklisted(&decoded) {
            continue;
        }

        let snippet: String = result
            .select(&snippet_sel)
            .next()
            .map(|e| e.text().collect::<Vec<_>>().join(" ").trim().to_string())
            .unwrap_or_default();

        if !decoded.is_empty() {
            count += 1;
            out.push_str(&format!("{}. [{}]\n", count, decoded));
            if !snippet.is_empty() {
                out.push_str(&format!("   {}\n", snippet));
            }
            out.push('\n');
        }
    }

    if count > 0 {
        format!("Search results ({}):\n\n{}", count, out)
    } else {
        String::new()
    }
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
pub fn extract_html_content(html: &str, url: &str) -> String {
    let doc = Html::parse_document(html);

    if url.contains("github.com")
        && let Some(text) = try_extract_github(&doc)
    {
        return text;
    }

    let main_selectors = [
        "article",
        "[role=main]",
        "main",
        ".post-content",
        ".entry-content",
        ".content",
        "#content",
        ".markdown-body",
        "body",
    ];

    for sel_str in &main_selectors {
        if let Ok(sel) = Selector::parse(sel_str)
            && let Some(element) = doc.select(&sel).next()
        {
            let text = collect_text(&element, 100_000);
            if text.len() > 80 {
                return text;
            }
        }
    }

    collect_text_from_root(&doc)
}

// -- GitHub content extraction --------------------------------------------------

fn try_extract_github(doc: &Html) -> Option<String> {
    if let Ok(sel) = Selector::parse(".highlight")
        && let Some(el) = doc.select(&sel).next()
    {
        let text = collect_text(&el, 100_000);
        if !text.is_empty() {
            return Some(format!("```\n{}\n```", collapse_whitespace(&text)));
        }
    }
    if let Ok(sel) = Selector::parse("article.markdown-body")
        && let Some(el) = doc.select(&sel).next()
    {
        let text = collect_text(&el, 50_000);
        if text.len() > 50 {
            return Some(text);
        }
    }
    if let Ok(sel) = Selector::parse(".repo-description")
        && let Some(el) = doc.select(&sel).next()
    {
        let text = el.text().collect::<Vec<_>>().join(" ");
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

// -- Text collection utilities --------------------------------------------------

fn collect_text(element: &scraper::ElementRef, max_chars: usize) -> String {
    let mut result = String::with_capacity(max_chars.min(4096));
    collect_text_inner(element, &mut result, max_chars);
    result.shrink_to_fit();
    collapse_whitespace(&result)
}

fn collect_text_inner(node: &scraper::ElementRef, result: &mut String, max: usize) {
    if result.len() >= max {
        return;
    }
    let skip_tags = [
        "script", "style", "nav", "footer", "header", "aside", "noscript", "svg", "form", "button",
        "select", "textarea", "iframe", "canvas",
    ];
    if skip_tags.contains(&node.value().name()) {
        return;
    }
    for child in node.children() {
        if let Some(text) = child.value().as_text() {
            let s = text.text.trim();
            if !s.is_empty() {
                if !result.is_empty() && !result.ends_with(' ') {
                    result.push(' ');
                }
                let remaining = max.saturating_sub(result.len());
                if remaining == 0 {
                    return;
                }
                if s.len() <= remaining {
                    result.push_str(s);
                } else {
                    result.push_str(&s[..remaining]);
                    return;
                }
            }
        } else if let Some(el) = child.value().as_element() {
            if skip_tags.contains(&el.name()) {
                continue;
            }
            if let Some(child_ref) = scraper::ElementRef::wrap(child) {
                collect_text_inner(&child_ref, result, max);
            }
        }
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

fn collect_text_from_root(doc: &Html) -> String {
    if let Ok(sel) = Selector::parse("body")
        && let Some(body) = doc.select(&sel).next()
    {
        return collect_text(&body, 100_000);
    }
    String::new()
}
