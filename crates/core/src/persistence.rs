use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

use crate::chunked_jsonl;
use crate::state::ChatMessage;

/// Commands sent to the background persistence thread.
pub enum PersistenceCommand {
    /// Append chat messages to the session's chunked JSONL directory.
    /// `dir` is the fully resolved per-session subdirectory path,
    /// computed at send time so directory renames don't orphan messages.
    AppendMessages {
        dir: PathBuf,
        messages: Vec<ChatMessage>,
    },
    /// Flush all pending operations and signal completion.
    Flush { done_tx: mpsc::Sender<()> },
    /// Shut down the persistence thread.
    Shutdown,
}

/// Information about a panic caught in the persistence thread.
#[derive(Debug, Clone)]
pub struct PanicInfo {
    pub thread_name: String,
    pub payload: String,
}

/// A background thread that handles all JSONL message persistence off the UI
/// thread. Metadata writes (session meta, project meta, project identity) remain
/// synchronous since they are tiny atomic writes.
pub struct PersistenceThread {
    tx: mpsc::Sender<PersistenceCommand>,
    handle: Option<thread::JoinHandle<()>>,
    running: std::sync::Arc<AtomicBool>,
    /// Kept alive to keep panic_rx connected — drop would close the channel.
    _panic_tx: mpsc::Sender<PanicInfo>,
    panic_rx: std::sync::Mutex<mpsc::Receiver<PanicInfo>>,
}

impl Default for PersistenceThread {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistenceThread {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<PersistenceCommand>();
        let running = std::sync::Arc::new(AtomicBool::new(true));
        let r = running.clone();
        let (panic_tx, panic_rx) = mpsc::channel::<PanicInfo>();
        let panic_tx_clone = panic_tx.clone();
        let handle = thread::Builder::new()
            .name("persistence".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Self::run_loop(rx);
                }));
                if let Err(panic_payload) = result {
                    let payload = if let Some(s) = panic_payload.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "unknown panic payload".to_string()
                    };
                    let info = PanicInfo {
                        thread_name: "persistence".to_string(),
                        payload,
                    };
                    let _ = panic_tx_clone.send(info);
                }
                r.store(false, Ordering::SeqCst);
            })
            .expect("failed to spawn persistence thread");

        Self {
            tx,
            handle: Some(handle),
            running,
            _panic_tx: panic_tx,
            panic_rx: std::sync::Mutex::new(panic_rx),
        }
    }

    fn run_loop(rx: mpsc::Receiver<PersistenceCommand>) {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                PersistenceCommand::AppendMessages { dir, messages } => {
                    if let Err(e) = chunked_jsonl::append_messages_chunked(&dir, "", "", &messages)
                    {
                        eprintln!(
                            "[persistence] Failed to append messages to {:?}: {}",
                            dir, e
                        );
                    }
                }
                PersistenceCommand::Flush { done_tx } => {
                    if done_tx.send(()).is_err() {
                        eprintln!("[persistence] Flush acknowledgment failed: receiver dropped");
                    }
                }
                PersistenceCommand::Shutdown => break,
            }
        }
    }

    pub fn send(&self, cmd: PersistenceCommand) {
        if let Err(e) = self.tx.send(cmd) {
            eprintln!("[persistence] Failed to send command: {}", e);
        }
    }

    /// Send a flush command and wait up to 30s for acknowledgment.
    pub fn flush(&self) {
        let (done_tx, done_rx) = mpsc::channel();
        if self.tx.send(PersistenceCommand::Flush { done_tx }).is_ok() {
            if let Err(e) = done_rx.recv_timeout(std::time::Duration::from_secs(30)) {
                eprintln!("[persistence] Flush timed out or failed: {}", e);
            }
        } else {
            eprintln!("[persistence] Failed to send flush command");
        }
    }

    /// Shut down the thread and wait for it to finish.
    pub fn shutdown(mut self) {
        let _ = self.tx.send(PersistenceCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Drain any panic reports that have accumulated since the last check.
    pub fn drain_panics(&self) -> Vec<PanicInfo> {
        let rx = self.panic_rx.lock().unwrap();
        let mut panics = Vec::new();
        while let Ok(info) = rx.try_recv() {
            panics.push(info);
        }
        panics
    }
}

impl Drop for PersistenceThread {
    fn drop(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            let _ = self.tx.send(PersistenceCommand::Shutdown);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }
}
