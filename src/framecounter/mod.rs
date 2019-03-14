use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime};

// Measures Frames Per Second (FPS).
pub struct FPSCounter {
    last_second_frames: VecDeque<Instant>,
    last_display: SystemTime,
}

impl FPSCounter {
    // Creates a new FPSCounter.
    pub fn new() -> FPSCounter {
        FPSCounter {
            last_second_frames: VecDeque::with_capacity(128),
            last_display: SystemTime::now(),
        }
    }

    // Updates the FPSCounter and returns number of frames.
    pub fn tick(&mut self) -> usize {
        let now = Instant::now();
        let a_second_ago = now - Duration::from_secs(1);

        while self
            .last_second_frames
            .front()
            .map_or(false, |t| *t < a_second_ago)
        {
            self.last_second_frames.pop_front();
        }

        self.last_second_frames.push_back(now);
        self.last_second_frames.len()
    }

    pub fn tick_and_display(&mut self) {
        let n = self.tick();
        if self.last_display.elapsed().unwrap().as_millis() > 1000 {
            println!("fps: {}", n);
            self.last_display = SystemTime::now();
        }
    }
}
