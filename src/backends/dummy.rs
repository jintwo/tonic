use std::sync::mpsc::Receiver;
use std::thread;

use crate::backends::Backend;
use crate::event::Event;

pub struct DummyBackend;

impl Backend for DummyBackend {
    fn run(&self, receiver: Receiver<Event>) {
        thread::spawn(move || loop {
            match receiver.recv() {
                Ok(event) => {
                    println!("[dummy] got event: {:?}", event);
                }
                Err(_) => {}
            }
        });
    }
}
