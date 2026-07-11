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
/// HTTP headers stripped. Supports both HTTP and HTTPS and transparently
/// follows up to 8 redirects (301/302/303/307/308), which many doc sites use
/// for version aliases, http->https upgrades, and trailing-slash normalisation.
/// The max_bytes limit applies to the body only (headers excluded).
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

    const MAX_REDIRECTS: usize = 8;
    let mut current = url.to_string();
    for _ in 0..MAX_REDIRECTS {
        let buffer = http_get_buffer(&current, timeout_secs, max_bytes, extra_headers)?;
        if let Some(code) = parse_status_code(&buffer)
            && (300..=399).contains(&code)
            && let Some(loc) = find_header_value(&buffer, "location")
        {
            current = resolve_redirect(&current, &loc);
            continue;
        }
        return Ok(extract_body_capped(&buffer, max_bytes));
    }
    Err("too many redirects".to_string())
}

/// Perform a single HTTP(S) GET and return the raw response buffer (headers
/// and body). Does not follow redirects on its own; `native_get` drives the
/// redirect loop.
fn http_get_buffer(
    url: &str,
    timeout_secs: u64,
    max_bytes: usize,
    extra_headers: Option<&[(&str, &str)]>,
) -> Result<Vec<u8>, String> {
    let (host, path, port, use_tls) = parse_url(url).map_err(|e| e.to_string())?;
    let addr = format!("{}:{}", host, port);
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

    Ok(buffer)
}

/// Strip HTTP response headers and return the body, capped at `max_bytes`.
/// Handles both `\r\n\r\n` and `\n\n` header/body separators and chunked
/// transfer encoding.
fn extract_body_capped(buffer: &[u8], max_bytes: usize) -> Vec<u8> {
    let (body_start, is_chunked) = {
        let header_str = String::from_utf8_lossy(buffer);
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

    let end = body.len().min(max_bytes);
    body[..end].to_vec()
}

/// Parse the HTTP status code from a response buffer (e.g. `HTTP/1.1 302`).
fn parse_status_code(buffer: &[u8]) -> Option<u16> {
    let text = String::from_utf8_lossy(buffer);
    let first = text.lines().next()?;
    let mut parts = first.split_whitespace();
    parts.next(); // "HTTP/1.1"
    parts.next().and_then(|c| c.parse::<u16>().ok())
}

/// Return the value of a response header (case-insensitive name), searching
/// only within the header section (before the blank line).
fn find_header_value(buffer: &[u8], name: &str) -> Option<String> {
    let text = String::from_utf8_lossy(buffer);
    let header_end = text.find("\r\n\r\n").unwrap_or_else(|| text.len());
    let headers = &text[..header_end];
    for line in headers.lines() {
        if let Some((k, v)) = line.split_once(':')
            && k.trim().eq_ignore_ascii_case(name)
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

/// Resolve a `Location` header against the request URL, supporting absolute
/// (`https://...`), root-relative (`/path`), and path-relative (`page.html`)
/// targets.
fn resolve_redirect(base: &str, loc: &str) -> String {
    if loc.starts_with("http://") || loc.starts_with("https://") {
        return loc.to_string();
    }
    let (host, path, port, use_tls) = match parse_url(base) {
        Ok(p) => p,
        Err(_) => return loc.to_string(),
    };
    let scheme = if use_tls { "https" } else { "http" };
    let authority = if (use_tls && port == 443) || (!use_tls && port == 80) {
        host.to_string()
    } else {
        format!("{}:{}", host, port)
    };
    if loc.starts_with('/') {
        format!("{}://{}{}", scheme, authority, loc)
    } else {
        let dir = match path.rfind('/') {
            Some(i) if i > 0 => &path[..i],
            _ => "",
        };
        format!("{}://{}{}/{}", scheme, authority, dir, loc)
    }
}

/// Minimal RFC 6455 WebSocket client (client frames masked, server frames
/// unmasked). Just enough to speak the Chrome DevTools Protocol over
/// `ws://localhost` without pulling in a WebSocket dependency.
struct WsConn {
    stream: std::net::TcpStream,
    buf: Vec<u8>,
    frag: Vec<u8>,
}

fn ws_base64(input: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(A[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(A[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn ws_nonce() -> String {
    let mut seed: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x1234_5678_9abc_def0);
    let mut bytes = [0u8; 16];
    for b in &mut bytes {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *b = (seed >> 33) as u8;
    }
    ws_base64(&bytes)
}

fn ws_connect(url: &str) -> Option<WsConn> {
    let u = url.strip_prefix("ws://")?;
    let (hostport, path) = u.split_once('/')?;
    let (host, port) = hostport.split_once(':').unwrap_or((hostport, "80"));
    let port: u16 = port.parse().ok()?;
    let mut stream = std::net::TcpStream::connect((host, port)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let key = ws_nonce();
    let req = format!(
        "GET /{path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;
    stream.flush().ok()?;
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    loop {
        let n = stream.read(&mut b).ok()?;
        if n == 0 {
            return None;
        }
        head.push(b[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
        if head.len() > 4096 {
            return None;
        }
    }
    let head_str = String::from_utf8_lossy(&head);
    if !head_str.contains(" 101 ") {
        return None;
    }
    Some(WsConn {
        stream,
        buf: Vec::new(),
        frag: Vec::new(),
    })
}

impl WsConn {
    fn send_text(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut frame = Vec::with_capacity(len + 10);
        frame.push(0x81);
        if len < 126 {
            frame.push(0x80 | (len as u8));
        } else if len < 65536 {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        let mask = [0x12u8, 0x34, 0x56, 0x78];
        frame.extend_from_slice(&mask);
        let mut masked = bytes.to_vec();
        for (i, b) in masked.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
        frame.extend_from_slice(&masked);
        let _ = self.stream.write_all(&frame);
        let _ = self.stream.flush();
    }

    fn send_pong(&mut self) {
        let frame = [0x8A, 0x00];
        let _ = self.stream.write_all(&frame);
        let _ = self.stream.flush();
    }

    /// Read the next complete text message. `Ok(None)` on EOF, `Err(())` if the
    /// socket would block (caller should retry after a short pause).
    fn recv_text(&mut self) -> Result<Option<String>, ()> {
        loop {
            if let Some(msg) = self.try_parse() {
                return Ok(Some(msg));
            }
            let mut tmp = [0u8; 8192];
            match self.stream.read(&mut tmp) {
                Ok(0) => return Ok(None),
                Ok(n) => self.buf.extend_from_slice(&tmp[..n]),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => return Err(()),
                Err(_) => return Ok(None),
            }
        }
    }

    fn try_parse(&mut self) -> Option<String> {
        if self.buf.len() < 2 {
            return None;
        }
        let b0 = self.buf[0];
        let b1 = self.buf[1];
        let opcode = b0 & 0x0f;
        let masked = (b1 & 0x80) != 0;
        let mut len = (b1 & 0x7f) as usize;
        let mut pos = 2;
        if len == 126 {
            if self.buf.len() < pos + 2 {
                return None;
            }
            len = u16::from_be_bytes([self.buf[pos], self.buf[pos + 1]]) as usize;
            pos += 2;
        } else if len == 127 {
            if self.buf.len() < pos + 8 {
                return None;
            }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&self.buf[pos..pos + 8]);
            len = u64::from_be_bytes(arr) as usize;
            pos += 8;
        }
        let mask_key = if masked {
            if self.buf.len() < pos + 4 {
                return None;
            }
            let k = [
                self.buf[pos],
                self.buf[pos + 1],
                self.buf[pos + 2],
                self.buf[pos + 3],
            ];
            pos += 4;
            Some(k)
        } else {
            None
        };
        if self.buf.len() < pos + len {
            return None;
        }
        let mut payload = self.buf[pos..pos + len].to_vec();
        if let Some(k) = mask_key {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= k[i % 4];
            }
        }
        self.buf.drain(..pos + len);
        match opcode {
            0x8 => None,
            0x9 => {
                self.send_pong();
                self.try_parse()
            }
            0x0..=0x2 => {
                if (b0 & 0x80) != 0 {
                    if opcode == 0x0 {
                        self.frag.extend_from_slice(&payload);
                        let m = String::from_utf8_lossy(&self.frag).into_owned();
                        self.frag.clear();
                        Some(m)
                    } else {
                        Some(String::from_utf8_lossy(&payload).into_owned())
                    }
                } else {
                    self.frag.extend_from_slice(&payload);
                    self.try_parse()
                }
            }
            _ => self.try_parse(),
        }
    }
}

/// Send one CDP command over `ws` and wait (up to 15s) for its response.
fn ws_cdp_once(
    ws: &mut WsConn,
    id: u64,
    method: &str,
    params: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let msg = serde_json::json!({
        "id": id,
        "method": method,
        "params": params.unwrap_or(serde_json::Value::Null)
    });
    ws.send_text(&msg.to_string());
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if Instant::now() >= deadline {
            return None;
        }
        match ws.recv_text() {
            Ok(Some(s)) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s)
                    && v.get("id").and_then(|i| i.as_u64()) == Some(id)
                {
                    return Some(v);
                }
            }
            Ok(None) => return None,
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Render a URL with a headless Chrome/Chromium instance and return the
/// serialized DOM after JavaScript has run. Used by `fetch_url` as a fallback
/// for JavaScript-rendered (SPA) pages that a plain HTTP GET cannot read.
///
/// `chrome` is the full path to the executable (from sysinfo detection). The
/// page is driven over Chrome's DevTools Protocol (CDP) via a WebSocket: we
/// wait for the load event and network idle (so client-fetched content such as
/// ReadMe docs has mounted), then retrieve the live outer HTML via
/// `DOM.getDocument` + `DOM.getOuterHTML`. The child is killed if it exceeds
/// `timeout_secs`.
pub fn render_via_chrome(
    url: &str,
    chrome: &str,
    timeout_secs: u64,
    max_bytes: usize,
) -> Option<String> {
    let ud = std::env::temp_dir().join(format!("autocode_chrome_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&ud);
    let ud_arg = format!("--user-data-dir={}", ud.to_string_lossy());

    let mut cmd = Command::new(chrome);
    cmd.arg("--headless=new")
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg("--disable-extensions")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg(ud_arg)
        .arg("--remote-debugging-port=0")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().ok()?;
    let mut child_stderr = child.stderr.take();

    // Chrome prints "DevTools listening on ws://..." to stderr once the
    // debugging endpoint is up; capture that WebSocket URL.
    let (tx_ws, rx_ws) = std::sync::mpsc::channel::<String>();
    let err_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 8192];
        if let Some(ref mut e) = child_stderr {
            while let Ok(n) = e.read(&mut tmp) {
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Ok(s) = std::str::from_utf8(&buf)
                    && let Some(pos) = s.find("DevTools listening on")
                    && let Some(u) = s[pos..].split_whitespace().find(|w| w.starts_with("ws://"))
                {
                    let _ = tx_ws.send(u.to_string());
                    break;
                }
            }
        }
    });

    let ws_url = match rx_ws.recv_timeout(Duration::from_secs(20)) {
        Ok(u) => u,
        Err(_) => {
            let _ = child.kill();
            return None;
        }
    };

    let mut ws = match ws_connect(&ws_url) {
        Some(w) => w,
        None => {
            let _ = child.kill();
            return None;
        }
    };

    // Find the page target spawned for `url` and attach a session to it.
    let targets = ws_cdp_once(&mut ws, 1, "Target.getTargets", None);
    let target_id = targets
        .as_ref()
        .and_then(|v| v.get("result"))
        .and_then(|r| r.get("targetInfos"))
        .and_then(|t| t.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|t| t.get("type").and_then(|x| x.as_str()) == Some("page"))
                .or_else(|| arr.first())
                .and_then(|t| t.get("targetId"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        });
    let target_id = match target_id {
        Some(t) => t,
        None => {
            let _ = child.kill();
            return None;
        }
    };
    let session = ws_cdp_once(
        &mut ws,
        2,
        "Target.attachToTarget",
        Some(serde_json::json!({ "targetId": target_id, "flatten": true })),
    );
    let session_id = session
        .as_ref()
        .and_then(|v| v.get("result"))
        .and_then(|r| r.get("sessionId"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let session_id = match session_id {
        Some(s) => s,
        None => {
            let _ = child.kill();
            return None;
        }
    };

    let mut next_id: u64 = 1000;
    let sid = session_id.clone();
    let mut cdp = |ws: &mut WsConn, method: &str, params: Option<serde_json::Value>| -> u64 {
        let id = next_id;
        next_id += 1;
        let msg = serde_json::json!({
            "id": id,
            "sessionId": sid,
            "method": method,
            "params": params.unwrap_or(serde_json::Value::Null)
        });
        ws.send_text(&msg.to_string());
        id
    };

    let _ = cdp(&mut ws, "Page.enable", None);
    let _ = cdp(&mut ws, "Network.enable", None);
    let _ = cdp(&mut ws, "DOM.enable", None);

    let overall = Instant::now() + Duration::from_secs(timeout_secs);
    let mut st = RenderState {
        loaded: false,
        pending: 0,
        saw_request: false,
        dom_id: None,
        outer_id: None,
        outer: None,
        dom_at: None,
    };

    // Drive the protocol until the page loads, the network goes idle, and the
    // rendered DOM has been captured (or we run out of time).
    while Instant::now() < overall {
        if st.outer.is_some() {
            break;
        }
        match ws.recv_text() {
            Ok(Some(s)) => {
                if handle_cdp(&s, &mut cdp, &mut ws, &mut st) {
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => {
                if Instant::now() >= overall {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
        // Once loaded with network idle, wait a short grace for client-rendered
        // content to mount, then ask for the document + outer HTML.
        if st.loaded && st.saw_request && st.pending <= 0 && st.dom_id.is_none() {
            match st.dom_at {
                None => st.dom_at = Some(Instant::now() + Duration::from_millis(800)),
                Some(t) if Instant::now() >= t => {
                    st.dom_id = Some(cdp(
                        &mut ws,
                        "DOM.getDocument",
                        Some(serde_json::json!({ "depth": -1, "pierce": true })),
                    ));
                }
                _ => {}
            }
        }
    }

    // Best-effort: if the DOM was never captured, request it directly and wait.
    if st.outer.is_none() && st.dom_id.is_none() {
        st.dom_id = Some(cdp(
            &mut ws,
            "DOM.getDocument",
            Some(serde_json::json!({ "depth": -1, "pierce": true })),
        ));
    }
    if st.outer.is_none() {
        let dl = Instant::now() + Duration::from_secs(20);
        while Instant::now() < dl {
            if st.outer.is_some() {
                break;
            }
            match ws.recv_text() {
                Ok(Some(s)) => {
                    if handle_cdp(&s, &mut cdp, &mut ws, &mut st) {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    if Instant::now() >= dl {
                        break;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }

    let _ = child.kill();
    let _ = err_reader.join();

    let dom = st.outer?;
    if dom.trim().is_empty() {
        return None;
    }
    // Allow some headroom over max_bytes so the cleaner can strip tags/whitespace
    // and still leave a meaningful chunk after the final cap in fetch_url.
    let cap = max_bytes.saturating_mul(4).max(65_536);
    Some(dom.chars().take(cap).collect())
}

/// Internal state for the CDP render loop in [`render_via_chrome`].
struct RenderState {
    loaded: bool,
    pending: i64,
    saw_request: bool,
    dom_id: Option<u64>,
    outer_id: Option<u64>,
    outer: Option<String>,
    dom_at: Option<Instant>,
}

/// Process one CDP message: update load/network state and, on the relevant
/// responses, chain `DOM.getDocument` → `DOM.getOuterHTML` to capture the
/// post-hydration DOM. Returns `true` once the outer HTML is captured.
fn handle_cdp<F: FnMut(&mut WsConn, &str, Option<serde_json::Value>) -> u64>(
    s: &str,
    send: &mut F,
    ws: &mut WsConn,
    st: &mut RenderState,
) -> bool {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        if let Some(m) = v.get("method").and_then(|m| m.as_str()) {
            match m {
                "Page.loadEventFired" => st.loaded = true,
                "Network.requestWillBeSent" => {
                    st.pending += 1;
                    st.saw_request = true;
                }
                "Network.loadingFinished" | "Network.loadingFailed" if st.pending > 0 => {
                    st.pending -= 1;
                }
                _ => {}
            }
        }
        if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
            if Some(id) == st.dom_id {
                if let Some(root) = v
                    .get("result")
                    .and_then(|r| r.get("root"))
                    .and_then(|r| r.get("nodeId"))
                    .and_then(|n| n.as_u64())
                {
                    st.outer_id = Some(send(
                        ws,
                        "DOM.getOuterHTML",
                        Some(serde_json::json!({ "nodeId": root })),
                    ));
                }
            } else if Some(id) == st.outer_id
                && let Some(oh) = v
                    .get("result")
                    .and_then(|r| r.get("outerHTML"))
                    .and_then(|o| o.as_str())
            {
                st.outer = Some(oh.to_string());
                return true;
            }
        }
    }
    false
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
    use super::{WsConn, native_get, ws_base64};
    use autocode_core::utils::extract::extract_ddg_results;

    /// Offline check of the hand-written WebSocket frame codec: a masked
    /// client frame written by one end must be decoded (unmasked) by the other.
    #[test]
    fn ws_frame_roundtrip() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::net::TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();
        let mut a = WsConn {
            stream: client,
            buf: Vec::new(),
            frag: Vec::new(),
        };
        let mut b = WsConn {
            stream: server,
            buf: Vec::new(),
            frag: Vec::new(),
        };
        // Exercise small, medium, and >126-byte (extended length) payloads.
        for msg in [
            "hi",
            "{\"id\":1,\"method\":\"Page.enable\",\"params\":null}",
            &"x".repeat(200),
        ] {
            a.send_text(msg);
            let got = b.recv_text().ok().flatten();
            assert_eq!(
                got.as_deref(),
                Some(msg),
                "roundtrip failed for len {}",
                msg.len()
            );
        }
    }

    #[test]
    fn ws_base64_known_vectors() {
        assert_eq!(ws_base64(b"Man"), "TWFu");
        assert_eq!(ws_base64(b"Ma"), "TWE=");
        assert_eq!(ws_base64(b"M"), "TQ==");
        assert_eq!(ws_base64(b"foobar"), "Zm9vYmFy");
    }

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

    /// Verify `native_get` follows the 302 redirect docs.rs uses for version
    /// aliases (e.g. `/bindgen/0.71/` -> `/bindgen/0.71.1/`) and returns the
    /// real page rather than an empty body.
    #[test]
    #[ignore]
    fn live_native_get_follows_redirect() {
        let url = "https://docs.rs/bindgen/0.71/bindgen/struct.Builder.html";
        let body = match native_get(url, 20, 65_536, None) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skipping (request failed: {}).", e);
                return;
            }
        };
        let text = String::from_utf8_lossy(&body);
        println!("==== docs.rs redirect follow ====");
        println!("body bytes: {}", body.len());
        println!(
            "redirect resolved (contains Builder struct): {}",
            text.contains("struct.Builder")
        );
        println!("contains '0.71.1': {}", text.contains("0.71.1"));
        println!("===============================");
        assert!(body.len() > 1000, "expected the redirected page body");
        assert!(text.contains("struct.Builder"));
    }
}
