// html.rs -- Minimal, dependency-free HTML cleaning for web tools.
//
// We deliberately avoid pulling in a full HTML parsing crate (the old
// `scraper` dependency). Instead this module does a single forward pass over
// the raw bytes: it drops non-content blocks (scripts, styles, svg, comments,
// etc.) and strips the remaining tags so the model receives the page *content*
// with the fluff removed. Context windows are large enough now that we can hand
// the model the full cleaned page and let it make the decisions.

/// Tags whose entire contents we discard (they carry no readable content).
const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "svg", "template", "head", "meta", "link", "iframe", "canvas",
    "nav", "footer", "header", "aside", "form", "button", "select", "textarea",
];

/// Void/self-closing elements that have no closing tag. When we hit one we must
/// drop only the tag itself and never enter "skip content" mode, otherwise we'd
/// swallow the rest of the document looking for a close that never comes.
const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Convert a raw HTML document into clean, readable text:
/// no scripts, no styles, no tags, with entities decoded and whitespace
/// collapsed. The full textual content is preserved (subject to any caller
/// imposed byte cap).
pub fn clean_html_to_text(html: &str) -> String {
    let chars: Vec<char> = html.chars().collect();
    let mut pos = 0usize;
    let mut out = String::with_capacity(html.len() / 2);
    let mut skip_stack: Vec<String> = Vec::new();

    while pos < chars.len() {
        let c = chars[pos];

        if c == '<' {
            pos += 1;

            // Comment? <!-- ... -->
            if chars.get(pos) == Some(&'!') {
                pos += 1;
                if chars.get(pos) == Some(&'-') {
                    pos += 1;
                    if chars.get(pos) == Some(&'-') {
                        pos += 1;
                        skip_until(&chars, &mut pos, &['-', '-', '>']);
                        continue;
                    }
                }
                // Some other <!...> declaration; read to '>'.
                skip_until(&chars, &mut pos, &['>']);
                continue;
            }

            // Read the whole tag (everything up to '>').
            let tag_start = pos;
            while pos < chars.len() && chars[pos] != '>' {
                pos += 1;
            }
            let tag: String = chars[tag_start..pos].iter().collect();
            if pos < chars.len() {
                pos += 1; // consume the '>'
            }

            let tag = tag.trim();
            let closing = tag.starts_with('/');
            let name = tag
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();

            if closing {
                // Pop the matching open tag from the skip stack. Only pop when
                // it's actually on the stack, so a stray/mismatched close can't
                // leave us stuck skipping the rest of the document.
                if let Some(idx) = skip_stack.iter().rposition(|t| t == &name) {
                    skip_stack.truncate(idx);
                }
            } else if tag.ends_with('/') || VOID_TAGS.contains(&name.as_str()) {
                // Self-closing or void element (e.g. <meta>, <br/>): the tag is
                // dropped but it carries no content, so we must NOT enter skip
                // mode or we'd swallow everything after it.
            } else if SKIP_TAGS.contains(&name.as_str()) {
                skip_stack.push(name);
            }
            continue;
        }

        // Inside a skipped block: drop everything.
        if !skip_stack.is_empty() {
            pos += 1;
            continue;
        }

        // Regular text: decode entities.
        if c == '&' {
            pos += 1;
            out.push_str(&decode_entity(&chars, &mut pos));
        } else {
            out.push(c);
            pos += 1;
        }
    }

    collapse_whitespace(&out)
}

/// Advance `pos` past the next occurrence of `needle` (needle included).
fn skip_until(chars: &[char], pos: &mut usize, needle: &[char]) {
    if needle.is_empty() {
        return;
    }
    while *pos + needle.len() <= chars.len() {
        if &chars[*pos..*pos + needle.len()] == needle {
            *pos += needle.len();
            return;
        }
        *pos += 1;
    }
    *pos = chars.len();
}

/// Decode a single HTML entity. `pos` points at the first character after the
/// leading `&` (already consumed by the caller). Known entities are replaced
/// with their character; unknown ones are returned verbatim (e.g. `&foo;`
/// stays `&foo;`) so no content is lost.
fn decode_entity(chars: &[char], pos: &mut usize) -> String {
    let start = *pos;
    let mut count = 0;
    while *pos < chars.len() && chars[*pos] != ';' && count < 32 {
        *pos += 1;
        count += 1;
    }
    let ent: String = chars[start..*pos].iter().collect();
    // Consume the trailing ';' if present.
    if *pos < chars.len() && chars[*pos] == ';' {
        *pos += 1;
    }

    let decoded = match ent.as_str() {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "copy" => '©',
        "reg" => '®',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        _ => {
            if let Some(code) = ent
                .strip_prefix("#x")
                .or_else(|| ent.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .and_then(char::from_u32)
            {
                code
            } else if let Some(code) = ent
                .strip_prefix('#')
                .and_then(|d| d.parse::<u32>().ok())
                .and_then(char::from_u32)
            {
                code
            } else if ent.is_empty() {
                '&'
            } else {
                // Unknown named entity: emit it verbatim so nothing is lost.
                return format!("&{};", ent);
            }
        }
    };
    decoded.to_string()
}

/// Collapse runs of whitespace into a single space and trim.
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
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::clean_html_to_text;

    #[test]
    fn keeps_plain_text() {
        assert_eq!(clean_html_to_text("hello world"), "hello world");
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(clean_html_to_text(""), "");
        assert_eq!(clean_html_to_text("   \n\t  "), "");
    }

    #[test]
    fn strips_script_content() {
        let html = "<p>before</p><script>var x = 1; alert('hi');</script><p>after</p>";
        let out = clean_html_to_text(html);
        assert!(!out.contains("alert"));
        assert!(!out.contains("var x"));
        assert!(out.contains("before"));
        assert!(out.contains("after"));
    }

    #[test]
    fn strips_style_content() {
        let html = "<style>.a{color:red}</style><p>visible</p>";
        let out = clean_html_to_text(html);
        assert!(!out.contains("color:red"));
        assert!(out.contains("visible"));
    }

    #[test]
    fn strips_comments() {
        let html = "<p>real</p><!-- this is a comment <p>fake</p> --><p>more</p>";
        let out = clean_html_to_text(html);
        assert!(!out.contains("this is a comment"));
        assert!(!out.contains("fake"));
        assert!(out.contains("real"));
        assert!(out.contains("more"));
    }

    #[test]
    fn strips_doctype_and_svg() {
        let html =
            "<!DOCTYPE html><html><body><svg><path d=\"M0 0\"/></svg><p>content</p></body></html>";
        let out = clean_html_to_text(html);
        assert!(!out.contains("DOCTYPE"));
        assert!(!out.contains("<path"));
        assert!(out.contains("content"));
    }

    #[test]
    fn strips_chrome_tags() {
        let html =
            "<header>top nav</header><nav>links</nav><main>body text</main><footer>legal</footer>";
        let out = clean_html_to_text(html);
        assert!(!out.contains("top nav"));
        assert!(!out.contains("links"));
        assert!(!out.contains("legal"));
        assert!(out.contains("body text"));
    }

    #[test]
    fn decodes_common_entities() {
        assert_eq!(clean_html_to_text("a &amp; b"), "a & b");
        assert_eq!(clean_html_to_text("&lt;tag&gt;"), "<tag>");
        assert_eq!(clean_html_to_text("&quot;hi&quot;"), "\"hi\"");
        assert_eq!(clean_html_to_text("it&#39;s"), "it's");
        assert_eq!(clean_html_to_text("a&nbsp;b"), "a b");
    }

    #[test]
    fn decodes_numeric_entities() {
        assert_eq!(clean_html_to_text("&#65;&#66;&#67;"), "ABC");
        assert_eq!(clean_html_to_text("&#x41;&#x42;"), "AB");
    }

    #[test]
    fn unknown_entity_preserved() {
        // Unknown named entities must survive verbatim, not be dropped.
        assert_eq!(clean_html_to_text("foo&bar;baz"), "foo&bar;baz");
    }

    #[test]
    fn collapses_whitespace() {
        let html = "<p>one\n\n   two\t\tthree</p>";
        assert_eq!(clean_html_to_text(html), "one two three");
    }

    #[test]
    fn nested_tags_preserved_as_text() {
        let html = "<div><span>hello </span><b>world</b></div>";
        assert_eq!(clean_html_to_text(html), "hello world");
    }

    #[test]
    fn full_page_roundtrip_keeps_main_content() {
        let html = r#"
            <!DOCTYPE html>
            <html>
              <head><title>Skip me</title><meta charset="utf-8"></head>
              <body>
                <script>console.log('tracking');</script>
                <header>banner</header>
                <article>
                  <h1>Great Article</h1>
                  <p>The quick brown fox jumps over the lazy dog.</p>
                  <p>Second paragraph with &amp; an ampersand.</p>
                </article>
                <footer>copyright</footer>
              </body>
            </html>
        "#;
        let out = clean_html_to_text(html);
        assert!(out.contains("Great Article"));
        assert!(out.contains("quick brown fox"));
        assert!(out.contains("lazy dog"));
        assert!(out.contains("ampersand"));
        assert!(!out.contains("tracking"));
        assert!(!out.contains("banner"));
        assert!(!out.contains("copyright"));
        assert!(!out.contains("Skip me"));
    }
}
