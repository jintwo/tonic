use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Clock {
    start: Instant,
    bar_start: Instant,
    bpm: u64,
    bpb: u64,
}

pub fn beat_ms(beat: u64, bpm: u64) -> Duration {
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

    pub fn beat_at(&self, beat: u64) -> Instant {
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
