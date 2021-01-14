use std::sync::mpsc::{channel, Sender};
use std::time::Instant;

use crate::backends::Backend;
use crate::event::Event;

pub struct Scheduler {
    thread_pool: scheduled_thread_pool::ScheduledThreadPool,
    producers: Vec<Sender<Event>>,
    backends: Vec<Box<dyn Backend>>,
}

impl Scheduler {
    pub fn new(backends: Vec<Box<dyn Backend>>) -> Self {
        let thread_pool = scheduled_thread_pool::ScheduledThreadPool::new(num_cpus::get());
        Self {
            thread_pool,
            producers: vec![],
            backends: backends,
        }
    }

    pub fn start_backends(&mut self) {
        for backend in self.backends.iter_mut() {
            let (sender, receiver) = channel();
            self.producers.push(sender);
            backend.run(receiver);
        }
    }

    pub fn schedule_at(&self, at: Instant, event: Event) {
        for producer in self.producers.iter() {
            let sender = producer.clone();
            let delay = at - Instant::now();
            let evt = event.clone();
            self.thread_pool.execute_after(delay, move || {
                sender.send(evt).unwrap();
            });
        }
    }
}
