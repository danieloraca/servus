use std::error::Error;
use std::fmt;

use crate::{Footprint, GridPosition, ServiceId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SolutionId(u64);

impl SolutionId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FoundationKind {
    SmallLot,
    TowerLot,
    MegatowerLot,
}

impl FoundationKind {
    #[must_use]
    pub const fn build_cost(self) -> u64 {
        match self {
            Self::SmallLot => 100,
            Self::TowerLot => 350,
            Self::MegatowerLot => 900,
        }
    }

    #[must_use]
    pub const fn footprint(self) -> Footprint {
        match self {
            Self::SmallLot => Footprint::new(2, 2),
            Self::TowerLot => Footprint::new(3, 3),
            Self::MegatowerLot => Footprint::new(4, 4),
        }
    }

    #[must_use]
    pub const fn maximum_floors(self) -> u16 {
        match self {
            Self::SmallLot => 4,
            Self::TowerLot => 10,
            Self::MegatowerLot => 24,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuildingScale {
    EmptyLot,
    LowRise,
    MidRise,
    HighRise,
    Skyscraper,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Solution {
    id: SolutionId,
    position: GridPosition,
    foundation: FoundationKind,
    services: Vec<ServiceId>,
}

impl Solution {
    pub(crate) const fn new(
        id: SolutionId,
        position: GridPosition,
        foundation: FoundationKind,
    ) -> Self {
        Self {
            id,
            position,
            foundation,
            services: Vec::new(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> SolutionId {
        self.id
    }

    #[must_use]
    pub const fn position(&self) -> GridPosition {
        self.position
    }

    #[must_use]
    pub const fn foundation(&self) -> FoundationKind {
        self.foundation
    }

    #[must_use]
    pub fn services(&self) -> &[ServiceId] {
        &self.services
    }

    #[must_use]
    pub fn floor_count(&self) -> u16 {
        u16::try_from(self.services.len()).unwrap_or(u16::MAX)
    }

    #[must_use]
    pub fn remaining_floors(&self) -> u16 {
        self.foundation
            .maximum_floors()
            .saturating_sub(self.floor_count())
    }

    #[must_use]
    pub fn scale(&self) -> BuildingScale {
        match self.floor_count() {
            0 => BuildingScale::EmptyLot,
            1..=3 => BuildingScale::LowRise,
            4..=7 => BuildingScale::MidRise,
            8..=15 => BuildingScale::HighRise,
            _ => BuildingScale::Skyscraper,
        }
    }

    pub(crate) fn install(&mut self, service: ServiceId) -> Result<(), SolutionError> {
        if self.services.contains(&service) {
            return Err(SolutionError::ServiceAlreadyInstalled {
                solution: self.id,
                service,
            });
        }
        if self.floor_count() >= self.foundation.maximum_floors() {
            return Err(SolutionError::BuildingFull {
                solution: self.id,
                maximum_floors: self.foundation.maximum_floors(),
            });
        }
        self.services.push(service);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolutionError {
    UnknownSolution(SolutionId),
    BuildingFull {
        solution: SolutionId,
        maximum_floors: u16,
    },
    ServiceAlreadyInstalled {
        solution: SolutionId,
        service: ServiceId,
    },
}

impl fmt::Display for SolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSolution(id) => {
                write!(formatter, "solution {} does not exist", id.value())
            }
            Self::BuildingFull {
                solution,
                maximum_floors,
            } => write!(
                formatter,
                "solution {} is full at {maximum_floors} floors",
                solution.value()
            ),
            Self::ServiceAlreadyInstalled { solution, service } => write!(
                formatter,
                "service {} is already installed in solution {}",
                service.value(),
                solution.value()
            ),
        }
    }
}

impl Error for SolutionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foundations_trade_land_and_capital_for_height() {
        assert!(FoundationKind::SmallLot.build_cost() < FoundationKind::TowerLot.build_cost());
        assert!(
            FoundationKind::TowerLot.maximum_floors()
                < FoundationKind::MegatowerLot.maximum_floors()
        );
        assert!(
            FoundationKind::SmallLot.footprint().width()
                < FoundationKind::MegatowerLot.footprint().width()
        );
    }

    #[test]
    fn installed_services_grow_the_building_one_floor_at_a_time() {
        let mut solution = Solution::new(
            SolutionId::new(1),
            GridPosition::new(2, 3),
            FoundationKind::TowerLot,
        );
        assert_eq!(solution.scale(), BuildingScale::EmptyLot);
        for id in 1..=8 {
            solution
                .install(ServiceId::new(id))
                .expect("floor available");
        }
        assert_eq!(solution.floor_count(), 8);
        assert_eq!(solution.remaining_floors(), 2);
        assert_eq!(solution.scale(), BuildingScale::HighRise);
    }

    #[test]
    fn a_foundation_cannot_exceed_its_maximum_height() {
        let mut solution = Solution::new(
            SolutionId::new(4),
            GridPosition::new(0, 0),
            FoundationKind::SmallLot,
        );
        for id in 1..=4 {
            solution
                .install(ServiceId::new(id))
                .expect("floor available");
        }
        assert_eq!(
            solution.install(ServiceId::new(5)),
            Err(SolutionError::BuildingFull {
                solution: SolutionId::new(4),
                maximum_floors: 4,
            })
        );
    }
}
