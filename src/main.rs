use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

extern crate chrono;
use chrono::Local;

/*
TODO: simple arbitrary precision monotonic clock
 */

const BPM: f64 = 120.0;
const TPB: f64 = 480.0;

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
pub struct Event {
    name: String,
    params: HashMap<String, String>,
    time: Instant,
}

impl Event {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            params: map! {"k0" => "v0", "k1" => "v1"},
            time: Instant::now(),
        }
    }
}

pub struct Timeline {
    current_time: Instant,
    tick_duration: Duration,
    current_tick: u32,
    current_beat: u32,
    events: Vec<Event>,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            current_time: Instant::now(),
            tick_duration: Duration::from_secs(0),
            current_tick: 0,
            current_beat: 0,
            events: Vec::new(),
        }
    }

    fn tick(&mut self) {
        self.current_time += self.tick_duration;
        self.current_tick += 1;
        loop {
            match self.events.pop() {
                Some(e) => println!("event: {:#?}", e),
                None => break,
            }
        }
    }

    fn schedule(&mut self, event: Event) {
        self.events.push(event);
    }
}

pub struct Clock {
    beats_per_minute: f64,
    ticks_per_beat: f64,
    tick_duration: Duration,
    tick_duration_doubled: Duration,
    timelines: Vec<Timeline>,
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

    fn add_timeline(&mut self, timeline: Timeline) {
        self.timelines.push(timeline);
    }

    fn run(&mut self) {
        loop {
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
}

pub fn main() {
    let mut timeline: Timeline = Timeline::new();
    timeline.schedule(Event::new("E0"));

    let mut clock: Clock = Clock::new(BPM, TPB);
    clock.add_timeline(timeline);
    let handle = thread::spawn(move || clock.run());

    handle.join().unwrap();
}
