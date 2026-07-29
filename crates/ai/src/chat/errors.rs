/// Shorten verbose OS error messages for display in the chat.
pub fn shorten_err(msg: &str) -> String {
    if let Some(pos) = msg.rfind(" (os error ") {
        let kind = if msg.contains("refused") {
            "connection refused"
        } else if msg.contains("timed out") || msg.contains("did not properly respond") {
            "connection timeout"
        } else if msg.contains("reset") {
            "connection reset"
        } else if msg.contains("No such host") || msg.contains("not known") {
            "dns resolution failed"
        } else if msg.contains("10060") || msg.contains("10061") || msg.contains("10054") {
            // Common WinSock codes: 10060=timeout, 10061=refused, 10054=reset
            "connection failed"
        } else {
            "connection failed"
        };
        let suffix = &msg[pos..];
        return format!("{} {}", kind, suffix);
    }
    msg.to_string()
}

/// Detect and fix provider parameter errors in error messages.
/// Returns true if a parameter was adjusted (caller should retry immediately).
pub fn fix_provider_params(state: &mut autocode_core::state::AppState, err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();

    // Extract param name from error text.
    let param = if lower.contains("top_p") {
        "top_p"
    } else if lower.contains("temperature") && lower.contains("must be") {
        "temperature"
    } else if lower.contains("frequency_penalty") {
        "frequency_penalty"
    } else if lower.contains("presence_penalty") {
        "presence_penalty"
    } else if lower.contains("max_tokens") {
        "max_tokens"
    } else {
        return false;
    };

    let prov_label = state.active_provider.clone();
    let model_id = state
        .active_provider()
        .map(|p| p.model.clone())
        .unwrap_or_default();
    if model_id.is_empty() {
        return false;
    }

    let changed = match param {
        "top_p" => {
            let v = 0.01;
            if let Some(prov) = state.providers.get_mut(&prov_label) {
                prov.top_p = v;
                true
            } else {
                false
            }
        }
        "temperature" => {
            let v = 0.7;
            if let Some(prov) = state.providers.get_mut(&prov_label) {
                prov.temperature = v;
                true
            } else {
                false
            }
        }
        "frequency_penalty" | "presence_penalty" => {
            let v = 0.0;
            if let Some(prov) = state.providers.get_mut(&prov_label) {
                prov.frequency_penalty = v;
                prov.presence_penalty = v;
                true
            } else {
                false
            }
        }
        "max_tokens" => {
            let v = 4096u32;
            if let Some(prov) = state.providers.get_mut(&prov_label) {
                prov.max_output_tokens = v;
                true
            } else {
                false
            }
        }
        _ => false,
    };

    if changed {
        // Persist the fix to the model config so it survives restarts.
        let label = prov_label.clone();
        if let Some(prov) = state.providers.get_mut(&label) {
            let mut mc = prov
                .models_config
                .as_ref()
                .and_then(|m| m.get(&model_id))
                .cloned()
                .unwrap_or_else(|| {
                    let defs = autocode_core::helpers::model_or_safe(&prov.kind, &model_id);
                    autocode_core::storage::provider_file::ModelEntry {
                        id: model_id.clone(),
                        context_window: defs.context_window,
                        max_output_tokens: defs.max_output_tokens,
                        max_output_tokens_thinking: defs.max_output_tokens_thinking,
                        thinking_api: defs.thinking_api.clone(),
                        reasoning_efforts: defs.reasoning_efforts.clone(),
                        supports_cache_control: defs.supports_cache_control,
                        requests_per_hour: defs.requests_per_hour,
                        thinking_overrides: defs.thinking_overrides.clone(),
                        handoff_percent: prov.handoff_percent,
                        temperature: prov.temperature,
                        top_p: prov.top_p,
                        frequency_penalty: prov.frequency_penalty,
                        presence_penalty: prov.presence_penalty,
                        loop_aggressiveness: autocode_core::state::LoopAggressiveness::default(),
                    }
                });
            match param {
                "top_p" => mc.top_p = 0.01,
                "temperature" => mc.temperature = 0.7,
                "frequency_penalty" | "presence_penalty" => {
                    mc.frequency_penalty = 0.0;
                    mc.presence_penalty = 0.0;
                }
                "max_tokens" => mc.max_output_tokens = 4096,
                _ => {}
            }
            let cm = prov
                .models_config
                .get_or_insert_with(std::collections::HashMap::new);
            cm.insert(model_id, mc);
        }
    }

    changed
}
