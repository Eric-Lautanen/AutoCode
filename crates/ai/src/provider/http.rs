// http.rs -- Low-level HTTP transport: TLS config, TCP connect, URL parsing,
//            request building, response processing, SSE parsing, chunked encoding.

use std::io;
use std::sync::Arc;
use std::sync::OnceLock;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::mpsc::Sender,
    time::Duration,
};

use rustls::pki_types::ServerName;

use autocode_core::state::ApiProvider;

use super::types::{ProviderEvent, ToolCall};

// -- TLS configuration ---------------------------------------------------------

pub(crate) fn tls_config() -> Arc<rustls::ClientConfig> {
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

// -- URL parsing ---------------------------------------------------------------

pub(crate) fn parse_url(
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

// -- TCP connection ------------------------------------------------------------

pub(crate) fn connect_tcp(host: &str, port: u16, timeout_secs: u64) -> std::io::Result<TcpStream> {
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

// -- Timeout configuration -----------------------------------------------------

pub(crate) struct TimeoutConfig {
    pub request: u64,
}

pub(crate) fn apply_timeouts(
    stream: &TcpStream,
    is_stream: bool,
    cfg: &TimeoutConfig,
) -> std::io::Result<()> {
    // Use the longer request timeout for streaming reads so that normal
    // gaps between reasoning chunks don't trigger a premature TCP timeout.
    // The application-level stall detector (poll_stream) is the first line
    // of defense against idle streams and uses the shorter stream_idle value.
    let read_timeout = if is_stream {
        cfg.request
    } else {
        cfg.request
    };
    stream.set_read_timeout(Some(Duration::from_secs(read_timeout)))?;
    stream.set_write_timeout(Some(Duration::from_secs(cfg.request)))
}

// -- HTTP connection / args structs --------------------------------------------

pub(crate) struct HttpConn<'a> {
    pub host: &'a str,
    pub port: u16,
    pub path: &'a str,
}

pub(crate) struct HttpArgs<'a> {
    pub conn: HttpConn<'a>,
    pub api_key: &'a str,
    pub body: &'a str,
    pub stream: bool,
    pub model: &'a str,
    pub tx: Sender<ProviderEvent>,
    pub timeouts: &'a TimeoutConfig,
    pub extra_headers: &'a [(String, String)],
}

// -- HTTP request building -----------------------------------------------------

pub(crate) fn build_http_request(
    host: &str,
    path: &str,
    api_key: &str,
    body: &str,
    extra_headers: &[(&str, &str)],
) -> String {
    let has_bearer = extra_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("Authorization"));
    let has_xapikey = extra_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("x-api-key"));

    let mut header_str = format!(
        "POST {path} HTTP/1.1\r\n\
        Host: {host}\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {len}\r\n",
        path = path,
        host = host,
        len = body.len(),
    );
    if !has_bearer && !has_xapikey {
        header_str.push_str(&format!("Authorization: Bearer {}\r\n", api_key));
    }
    for (key, value) in extra_headers {
        header_str.push_str(&format!("{}: {}\r\n", key, value));
    }
    header_str.push_str("Connection: close\r\n\r\n");
    header_str.push_str(body);
    header_str
}

// -- Auth headers from manifest -----------------------------------------------

pub(crate) fn auth_headers_from_manifest(provider: &ApiProvider) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if let Some(prov) = autocode_core::helpers::provider_manifest(&provider.kind) {
        if prov.auth_type.as_deref() == Some("x-api-key") {
            headers.push(("x-api-key".into(), provider.api_key.as_str().to_string()));
        }
        if let Some(ver) = &prov.anthropic_version {
            headers.push(("anthropic-version".into(), ver.clone()));
        }
    }
    headers
}

// -- Sanitize for logging ------------------------------------------------------

pub(crate) fn sanitize_for_log(s: &str) -> String {
    let prefixes = ["sk-ant-", "sk-proj-", "sk-", "nvapi-", "xai-"];
    let mut result = s.to_string();

    // 1. Prefix-based redaction (known key formats).
    for prefix in prefixes {
        while let Some(start) = result.find(prefix) {
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

    // 2. Catch any key-like token after "Bearer " or "x-api-key: " markers.
    //    This covers providers with custom key formats not in the prefix list.
    for marker in &["Bearer ", "x-api-key: "] {
        let mut search_start = 0;
        while let Some(pos) = result[search_start..].find(marker) {
            let key_start = search_start + pos + marker.len();
            let remaining = &result[key_start..];
            // Find end of the API key (alphanumeric, dash, underscore).
            let key_len = remaining
                .chars()
                .position(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(remaining.len());
            if key_len >= 8 {
                let replacement = format!("{}[REDACTED]", marker.trim_end());
                let end = key_start + key_len;
                result.replace_range(search_start + pos..end, &replacement);
                search_start = search_start + pos + replacement.len();
            } else {
                search_start = key_start + key_len;
            }
        }
    }

    result
}

// -- HTTP/HTTPS send -----------------------------------------------------------

pub(crate) fn send_http(
    args: HttpArgs<'_>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _t0 = std::time::Instant::now();
    let mut stream_conn = connect_tcp(args.conn.host, args.conn.port, args.timeouts.request)?;
    apply_timeouts(&stream_conn, args.stream, args.timeouts)?;

    let extra_refs: Vec<(&str, &str)> = args
        .extra_headers
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let request = build_http_request(
        args.conn.host,
        args.conn.path,
        args.api_key,
        args.body,
        &extra_refs,
    );
    let _t1 = std::time::Instant::now();
    stream_conn.write_all(request.as_bytes())?;
    stream_conn.flush()?;
    let mut reader = BufReader::with_capacity(8192, stream_conn);
    process_http_response(&mut reader, args.stream, args.model, args.tx)
}

pub(crate) fn send_https(
    args: HttpArgs<'_>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _t0 = std::time::Instant::now();
    let stream = connect_tcp(args.conn.host, args.conn.port, args.timeouts.request)?;
    apply_timeouts(&stream, args.stream, args.timeouts)?;

    let config = tls_config();
    let dns_name = rustls::pki_types::DnsName::try_from(args.conn.host.to_string())
        .map_err(|_| "invalid DNS name")?;
    let server_name = ServerName::DnsName(dns_name);
    let client = rustls::ClientConnection::new(config, server_name)?;
    let _t1 = std::time::Instant::now();
    let mut tls_stream = rustls::StreamOwned::new(client, stream);
    let extra_refs: Vec<(&str, &str)> = args
        .extra_headers
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let request = build_http_request(
        args.conn.host,
        args.conn.path,
        args.api_key,
        args.body,
        &extra_refs,
    );
    let _t2 = std::time::Instant::now();
    tls_stream.write_all(request.as_bytes())?;
    tls_stream.flush()?;
    let mut reader = BufReader::with_capacity(16384, tls_stream);
    process_http_response(&mut reader, args.stream, args.model, args.tx)
}

// -- HTTP response processing --------------------------------------------------

pub(crate) fn process_http_response<R: BufRead>(
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
            let fr = v["choices"][0]["finish_reason"]
                .as_str()
                .map(|s| s.to_string());
            let _ = tx.send(ProviderEvent::Done {
                prompt_tokens: p,
                completion_tokens: c,
                finish_reason: fr,
            });
        }
    }
    Ok(())
}

// -- SSE stream parsing --------------------------------------------------------

/// Splits inline `<think>...</think>` content out of a text stream, even when
/// tags are split across multiple chunks. One instance per stream; state must
/// persist across calls for the lifetime of that stream.
struct ThinkTagFilter {
    in_think: bool,
    carry: String,
}

impl ThinkTagFilter {
    fn new() -> Self {
        Self {
            in_think: false,
            carry: String::new(),
        }
    }

    /// Feed raw delta text in; get back (visible_text, reasoning_text).
    /// Either may be empty. Call once per content delta, in order.
    fn process(&mut self, chunk: &str) -> (String, String) {
        self._process(chunk)
    }

    /// Flush any remaining carry buffer content at stream end.
    /// Returns (visible_text, reasoning_text) for text held back while
    /// waiting for a potential split tag that never arrived.
    fn flush(&mut self) -> (String, String) {
        let mut visible = String::new();
        let mut reasoning = String::new();
        if !self.carry.is_empty() {
            let flushed = std::mem::take(&mut self.carry);
            if self.in_think {
                reasoning = flushed;
            } else {
                visible = flushed;
            }
        }
        (visible, reasoning)
    }

    fn _process(&mut self, chunk: &str) -> (String, String) {
        self.carry.push_str(chunk);
        let mut visible = String::new();
        let mut reasoning = String::new();

        loop {
            let tag = if self.in_think { "</think>" } else { "<think>" };
            match self.carry.find(tag) {
                Some(idx) => {
                    let (before, after) = self.carry.split_at(idx);
                    if self.in_think {
                        reasoning.push_str(before);
                    } else {
                        visible.push_str(before);
                    }
                    let rest = after[tag.len()..].to_string();
                    self.carry = rest;
                    self.in_think = !self.in_think;
                }
                None => {
                    // No full tag in the buffer yet. Hold back a suffix that
                    // could be the start of a split tag, flush the rest now.
                    let hold = tag.len().saturating_sub(1);
                    let flush_len = self.carry.len().saturating_sub(hold);
                    let flush_len = self.carry.floor_char_boundary(flush_len);
                    let flushed: String = self.carry.drain(..flush_len).collect();
                    if self.in_think {
                        reasoning.push_str(&flushed);
                    } else {
                        visible.push_str(&flushed);
                    }
                    break;
                }
            }
        }
        (visible, reasoning)
    }
}

pub(crate) fn parse_sse_stream_from_reader<R: BufRead>(
    reader: R,
    tx: &Sender<ProviderEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _sse_start = std::time::Instant::now();
    let mut lines = reader.lines();
    let mut tool_acc: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new();
    let mut prompt_tokens = 0usize;
    let mut completion_tokens = 0usize;
    let mut saw_data_line = false;
    let mut saw_finish = false;
    let mut had_error = false;
    let mut finish_reason: Option<String> = None;
    let mut raw_buf = String::new();
    let mut last_log = std::time::Instant::now();
    let mut tag_filter = ThinkTagFilter::new();

    // Validate tool call arguments as valid JSON; repair if possible.
    fn fix_args(args: &mut String) -> bool {
        if serde_json::from_str::<serde_json::Value>(args).is_ok() {
            return true;
        }
        // Try to quote-escape and re-parse the args as a JSON string.
        if let Ok(repaired) = serde_json::from_str::<String>(&format!("\"{}\"", args))
            && serde_json::from_str::<serde_json::Value>(&repaired).is_ok()
        {
            *args = repaired;
            return true;
        }
        // Find the longest valid JSON prefix.
        let max_steps = args.len().min(256);
        let mut end = args.len();
        for _ in 0..max_steps {
            if end <= 2 {
                break;
            }
            end = args.floor_char_boundary(end - 1);
            if serde_json::from_str::<serde_json::Value>(&args[..end]).is_ok() {
                args.truncate(end);
                return true;
            }
            if let Some(prev_quote) = args[..end].rfind('"') {
                end = prev_quote + 1;
            }
        }
        false
    }

    for line in &mut lines {
        let line = match line {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };

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
            let (visible, reasoning) = tag_filter.process(text);
            if !visible.is_empty() && tx.send(ProviderEvent::Delta(visible)).is_err() {
                return Err("channel closed".into());
            }
            if !reasoning.is_empty() && tx.send(ProviderEvent::Reasoning(reasoning)).is_err() {
                return Err("channel closed".into());
            }
        }
        if let Some(reasoning) = delta["reasoning_content"]
            .as_str()
            .filter(|s| !s.is_empty())
            && tx
                .send(ProviderEvent::Reasoning(reasoning.to_string()))
                .is_err()
        {
            return Err("channel closed".into());
        }
        // OpenRouter (and possibly others) use "reasoning" instead of
        // "reasoning_content" as the delta field name.
        if let Some(reasoning) = delta["reasoning"].as_str().filter(|s| !s.is_empty())
            && tx
                .send(ProviderEvent::Reasoning(reasoning.to_string()))
                .is_err()
        {
            return Err("channel closed".into());
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
            finish_reason = Some(reason.to_string());
            if reason == "tool_calls" {
                let mut indices: Vec<usize> = tool_acc.keys().cloned().collect();
                indices.sort();
                for idx in indices {
                    if let Some((id, name, mut args)) = tool_acc.remove(&idx) {
                        if !fix_args(&mut args) {
                            continue;
                        }
                        if tx
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
            }
            if reason == "stop" || reason == "tool_calls" || reason == "length" {
                saw_finish = true;
            }
            if reason == "content_filter" {
                had_error = true;
                saw_finish = true;
            }
        }
    }

    if !tool_acc.is_empty() {
        let mut indices: Vec<usize> = tool_acc.keys().cloned().collect();
        indices.sort();
        for idx in indices {
            if let Some((id, name, mut args)) = tool_acc.remove(&idx) {
                if !fix_args(&mut args) {
                    continue;
                }
                if tx
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

    // Flush any text held back in the tag_filter carry buffer before
    // terminating — otherwise the last ~6 characters of the response
    // are silently dropped (they were held waiting for a potential split
    // <think> / </think> tag across chunks).
    let (carry_visible, carry_reasoning) = tag_filter.flush();
    if !carry_visible.is_empty() && tx.send(ProviderEvent::Delta(carry_visible)).is_err() {
        return Err("channel closed".into());
    }
    if !carry_reasoning.is_empty() && tx.send(ProviderEvent::Reasoning(carry_reasoning)).is_err() {
        return Err("channel closed".into());
    }

    if !saw_finish && saw_data_line {
        // Connection lost — the carry was already flushed above.
        let _ = tx.send(ProviderEvent::Error(
            "Connection lost mid-stream — response may be truncated".to_string(),
        ));
        return Ok(());
    }

    if !had_error {
        let _ = tx.send(ProviderEvent::Done {
            prompt_tokens,
            completion_tokens,
            finish_reason,
        });
    } else {
        let _ = tx.send(ProviderEvent::Error(
            "Response filtered by provider content policy (content_filter)".to_string(),
        ));
    }
    Ok(())
}

// -- Chunked transfer-encoding decoder ----------------------------------------

/// Std-only HTTP chunked-transfer decoder implementing `std::io::Read`.
/// Wraps any `Read` and yields the decoded body bytes on-the-fly.
pub(crate) struct ChunkedReader<R: Read> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    ended: bool,
}

impl<R: Read> ChunkedReader<R> {
    pub(crate) fn new(inner: R) -> Self {
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

// -- Decode chunked body (buffer-based) ----------------------------------------

/// Decode HTTP chunked transfer-encoding: strip hex size prefixes and
/// chunk separators, returning the reassembled body.
pub(crate) fn decode_chunked(raw: &[u8]) -> Vec<u8> {
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

// -- HTTP response body extraction ----------------------------------------------

/// Extract the HTTP response body from a raw HTTP response buffer,
/// stripping headers and decoding chunked transfer-encoding.
pub(crate) fn http_response_body(buffer: &[u8]) -> Vec<u8> {
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

// -- Request body serialization ------------------------------------------------

#[derive(serde::Serialize)]
pub(crate) struct ReqMsg<'a> {
    pub role: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
pub(crate) struct RequestBody<'a> {
    pub model: &'a str,
    pub messages: Vec<ReqMsg<'a>>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<serde_json::Value>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "parallel_tool_calls"
    )]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<&'a str>,
    /// Raw JSON merged into the request body root when a per-model
    /// thinking_overrides entry matches the active effort/off key.
    /// Bypasses the ThinkingApi convention entirely when set.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}
