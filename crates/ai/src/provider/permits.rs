// provider/permits.rs -- Hand-rolled concurrency gate for in-flight requests.
// std has no Semaphore and new crates are out: a Mutex<usize> + Condvar
// counter bounds concurrent provider requests (one OS thread per request,
// spawned by the caller). Blocked acquires poll the cancel flag every second
// so dropping a CompletionStream never leaves a zombie waiter.

use std::sync::{
    Condvar, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};

/// Upper bound on concurrently executing provider requests.
pub const MAX_IN_FLIGHT_REQUESTS: usize = 16;

fn gate() -> &'static (Mutex<usize>, Condvar) {
    static GATE: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();
    GATE.get_or_init(|| (Mutex::new(0), Condvar::new()))
}

/// RAII permit slot: released and announced on drop, including through a
/// panic unwinding out of the request body.
struct PermitSlot;

impl Drop for PermitSlot {
    fn drop(&mut self) {
        let (lock, cvar) = gate();
        let mut held = lock.lock().unwrap_or_else(|p| {
            lock.clear_poison();
            p.into_inner()
        });
        *held = held.saturating_sub(1);
        cvar.notify_one();
    }
}

/// Run `f` while holding one of [`MAX_IN_FLIGHT_REQUESTS`] slots, blocking
/// until one frees up. Returns `None` without running `f` when `cancel` flips
/// while waiting for a slot.
pub(crate) fn with_permit<R>(cancel: &AtomicBool, f: impl FnOnce() -> R) -> Option<R> {
    let permit = loop {
        let (lock, cvar) = gate();
        let mut held = lock.lock().unwrap_or_else(|p| {
            lock.clear_poison();
            p.into_inner()
        });
        if *held < MAX_IN_FLIGHT_REQUESTS {
            *held += 1;
            break PermitSlot;
        }
        // Releases the mutex while waiting; wakes every second so the
        // cancellation flag below is re-checked even with no traffic.
        let _ = cvar.wait_timeout(held, std::time::Duration::from_secs(1));
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
    };
    if cancel.load(Ordering::Relaxed) {
        drop(permit);
        return None;
    }
    Some(f())
}
