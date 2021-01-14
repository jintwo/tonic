#[derive(Debug, Clone)]
pub struct Event {
    pub value: String,
    pub beat: u64,
}

impl Event {
    pub fn new(value: String, beat: u64) -> Self {
        Self { value, beat }
    }
}
