extern crate midir;

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use midir::{MidiOutput, MidiOutputConnection};

/*
TODO: simple arbitrary precision monotonic clock
+1+. Use shared midi connection
2. Improve scheduling (distribute events in bar?)
2.1. get_next_tick / get_next_beat / get_next_bar
3. Use parametrized events
 */

const BPM: f64 = 142.0; // beats per minute
const TPB: f64 = 480.0; // ticks per beat
const BPB: f64 = 16.0; // beats per bar

const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;
const VELOCITY: u8 = 0x64;

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

#[derive(Debug)]
pub enum NoteEvent {
    NoteOn(u8, u8),
    NoteOff(u8),
    Sustain(u64),
}

#[derive(Debug, Clone)]
pub struct Event {
    params: HashMap<String, String>,
}

impl Event {
    pub fn new(params: HashMap<String, String>) -> Self {
        Self { params: params }
    }

    fn get_note(&self) -> u8 {
        self.params
            .get("note")
            .unwrap()
            .trim()
            .parse::<u8>()
            .unwrap()
    }

    fn get_duration(&self) -> u64 {
        self.params
            .get("duration")
            .unwrap()
            .trim()
            .parse::<u64>()
            .unwrap()
    }
}

trait NoteEventsProducer {
    fn get_notes(&self) -> Vec<NoteEvent>;
}

impl NoteEventsProducer for Event {
    fn get_notes(&self) -> Vec<NoteEvent> {
        let note = self.get_note();
        let duration = self.get_duration();
        vec![
            NoteEvent::NoteOn(note, VELOCITY),
            NoteEvent::Sustain(duration),
            NoteEvent::NoteOff(note),
        ]
    }
}

#[derive(Debug)]
pub enum Quantize {
    Bar,
    Beat,
    Tick,
}

#[derive(Debug)]
pub struct TimelineEvent {
    event: Event,
    scheduled_at: Instant,
    reschedule: bool,
    quantize: Quantize,
}

#[derive(Debug)]
pub struct Timeline {
    current_time: Instant,
    tick_duration: Duration,
    ticks_per_beat: f64,
    ticks_per_bar: f64,
    current_tick: u32,
    current_beat: i32,
    current_bar: i32,
    events: VecDeque<Box<TimelineEvent>>,
    sender: Sender<NoteEvent>,
}

impl Timeline {
    pub fn new(bpm: f64, tpb: f64, sender: Sender<NoteEvent>) -> Self {
        let tick_duration = Duration::from_secs_f64(60.0 / bpm / tpb);

        Self {
            current_time: Instant::now(),
            tick_duration: tick_duration,
            current_tick: 0,
            current_beat: 0,
            current_bar: 0,
            events: VecDeque::new(),
            sender: sender,
            ticks_per_beat: tpb,
            ticks_per_bar: tpb * BPB,
        }
    }

    // TODO:
    // +1. get event from scheduled events (nearest)
    // +2. check event timing
    // +?3. quantize?

    fn process(&mut self) {
        println!("events = {:?}", self.events);

        match self.events.pop_front() {
            Some(te) => {
                let e = te.event;

                if te.scheduled_at <= self.current_time {
                    let notes = e.get_notes();
                    for note in notes {
                        let _ = self.sender.send(note);
                    }

                    if te.reschedule {
                        match te.quantize {
                            Quantize::Bar => self.schedule_at_next_bar(e, true),
                            Quantize::Beat => self.schedule_at_next_beat(e, true),
                            quantize => self.schedule_at(e, te.scheduled_at, true, quantize),
                        }
                    }
                } else {
                    self.schedule_at(e, te.scheduled_at, te.reschedule, te.quantize);
                }
            }
            None => (),
        }
    }

    fn beat(&mut self) {
        println!("beat: {}", self.current_beat);
        self.process();
    }

    fn bar(&mut self) {
        println!("bar: {}", self.current_bar);
    }

    fn tick_to_beat(&self, tick: u32) -> u32 {
        return tick / TPB as u32 % BPB as u32;
    }

    fn tick_to_bar(&self, tick: u32) -> u32 {
        return tick / TPB as u32 / BPB as u32;
    }

    fn next_beat_at(&self) -> Instant {
        let ticks_to_next_beat = self.current_tick % self.ticks_per_beat as u32;
        self.current_time + self.tick_duration * ticks_to_next_beat
    }

    fn next_bar_at(&self) -> Instant {
        let ticks_to_next_bar =
            self.ticks_per_bar as u32 - self.current_tick % self.ticks_per_bar as u32;
        self.current_time + self.tick_duration * ticks_to_next_bar
    }

    fn tick(&mut self) {
        self.current_time += self.tick_duration;
        self.current_tick += 1;

        // advance bar
        let bar = self.tick_to_bar(self.current_tick) as i32;
        if self.current_bar != bar {
            self.current_bar = bar;
            self.bar();
        }

        // advance beat
        let beat = self.tick_to_beat(self.current_tick) as i32;
        if self.current_beat != beat {
            self.current_beat = beat;
            self.beat();
        }

        // println!(
        //     "time = {:?}, tick = {}, beat = {}",
        //     self.current_time, self.current_tick, self.current_beat
        // );
    }

    fn update_time(&mut self) {
        self.current_time = Instant::now();
    }

    fn schedule(&mut self, event: Event, reschedule: bool) {
        self.update_time();

        let te = TimelineEvent {
            event: event,
            scheduled_at: self.current_time,
            reschedule: reschedule,
            quantize: Quantize::Tick,
        };

        self.events.push_back(Box::new(te));
    }

    fn schedule_at(&mut self, event: Event, at: Instant, reschedule: bool, quantize: Quantize) {
        self.update_time();

        let te = TimelineEvent {
            event: event,
            scheduled_at: at,
            reschedule: reschedule,
            quantize: quantize,
        };

        self.events.push_back(Box::new(te));
    }

    fn schedule_at_next_beat(&mut self, event: Event, reschedule: bool) {
        self.update_time();

        let te = TimelineEvent {
            event: event,
            scheduled_at: self.next_beat_at(),
            reschedule: reschedule,
            quantize: Quantize::Beat,
        };

        self.events.push_back(Box::new(te));
    }

    fn schedule_at_next_bar(&mut self, event: Event, reschedule: bool) {
        self.update_time();

        let te = TimelineEvent {
            event: event,
            scheduled_at: self.next_bar_at(),
            reschedule: reschedule,
            quantize: Quantize::Bar,
        };

        self.events.push_back(Box::new(te));
    }
}

#[derive(Debug)]
pub struct Clock {
    beats_per_minute: f64,
    ticks_per_beat: f64,
    tick_duration: Duration,
    tick_duration_doubled: Duration,
    timelines: Vec<Box<Timeline>>,
}

impl Clock {
    // UNSTOPPABLE CLOCK!
    // IMMUTABLE CLOCK!
    pub fn new(bpm: f64, tpb: f64) -> Self {
        let tick_duration = Duration::from_secs_f64(60.0 / bpm / tpb);

        Self {
            beats_per_minute: bpm,
            ticks_per_beat: tpb,
            tick_duration: tick_duration,
            tick_duration_doubled: tick_duration * 2,
            timelines: Vec::new(),
        }
    }

    fn new_timeline(&mut self, sender: Sender<NoteEvent>) -> Timeline {
        Timeline::new(self.beats_per_minute, self.ticks_per_beat, sender)
    }

    fn add_timeline(&mut self, timeline: Timeline) {
        self.timelines.push(Box::new(timeline));
    }

    fn run(&mut self) {
        let mut clock0 = Instant::now();
        let mut clock1 = clock0;

        loop {
            if clock1 - clock0 >= self.tick_duration_doubled {
                let delayed_time = clock1 - clock0 - self.tick_duration_doubled;
                println!("delayed... {:#?} / {:#?}", delayed_time, self.tick_duration);
            }

            while clock1 - clock0 >= self.tick_duration {
                clock0 += self.tick_duration;
                for t in self.timelines.iter_mut() {
                    t.tick()
                }
            }

            thread::sleep(Duration::from_secs_f64(0.0001));
            clock1 = Instant::now();
        }
    }
}

trait Backend {
    fn run(&mut self, receiver: Receiver<NoteEvent>) {
        loop {
            let e = receiver.recv().unwrap();
            self.send(e)
        }
    }

    fn send(&mut self, event: NoteEvent);
}

struct DummyBackend {}

impl DummyBackend {
    fn new() -> Self {
        Self {}
    }
}

impl Backend for DummyBackend {
    fn send(&mut self, event: NoteEvent) {
        println!("got event: {:?}", event)
    }
}

struct MidiBackend {
    out: Option<MidiOutputConnection>,
}

impl MidiBackend {
    fn new(device_name: &str) -> Self {
        let midi_out = MidiOutput::new(&device_name).unwrap();
        let out_ports = midi_out.ports();
        let out_port = out_ports.get(1).unwrap();

        Self {
            out: Some(midi_out.connect(out_port, "tonic-test").unwrap()),
        }
    }
}

impl Backend for MidiBackend {
    fn send(&mut self, event: NoteEvent) {
        let out_port = self.out.as_mut().unwrap();

        let _ = match event {
            NoteEvent::NoteOn(note, velocity) => out_port.send(&[NOTE_ON_MSG, note, velocity]),
            NoteEvent::NoteOff(note) => out_port.send(&[NOTE_OFF_MSG, note, VELOCITY]),
            NoteEvent::Sustain(duration) => Ok(thread::sleep(Duration::from_millis(duration))),
        };
    }
}

pub fn main() {
    let (note_sender, note_receiver) = channel();

    let mut clock = Clock::new(BPM, TPB);

    let mut timeline = clock.new_timeline(note_sender);

    timeline.schedule_at_next_bar(Event::new(map! {"note" => "76", "duration" => "50"}), true);

    timeline.schedule_at_next_beat(
        Event::new(map! {"note" => "60", "duration" => "100"}),
        false,
    );
    timeline.schedule_at_next_beat(
        Event::new(map! {"note" => "61", "duration" => "100"}),
        false,
    );
    timeline.schedule_at_next_beat(
        Event::new(map! {"note" => "62", "duration" => "100"}),
        false,
    );

    timeline.schedule_at_next_beat(
        Event::new(map! {"note" => "63", "duration" => "100"}),
        false,
    );

    clock.add_timeline(timeline);

    // let mut backend = MidiBackend::new("IAC Driver");
    let mut backend = DummyBackend::new();
    thread::spawn(move || backend.run(note_receiver));

    let handle = thread::spawn(move || clock.run());
    handle.join().unwrap();
}
