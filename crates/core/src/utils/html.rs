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

/// Block-level elements: we insert a line break at their open/close boundaries
/// so the cleaned text preserves document structure (paragraphs, table rows,
/// list items, headings, ...) instead of being flattened onto a single line.
const BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "br",
    "dd",
    "div",
    "dl",
    "dt",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
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
    // Tracks whether the next non-space character needs a separating space
    // (i.e. we just emitted inline whitespace). Block boundaries reset this so
    // leading indentation is dropped and no spurious space starts a new line.
    let mut needs_space = false;

    while pos < chars.len() {
        // Inside a skipped block (script/style/head/...): scan for the matching
        // close tag and drop everything else. Scanning for the literal close
        // tag (instead of re-parsing every `<`) keeps us robust against `<` and
        // `>` characters that legitimately appear inside script/style content,
        // which previously desynced the parser and could swallow the whole
        // document (e.g. a `<` inside a CSS media query like `media="(width <
        // 40rem)"`, or `<`/`>` inside embedded JSON).
        if !skip_stack.is_empty() {
            let top = skip_stack.last().unwrap().clone();
            let needle: Vec<char> = format!("</{}", top).chars().collect();
            let mut closed = false;
            while pos + needle.len() <= chars.len() {
                let window: Vec<char> = chars[pos..pos + needle.len()]
                    .iter()
                    .map(|c| c.to_ascii_lowercase())
                    .collect();
                if window == needle {
                    pos += needle.len();
                    // Consume any trailing whitespace and the closing '>'.
                    while pos < chars.len() && chars[pos].is_whitespace() {
                        pos += 1;
                    }
                    if chars.get(pos) == Some(&'>') {
                        pos += 1;
                    }
                    skip_stack.pop();
                    closed = true;
                    break;
                }
                pos += 1;
            }
            if !closed {
                // No matching close tag: skip to end of input rather than
                // risk desyncing on the remaining document.
                pos = chars.len();
            }
            continue;
        }

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

            // Read the whole tag (everything up to '>'), but treat `<` and `>`
            // inside quoted attribute values as ordinary characters so they
            // can't break the tag boundary. Without this, a URL/attribute that
            // legitimately contains `<` or `>` (e.g. a CSS media query
            // `media="(width < 40rem)"` or a query string with `>`) would
            // prematurely end the tag and desync the rest of the document.
            let tag_start = pos;
            while pos < chars.len() {
                let ch = chars[pos];
                if ch == '>' {
                    break;
                }
                if ch == '"' || ch == '\'' {
                    let quote = ch;
                    pos += 1;
                    while pos < chars.len() && chars[pos] != quote {
                        pos += 1;
                    }
                    if pos < chars.len() {
                        pos += 1; // consume the closing quote
                    }
                    continue;
                }
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
                skip_stack.push(name.clone());
            }

            // Insert a line break at block-level boundaries so the cleaned text
            // keeps its structure (paragraphs, table rows, list items, ...) and
            // isn't flattened onto a single line. A leading/trailing break is
            // harmless — the final whitespace pass trims runs.
            if BLOCK_TAGS.contains(&name.as_str()) {
                out.push('\n');
                needs_space = false;
            }
            continue;
        }

        // Regular text: decode entities. Source whitespace (including newlines
        // from the original markup) is collapsed to a single separating space so
        // that line breaks only appear at the block boundaries we inserted above.
        if c == '&' {
            pos += 1;
            let decoded = decode_entity(&chars, &mut pos);
            for ch in decoded.chars() {
                emit_text_char(&mut out, &mut needs_space, ch);
            }
        } else {
            emit_text_char(&mut out, &mut needs_space, c);
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

/// Emit a single character of text content, collapsing runs of inline
/// whitespace into a single separating space. Block-level line breaks (real
/// `'\n'`) are emitted separately by the caller and are *not* produced here, so
/// the only newlines in the output are the structural ones we inserted.
fn emit_text_char(out: &mut String, needs_space: &mut bool, c: char) {
    if c.is_whitespace() {
        *needs_space = true;
    } else {
        if *needs_space && !out.is_empty() && !out.ends_with('\n') {
            out.push(' ');
        }
        out.push(c);
        *needs_space = false;
    }
}

/// Collapse runs of whitespace while preserving the structural line breaks
/// inserted at block-level boundaries. Consecutive spaces become a single
/// space, consecutive newlines become a single newline, and leading/trailing
/// whitespace (including breaks) is trimmed.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    let mut in_newline = false;
    for c in s.chars() {
        if c == '\n' {
            if !in_newline {
                out.push('\n');
            }
            in_newline = true;
            in_space = false;
        } else if c.is_whitespace() {
            in_space = true;
            in_newline = false;
        } else {
            if in_space && !out.is_empty() && !out.ends_with('\n') {
                out.push(' ');
            }
            out.push(c);
            in_space = false;
            in_newline = false;
        }
    }
    out.trim().to_string()
}

/// Remove the auto-generated table-of-contents that `doctoc` (and similar
/// tools) inject into Markdown documents. These TOCs live between a
/// `<!-- START doctoc ... -->` and a `<!-- END doctoc ... -->` HTML comment and
/// consist entirely of anchor links — pure navigation noise that can dwarf the
/// actual document text and, because it sits at the very top, crowd out the
/// real content when the fetch is byte-capped. The surrounding document is
/// returned unchanged when no such markers are present.
pub fn strip_doctoc_toc(s: &str) -> String {
    let start_marker = "<!-- START doctoc";
    let end_marker = "<!-- END doctoc";
    let Some(start) = s.find(start_marker) else {
        return s.to_string();
    };
    let open = s[..start].rfind("<!--").unwrap_or(start);
    let Some(end_rel) = s[start..].find(end_marker) else {
        return s.to_string();
    };
    let end_abs = start + end_rel;
    let Some(close_rel) = s[end_abs..].find("-->") else {
        return s.to_string();
    };
    let close = end_abs + close_rel + 3; // include the "-->"

    let mut out = String::with_capacity(s.len());
    out.push_str(s[..open].trim_end());
    let tail = s[close..].trim_start();
    out.push_str(tail);
    out
}

/// Recover human-readable documentation prose that modern client-rendered
/// SPAs (ReadMe, Next.js, etc.) embed inside inline `<script>` JSON blobs.
///
/// These pages ship an essentially empty `<body>` and hydrate from data that
/// lives in JSON, so `clean_html_to_text` (which strips `<script>` content)
/// sees nothing. This walks every inline JSON `<script>` and lifts the
/// descriptive string values out, giving `fetch_url` real content without
/// needing a headless browser. It is intentionally conservative: only
/// sentence-like strings (spaces present, mostly alphabetic, no markup or code
/// punctuation) are kept, which filters out keys, code samples, and JSON
/// fragments while preserving the actual prose.
pub fn extract_embedded_json_prose(html: &str) -> String {
    let total = html.len();
    let mut i = 0;
    let mut found: Vec<String> = Vec::new();

    while i < total {
        let rest = &html[i..];
        let Some(open_rel) = rest.find("<script") else {
            break;
        };
        let open = i + open_rel;
        let Some(tag_end_rel) = html[open..].find('>') else {
            break;
        };
        let tag_end = open + tag_end_rel;
        let Some(close_rel) = html[tag_end..].find("</script>") else {
            break;
        };
        let close = tag_end + close_rel;
        let inner = &html[tag_end + 1..close];

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(inner) {
            collect_prose(&json, &mut found);
        }

        i = close + "</script>".len();
    }

    let mut seen = std::collections::HashSet::new();
    let mut out = String::new();
    for s in found {
        if seen.insert(s.clone()) {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(s.trim());
        }
    }
    out
}

/// Recursively collect prose-like string leaves from a JSON value.
fn collect_prose(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => {
            let t = s.trim();
            if is_prose_like(t) {
                out.push(t.to_string());
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                collect_prose(x, out);
            }
        }
        serde_json::Value::Object(o) => {
            for (_, x) in o {
                collect_prose(x, out);
            }
        }
        _ => {}
    }
}

/// Heuristic: is this string a chunk of documentation prose rather than a key,
/// code fragment, URL, or JSON snippet?
fn is_prose_like(t: &str) -> bool {
    let len = t.len();
    if !(30..=3000).contains(&len) {
        return false;
    }
    if !(t.contains(' ') || t.contains('\n')) {
        return false;
    }
    if t.contains('<')
        || t.contains('{')
        || t.contains(';')
        || t.contains('=')
        || t.contains("://")
        || t.contains("function")
        || t.contains("=>")
    {
        return false;
    }
    let alpha = t.chars().filter(|c| c.is_alphabetic()).count();
    let ratio = alpha as f64 / t.chars().count() as f64;
    ratio > 0.5
}

#[cfg(test)]
mod tests {
    use super::{clean_html_to_text, extract_embedded_json_prose, strip_doctoc_toc};

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
    fn link_with_lt_in_attribute_does_not_leak() {
        // Verbatim Discourse-style fragment: a <script> containing a JS template
        // literal with an embedded <svg> tag and an HTML comment, followed by
        // <link> tags whose media query contains a `<` (media="(width < 40rem)").
        let html = r##"<body>
    <script nonce="Wn7bYvsSrnClRvRuB13cS8SoC">
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1"><!-- LCP candidate image ${".".repeat(5000)} --></svg>`;
    document.querySelector("#d-splash .preloader-image").style.backgroundImage = `url('data:image/svg+xml,${svg}')`
  </script>

  <noscript>
    <style>
      html { overflow-y: revert !important; }
      #d-splash { display: none; }
    </style>
  </noscript>
</section>


    <discourse-assets>
      <discourse-assets-stylesheets>
        <link href="x.css?ws=dev" media="(prefers-color-scheme: light)" rel="stylesheet" data-scheme-id="4"/><link href="y.css?ws=dev" media="(prefers-color-scheme: dark)" rel="stylesheet" data-scheme-id="1"/>

<link href="common.css?ws=dev" media="all" rel="stylesheet" data-target="common"  />

<link href="mobile.css?ws=dev" media="(width < 40rem)" rel="stylesheet" data-target="mobile"  />
<link href="desktop.css?ws=dev" media="(max-width: 40rem)" rel="stylesheet" data-target="desktop"  />
<p>hello world</p></discourse-assets-stylesheets></discourse-assets></body>"##;
        let out = clean_html_to_text(html);
        assert!(!out.contains("40rem"), "attribute text leaked: {}", out);
        assert!(out.contains("hello world"));
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
    fn table_cells_and_rows_break_onto_lines() {
        // Mirrors the doxygen struct-reference layout the scraper was
        // flattening onto one line: a table whose cells/rows must stay on
        // their own lines while preserving order.
        let html = "<table>\
            <tr><td>av_class</td><td>A class for logging.</td></tr>\
            <tr><td>iformat</td><td>The input container format.</td></tr>\
            <tr><td>pb</td><td>I/O context.</td></tr>\
        </table>";
        let out = clean_html_to_text(html);
        // Content order preserved...
        let a = out.find("av_class").unwrap();
        let b = out.find("iformat").unwrap();
        let c = out.find("pb").unwrap();
        assert!(a < b && b < c, "order not preserved: {}", out);
        // ...and each field sits on its own line (>= 2 real newlines).
        assert!(out.matches('\n').count() >= 2, "no line breaks: {}", out);
    }

    #[test]
    fn br_and_hr_insert_line_breaks() {
        assert_eq!(clean_html_to_text("a<br>b"), "a\nb");
        assert_eq!(clean_html_to_text("x<hr>y"), "x\ny");
        assert_eq!(clean_html_to_text("a<br/>b"), "a\nb");
    }

    #[test]
    fn headings_and_lists_structure_preserved() {
        let html = "<h1>Title</h1><p>Intro.</p>\
            <ul><li>one</li><li>two</li></ul>\
            <h2>Section</h2><p>Body text.</p>";
        let out = clean_html_to_text(html);
        let title = out.find("Title").unwrap();
        let intro = out.find("Intro.").unwrap();
        let one = out.find("one").unwrap();
        let two = out.find("two").unwrap();
        let section = out.find("Section").unwrap();
        let body = out.find("Body text.").unwrap();
        assert!(title < intro && intro < one && one < two && two < section && section < body);
        // Lists/headings produce multiple lines.
        assert!(out.matches('\n').count() >= 4, "structure lost: {}", out);
    }

    #[test]
    fn single_paragraph_stays_one_line() {
        // Source newlines inside a block must NOT become breaks; only the block
        // boundaries we insert should.
        let html = "<p>the quick\nbrown fox\njumps</p>";
        assert_eq!(clean_html_to_text(html), "the quick brown fox jumps");
    }

    #[test]
    fn nested_block_tags_collapse_to_single_breaks() {
        let html = "<div><p>a</p><p>b</p></div>";
        let out = clean_html_to_text(html);
        assert!(out.contains("a\nb"), "nested blocks lost: {}", out);
        // No more than one consecutive newline (no empty filler lines).
        assert!(!out.contains("\n\n"), "extra blank lines: {:?}", out);
    }

    #[test]
    fn block_break_drops_leading_indentation() {
        // Whitespace between a block boundary and following text must not leave
        // a leading space on the new line.
        let html = "<p>first</p>   \n   <p>second</p>";
        let out = clean_html_to_text(html);
        assert!(out.contains("first\nsecond"), "indent leaked: {:?}", out);
        assert!(
            !out.contains("first\n "),
            "leading space after break: {:?}",
            out
        );
    }

    #[test]
    fn skip_tag_boundary_does_not_swallow_document() {
        // A block-level tag that is also skipped (e.g. <header>) must still emit
        // a clean break and not eat the following content.
        let html = "<header>nav stuff</header><p>real content</p>";
        let out = clean_html_to_text(html);
        assert!(!out.contains("nav stuff"));
        assert!(out.contains("real content"));
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

    #[test]
    fn embedded_json_prose_recovered() {
        // ReadMe-style SPA: empty body, real content embedded as JSON.
        let html = r#"<!DOCTYPE html><html><head><title>x</title></head><body>
            <div id="root"></div>
            <script type="application/json">{"page":{
                "title":"GLM-5.2 Reference",
                "description":"GLM-5.2 is the latest flagship model from Z.ai.",
                "params":[{"name":"temperature","note":"Controls randomness. Use a lower value for more deterministic output."}],
                "examples":[{"code":"curl -X POST https://api.example.com/v1/chat"}]
            }}</script>
        </body></html>"#;
        let out = extract_embedded_json_prose(html);
        assert!(out.contains("GLM-5.2 is the latest flagship model from Z.ai."));
        assert!(out.contains("Controls randomness."));
        // Keys, short values, and code snippets must be excluded.
        assert!(!out.contains("temperature"));
        assert!(!out.contains("curl -X POST"));
        assert!(!out.is_empty());
    }

    #[test]
    fn embedded_json_ignores_non_json_script() {
        // A plain JS script (not JSON) yields nothing.
        let html = "<body><div id=root></div><script>var x = 1; window.y = 'hi';</script></body>";
        assert!(extract_embedded_json_prose(html).is_empty());
    }

    #[test]
    fn strip_doctoc_toc_removes_generated_toc() {
        let md = "<!-- START doctoc generated TOC please keep comment here -->\n\
                  <!-- DON'T EDIT THIS SECTION -->\n\
                  - [Unreleased](#unreleased)\n  - [Added](#added)\n- [0.72.0](#0720)\n\
                  <!-- END doctoc generated TOC please keep comment here -->\n\
                  \n--------------------------------------------------------------------------------\n\
                  # Unreleased\n## Added\n- Did a thing.\n";
        let out = strip_doctoc_toc(md);
        assert!(!out.contains("START doctoc"));
        assert!(!out.contains("END doctoc"));
        assert!(!out.contains("[Unreleased](#unreleased)"));
        assert!(out.contains("# Unreleased"));
        assert!(out.contains("Did a thing."));
    }

    #[test]
    fn strip_doctoc_toc_noop_without_markers() {
        let md = "# Title\n\nSome real content with no TOC.\n";
        assert_eq!(strip_doctoc_toc(md), md);
    }
}
