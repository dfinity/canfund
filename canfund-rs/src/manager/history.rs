use std::collections::VecDeque;

#[derive(Clone)]
pub struct ConsumptionHistory {
    /// The history of the consumed cycles.
    samples: VecDeque<u64>,
    /// The sum of the consumed cycles.
    sum: u64,
    /// The number of history elements to keep.
    window_size: usize,
}

impl ConsumptionHistory {
    /// Constructs a new ConsumptionHistory with the specified window size.
    pub fn new(window_size: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(window_size),
            sum: 0,
            window_size,
        }
    }

    /// Adds a new sample to the history.
    pub fn add_sample(&mut self, consumption: u64) {
        if self.window_size == 0 {
            return;
        }

        if self.samples.len() == self.window_size {
            let oldest_sample = self.samples.pop_front().unwrap();
            self.sum -= oldest_sample;
        }

        self.samples.push_back(consumption);

        self.sum += consumption;
    }

    /// Returns the average of the samples in the history.
    pub fn average(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }

        self.sum / self.samples.len() as u64
    }
}
