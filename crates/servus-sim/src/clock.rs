/// A monotonically increasing simulation tick.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Tick(u64);

impl Tick {
    #[must_use]
    pub const fn number(self) -> u64 {
        self.0
    }

    pub(crate) fn advance(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_tick_starts_at_zero() {
        assert_eq!(Tick::default().number(), 0);
    }

    #[test]
    fn advance_increments_the_tick() {
        let mut tick = Tick::default();
        tick.advance();
        assert_eq!(tick.number(), 1);
    }
}
