use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
    Job(Job),
    Shutdown,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

struct Worker {
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(receiver: Arc<Mutex<mpsc::Receiver<Message>>>, thread_name: String) -> Self {
        let thread = thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                loop {
                    let msg = {
                        let lock = receiver.lock().unwrap();
                        lock.recv()
                    };
                    match msg {
                        Ok(Message::Job(job)) => {
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                        }
                        Ok(Message::Shutdown) | Err(_) => break,
                    }
                }
            })
            .expect("failed to spawn provider pool thread");
        Self {
            thread: Some(thread),
        }
    }
}

impl ThreadPool {
    pub fn new(size: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<Message>();
        let receiver = Arc::new(Mutex::new(receiver));

        let workers: Vec<Worker> = (0..size)
            .map(|i| Worker::new(Arc::clone(&receiver), format!("provider-pool-{}", i)))
            .collect();

        Self { workers, sender }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _ = self.sender.send(Message::Job(Box::new(f)));
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
