use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::state::ChatMessage;
use crate::chunked_jsonl;

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
    Flush {
        done_tx: mpsc::Sender<()>,
    },
    /// Shut down the persistence thread.
    Shutdown,
}

/// A background thread that handles all JSONL message persistence off the UI
/// thread. Metadata writes (session meta, project meta, project identity) remain
/// synchronous since they are tiny atomic writes.
pub struct PersistenceThread {
    tx: mpsc::Sender<PersistenceCommand>,
    handle: Option<thread::JoinHandle<()>>,
    running: std::sync::Arc<AtomicBool>,
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
        let handle = thread::Builder::new()
            .name("persistence".into())
            .spawn(move || {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Self::run_loop(rx);
                }));
                r.store(false, Ordering::SeqCst);
            })
            .expect("failed to spawn persistence thread");

        Self {
            tx,
            handle: Some(handle),
            running,
        }
    }

    fn run_loop(rx: mpsc::Receiver<PersistenceCommand>) {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                PersistenceCommand::AppendMessages { dir, messages } => {
                    let _ = chunked_jsonl::append_messages_chunked(
                        &dir, "", "", &messages,
                    );
                }
                PersistenceCommand::Flush { done_tx } => {
                    let _ = done_tx.send(());
                }
                PersistenceCommand::Shutdown => break,
            }
        }
    }

    pub fn send(&self, cmd: PersistenceCommand) {
        let _ = self.tx.send(cmd);
    }

    /// Send a flush command and wait up to 30s for acknowledgment.
    pub fn flush(&self) {
        let (done_tx, done_rx) = mpsc::channel();
        let _ = self.tx.send(PersistenceCommand::Flush { done_tx });
        let _ = done_rx.recv_timeout(std::time::Duration::from_secs(30));
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
