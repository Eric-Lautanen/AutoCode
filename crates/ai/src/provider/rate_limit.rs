// rate_limit.rs -- API rate limiting.

use std::collections::HashMap;
use std::sync::Mutex;

use autocode_core::state::ApiProvider;

/// Per-provider-model API rate limiter.
/// Enforces a minimum interval between successive requests so that
/// the average rate does not exceed `requests_per_hour`.  For example,
/// 30 RPH → one request every 120 s, 40 RPH → every 90 s, etc.
static LAST_API_REQUEST: Mutex<Option<HashMap<(String, String), std::time::Instant>>> =
    Mutex::new(None);

/// Returns the number of milliseconds to wait before the next request
/// to this provider+model, or 0 if no wait is needed.  Does NOT sleep.
pub fn api_rate_limit_wait_ms(provider: &ApiProvider, label: &str) -> u64 {
    let Some(limit) = provider.requests_per_hour else {
        return 0;
    };
    if limit == 0 {
        return 0;
    }
    let interval = (3600u64 * 1000) / limit as u64;

    let last_requests = match LAST_API_REQUEST.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            LAST_API_REQUEST.clear_poison();
            poisoned.into_inner()
        }
    };
    let map = match last_requests.as_ref() {
        Some(m) => m,
        None => return 0,
    };
    let key = (label.to_string(), provider.model.clone());
    if let Some(last) = map.get(&key) {
        let elapsed_ms = last.elapsed().as_millis() as u64;
        if elapsed_ms < interval {
            return interval - elapsed_ms;
        }
    }
    0
}

/// Reset the API rate limit timer so the next request won't be delayed.
pub fn api_rate_limit_reset() {
    if let Ok(mut guard) = LAST_API_REQUEST.lock() {
        *guard = None;
    }
}

/// Record that a request was made (updates the last-request timestamp).
pub fn api_rate_limit_record(provider: &ApiProvider, label: &str) {
    let mut last_requests = match LAST_API_REQUEST.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            LAST_API_REQUEST.clear_poison();
            poisoned.into_inner()
        }
    };
    let map = last_requests.get_or_insert_with(HashMap::new);
    map.insert(
        (label.to_string(), provider.model.clone()),
        std::time::Instant::now(),
    );
}
