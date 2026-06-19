use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
    Job(Job),
    Shutdown,
}

/// Result of a panic caught in the thread pool.
#[derive(Debug)]
pub struct PanicInfo {
    pub thread_name: String,
    pub payload: String,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
    panic_tx: mpsc::Sender<PanicInfo>,
    panic_rx: Mutex<mpsc::Receiver<PanicInfo>>,
}

struct Worker {
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(
        receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
        panic_tx: mpsc::Sender<PanicInfo>,
        thread_name: String,
    ) -> Self {
        let thread = thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || loop {
                let msg = {
                    let lock = receiver.lock().unwrap();
                    lock.recv()
                };
                match msg {
                    Ok(Message::Job(job)) => {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                        if let Err(panic_payload) = result {
                            let payload = if let Some(s) = panic_payload.downcast_ref::<String>() {
                                s.clone()
                            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                                s.to_string()
                            } else {
                                "unknown panic payload".to_string()
                            };
                            let info = PanicInfo {
                                thread_name: thread_name.clone(),
                                payload,
                            };
                            let _ = panic_tx.send(info);
                        }
                    }
                    Ok(Message::Shutdown) | Err(_) => break,
                }
            })
            .expect("failed to spawn provider pool thread");
        Self { thread: Some(thread) }
    }
}

impl ThreadPool {
    pub fn new(size: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<Message>();
        let receiver = Arc::new(Mutex::new(receiver));
        let (panic_tx, panic_rx) = mpsc::channel::<PanicInfo>();

        let workers: Vec<Worker> = (0..size)
            .map(|i| {
                Worker::new(
                    Arc::clone(&receiver),
                    panic_tx.clone(),
                    format!("provider-pool-{}", i),
                )
            })
            .collect();

        Self {
            workers,
            sender,
            panic_tx,
            panic_rx: Mutex::new(panic_rx),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _ = self.sender.send(Message::Job(Box::new(f)));
    }

    pub fn pool_size(&self) -> usize {
        self.workers.len()
    }

    /// Drain any panic reports that have accumulated since the last check.
    /// Call this periodically (e.g. once per frame) to surface worker panics.
    pub fn drain_panics(&self) -> Vec<PanicInfo> {
        let rx = self.panic_rx.lock().unwrap();
        let mut panics = Vec::new();
        while let Ok(info) = rx.try_recv() {
            panics.push(info);
        }
        panics
    }

    /// Check if any panics have occurred without draining them.
    pub fn has_panics(&self) -> bool {
        let rx = self.panic_rx.lock().unwrap();
        rx.try_recv().is_ok()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in &self.workers {
            let _ = self.sender.send(Message::Shutdown);
        }
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
