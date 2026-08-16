use std::error::Error;
use std::fmt;

use crate::ServiceId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GridPosition {
    pub x: u16,
    pub y: u16,
}

impl GridPosition {
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Footprint {
    width: u16,
    height: u16,
}

impl Footprint {
    pub(crate) const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapSize {
    width: u16,
    height: u16,
}

impl MapSize {
    pub fn new(width: u16, height: u16) -> Result<Self, MapSizeError> {
        if width == 0 || height == 0 {
            return Err(MapSizeError { width, height });
        }
        Ok(Self { width, height })
    }

    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapSizeError {
    pub width: u16,
    pub height: u16,
}

impl fmt::Display for MapSizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "map dimensions must be non-zero, got {}x{}",
            self.width, self.height
        )
    }
}

impl Error for MapSizeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementError {
    OutOfBounds {
        position: GridPosition,
        footprint: Footprint,
        map_size: MapSize,
    },
    Occupied {
        position: GridPosition,
        service_id: ServiceId,
    },
}

impl fmt::Display for PlacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds {
                position,
                footprint,
                map_size,
            } => write!(
                formatter,
                "{}x{} service at ({}, {}) does not fit on {}x{} map",
                footprint.width,
                footprint.height,
                position.x,
                position.y,
                map_size.width,
                map_size.height
            ),
            Self::Occupied {
                position,
                service_id,
            } => write!(
                formatter,
                "tile ({}, {}) is occupied by service {}",
                position.x,
                position.y,
                service_id.value()
            ),
        }
    }
}

impl Error for PlacementError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridMap {
    size: MapSize,
    occupied: Vec<Option<ServiceId>>,
}

impl GridMap {
    #[must_use]
    pub fn new(size: MapSize) -> Self {
        let tile_count = usize::from(size.width) * usize::from(size.height);
        Self {
            size,
            occupied: vec![None; tile_count],
        }
    }

    #[must_use]
    pub const fn size(&self) -> MapSize {
        self.size
    }

    #[must_use]
    pub fn service_at(&self, position: GridPosition) -> Option<ServiceId> {
        self.index(position).and_then(|index| self.occupied[index])
    }

    pub(crate) fn validate_placement(
        &self,
        position: GridPosition,
        footprint: Footprint,
    ) -> Result<(), PlacementError> {
        let Some(end_x) = position.x.checked_add(footprint.width) else {
            return Err(self.out_of_bounds(position, footprint));
        };
        let Some(end_y) = position.y.checked_add(footprint.height) else {
            return Err(self.out_of_bounds(position, footprint));
        };
        if end_x > self.size.width || end_y > self.size.height {
            return Err(self.out_of_bounds(position, footprint));
        }

        for y in position.y..end_y {
            for x in position.x..end_x {
                let tile = GridPosition::new(x, y);
                if let Some(service_id) = self.service_at(tile) {
                    return Err(PlacementError::Occupied {
                        position: tile,
                        service_id,
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn occupy(
        &mut self,
        position: GridPosition,
        footprint: Footprint,
        service_id: ServiceId,
    ) {
        let end_x = position.x + footprint.width;
        let end_y = position.y + footprint.height;
        for y in position.y..end_y {
            for x in position.x..end_x {
                let index = self
                    .index(GridPosition::new(x, y))
                    .expect("validated footprints contain only map positions");
                self.occupied[index] = Some(service_id);
            }
        }
    }

    fn index(&self, position: GridPosition) -> Option<usize> {
        if position.x >= self.size.width || position.y >= self.size.height {
            return None;
        }
        Some(usize::from(position.y) * usize::from(self.size.width) + usize::from(position.x))
    }

    const fn out_of_bounds(&self, position: GridPosition, footprint: Footprint) -> PlacementError {
        PlacementError::OutOfBounds {
            position,
            footprint,
            map_size: self.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_size(width: u16, height: u16) -> MapSize {
        MapSize::new(width, height).expect("test maps have valid dimensions")
    }

    #[test]
    fn map_dimensions_must_be_non_zero() {
        assert_eq!(
            MapSize::new(0, 4),
            Err(MapSizeError {
                width: 0,
                height: 4,
            })
        );
        assert_eq!(
            MapSize::new(4, 0),
            Err(MapSizeError {
                width: 4,
                height: 0,
            })
        );
    }

    #[test]
    fn map_size_errors_have_a_readable_message() {
        let error = MapSizeError {
            width: 0,
            height: 5,
        };
        assert_eq!(
            error.to_string(),
            "map dimensions must be non-zero, got 0x5"
        );
    }

    #[test]
    fn valid_empty_tile_accepts_a_footprint() {
        let map = GridMap::new(map_size(4, 3));
        assert_eq!(map.size().width(), 4);
        assert_eq!(map.size().height(), 3);
        assert_eq!(
            map.validate_placement(GridPosition::new(2, 1), Footprint::new(2, 2)),
            Ok(())
        );
    }

    #[test]
    fn footprints_must_fit_inside_the_map() {
        let map = GridMap::new(map_size(4, 3));
        let position = GridPosition::new(3, 2);
        let footprint = Footprint::new(2, 1);
        assert_eq!(
            map.validate_placement(position, footprint),
            Err(PlacementError::OutOfBounds {
                position,
                footprint,
                map_size: map_size(4, 3),
            })
        );
    }

    #[test]
    fn positions_outside_the_map_are_empty_but_invalid_for_placement() {
        let map = GridMap::new(map_size(2, 2));
        let position = GridPosition::new(2, 0);
        assert_eq!(map.service_at(position), None);
        assert!(
            matches!(
                map.validate_placement(position, Footprint::new(1, 1)),
                Err(PlacementError::OutOfBounds { .. })
            ),
            "an out-of-bounds position must not be buildable"
        );
    }

    #[test]
    fn overflowing_footprint_coordinates_are_out_of_bounds() {
        let map = GridMap::new(map_size(2, 2));
        assert!(matches!(
            map.validate_placement(GridPosition::new(u16::MAX, 0), Footprint::new(1, 1)),
            Err(PlacementError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn occupied_tiles_report_the_existing_service() {
        let mut map = GridMap::new(map_size(3, 3));
        let service_id = ServiceId::new(9);
        map.occupy(GridPosition::new(1, 1), Footprint::new(2, 1), service_id);
        assert_eq!(map.service_at(GridPosition::new(2, 1)), Some(service_id));
        assert_eq!(
            map.validate_placement(GridPosition::new(2, 1), Footprint::new(1, 1)),
            Err(PlacementError::Occupied {
                position: GridPosition::new(2, 1),
                service_id,
            })
        );
    }

    #[test]
    fn placement_errors_have_readable_messages() {
        let error = PlacementError::Occupied {
            position: GridPosition::new(3, 7),
            service_id: ServiceId::new(4),
        };
        assert_eq!(error.to_string(), "tile (3, 7) is occupied by service 4");

        let error = PlacementError::OutOfBounds {
            position: GridPosition::new(3, 2),
            footprint: Footprint::new(2, 1),
            map_size: map_size(4, 3),
        };
        assert_eq!(
            error.to_string(),
            "2x1 service at (3, 2) does not fit on 4x3 map"
        );
    }
}
