// client.rs -- ProviderClient, request execution, body building, model fetching, token counting.

use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Sender},
    },
    time::Duration,
};

use rustls::pki_types::ServerName;

use autocode_core::state::ApiProvider;

use super::permits::with_permit;

use super::http::{
    HttpArgs, HttpConn, TimeoutConfig, auth_headers_from_manifest, decode_chunked, parse_url,
    send_http, send_https, tls_config,
};
use super::tool_defs::tool_definitions;
use super::types::{CompletionRequest, CompletionStream, ProviderEvent};

pub struct ProviderClient;

impl ProviderClient {
    pub fn complete(provider: ApiProvider, request: CompletionRequest) -> CompletionStream {
        let (tx, rx) = mpsc::channel();
        // Cancel flag shared with the worker: set when the returned handle is
        // dropped, unblocking the worker's socket read so the thread is
        // released immediately instead of lingering until the request timeout.
        let cancel = Arc::new(AtomicBool::new(false));
        let gate_cancel = Arc::clone(&cancel);
        let worker_cancel = Arc::clone(&cancel);
        // One dedicated thread per in-flight request; a hand-rolled permit
        // gate (std has no Semaphore) bounds concurrency without a fixed
        // worker pool's silent-queue failure mode.
        let worker_tx = tx.clone();
        if std::thread::Builder::new()
            .name("provider-request".into())
            .spawn(move || {
                with_permit(gate_cancel.as_ref(), || {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run_request_once(provider, request, worker_tx, worker_cancel);
                    }));
                });
            })
            .is_err()
        {
            let _ = tx.send(ProviderEvent::Error(
                "Failed to spawn request thread".to_string(),
            ));
        }
        CompletionStream::new(rx, cancel)
    }
}

// -- Request wrapper (single-shot, retry is handled by chat.rs outer layer) ------

fn run_request_once(
    provider: ApiProvider,
    request: CompletionRequest,
    tx: Sender<ProviderEvent>,
    cancel: Arc<AtomicBool>,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_request(provider, request, tx.clone(), cancel)
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
        _ => {}
    }
}

// -- HTTP request execution ----------------------------------------------------

fn run_request(
    provider: ApiProvider,
    req: CompletionRequest,
    tx: Sender<ProviderEvent>,
    cancel: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let thinking_key: &str = if req.thinking_mode {
        req.reasoning_effort.as_str()
    } else {
        "off"
    };
    let thinking_override = req.thinking_overrides.get(thinking_key).cloned();

    let body = build_request_body(
        &req,
        provider.kind.supports_cache_control(),
        provider.supports_strict_tools(),
        thinking_override,
    )?;
    let url = provider.chat_endpoint_url();

    let (host, path, port, use_tls) = parse_url(&url)?;

    let extra_headers = auth_headers_from_manifest(&provider);

    let timeouts = TimeoutConfig {
        request: req.request_timeout_secs,
    };
    if use_tls {
        let conn = HttpConn {
            host: &host,
            port,
            path: &path,
        };
        send_https(HttpArgs {
            conn,
            api_key: provider.api_key.as_str(),
            body: &body,
            stream: req.stream,
            model: &req.model,
            tx,
            timeouts: &timeouts,
            extra_headers: &extra_headers,
            cancel,
        })
    } else {
        send_http(HttpArgs {
            conn: HttpConn {
                host: &host,
                port,
                path: &path,
            },
            api_key: provider.api_key.as_str(),
            body: &body,
            stream: req.stream,
            model: &req.model,
            tx,
            timeouts: &timeouts,
            extra_headers: &extra_headers,
            cancel,
        })
    }
}

fn build_request_body(
    req: &CompletionRequest,
    supports_cache: bool,
    supports_strict: bool,
    thinking_override: Option<serde_json::Value>,
) -> Result<String, serde_json::Error> {
    use super::http::{ReqMsg, RequestBody};

    let handoff_enabled = req.handoff_enabled;
    let messages: Vec<ReqMsg> = req
        .messages
        .iter()
        .map(|m| ReqMsg {
            role: &m.role,
            content: if m.tool_calls.is_some() {
                None
            } else {
                Some(&m.content)
            },
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

    let top_p = if (req.top_p - 1.0).abs() < f32::EPSILON {
        None
    } else {
        Some(req.top_p)
    };
    let frequency_penalty = if req.frequency_penalty.abs() < f32::EPSILON {
        None
    } else {
        Some(req.frequency_penalty)
    };
    let presence_penalty = if req.presence_penalty.abs() < f32::EPSILON {
        None
    } else {
        Some(req.presence_penalty)
    };

    let mut body = RequestBody {
        model: &req.model,
        messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        stream: req.stream,
        top_p,
        frequency_penalty,
        presence_penalty,
        tools: None,
        tool_choice: None,
        stream_options: None,
        parallel_tool_calls: None,
        thinking: None,
        reasoning_effort: None,
        extra: None,
    };

    match thinking_override {
        // A manifest-supplied override for the active effort/off key always
        // wins over the built-in convention below.
        Some(v) => {
            body.extra = Some(v);
        }
        None => match &req.thinking_api {
            autocode_core::state::ThinkingApi::DeepSeek if req.thinking_mode => {
                body.thinking = Some(serde_json::json!({"type": "enabled"}));
                body.reasoning_effort = Some(&req.reasoning_effort);
            }
            autocode_core::state::ThinkingApi::OpenAI if req.thinking_mode => {
                body.reasoning_effort = Some(&req.reasoning_effort);
            }
            autocode_core::state::ThinkingApi::Anthropic if req.thinking_mode => {
                body.thinking =
                    Some(serde_json::json!({"type": "enabled", "budget_tokens": 16000}));
            }
            autocode_core::state::ThinkingApi::Gemini if req.thinking_mode => {
                body.thinking = Some(serde_json::json!({"type": "enabled"}));
            }
            autocode_core::state::ThinkingApi::Grok if req.thinking_mode => {
                body.thinking = Some(serde_json::json!({"type": "enabled"}));
            }
            autocode_core::state::ThinkingApi::OpenRouter => {
                let effort = if req.thinking_mode {
                    req.reasoning_effort.as_str()
                } else {
                    "none"
                };
                body.extra = Some(serde_json::json!({"reasoning": {"effort": effort}}));
            }
            _ => {}
        },
    }

    if req.stream {
        body.stream_options = Some(serde_json::json!({"include_usage": true}));
    }

    if req.tools {
        body.tools = Some(tool_definitions(
            supports_strict,
            super::tool_defs::ToolDefOptions {
                handoff_enabled,
                agent_session: req.agent_session,
            },
        ));
        body.tool_choice = Some(req.tool_choice.to_json());
        body.parallel_tool_calls = Some(req.parallel_tool_calls);
    }

    serde_json::to_string(&body)
}

// -- Model list fetcher --------------------------------------------------------

pub fn fetch_models(provider: &ApiProvider) -> Vec<String> {
    let base = provider.base_url.trim_end_matches('/');
    let url = if !provider.models_list_url.is_empty() {
        provider.models_list_url.replace("{base_url}", base)
    } else {
        format!("{}/models", base)
    };
    let (host, path, port, use_tls) = match parse_url(&url) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let result = (|| -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(15)))?;

        let max_total = 2_000_000 + 8192; // 2 MB body cap + headroom for headers
        let mut buffer = Vec::with_capacity(8192.min(max_total));
        let auth_type = autocode_core::helpers::provider_manifest(&provider.kind)
            .and_then(|m| m.auth_type.as_deref())
            .unwrap_or("Bearer");
        let auth_header = match auth_type {
            "x-api-key" => format!("x-api-key: {}", provider.api_key.as_str()),
            _ => format!("Authorization: Bearer {}", provider.api_key.as_str()),
        };
        let request = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host}\r\n\
             {auth}\r\n\
             Connection: close\r\n\
             \r\n",
            auth = auth_header,
        );

        if use_tls {
            let config = tls_config();
            let dns_name = rustls::pki_types::DnsName::try_from(host.clone())
                .map_err(|_| "invalid DNS name")?;
            let server_name = ServerName::DnsName(dns_name);
            let client = rustls::ClientConnection::new(config, server_name)?;
            let mut tls_stream = rustls::StreamOwned::new(client, stream);
            tls_stream.write_all(request.as_bytes())?;
            let mut buf = [0u8; 8192];
            loop {
                let remaining = max_total.saturating_sub(buffer.len());
                if remaining == 0 {
                    break;
                }
                let n = tls_stream.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                let to_copy = n.min(remaining);
                buffer.extend_from_slice(&buf[..to_copy]);
            }
        } else {
            let mut stream = stream;
            stream.write_all(request.as_bytes())?;
            let mut buf = [0u8; 8192];
            loop {
                let remaining = max_total.saturating_sub(buffer.len());
                if remaining == 0 {
                    break;
                }
                let n = stream.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                let to_copy = n.min(remaining);
                buffer.extend_from_slice(&buf[..to_copy]);
            }
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

// -- Token counting ------------------------------------------------------------

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
    if let Some(prov) = autocode_core::helpers::provider_manifest(&provider.kind) {
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

    let response = super::web::native_post(
        &url,
        provider.api_key.as_str(),
        &body_str,
        timeout_secs,
        65536, // 64 KB cap — token-count responses are tiny
        &extra_refs,
    )?;

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
        .map(|n| n as usize)
        .ok_or_else(|| format!("no token count in response: {}", response.trim()))
}
