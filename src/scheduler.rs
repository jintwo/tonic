use std::sync::mpsc::{channel, Sender};
use std::time::Instant;
use RefCell;

use crate::backends::Backend;
use crate::event::Event;

pub struct Scheduler {
    thread_pool: scheduled_thread_pool::ScheduledThreadPool,
    producers: RefCell<Vec<Sender<Event>>>,
    backends: RefCell<Vec<Box<dyn Backend>>>,
}

impl Scheduler {
    pub fn new(backends: RefCell<Vec<Box<dyn Backend>>>) -> Self {
        let thread_pool = scheduled_thread_pool::ScheduledThreadPool::new(num_cpus::get());
        Self {
            thread_pool,
            producers: RefCell::new(vec![]),
            backends: backends,
        }
    }

    pub fn start_backends(&self) {
        for backend in self.backends.borrow_mut().iter_mut() {
            let (sender, receiver) = channel();
            self.producers.borrow_mut().push(sender);
            backend.run(receiver);
        }
    }

    pub fn schedule_at(&self, at: Instant, event: Event) {
        for producer in self.producers.borrow().iter() {
            let sender = producer.clone();
            let delay = at - Instant::now();
            let evt = event.clone();
            self.thread_pool.execute_after(delay, move || {
                sender.send(evt).unwrap();
            });
        }
    }
}
