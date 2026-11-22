use std::{sync::mpsc, thread};

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Job>,
}

struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

struct Job;

impl Worker {

    pub fn new(id: usize, reciever: mpsc::Receiver<Job>) -> Self {
        let thread = thread::spawn(|| {
            reciever;
        });

        Worker { id, thread }
    }

}

impl ThreadPool {
    // Generates Threadpool
    //
    // "Size" is the number of threads
    //
    // # Panics
    //
    // Will panic if size is zero (Get into the habit of saying when code will panic)
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let (sender, reciever) = mpsc::channel();

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, reciever))
        }
        Self { workers, sender }
    }


    pub fn execute <F> (&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
    }
}
