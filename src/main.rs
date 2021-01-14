#![feature(div_duration)]

extern crate midir;
extern crate num_cpus;
extern crate scheduled_thread_pool;

use std::fmt;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use midir::{MidiOutput, MidiOutputConnection};
use scheduled_thread_pool::ScheduledThreadPool;

const BPM: u64 = 120; // beats per minute

const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;
const VELOCITY: u8 = 0x64;

#[derive(Debug, Clone)]
pub struct Clock {
    start: Instant,
    bar_start: Instant,
    bpm: u64,
    bpb: u64,
}

fn beat_ms(beat: u64, bpm: u64) -> Duration {
    Duration::from_millis(beat * (60000 / bpm))
}

impl Clock {
    pub fn new(bpm: u64) -> Self {
        let now = Instant::now();

        Self {
            start: now,
            bar_start: now,
            bpm: bpm,
            bpb: 4,
        }
    }

    fn start(&self) -> Instant {
        self.start
    }

    fn start_at(&mut self, start_beat: u64) {
        let new_start = Instant::now() - self.tick() * start_beat as u32;
        self.start = new_start;
    }

    fn bar_start(&self) -> Instant {
        self.bar_start
    }

    fn bar_start_at(&mut self, start_bar: u64) {
        let new_bar_start = Instant::now() - self.tock() * start_bar as u32;
        self.bar_start = new_bar_start;
    }

    fn tick(&self) -> Duration {
        beat_ms(1, self.bpm)
    }

    fn tock(&self) -> Duration {
        beat_ms(self.bpb, self.bpm)
    }

    fn beat(&self) -> u64 {
        let delta: Duration = Instant::now() - self.start;
        let current_beat = delta.div_duration_f64(self.tick());
        (current_beat + 1.0) as u64
    }

    fn beat_at(&self, beat: u64) -> Instant {
        self.start + beat as u32 * self.tick()
    }

    fn beat_phase(&self) -> f64 {
        let delta = Instant::now() - self.start;
        let current_beat = delta.div_duration_f64(self.tick());
        current_beat - current_beat.trunc()
    }

    fn bar(&self) -> u64 {
        let delta: Duration = Instant::now() - self.bar_start;
        let current_bar = delta.div_duration_f64(self.tock());
        (current_bar + 1.0) as u64
    }

    fn bar_at(&self, bar: u64) -> Instant {
        self.bar_start + bar as u32 * self.tock()
    }

    fn bar_phase(&self) -> f64 {
        let delta: Duration = Instant::now() - self.start;
        let current_bar = delta.div_duration_f64(self.tock());
        current_bar - current_bar.trunc()
    }

    fn bpm(&self) -> u64 {
        self.bpm
    }

    fn set_bpm(&mut self, new_bpm: u64) {
        let current_beat = self.beat();
        let current_bar = self.bar();
        let new_tick = beat_ms(1, new_bpm);
        let new_tock = new_tick * self.bpb as u32;
        let new_start = self.beat_at(current_beat) - new_tick * current_beat as u32;
        let new_bar_start = self.bar_at(current_bar) - new_tock * current_bar as u32;
        self.start = new_start;
        self.bar_start = new_bar_start;
        self.bpm = new_bpm;
    }

    fn bpb(&self) -> u64 {
        self.bpb
    }

    fn set_bpb(&mut self, new_bpb: u64) {
        let current_bar = self.bar();
        let new_tock = beat_ms(new_bpb, self.bpm);
        let new_bar_start = self.bar_at(current_bar) - new_tock * current_bar as u32;
        self.bar_start = new_bar_start;
        self.bpb = new_bpb;
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    value: String,
    beat: u64,
}

impl Event {
    pub fn new(value: String, beat: u64) -> Self {
        Self { value, beat }
    }
}

pub trait Backend {
    fn run(&mut self, receiver: Receiver<Event>);
}

struct DummyBackend;

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

struct MidiBackend {
    device_name: String,
}

impl MidiBackend {
    fn new(device_name: &str) -> Self {
        Self {
            device_name: String::from(device_name),
        }
    }

    fn init_output(&mut self) -> MidiOutputConnection {
        let midi_out = MidiOutput::new(self.device_name.as_ref()).unwrap();
        let out_ports = midi_out.ports();
        let out_port = out_ports.get(1).unwrap();
        midi_out.connect(out_port, "tonic-test").unwrap()
    }
}

impl Backend for MidiBackend {
    fn run(&mut self, receiver: Receiver<Event>) {
        let mut out = self.init_output();

        thread::spawn(move || loop {
            let event = receiver.recv().unwrap();
            let note = event.value.parse::<u8>().unwrap();
            out.send(&[NOTE_ON_MSG, note, VELOCITY]).unwrap();
        });
    }
}

pub struct Scheduler {
    thread_pool: ScheduledThreadPool,
    producers: Vec<Sender<Event>>,
    backends: Vec<Box<dyn Backend>>,
}

impl Scheduler {
    pub fn new(backends: Vec<Box<dyn Backend>>) -> Self {
        let thread_pool = ScheduledThreadPool::new(num_cpus::get());
        Self {
            thread_pool,
            producers: vec![],
            backends: backends,
        }
    }

    fn start_backends(&mut self) {
        for backend in self.backends.iter_mut() {
            let (sender, receiver) = channel();
            self.producers.push(sender);
            backend.run(receiver);
        }
    }

    fn schedule_at(&self, at: Instant, event: Event) {
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

impl fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scheduler").finish()
    }
}

fn gen(s: &Sender<Event>, f: fn(&u64) -> Vec<Event>) {
    let out = s.clone();
    thread::spawn(move || {
        let mut beat = 1;
        loop {
            let events = f(&beat);
            for e in events {
                out.send(e).unwrap();
            }
            beat += 1;
            // sleep for a beat
            thread::sleep(beat_ms(1, BPM));
        }
    });
}

// TODO: graceful shutdown
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
            Box::new(MidiBackend::new("IAC Driver")),
            Box::new(DummyBackend::new()),
        ]);

        scheduler.start_backends();

        loop {
            let event = receiver.recv().unwrap();
            scheduler.schedule_at(clock.beat_at(event.beat), event);
        }
    });

    player.join().unwrap();
}
