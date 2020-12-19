#![feature(div_duration)]

extern crate midir;
extern crate num_cpus;
extern crate scheduled_thread_pool;

use std::collections::{HashMap, VecDeque};
use std::env;
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

fn is_debug() -> bool {
    match env::var("DEBUG") {
        Ok(_) => true,
        Err(_) => false,
    }
}

macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert(String::from($key), String::from($value));
            )+
            m
        }
     };
);

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
}

impl Event {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

#[derive(Debug)]
pub struct Timeline<'a> {
    clock: &'a Clock,
    scheduler: &'a Scheduler,
    receiver: Receiver<(Event, u64)>,
}

impl<'a> Timeline<'a> {
    pub fn new(
        clock: &'a Clock,
        scheduler: &'a Scheduler,
        receiver: Receiver<(Event, u64)>,
    ) -> Self {
        Self {
            clock,
            scheduler,
            receiver,
        }
    }

    fn run(&mut self) {
        loop {
            match self.receiver.recv_timeout(Duration::from_secs_f64(0.0001)) {
                Ok((event, beat)) => self.scheduler.schedule_at(self.clock.beat_at(beat), event),
                Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        }
    }
}

struct MidiBackend {
    out: Option<MidiOutputConnection>,
    receiver: Receiver<Event>,
}

impl MidiBackend {
    fn new(device_name: &str, receiver: Receiver<Event>) -> Self {
        let midi_out = MidiOutput::new(&device_name).unwrap();
        let out_ports = midi_out.ports();
        let out_port = out_ports.get(1).unwrap();
        let out = Some(midi_out.connect(out_port, "tonic-test").unwrap());
        Self { out, receiver }
    }

    fn run(&mut self) {
        loop {
            match self.receiver.recv_timeout(Duration::from_secs_f64(0.0001)) {
                Ok(event) => self.on_event(event),
                Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        }
    }

    fn on_event(&mut self, event: Event) {
        let out_port = self.out.as_mut().unwrap();
        let note = event.value.parse::<u8>().unwrap();
        out_port.send(&[NOTE_ON_MSG, note, VELOCITY]);
    }
}

pub struct Scheduler {
    thread_pool: ScheduledThreadPool,
    sender: Option<Sender<Event>>,
}

impl Scheduler {
    pub fn new() -> Self {
        let thread_pool = ScheduledThreadPool::new(num_cpus::get());
        Self {
            thread_pool,
            sender: None,
        }
    }

    fn start_backend(&mut self) {
        let (sender, receiver) = channel();
        self.sender = Some(sender);
        thread::spawn(move || {
            let mut backend = MidiBackend::new("IAC Driver", receiver);
            backend.run()
        });
    }

    fn schedule_at(&self, at: Instant, event: Event) {
        let now = Instant::now();
        let delay = at - now;
        let sender = self.sender.as_ref().unwrap().clone();
        self.thread_pool.execute_after(delay, move || {
            sender.send(event);
        });
    }
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scheduler").finish()
    }
}

pub fn main() {
    let (sender, receiver) = channel();

    let s1 = sender.clone();
    let generator1 = thread::spawn(move || {
        let mut b = 1;
        loop {
            s1.send((Event::new("60".to_string()), b));
            s1.send((Event::new("65".to_string()), b + 1));
            s1.send((Event::new("73".to_string()), b + 2));
            b += 4;
            thread::sleep(Duration::from_millis(50));
        }
    });

    let s2 = sender.clone();
    let generator2 = thread::spawn(move || {
        let mut b = 5;
        loop {
            s2.send((Event::new("35".to_string()), b));
            s2.send((Event::new("40".to_string()), b + 1));
            s2.send((Event::new("43".to_string()), b + 2));
            b += 8;
            thread::sleep(Duration::from_millis(50));
        }
    });

    let player = thread::spawn(move || {
        let mut clock = Clock::new(BPM);

        let mut scheduler = Scheduler::new();
        scheduler.start_backend();

        let mut timeline = Timeline::new(&clock, &scheduler, receiver);
        timeline.run();
    });

    player.join();
}
