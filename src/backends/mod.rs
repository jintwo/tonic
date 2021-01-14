use std::sync::mpsc::Receiver;

use crate::event::Event;

pub mod dummy;
pub mod midi;

pub trait Backend {
    fn run(&mut self, receiver: Receiver<Event>);
}
