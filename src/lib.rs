use std::{sync::{Arc, Mutex, mpsc}, thread};

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl Drop for ThreadPool {
    fn drop(&mut self){
        drop(self.sender.take());

        for worker in self.workers.drain(..){
            println!("Shutting down worker {}", worker.id);

            worker.thread.join().unwrap();
        }
    }
}

impl Worker {

    pub fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || {
            loop {
                let message = receiver.lock().unwrap().recv();
                match message{
                    Ok(job) => {
                        println!("Worker {id} aquired a job, now executing!");
                        job();
                    }

                    Err(_) => {
                        println!("Worker {id} has disconnected from job, shutting down!");
                        break;
                    }
                }
            }
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

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }
        Self { workers, sender: Some((sender)) }
    }


    pub fn execute <F> (&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}
