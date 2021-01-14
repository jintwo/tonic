use std::sync::mpsc::Receiver;
use std::thread;

use crate::backends::Backend;
use crate::event::Event;

pub struct DummyBackend;

impl DummyBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl Backend for DummyBackend {
    fn run(&mut self, receiver: Receiver<Event>) {
        thread::spawn(move || loop {
            let event = receiver.recv().unwrap();
            println!("[dummy] got event: {:?}", event);
        });
    }
}
