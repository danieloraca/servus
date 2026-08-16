use crate::Tick;

/// Incoming demand for a solution during every simulation tick.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Traffic {
    requests_per_tick: u64,
}

impl Traffic {
    #[must_use]
    pub const fn new(requests_per_tick: u64) -> Self {
        Self { requests_per_tick }
    }

    #[must_use]
    pub const fn requests_per_tick(self) -> u64 {
        self.requests_per_tick
    }

    pub fn set_requests_per_tick(&mut self, requests_per_tick: u64) {
        self.requests_per_tick = requests_per_tick;
    }
}

/// The result of processing one simulation tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TickReport {
    pub tick: Tick,
    pub received: u64,
    pub served: u64,
    pub dropped: u64,
    pub revenue: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_demand_can_change() {
        let mut traffic = Traffic::new(30);
        traffic.set_requests_per_tick(75);
        assert_eq!(traffic.requests_per_tick(), 75);
    }

    #[test]
    fn traffic_defaults_to_no_requests() {
        assert_eq!(Traffic::default().requests_per_tick(), 0);
    }
}
