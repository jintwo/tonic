#![feature(div_duration)]

mod clock;
use clock::{beat_ms, Clock};

mod event;
use event::Event;

mod scheduler;
use scheduler::Scheduler;

mod backends;
use backends::dummy::DummyBackend;
use backends::midi::MidiBackend;

use std::sync::mpsc::{channel, Sender};
use std::thread;

const BPM: u64 = 120; // beats per minute

fn gen(s: &Sender<Event>, f: fn(&u64) -> Vec<Event>) {
    let out = s.clone();
    thread::spawn(move || {
        let mut beat = 1;
        loop {
            let events = f(&beat);
            for e in events {
                out.send(e).unwrap();
            }
            // sleep for a beat
            beat += 1;
            thread::sleep(beat_ms(1, BPM));
        }
    });
}

/* TODO:
1. graceful shutdown
2. lock generators to until clock is started
3. ableton-link
4. generators composition (beat merge?)
*/

pub fn main() {
    let (sender, receiver) = channel();

    gen(&sender, |&beat| {
        if beat < 50 && beat % 4 == 0 {
            return vec![
                Event::new("60".to_string(), beat),
                Event::new("65".to_string(), beat + 1),
                Event::new("73".to_string(), beat + 2),
            ];
        }

        vec![]
    });

    gen(&sender, |&beat| {
        if beat < 100 && beat % 7 == 0 {
            return vec![
                Event::new("35".to_string(), beat),
                Event::new("40".to_string(), beat + 1),
                Event::new("43".to_string(), beat + 2),
            ];
        }

        vec![]
    });

    gen(&sender, |&beat| {
        let mut events: Vec<Event> = vec![];

        if beat > 50 && beat % 3 == 0 {
            events.push(Event::new("81".to_string(), beat))
        }

        if beat > 100 && beat % 5 == 0 {
            events.push(Event::new("86".to_string(), beat))
        }

        events
    });

    let player = thread::spawn(move || {
        let clock = Clock::new(BPM);

        let mut scheduler = Scheduler::new(vec![
            Box::new(MidiBackend {
                device_name: String::from("IAC Driver"),
            }),
            Box::new(DummyBackend {}),
        ]);

        scheduler.start_backends();

        loop {
            let event = receiver.recv().unwrap();
            scheduler.schedule_at(clock.beat_at(event.beat), event);
        }
    });

    player.join().unwrap();
}
