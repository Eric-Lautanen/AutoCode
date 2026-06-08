// provider.rs -- HTTP API client for AI providers.
// Uses only std::net + manual HTTP/HTTPS via a thin blocking wrapper.
// To avoid a heavy async runtime we spawn threads and use channels.


use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::{
        Arc, OnceLock,
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};

/// Global cookie jar: hostname → "NAME=VALUE" cookie string.
/// Persisted across calls so DDG doesn't treat each request as a new session.
static COOKIE_JAR: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

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

fn enforce_web_rate_limit() {
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

/// Returns the Cookie header value for a given host, or None if no cookie is stored.
fn cookie_header(host: &str) -> Option<String> {
    let jar = match COOKIE_JAR.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            COOKIE_JAR.clear_poison();
            poisoned.into_inner()
        }
    };
    let map = jar.as_ref()?;
    let cookie = map.get(host)?;
    Some(format!("Cookie: {}\r\n", cookie))
}

/// Parse and store Set-Cookie headers from the raw HTTP response (including headers).
/// Returns the updated header_str with any trailing whitespace cleaned.
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
            // Extract NAME=VALUE before the first semicolon or end
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
    map.insert(host.to_string(), new_cookies.join("; "));
}

use std::io;

/// Std-only HTTP chunked-transfer decoder implementing `std::io::Read`.
/// Wraps any `Read` and yields the decoded body bytes on-the-fly.
struct ChunkedReader<R: Read> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    ended: bool,
}

impl<R: Read> ChunkedReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Vec::new(),
            pos: 0,
            ended: false,
        }
    }
}

impl<R: Read> Read for ChunkedReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.ended {
            return Ok(0);
        }
        if self.pos < self.buf.len() {
            let n = (self.buf.len() - self.pos).min(out.len());
            out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        self.buf.clear();
        self.pos = 0;

        // Read chunk size line until \n
        let mut size_line = Vec::new();
        loop {
            let mut b = [0u8; 1];
            match self.inner.read_exact(&mut b) {
                Ok(()) => {
                    if b[0] == b'\n' {
                        break;
                    }
                    if b[0] != b'\r' {
                        size_line.push(b[0]);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    self.ended = true;
                    return Ok(0);
                }
                Err(e) => return Err(e),
            }
        }
        let size_str = std::str::from_utf8(&size_line)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "chunk size"))?;
        let size = usize::from_str_radix(size_str.trim(), 16)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "chunk size"))?;
        if size == 0 {
            let mut trailing = [0u8; 2];
            let _ = self.inner.read_exact(&mut trailing);
            self.ended = true;
            return Ok(0);
        }
        self.buf.resize(size, 0);
        self.inner.read_exact(&mut self.buf)?;
        self.pos = 0;
        let mut trailing = [0u8; 2];
        self.inner.read_exact(&mut trailing)?;

        let n = size.min(out.len());
        out[..n].copy_from_slice(&self.buf[..n]);
        self.pos = n;
        Ok(n)
    }
}

use rustls::pki_types::ServerName;

use autocode_core::state::{ApiProvider, ChatMessage};

// -- Request / Response types --------------------------------------------------

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<ApiMessage>,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
    pub tools: bool,
    pub tool_choice: ToolChoice,
    pub parallel_tool_calls: bool,
    pub request_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    pub thinking_mode: bool,
    pub reasoning_effort: String,
    pub thinking_api: autocode_core::state::ThinkingApi,
}

#[derive(Debug, Clone, Default)]
pub enum ToolChoice {
    #[default]
    Auto,
}

impl ToolChoice {
    fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Auto => serde_json::Value::String("auto".into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<serde_json::Value>,
    pub cache_control: bool,
    pub reasoning_content: Option<String>,
}

impl ApiMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_call_id: None,
            tool_calls: None,
            cache_control: false,
            reasoning_content: None,
        }
    }
}

impl From<&ChatMessage> for ApiMessage {
    fn from(m: &ChatMessage) -> Self {
        Self {
            role: m.role.label().to_string(),
            content: m.content.clone(),
            tool_call_id: m.tool_call_id.clone(),
            tool_calls: m.tool_calls.clone(),
            cache_control: false,
            reasoning_content: m.reasoning_content.clone(),
        }
    }
}

/// A tool call requested by the model.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug)]
pub enum ProviderEvent {
    Delta(String),
    /// Internal model reasoning (e.g. extended thinking). Stored separately
    /// so the UI can display it in a collapsible section and it doesn't pollute
    /// the main response text or consume context budget on subsequent turns.
    Reasoning(String),
    ToolCall(ToolCall),
    Done {
        prompt_tokens: usize,
        completion_tokens: usize,
    },
    Error(String),
}

// -- Tool definitions (sent to the API) ---------------------------------------

pub fn tool_definitions() -> serde_json::Value {
    let grep_note = autocode_core::sysinfo::grep_note();
    let grep_desc = if grep_note.is_empty() {
        "Search code. Returns file:line matches. Literal by default; use ^prefix or suffix$ for regex. Glob filter, .gitignore respect.".to_string()
    } else {
        format!("Search code. Returns file:line matches. Literal by default; use ^prefix or suffix$ for regex. Glob filter, .gitignore respect. [!] {}", grep_note)
    };

    let shell_note = autocode_core::sysinfo::shell_tools_note();
    let shell_desc = format!(
        "Run shell command. Use ONLY for: builds, tests, git, cargo/npm, listing dirs. NEVER for file I/O or code search. {}",
        shell_note
    );

    serde_json::json!([
        {"type":"function","function":{"name":"run_shell","strict":true,"description":shell_desc,"parameters":{"type":"object","properties":{"command":{"type":"string","description":"Shell command."},"cwd":{"type":"string","description":"Working dir (default: project root)."},"timeout_secs":{"type":"integer","description":"Timeout secs (default 120, max 600)."}},"required":["command"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"read_file","strict":true,"description":"Read a file. Numbered lines, line/byte totals. Use offset+limit for large files. For multi-file use read_files.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path."},"offset":{"type":"integer","description":"Start line (1-based, default 1)."},"limit":{"type":"integer","description":"Max lines (default 2000)."}},"required":["path"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"read_files","strict":true,"description":"Read multiple files at once (max 10). Use instead of repeated read_file.","parameters":{"type":"object","properties":{"paths":{"type":"array","items":{"type":"string"},"description":"File paths (max 10)."}},"required":["paths"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"read_entire_file","strict":true,"description":"Read entire file without truncation. Use sparingly -- only when patch_file fails or you need absolute certainty about content.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path."},"entire":{"type":"boolean","description":"Must be true to use this tool."}},"required":["path","entire"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"write_file","strict":true,"description":"Write/overwrite a file. Creates parent dirs. For small edits prefer patch_file.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File path."},"content":{"type":"string","description":"Full file content."}},"required":["path","content"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"list_dir","strict":true,"description":"List directory contents. Trailing / for dirs. Respects .gitignore.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"Dir path (default: project root)."}},"required":["path"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"delete_file","strict":true,"description":"Delete a file or empty directory. Irreversible.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"Path to delete."}},"required":["path"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"rename_file","strict":true,"description":"Move/rename file or directory. Creates dest parent dirs.","parameters":{"type":"object","properties":{"from":{"type":"string","description":"Source path. Must exist."},"to":{"type":"string","description":"Destination path."}},"required":["from","to"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"create_dir","strict":true,"description":"Create directory tree (mkdir -p). No-op if exists.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"Directory to create."}},"required":["path"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"grep","strict":true,"description":grep_desc,"parameters":{"type":"object","properties":{"pattern":{"type":"string","description":"Search pattern (literal by default; use ^ or $ for regex)."},"path":{"type":"string","description":"Dir/file to search (default: project root)."},"file_glob":{"type":"string","description":"Glob filter e.g. '*.rs'."},"case_sensitive":{"type":"boolean","description":"Case sensitive? (default true)."},"max_results":{"type":"integer","description":"Max matches (default 50, max 200)."}},"required":["pattern"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"patch_file","strict":true,"description":"Surgical find-and-replace edit. Fuzzy-matches similar lines, handles CRLF/tab differences. Fails on ambiguous match. For multi-line edits prefer patch_lines.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File to patch."},"old_text":{"type":"string","description":"Text to replace (copy exact lines from read_file, numbers auto-stripped)."},"new_text":{"type":"string","description":"Replacement text. Empty to delete."},"replace_all":{"type":"boolean","description":"Replace all occurrences (default: first only)."}},"required":["path","old_text","new_text"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"patch_lines","strict":true,"description":"Replace a range of lines by line number. Use read_file first to see numbered lines. Faster and more reliable than patch_file for multi-line edits.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"File to patch."},"start_line":{"type":"integer","description":"First line to replace (1-based, inclusive)."},"end_line":{"type":"integer","description":"Last line to replace (1-based, inclusive)."},"new_text":{"type":"string","description":"Replacement text."}},"required":["path","start_line","end_line","new_text"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"web_search","strict":true,"description":"Search the web. Returns summary text + URLs. Use fetch_url to read pages.","parameters":{"type":"object","properties":{"query":{"type":"string","description":"Search query. Be specific."},"num_results":{"type":"integer","description":"Results to return (1-10, default 5)."}},"required":["query"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"fetch_url","strict":true,"description":"Fetch URL text content. HTML auto-stripped.","parameters":{"type":"object","properties":{"url":{"type":"string","description":"Full URL."},"max_bytes":{"type":"integer","description":"Max bytes (default 32768, max 131072)."}},"required":["url"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"todo_list","strict":true,"description":"Track multi-step tasks. Send full list on every update.","parameters":{"type":"object","properties":{"title":{"type":"string","description":"Short title (max 35 chars)."},"items":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string","description":"Stable id e.g. '1'."},"content":{"type":"string","description":"Task description."},"status":{"type":"string","enum":["pending","in_progress","completed","cancelled"],"description":"Status."},"priority":{"type":"string","enum":["high","medium","low"],"description":"Priority (default medium)."}},"required":["id","content","status"],"additionalProperties":false},"description":"All items."}},"required":["title","items"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"glob","strict":true,"description":"Find files by glob pattern. Returns sorted relative paths.","parameters":{"type":"object","properties":{"pattern":{"type":"string","description":"Glob pattern (*, **, ?). e.g. '**/*.rs'."},"path":{"type":"string","description":"Search dir (default: project root)."}},"required":["pattern"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"handoff","strict":true,"description":"End this session, resume in a fresh one. Save RESUME.md first.","parameters":{"type":"object","properties":{"reason":{"type":"string","description":"Why handoff is needed (e.g. 'context nearing limit')."}},"required":["reason"],"additionalProperties":false}}},
        {"type":"function","function":{"name":"name_session","strict":true,"description":"Set a descriptive label for this session. Call first in every session.","parameters":{"type":"object","properties":{"name":{"type":"string","description":"Short name e.g. 'fixing_build'."}},"required":["name"],"additionalProperties":false}}},
    ])
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let root_store =
                rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            Arc::new(
                rustls::ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth(),
            )
        })
        .clone()
}

// -- Client --------------------------------------------------------------------

pub struct ProviderClient;

impl ProviderClient {
    pub fn complete(provider: ApiProvider, request: CompletionRequest) -> Receiver<ProviderEvent> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_request_once(provider, request, tx);
            }));
            });
        rx
    }
}

// -- Request wrapper (single-shot, retry is handled by chat.rs outer layer) ------

fn run_request_once(provider: ApiProvider, request: CompletionRequest, tx: Sender<ProviderEvent>) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_request(provider, request, tx.clone())
    }));
    match result {
        Ok(Err(e)) => {
            let _ = tx.send(ProviderEvent::Error(e.to_string()));
        }
        Err(panic_info) => {
            let msg = format!(
                "Internal error (panic): {}",
                autocode_core::helpers::panic_msg(&panic_info)
            );
            let _ = tx.send(ProviderEvent::Error(msg));
        }
        _ => {
            }
    }
}

// -- HTTP request execution ----------------------------------------------------

struct HttpConn<'a> {
    host: &'a str,
    port: u16,
    path: &'a str,
}

fn run_request(
    provider: ApiProvider,
    req: CompletionRequest,
    tx: Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let body = build_request_body(&req, provider.kind.supports_cache_control())?;
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim_end_matches('/')
    );

    let (host, path, port, use_tls) = parse_url(&url)?;

    let timeouts = TimeoutConfig {
        request: req.request_timeout_secs,
        stream_idle: req.stream_idle_timeout_secs,
    };
    if use_tls {
        let conn = HttpConn {
            host: &host,
            port,
            path: &path,
        };
        send_https(
            &conn,
            provider.api_key.as_str(),
            &body,
            req.stream,
            &req.model,
            tx,
            &timeouts,
        )
    } else {
        let conn = HttpConn {
            host: &host,
            port,
            path: &path,
        };
        send_http(
            conn,
            provider.api_key.as_str(),
            &body,
            req.stream,
            &req.model,
            tx,
            &timeouts,
        )
    }
}

#[derive(serde::Serialize)]
struct ReqMsg<'a> {
    role: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
struct RequestBody<'a> {
    model: &'a str,
    messages: Vec<ReqMsg<'a>>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<serde_json::Value>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "parallel_tool_calls"
    )]
    parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

fn build_request_body(
    req: &CompletionRequest,
    supports_cache: bool,
) -> Result<String, serde_json::Error> {
    let messages: Vec<ReqMsg> = req
        .messages
        .iter()
        .map(|m| ReqMsg {
            role: &m.role,
            content: if m.tool_calls.is_some() { None } else { Some(&m.content) },
            tool_call_id: m.tool_call_id.as_deref(),
            tool_calls: m.tool_calls.as_ref(),
            reasoning_content: m.reasoning_content.as_deref(),
            cache_control: if m.cache_control && supports_cache {
                Some(serde_json::json!({"type": "ephemeral"}))
            } else {
                None
            },
        })
        .collect();

    let mut body = RequestBody {
        model: &req.model,
        messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        stream: req.stream,
        tools: None,
        tool_choice: None,
        stream_options: None,
        parallel_tool_calls: None,
        thinking: None,
        reasoning_effort: None,
    };

    match &req.thinking_api {
        autocode_core::state::ThinkingApi::DeepSeek if req.thinking_mode => {
            body.thinking = Some(serde_json::json!({"type": "enabled"}));
            body.reasoning_effort = Some(&req.reasoning_effort);
        }
        autocode_core::state::ThinkingApi::OpenAI if req.thinking_mode => {
            body.reasoning_effort = Some(&req.reasoning_effort);
        }
        _ => {}
    }

    if req.stream {
        body.stream_options = Some(serde_json::json!({"include_usage": true}));
    }

    if req.tools {
        body.tools = Some(tool_definitions());
        body.tool_choice = Some(req.tool_choice.to_json());
        body.parallel_tool_calls = Some(req.parallel_tool_calls);
    }

    serde_json::to_string(&body)
}

fn parse_url(
    url: &str,
) -> Result<(String, String, u16, bool), Box<dyn std::error::Error + Send + Sync>> {
    let use_tls = url.starts_with("https://");
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let (hostport, path) = stripped.split_once('/').unwrap_or((stripped, ""));
    let path = format!("/{}", path);
    let (host, port_str) = hostport
        .split_once(':')
        .unwrap_or((hostport, if use_tls { "443" } else { "80" }));
    let port: u16 = port_str.parse().unwrap_or(if use_tls { 443 } else { 80 });
    Ok((host.to_string(), path, port, use_tls))
}

fn connect_tcp(host: &str, port: u16, timeout_secs: u64) -> std::io::Result<TcpStream> {
    use std::net::ToSocketAddrs;
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let _start = std::time::Instant::now();
    let addrs = match (host, port).to_socket_addrs() {
        Ok(a) => a,
        Err(e) => {
            return Err(e);
        }
    };
    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(s) => {
                return Ok(s);
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    return Err(e);
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("could not connect to {}:{}", host, port),
        )
    }))
}

struct TimeoutConfig {
    request: u64,
    stream_idle: u64,
}

fn apply_timeouts(stream: &TcpStream, is_stream: bool, cfg: &TimeoutConfig) -> std::io::Result<()> {
    let read_timeout = if is_stream {
        cfg.stream_idle
    } else {
        cfg.request
    };
    stream.set_read_timeout(Some(Duration::from_secs(read_timeout)))?;
    stream.set_write_timeout(Some(Duration::from_secs(cfg.request)))
}

fn send_http(
    conn: HttpConn<'_>,
    api_key: &str,
    body: &str,
    stream: bool,
    model: &str,
    tx: Sender<ProviderEvent>,
    timeouts: &TimeoutConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _t0 = std::time::Instant::now();
    let mut stream_conn = connect_tcp(conn.host, conn.port, timeouts.request)?;
    apply_timeouts(&stream_conn, stream, timeouts)?;

    let request = build_http_request(conn.host, conn.path, api_key, body);
    let _t1 = std::time::Instant::now();
    stream_conn.write_all(request.as_bytes())?;
    stream_conn.flush()?;
    let mut reader = BufReader::with_capacity(8192, stream_conn);
    process_http_response(&mut reader, stream, model, tx)
}

fn send_https(
    conn: &HttpConn<'_>,
    api_key: &str,
    body: &str,
    is_stream: bool,
    model: &str,
    tx: Sender<ProviderEvent>,
    timeouts: &TimeoutConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _t0 = std::time::Instant::now();
    let stream = connect_tcp(conn.host, conn.port, timeouts.request)?;
    apply_timeouts(&stream, is_stream, timeouts)?;

    let config = tls_config();
    let dns_name = rustls::pki_types::DnsName::try_from(conn.host.to_string())
        .map_err(|_| "invalid DNS name")?;
    let server_name = ServerName::DnsName(dns_name);
    let client = rustls::ClientConnection::new(config, server_name)?;
    let _t1 = std::time::Instant::now();
    let mut tls_stream = rustls::StreamOwned::new(client, stream);
    let request = build_http_request(conn.host, conn.path, api_key, body);
    let _t2 = std::time::Instant::now();
    tls_stream.write_all(request.as_bytes())?;
    tls_stream.flush()?;
    let mut reader = BufReader::with_capacity(16384, tls_stream);
    process_http_response(&mut reader, is_stream, model, tx)
}

fn build_http_request(host: &str, path: &str, api_key: &str, body: &str) -> String {
    format!(
        "POST {path} HTTP/1.1\r\n\
        Host: {host}\r\n\
        Authorization: Bearer {api_key}\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {len}\r\n\
        Connection: close\r\n\
        \r\n\
        {body}",
        path = path,
        host = host,
        api_key = api_key,
        len = body.len(),
        body = body
    )
}

/// Redact common API key patterns from a string for safe debug logging.
fn sanitize_for_log(s: &str) -> String {
    let prefixes = ["sk-ant-", "sk-proj-", "sk-"];
    let mut result = s.to_string();
    for prefix in prefixes {
        loop {
            let Some(start) = result.find(prefix) else { break };
            let after = start + prefix.len();
            let end = after
                + result[after..]
                    .chars()
                    .position(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                    .unwrap_or(result.len() - after);
            if end - after >= 5 {
                let replacement = format!("{}...[REDACTED]", prefix.trim_end_matches('-'));
                result.replace_range(start..end, &replacement);
            } else {
                break;
            }
        }
    }
    result
}

fn process_http_response<R: BufRead>(
    reader: &mut R,
    stream: bool,
    model: &str,
    tx: Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut status_code: u16 = 200;
    let mut status_text = String::new();
    let mut retry_after_secs: Option<u64> = None;
    let mut is_chunked = false;
    for line in reader.by_ref().lines().map_while(Result::ok) {
        if line.starts_with("HTTP/") {
            let mut parts = line.splitn(3, ' ');
            let _ = parts.next();
            if let Some(code) = parts.next() {
                status_code = code.trim().parse().unwrap_or(200);
            }
            if let Some(reason) = parts.next() {
                status_text = reason.trim().to_string();
            }
        }
        let lower = line.to_ascii_lowercase();
        if let Some(val) = lower.strip_prefix("retry-after:") {
            let val = val.trim();
            retry_after_secs = val.parse::<u64>().ok();
        }
        if lower.contains("transfer-encoding:") && lower.contains("chunked") {
            is_chunked = true;
        }
        if line.trim().is_empty() {
            break;
        }
    }

    if status_code >= 400 {
        let mut raw_body = Vec::new();
        if let Err(e) = reader.read_to_end(&mut raw_body)
            && e.kind() != std::io::ErrorKind::UnexpectedEof
        {
            return Err(e.into());
        }
        let body_bytes = if is_chunked {
            decode_chunked(&raw_body)
        } else {
            raw_body
        };
        let body_str = String::from_utf8_lossy(&body_bytes).to_string();
        let api_msg = serde_json::from_str::<serde_json::Value>(&body_str)
            .ok()
            .and_then(|v| {
                v["error"]["message"]
                    .as_str()
                    .or_else(|| v["error"].as_str().filter(|s| !s.is_empty()))
                    .or_else(|| v["message"].as_str())
                    .or_else(|| v["detail"].as_str())
                    .or_else(|| v["error"]["code"].as_str())
                    .map(|s| s.to_string())
            });
        let body_retry_after_ms: Option<u64> = serde_json::from_str::<serde_json::Value>(&body_str)
            .ok()
            .and_then(|v| v["error"]["retry_after_ms"].as_u64());
        let mut msg = format!("[{}] {} ({})", model, status_text, status_code);
        if let Some(detail) = api_msg {
            msg.push_str(&format!(" — {}", sanitize_for_log(&detail)));
        } else if !body_str.is_empty() {
            let preview: String = body_str.chars().take(200).collect();
            msg.push_str(&format!(" — {}", sanitize_for_log(&preview)));
        }
        if let Some(secs) = retry_after_secs {
            msg.push_str(&format!(" (retry after {}s)", secs));
        } else if let Some(ms) = body_retry_after_ms {
            msg.push_str(&format!(" (retry after {}ms)", ms));
        }
        let _ = tx.send(ProviderEvent::Error(msg));
        return Ok(());
    }

    if stream {
        if is_chunked {
            let chunked = ChunkedReader::new(reader);
            let buf_reader = std::io::BufReader::new(chunked);
            parse_sse_stream_from_reader(buf_reader, &tx)?;
        } else {
            parse_sse_stream_from_reader(reader, &tx)?;
        }
    } else {
        let mut raw_body = Vec::new();
        if let Err(e) = reader.read_to_end(&mut raw_body)
            && e.kind() != std::io::ErrorKind::UnexpectedEof
        {
            return Err(e.into());
        }
        let body_bytes = if is_chunked {
            decode_chunked(&raw_body)
        } else {
            raw_body
        };
        let body_str = String::from_utf8_lossy(&body_bytes).to_string();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(body_str.trim()) {
            if let Some(text) = v["choices"][0]["message"]["content"].as_str() {
                let _ = tx.send(ProviderEvent::Delta(text.to_string()));
            }
            let p = v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as usize;
            let c = v["usage"]["completion_tokens"].as_u64().unwrap_or(0) as usize;
            let _ = tx.send(ProviderEvent::Done {
                prompt_tokens: p,
                completion_tokens: c,
            });
        }
    }
    Ok(())
}

fn parse_sse_stream_from_reader<R: BufRead>(
    reader: R,
    tx: &Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _sse_start = std::time::Instant::now();
    let mut lines = reader.lines();
    let mut tool_acc: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new();
    let mut content_count = 0u32;
    let mut reasoning_count = 0u32;
    let mut prompt_tokens = 0usize;
    let mut completion_tokens = 0usize;
    let mut saw_data_line = false;
    let mut saw_finish = false;
    let mut had_error = false;
    let mut raw_buf = String::new();
    let mut line_count = 0u32;
    let mut last_log = std::time::Instant::now();

    for line in &mut lines {
        let line = match line {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        line_count += 1;
        if line_count <= 10 {
            }
        if last_log.elapsed().as_secs() >= 30 {
            last_log = std::time::Instant::now();
        }
        if line.starts_with(':') {
            continue;
        }
        if !line.starts_with("data: ") {
            raw_buf.push_str(&line);
            raw_buf.push('\n');
            continue;
        }
        if !saw_data_line {
            }
        saw_data_line = true;
        let data = line["data: ".len()..].trim();
        if data == "[DONE]" {
            saw_finish = true;
            break;
        }
        let v = match serde_json::from_str::<serde_json::Value>(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let (Some(p), Some(c)) = (
            v["usage"]["prompt_tokens"].as_u64(),
            v["usage"]["completion_tokens"].as_u64(),
        ) {
            prompt_tokens = p as usize;
            completion_tokens = c as usize;
        }
        let delta = &v["choices"][0]["delta"];
        if let Some(text) = delta["content"].as_str().filter(|s| !s.is_empty()) {
            if content_count < 3 {
                }
            content_count += 1;
            if tx.send(ProviderEvent::Delta(text.to_string())).is_err() {
                return Err("channel closed".into());
            }
        }
        if let Some(reasoning) = delta["reasoning_content"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            if reasoning_count < 3 {
                }
            reasoning_count += 1;
            if tx
                .send(ProviderEvent::Reasoning(reasoning.to_string()))
                .is_err()
            {
                return Err("channel closed".into());
            }
        }
        if let Some(tc_arr) = delta["tool_calls"].as_array() {
            for tc in tc_arr {
                let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                let entry = tool_acc
                    .entry(idx)
                    .or_insert_with(|| (String::new(), String::new(), String::new()));
                if let Some(id) = tc["id"].as_str() {
                    entry.0 = id.to_string();
                }
                if let Some(name) = tc["function"]["name"].as_str() {
                    entry.1 = name.to_string();
                }
                if let Some(args) = tc["function"]["arguments"].as_str() {
                    entry.2.push_str(args);
                }
            }
        }
        if let Some(tc_arr) = v["choices"][0]["message"]["tool_calls"].as_array() {
            for (idx, tc) in tc_arr.iter().enumerate() {
                let id = tc["id"].as_str().unwrap_or("").to_string();
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args = tc["function"]["arguments"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if !name.is_empty() {
                    tool_acc.insert(idx, (id, name, args));
                }
            }
        }
        if let Some(reason) = v["choices"][0]["finish_reason"].as_str() {
            if reason == "tool_calls" {
                let mut indices: Vec<usize> = tool_acc.keys().cloned().collect();
                indices.sort();
                for idx in indices {
                    if let Some((id, name, args)) = tool_acc.remove(&idx)
                        && tx
                            .send(ProviderEvent::ToolCall(ToolCall {
                                id,
                                name,
                                arguments: args,
                            }))
                            .is_err()
                    {
                        return Err("channel closed".into());
                    }
                }
            }
            if reason == "stop" || reason == "tool_calls" || reason == "length" {
                saw_finish = true;
            }
            if reason == "content_filter" {
                let _ = tx.send(ProviderEvent::Error(
                    "Response filtered by provider content policy (content_filter)".to_string(),
                ));
                had_error = true;
                saw_finish = true;
            }
        }
    }

    if !tool_acc.is_empty() {
        let mut indices: Vec<usize> = tool_acc.keys().cloned().collect();
        indices.sort();
        for idx in indices {
            if let Some((id, name, args)) = tool_acc.remove(&idx)
                && tx
                    .send(ProviderEvent::ToolCall(ToolCall {
                        id,
                        name,
                        arguments: args,
                    }))
                    .is_err()
            {
                return Err("channel closed".into());
            }
        }
    }

    if !saw_data_line && !raw_buf.trim().is_empty() {
        let api_msg = serde_json::from_str::<serde_json::Value>(raw_buf.trim())
            .ok()
            .and_then(|v| {
                v["error"]["message"]
                    .as_str()
                    .or_else(|| v["message"].as_str())
                    .or_else(|| v["error"].as_str())
                    .map(|s| s.to_string())
            });
        if let Some(msg) = api_msg {
            let _ = tx.send(ProviderEvent::Error(msg));
            return Ok(());
        }
        let preview: String = raw_buf.trim().chars().take(300).collect();
        let _ = tx.send(ProviderEvent::Error(format!(
            "Unexpected response: {}",
            preview
        )));
        return Ok(());
    }

    if !saw_finish && saw_data_line {
        let _ = tx.send(ProviderEvent::Error(
            "Connection lost mid-stream — response may be truncated".to_string(),
        ));
        return Ok(());
    }

    if !had_error {
        let _ = tx.send(ProviderEvent::Done {
            prompt_tokens,
            completion_tokens,
        });
    }
    Ok(())
}

// -- Model list fetcher --------------------------------------------------------

pub fn fetch_models(provider: &ApiProvider) -> Vec<String> {
    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let (host, path, port, use_tls) = match parse_url(&url) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let result = (|| -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(15)))?;

        let mut buffer = Vec::new();
        let request = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host}\r\n\
             Authorization: Bearer {api_key}\r\n\
             Connection: close\r\n\
             \r\n",
            api_key = provider.api_key.as_str(),
        );

        if use_tls {
            let config = tls_config();
            let dns_name = rustls::pki_types::DnsName::try_from(host.clone())
                .map_err(|_| "invalid DNS name")?;
            let server_name = ServerName::DnsName(dns_name);
            let client = rustls::ClientConnection::new(config, server_name)?;
            let mut tls_stream = rustls::StreamOwned::new(client, stream);
            tls_stream.write_all(request.as_bytes())?;
            tls_stream.read_to_end(&mut buffer)?;
        } else {
            let mut stream = stream;
            stream.write_all(request.as_bytes())?;
            stream.read_to_end(&mut buffer)?;
        };

        // Strip headers and decode chunked encoding
        let (header_end, is_chunked) = {
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
        let body = if header_end > 0 && header_end < buffer.len() {
            if is_chunked {
                decode_chunked(&buffer[header_end..])
            } else {
                buffer[header_end..].to_vec()
            }
        } else {
            buffer
        };
        Ok(String::from_utf8_lossy(&body).to_string())
    })();

    match result {
        Ok(text) => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                v["data"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        }
        Err(_) => Vec::new(),
    }
}

/// Extract the HTTP response body from a raw HTTP response buffer,
/// stripping headers and decoding chunked transfer-encoding.
fn http_response_body(buffer: &[u8]) -> Vec<u8> {
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
    if body_start > 0 && body_start < buffer.len() {
        let raw = &buffer[body_start..];
        if is_chunked {
            decode_chunked(raw)
        } else {
            raw.to_vec()
        }
    } else {
        buffer.to_vec()
    }
}

/// Perform a native HTTP POST request, returning the response body with
/// HTTP headers stripped. Supports both HTTP and HTTPS.
/// `extra_headers` allows additional headers like `x-api-key` for Anthropic.
pub fn native_post(
    url: &str,
    api_key: &str,
    body: &str,
    timeout_secs: u64,
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
    let mut buffer = Vec::with_capacity(8192);

    let read_result: Result<(), String> = (|| {
        if use_tls {
            let config = tls_config();
            let dns_name = rustls::pki_types::DnsName::try_from(host.clone())
                .map_err(|_| "invalid DNS name".to_string())?;
            let server_name = ServerName::DnsName(dns_name);
            let client =
                rustls::ClientConnection::new(config, server_name).map_err(|e| format!("tls: {}", e))?;
            let mut tls_stream = rustls::StreamOwned::new(client, stream);
            tls_stream
                .write_all(request_bytes)
                .map_err(|e| format!("write: {}", e))?;

            let mut buf = [0u8; 8192];
            loop {
                match tls_stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => buffer.extend_from_slice(&buf[..n]),
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
                match plain.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => buffer.extend_from_slice(&buf[..n]),
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

    read_result.map_err(|e| {
        e
    })?;

    let body_bytes = http_response_body(&buffer);
    Ok(String::from_utf8_lossy(&body_bytes).to_string())
}

/// Call a provider's token counting API and return the input token count.
/// Supports:
/// - OpenAI: `POST /v1/responses/input_tokens` (Responses API format)
/// - Anthropic: `POST /v1/messages/count_tokens` (Messages API format)
/// - OpenRouter: `POST /api/v1/tokenize` (OpenAI-compatible format)
/// - NVIDIA NIM: `POST /v1/tokenize` (OpenAI-compatible format)
/// - Generic OpenAI-compatible: `POST /v1/tokenize` (OpenAI-compatible format)
///
/// `request_json` is the pre-serialized `{"messages": [...], "tools": [...]}` body
/// from the pre-flight check. The body is transformed as needed for each provider.
pub fn count_input_tokens(
    provider: &ApiProvider,
    request_json: &str,
    model: &str,
    timeout_secs: u64,
) -> Result<usize, String> {
    let _t0 = std::time::Instant::now();
    let url = provider
        .counting_endpoint_url()
        .ok_or_else(|| "no counting API for this provider".to_string())?;
    // Parse and transform the body for the provider's counting endpoint
    let mut base: serde_json::Value =
        serde_json::from_str(request_json).map_err(|e| format!("json parse: {}", e))?;
    base["model"] = serde_json::json!(model);

    let body_str = serde_json::to_string(&base).map_err(|e| format!("json stringify: {}", e))?;

    let mut extra_headers: Vec<(String, String)> = Vec::new();
    if let Some(prov) = autocode_core::state::provider_manifest(&provider.kind) {
        if prov.auth_type.as_deref() == Some("x-api-key") {
            extra_headers.push(("x-api-key".into(), provider.api_key.as_str().to_string()));
        }
        if let Some(ver) = &prov.anthropic_version {
            extra_headers.push(("anthropic-version".into(), ver.clone()));
        }
    }
    let extra_refs: Vec<(&str, &str)> = extra_headers
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let response = native_post(&url, provider.api_key.as_str(), &body_str, timeout_secs, &extra_refs)?;

    let v: serde_json::Value =
        serde_json::from_str(&response).map_err(|e| format!("json parse response: {}", e))?;

    // Try different response field names used by different providers.
    // total_tokens is last because it typically includes completion tokens (overestimate).
    v["input_tokens"]
        .as_u64()
        .or_else(|| v["token_count"].as_u64())
        .or_else(|| v["count"].as_u64())
        .or_else(|| v["usage"]["prompt_tokens"].as_u64())
        .or_else(|| v["total_tokens"].as_u64())
        .or_else(|| v["usage"]["total_tokens"].as_u64())
        .map(|n| {
            n as usize
        })
        .ok_or_else(|| {
            format!("no token count in response: {}", response.trim())
        })
}

/// Perform a native HTTP GET request, returning the response body with
/// HTTP headers stripped. Supports both HTTP and HTTPS. Does not follow
/// redirects. The max_bytes limit applies to the body only (headers excluded).
pub fn native_get(url: &str, timeout_secs: u64, max_bytes: usize) -> Result<Vec<u8>, String> {
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

/// Decode HTTP chunked transfer-encoding: strip hex size prefixes and
/// chunk separators, returning the reassembled body.
fn decode_chunked(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut pos = 0;
    while pos < raw.len() {
        // Find the end of the chunk size line
        let size_end = match raw[pos..].iter().position(|&b| b == b'\r' || b == b'\n') {
            Some(i) => pos + i,
            None => break,
        };
        let size_line = String::from_utf8_lossy(&raw[pos..size_end]);
        let chunk_size = match usize::from_str_radix(size_line.trim(), 16) {
            Ok(0) => break, // final chunk
            Ok(n) => n,
            Err(_) => break,
        };
        // Skip past the size line and any \r\n
        pos = size_end;
        while pos < raw.len() && (raw[pos] == b'\r' || raw[pos] == b'\n') {
            pos += 1;
        }
        // Copy chunk_size bytes to output
        let chunk_data_end = (pos + chunk_size).min(raw.len());
        out.extend_from_slice(&raw[pos..chunk_data_end]);
        pos = chunk_data_end;
        // Skip trailing \r\n
        while pos < raw.len() && (raw[pos] == b'\r' || raw[pos] == b'\n') {
            pos += 1;
        }
    }
    out
}
