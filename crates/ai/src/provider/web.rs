// web.rs -- Web scraping infrastructure: browser profiles, cookie jar,
//           web rate limiting, native_get, native_post.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::{
    io::{Read, Write},
    net::TcpStream,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rustls::pki_types::ServerName;

use super::http::{connect_tcp, decode_chunked, http_response_body, parse_url, tls_config};

/// A cookie value with a creation timestamp for TTL-based expiry.
struct CookieEntry {
    value: String,
    created: std::time::Instant,
}

const COOKIE_TTL_SECS: u64 = 3600; // 1 hour
const MAX_COOKIES: usize = 100;

/// Global cookie jar: hostname → cookie entry with TTL.
/// Persisted across calls so DDG doesn't treat each request as a new session.
static COOKIE_JAR: Mutex<Option<HashMap<String, CookieEntry>>> = Mutex::new(None);

/// Rotating user-agent strings that mimic real browsers across OS and version.
static PROFILE_NEXT: AtomicUsize = AtomicUsize::new(0);

struct BrowserProfile {
    user_agent: &'static str,
    sec_ch_ua: &'static str,
    sec_ch_ua_platform: &'static str,
    accept: &'static str,
    accept_language: &'static str,
}

const BROWSER_PROFILES: &[BrowserProfile] = &[
    // Chrome 125 Windows
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Google Chrome\";v=\"125\", \"Chromium\";v=\"125\", \"Not.A/Brand\";v=\"24\"",
        sec_ch_ua_platform: "\"Windows\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        accept_language: "en-US,en;q=0.9",
    },
    // Chrome 126 Windows
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Google Chrome\";v=\"126\", \"Chromium\";v=\"126\", \"Not.A/Brand\";v=\"24\"",
        sec_ch_ua_platform: "\"Windows\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        accept_language: "en-US,en;q=0.9",
    },
    // Chrome 125 macOS
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Google Chrome\";v=\"125\", \"Chromium\";v=\"125\", \"Not.A/Brand\";v=\"24\"",
        sec_ch_ua_platform: "\"macOS\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        accept_language: "en-US,en;q=0.9,fr;q=0.8",
    },
    // Safari 17.5 macOS
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
        sec_ch_ua: "\"Safari\";v=\"17.5\", \"AppleWebKit\";v=\"605.1.15\"",
        sec_ch_ua_platform: "\"macOS\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        accept_language: "en-US,en;q=0.9",
    },
    // Firefox 127 Windows
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:127.0) Gecko/20100101 Firefox/127.0",
        sec_ch_ua: "\"Firefox\";v=\"127\", \"Gecko\";v=\"127\"",
        sec_ch_ua_platform: "\"Windows\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
        accept_language: "en-US,en;q=0.9",
    },
    // Chrome 125 Linux
    BrowserProfile {
        user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Google Chrome\";v=\"125\", \"Chromium\";v=\"125\", \"Not.A/Brand\";v=\"24\"",
        sec_ch_ua_platform: "\"Linux\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        accept_language: "en-US,en;q=0.9",
    },
    // Chrome 124 Windows
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Google Chrome\";v=\"124\", \"Chromium\";v=\"124\", \"Not.A/Brand\";v=\"24\"",
        sec_ch_ua_platform: "\"Windows\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        accept_language: "en-GB,en;q=0.9,en-US;q=0.8",
    },
    // Chrome 126 macOS
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
        sec_ch_ua: "\"Google Chrome\";v=\"126\", \"Chromium\";v=\"126\", \"Not.A/Brand\";v=\"24\"",
        sec_ch_ua_platform: "\"macOS\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        accept_language: "en-US,en;q=0.9,de;q=0.8",
    },
    // Firefox 127 macOS
    BrowserProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:127.0) Gecko/20100101 Firefox/127.0",
        sec_ch_ua: "\"Firefox\";v=\"127\", \"Gecko\";v=\"127\"",
        sec_ch_ua_platform: "\"macOS\"",
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
        accept_language: "en-US,en;q=0.9",
    },
];

fn next_profile() -> &'static BrowserProfile {
    let i = PROFILE_NEXT.fetch_add(1, Ordering::Relaxed) % BROWSER_PROFILES.len();
    &BROWSER_PROFILES[i]
}

/// Global web request rate limit in milliseconds (0 = disabled).
static WEB_RATE_LIMIT_MS: AtomicU64 = AtomicU64::new(1500);

/// Timestamp of the last web request (any web_search or fetch_url).
static LAST_WEB_REQUEST: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// Set the minimum delay (ms) enforced between web requests. 0 disables.
pub fn set_web_rate_limit_ms(ms: u64) {
    WEB_RATE_LIMIT_MS.store(ms, Ordering::Relaxed);
}

pub(crate) fn enforce_web_rate_limit() {
    let rate_ms = WEB_RATE_LIMIT_MS.load(Ordering::Relaxed);
    if rate_ms == 0 {
        return;
    }
    let mut last = match LAST_WEB_REQUEST.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            LAST_WEB_REQUEST.clear_poison();
            poisoned.into_inner()
        }
    };
    let now = std::time::Instant::now();
    if let Some(prev) = *last {
        let elapsed = now.duration_since(prev);
        let elapsed_ms = elapsed.as_millis() as u64;
        if elapsed_ms < rate_ms {
            let sleep = Duration::from_millis(rate_ms - elapsed_ms);
            std::thread::sleep(sleep);
        }
    }
    *last = Some(std::time::Instant::now());
}

/// Returns the Cookie header value for a given host, or None if no cookie is
/// stored or the stored cookie has expired (TTL: 1 hour).
fn cookie_header(host: &str) -> Option<String> {
    let mut jar = match COOKIE_JAR.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            COOKIE_JAR.clear_poison();
            poisoned.into_inner()
        }
    };
    let map = jar.as_mut()?;
    let now = std::time::Instant::now();

    // Check entry and remove if expired.
    match map.get(host) {
        Some(entry) if now.duration_since(entry.created).as_secs() < COOKIE_TTL_SECS => {
            Some(format!("Cookie: {}\r\n", entry.value))
        }
        _ => {
            map.remove(host);
            None
        }
    }
}

/// Parse and store Set-Cookie headers from the raw HTTP response (including headers).
/// Enforces TTL (1 hour) and max size (100 entries), evicting oldest on overflow.
fn store_cookies(host: &str, buffer: &[u8]) {
    let header_end = buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(buffer.len());
    let header_section = &buffer[..header_end];
    let header_str = String::from_utf8_lossy(header_section);

    let mut new_cookies: Vec<String> = Vec::new();
    for line in header_str.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(val) = lower.strip_prefix("set-cookie:") {
            let cookie_val = val
                .trim()
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !cookie_val.is_empty() {
                new_cookies.push(cookie_val);
            }
        }
    }

    if new_cookies.is_empty() {
        return;
    }

    let mut jar = match COOKIE_JAR.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            COOKIE_JAR.clear_poison();
            poisoned.into_inner()
        }
    };
    let map = jar.get_or_insert_with(HashMap::new);
    let now = std::time::Instant::now();

    // Evict expired entries.
    map.retain(|_, entry| now.duration_since(entry.created).as_secs() < COOKIE_TTL_SECS);

    // Evict oldest entries if over the limit.
    while map.len() >= MAX_COOKIES {
        let oldest_host = map
            .iter()
            .min_by_key(|(_, e)| e.created)
            .map(|(k, _)| k.clone());
        if let Some(h) = oldest_host {
            map.remove(&h);
        } else {
            break;
        }
    }

    map.insert(
        host.to_string(),
        CookieEntry {
            value: new_cookies.join("; "),
            created: now,
        },
    );
}

/// Perform a native HTTP GET request, returning the response body with
/// HTTP headers stripped. Supports both HTTP and HTTPS. Does not follow
/// redirects. The max_bytes limit applies to the body only (headers excluded).
/// `extra_headers` (e.g. a custom `Accept`) is appended after the rotating
/// browser-profile headers.
pub fn native_get(
    url: &str,
    timeout_secs: u64,
    max_bytes: usize,
    extra_headers: Option<&[(&str, &str)]>,
) -> Result<Vec<u8>, String> {
    // Rate limit: enforce minimum delay between web requests.
    enforce_web_rate_limit();

    let _t0 = std::time::Instant::now();
    let (host, path, port, use_tls) = parse_url(url).map_err(|e| e.to_string())?;
    let addr = format!("{}:{}", host, port);
    // If we already have a cached TLS connection for this host, reuse it.
    // Otherwise create a fresh TCP connection.
    let stream = TcpStream::connect(&addr).map_err(|e| format!("connect: {}", e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(timeout_secs)))
        .map_err(|e| format!("set_read_timeout: {}", e))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(timeout_secs)))
        .map_err(|e| format!("set_write_timeout: {}", e))?;

    // Read all data into a scratch buffer (headers + body). We allocate
    // extra headroom past max_bytes so headers don't eat into the body limit.
    let scratch_max = max_bytes + 8192;
    let mut buffer = Vec::with_capacity(scratch_max.min(16384));

    fn read_all(
        stream: &mut dyn std::io::Read,
        buffer: &mut Vec<u8>,
        max_total: usize,
    ) -> Result<(), String> {
        let mut buf = [0u8; 8192];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let remaining = max_total.saturating_sub(buffer.len());
                    if remaining == 0 {
                        break;
                    }
                    let to_copy = n.min(remaining);
                    buffer.extend_from_slice(&buf[..to_copy]);
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    return Err("read: timed out".into());
                }
                Err(e) => return Err(format!("read: {}", e)),
            }
        }
        Ok(())
    }

    // Build a coherent browser-fingerprint header set from the rotating profile.
    let profile = next_profile();
    let cookie_line = cookie_header(&host);
    let cookie_str = cookie_line.as_deref().unwrap_or("");
    let extra = extra_headers
        .map(|hs| {
            hs.iter()
                .map(|(k, v)| format!("{}: {}\r\n", k, v))
                .collect::<String>()
        })
        .unwrap_or_default();

    let request = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         User-Agent: {ua}\r\n\
         Accept: {accept}\r\n\
         Accept-Language: {lang}\r\n\
         Accept-Encoding: identity\r\n\
         Upgrade-Insecure-Requests: 1\r\n\
         Sec-Ch-Ua: {sec_ua}\r\n\
         Sec-Ch-Ua-Mobile: ?0\r\n\
         Sec-Ch-Ua-Platform: {sec_plat}\r\n\
         Sec-Fetch-Dest: document\r\n\
         Sec-Fetch-Mode: navigate\r\n\
         Sec-Fetch-Site: none\r\n\
         Sec-Fetch-User: ?1\r\n\
         {cookie_str}\
         {extra}\
         Connection: close\r\n\
         \r\n",
        ua = profile.user_agent,
        accept = profile.accept,
        lang = profile.accept_language,
        sec_ua = profile.sec_ch_ua,
        sec_plat = profile.sec_ch_ua_platform,
    );

    if use_tls {
        let config = tls_config();
        let dns_name = rustls::pki_types::DnsName::try_from(host.clone())
            .map_err(|_| "invalid DNS name".to_string())?;
        let server_name = ServerName::DnsName(dns_name);
        let client = rustls::ClientConnection::new(config, server_name)
            .map_err(|e| format!("tls: {}", e))?;
        let mut tls_stream = rustls::StreamOwned::new(client, stream);
        tls_stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("write: {}", e))?;
        read_all(&mut tls_stream, &mut buffer, scratch_max)?;
    } else {
        let mut stream = stream;
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("write: {}", e))?;
        read_all(&mut stream, &mut buffer, scratch_max)?;
    }

    // Store any Set-Cookie from the response for subsequent requests
    store_cookies(&host, &buffer);

    // Strip HTTP response headers: find the blank line separating headers from body.
    // We look for \r\n\r\n (HTTP standard) with a fallback for servers that use \n\n.
    let (body_start, is_chunked) = {
        let header_str = String::from_utf8_lossy(&buffer);
        let is_chunked = header_str.contains("Transfer-Encoding: chunked")
            || header_str.contains("transfer-encoding: chunked");
        let start = buffer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|i| i + 4)
            .or_else(|| buffer.windows(2).position(|w| w == b"\n\n").map(|i| i + 2))
            .unwrap_or(0);
        (start, is_chunked)
    };

    let body = if body_start > 0 && body_start < buffer.len() {
        let raw = &buffer[body_start..];
        if is_chunked {
            decode_chunked(raw)
        } else {
            raw.to_vec()
        }
    } else {
        buffer.to_vec()
    };

    // Cap body to max_bytes
    let end = body.len().min(max_bytes);
    Ok(body[..end].to_vec())
}

/// Render a URL with a headless Chrome/Chromium instance and return the
/// serialized DOM after JavaScript has run. Used by `fetch_url` as a fallback
/// for JavaScript-rendered (SPA) pages that a plain HTTP GET cannot read.
///
/// `chrome` is the full path to the executable (from sysinfo detection). The
/// page is loaded with `--dump-dom`, which prints the post-hydration DOM to
/// stdout. A virtual-time budget lets async JS settle before the dump. The
/// child is killed if it exceeds `timeout_secs`.
pub fn render_via_chrome(
    url: &str,
    chrome: &str,
    timeout_secs: u64,
    max_bytes: usize,
) -> Option<String> {
    let mut cmd = Command::new(chrome);
    cmd.arg("--headless=old")
        .arg("--no-startup-window")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg("--disable-extensions")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-background-networking")
        .arg("--virtual-time-budget=8000")
        .arg("--dump-dom")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;

    let reader = thread::spawn(move || {
        let mut buf = Vec::with_capacity(64 * 1024);
        let mut tmp = [0u8; 8192];
        loop {
            match stdout.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(_) => break,
            }
        }
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    break;
                }
                thread::sleep(Duration::from_millis(150));
            }
            Err(_) => break,
        }
    }
    // Ensure the process is reaped even on early exit.
    let _ = child.kill();

    let raw = reader.join().ok()?;
    let dom = String::from_utf8_lossy(&raw);
    if dom.trim().is_empty() {
        return None;
    }
    // Allow some headroom over max_bytes so the cleaner can strip tags/whitespace
    // and still leave a meaningful chunk after the final cap in fetch_url.
    let cap = max_bytes.saturating_mul(4).max(65_536);
    Some(dom.chars().take(cap).collect())
}

/// Perform a native HTTP POST request, returning the response body with
/// HTTP headers stripped. Supports both HTTP and HTTPS.
/// `extra_headers` allows additional headers like `x-api-key` for Anthropic.
/// Reads at most `max_bytes` of body data (plus 8 KB headroom for headers).
pub fn native_post(
    url: &str,
    api_key: &str,
    body: &str,
    timeout_secs: u64,
    max_bytes: usize,
    extra_headers: &[(&str, &str)],
) -> Result<String, String> {
    let _t0 = std::time::Instant::now();
    let (host, path, port, use_tls) = parse_url(url).map_err(|e| e.to_string())?;
    let stream = connect_tcp(&host, port, timeout_secs).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(timeout_secs)))
        .map_err(|e| format!("set_read_timeout: {}", e))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(timeout_secs)))
        .map_err(|e| format!("set_write_timeout: {}", e))?;

    // Build request headers
    let mut header_str = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {len}\r\n",
        path = path,
        host = host,
        len = body.len()
    );

    // Track whether an auth header was already set via extra_headers
    let has_bearer = extra_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("Authorization"));
    let has_xapikey = extra_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("x-api-key"));

    if !has_bearer && !has_xapikey {
        header_str.push_str(&format!("Authorization: Bearer {}\r\n", api_key));
    }
    for (key, value) in extra_headers {
        header_str.push_str(&format!("{}: {}\r\n", key, value));
    }
    header_str.push_str("Connection: close\r\n\r\n");
    header_str.push_str(body);

    let request_bytes = header_str.as_bytes();
    let max_total = max_bytes + 8192;
    let mut buffer = Vec::with_capacity(8192.min(max_total));

    let read_result: Result<(), String> = (|| {
        if use_tls {
            let config = tls_config();
            let dns_name = rustls::pki_types::DnsName::try_from(host.clone())
                .map_err(|_| "invalid DNS name".to_string())?;
            let server_name = ServerName::DnsName(dns_name);
            let client = rustls::ClientConnection::new(config, server_name)
                .map_err(|e| format!("tls: {}", e))?;
            let mut tls_stream = rustls::StreamOwned::new(client, stream);
            tls_stream
                .write_all(request_bytes)
                .map_err(|e| format!("write: {}", e))?;

            let mut buf = [0u8; 8192];
            loop {
                let remaining = max_total.saturating_sub(buffer.len());
                if remaining == 0 {
                    break;
                }
                match tls_stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let to_copy = n.min(remaining);
                        buffer.extend_from_slice(&buf[..to_copy]);
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        return Err("read: timed out".to_string());
                    }
                    Err(e) => return Err(format!("read: {}", e)),
                }
            }
        } else {
            let mut plain = stream;
            plain
                .write_all(request_bytes)
                .map_err(|e| format!("write: {}", e))?;

            let mut buf = [0u8; 8192];
            loop {
                let remaining = max_total.saturating_sub(buffer.len());
                if remaining == 0 {
                    break;
                }
                match plain.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let to_copy = n.min(remaining);
                        buffer.extend_from_slice(&buf[..to_copy]);
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        return Err("read: timed out".to_string());
                    }
                    Err(e) => return Err(format!("read: {}", e)),
                }
            }
        }
        Ok(())
    })();

    read_result?;

    let body_bytes = http_response_body(&buffer);
    Ok(String::from_utf8_lossy(&body_bytes).to_string())
}

#[cfg(test)]
mod tests {
    use super::native_get;
    use autocode_core::utils::extract::extract_ddg_results;

    /// Live end-to-end check against DuckDuckGo's HTML endpoint.
    ///
    /// This performs a real network request, so it is marked `#[ignore]` to
    /// keep `cargo test` deterministic/offline-friendly. Run it explicitly with:
    ///
    ///   cargo test -p autocode-ai --lib -- --ignored live_ddg_search
    #[test]
    #[ignore]
    fn live_ddg_search_nvidia_nim_glm() {
        let query = "Nvidia NIM GLM 5.2";
        let encoded: String = query
            .chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                ' ' => "+".to_string(),
                c => format!("%{:02X}", c as u32),
            })
            .collect();
        let url = format!("https://html.duckduckgo.com/html/?q={}", encoded);

        let data = match native_get(&url, 15, 512_000, None) {
            Ok(d) => d,
            Err(e) => {
                // No network / blocked: skip rather than fail the suite.
                eprintln!("live_ddg_search: skipping (request failed: {}).", e);
                return;
            }
        };

        let html = String::from_utf8_lossy(&data);
        let results = extract_ddg_results(&html, 5);

        println!("==== DDG search results for \"{}\" ====", query);
        println!("{}", results);
        println!("===========================================");

        // The parser should have extracted at least one real http(s) link.
        assert!(
            results.contains("http://") || results.contains("https://"),
            "expected at least one search result URL in the parsed output"
        );
        assert!(
            results.starts_with("Search results"),
            "expected the 'Search results (N):' header"
        );

        // Print the first result line for quick eyeballing.
        let first = results
            .lines()
            .find(|l| l.trim_start().starts_with(|c: char| c.is_ascii_digit()))
            .unwrap_or("(none)");
        println!("FIRST RESULT: {}", first);
    }
}
