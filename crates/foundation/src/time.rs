use web_time::Instant;

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTime {
    pub delta: f32,
    pub elapsed: f32,
    pub frame_index: u64,
}

pub struct FrameClock {
    start: Instant,
    last: Instant,
    elapsed: f32,
    frame_index: u64,
}

impl FrameClock {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            elapsed: 0.0,
            frame_index: 0,
        }
    }

    pub fn tick(&mut self) -> FrameTime {
        let now = Instant::now();
        let delta = (now - self.last).as_secs_f32().min(0.25);
        self.last = now;
        self.elapsed = (now - self.start).as_secs_f32();
        self.frame_index += 1;
        FrameTime {
            delta,
            elapsed: self.elapsed,
            frame_index: self.frame_index,
        }
    }
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}
