//! Beat detection on bass energy: low-pass the signal, compare short-term energy
//! with a running average, and report a beat on a jump.

pub struct BeatDetector {
    alpha: f32,
    lowpassed: f32,
    block: Vec<f32>,
    block_len: usize,
    average: f32,
    since_beat: usize,
    min_gap_blocks: usize,
}

impl BeatDetector {
    pub fn new(sample_rate: f32) -> Self {
        let cutoff = 150.0;
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
        let block_len = (sample_rate / 100.0).max(1.0) as usize; // 10 ms blocks
        Self {
            alpha: dt / (rc + dt),
            lowpassed: 0.0,
            block: Vec::with_capacity(block_len),
            block_len,
            average: 0.0,
            since_beat: usize::MAX / 2,
            min_gap_blocks: 18, // ~180 ms, about 330 BPM
        }
    }

    /// Feeds mono samples; returns true if a beat started in them.
    pub fn process(&mut self, samples: impl Iterator<Item = f32>) -> bool {
        let mut beat = false;
        for s in samples {
            self.lowpassed += self.alpha * (s - self.lowpassed);
            self.block.push(self.lowpassed);
            if self.block.len() == self.block_len {
                let energy =
                    (self.block.iter().map(|x| x * x).sum::<f32>() / self.block_len as f32).sqrt();
                self.block.clear();
                self.since_beat = self.since_beat.saturating_add(1);
                if energy > self.average * 1.5
                    && energy > 0.01
                    && self.since_beat >= self.min_gap_blocks
                {
                    self.since_beat = 0;
                    beat = true;
                }
                self.average = self.average * 0.97 + energy * 0.03;
            }
        }
        beat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kick_drums_not_silence() {
        let rate = 48_000.0;
        let mut d = BeatDetector::new(rate);
        let silence = vec![0.0f32; 48_000];
        assert!(!d.process(silence.iter().copied()));

        // 2 seconds at 120 BPM: a 60 Hz burst every 0.5 s.
        let mut beats = 0;
        for _ in 0..4 {
            let chunk: Vec<f32> = (0..24_000)
                .map(|i| {
                    let t = i as f32 / rate;
                    if t < 0.08 {
                        (t * 60.0 * std::f32::consts::TAU).sin() * 0.8
                    } else {
                        0.0
                    }
                })
                .collect();
            if d.process(chunk.iter().copied()) {
                beats += 1;
            }
        }
        assert_eq!(beats, 4);
    }
}
