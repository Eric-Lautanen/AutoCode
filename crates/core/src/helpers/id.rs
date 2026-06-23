use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

pub static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

const ID_CHARSET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

pub fn generate_id() -> String {
    let ts = unix_now();
    let ctr = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ts.hash(&mut hasher);
    ctr.hash(&mut hasher);
    let hash = hasher.finish();
    let mut id = String::with_capacity(5);
    let mut n = hash;
    for _ in 0..5 {
        id.push(ID_CHARSET[(n % 36) as usize] as char);
        n /= 36;
    }
    id
}

/// Generate a session ID that does not collide with any existing IDs.
/// Retries `generate_id()` until a unique value is produced.
pub fn generate_session_id(existing: &[String]) -> String {
    loop {
        let id = generate_id();
        if !existing.contains(&id) {
            return id;
        }
    }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
