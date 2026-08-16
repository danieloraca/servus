use std::error::Error;
use std::fmt;

/// The spendable credits owned by a company.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Budget(u64);

impl Budget {
    #[must_use]
    pub const fn new(credits: u64) -> Self {
        Self(credits)
    }

    #[must_use]
    pub const fn credits(self) -> u64 {
        self.0
    }

    pub fn spend(&mut self, amount: u64) -> Result<(), BudgetError> {
        if amount > self.0 {
            return Err(BudgetError {
                required: amount,
                available: self.0,
            });
        }
        self.0 -= amount;
        Ok(())
    }

    pub(crate) fn credit(&mut self, amount: u64) {
        self.0 = self.0.saturating_add(amount);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetError {
    pub required: u64,
    pub available: u64,
}

impl fmt::Display for BudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "not enough credits: {} required, {} available",
            self.required, self.available
        )
    }
}

impl Error for BudgetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spending_reduces_the_budget() {
        let mut budget = Budget::new(100);
        assert_eq!(budget.spend(40), Ok(()));
        assert_eq!(budget.credits(), 60);
    }

    #[test]
    fn failed_spending_does_not_change_the_budget() {
        let mut budget = Budget::new(25);
        assert_eq!(
            budget.spend(40),
            Err(BudgetError {
                required: 40,
                available: 25,
            })
        );
        assert_eq!(budget.credits(), 25);
    }

    #[test]
    fn crediting_adds_to_the_budget() {
        let mut budget = Budget::new(25);
        budget.credit(15);
        assert_eq!(budget.credits(), 40);
    }

    #[test]
    fn budget_errors_have_a_readable_message() {
        let error = BudgetError {
            required: 50,
            available: 20,
        };
        assert_eq!(
            error.to_string(),
            "not enough credits: 50 required, 20 available"
        );
    }
}
