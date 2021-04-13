use std::sync::mpsc::Receiver;
use std::thread;

use crate::backends::Backend;
use crate::event::Event;

const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;
const VELOCITY: u8 = 0x64;

trait MidiEvent {
    fn to_midi(&self) -> [u8; 3];
}

impl MidiEvent for Event {
    fn to_midi(&self) -> [u8; 3] {
        let note = self.value.parse::<u8>().unwrap();
        [NOTE_ON_MSG, note, VELOCITY]
    }
}

pub struct MidiBackend {
    pub device_name: String,
}

impl MidiBackend {
    fn init_output(&self) -> midir::MidiOutputConnection {
        let midi_out = midir::MidiOutput::new(self.device_name.as_ref()).unwrap();
        let out_ports = midi_out.ports();
        let out_port = out_ports.get(1).unwrap();
        midi_out.connect(out_port, "tonic-test").unwrap()
    }
}

impl Backend for MidiBackend {
    fn run(&self, receiver: Receiver<Event>) {
        let mut out = self.init_output();

        thread::spawn(move || loop {
            match receiver.recv() {
                Ok(event) => {
                    println!("[midi] got event: {:?}", event);
                    let midi_event = event.to_midi();
                    out.send(&midi_event).unwrap();
                }
                Err(_) => {}
            }
        });
    }
}
