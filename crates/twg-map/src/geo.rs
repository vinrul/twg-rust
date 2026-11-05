use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_MERCATOR_LATITUDE: f64 = 85.051_128_78;
pub const EARTH_RADIUS_METERS: f64 = 6_378_137.0;
pub const EQUATOR_CIRCUMFERENCE_METERS: f64 = 2.0 * std::f64::consts::PI * EARTH_RADIUS_METERS;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub longitude: f64,
    pub latitude: f64,
}

impl GeoPoint {
    pub const fn new(longitude: f64, latitude: f64) -> Self {
        Self {
            longitude,
            latitude,
        }
    }

    pub fn validate(self) -> Result<Self, GeoError> {
        if !self.longitude.is_finite() || !self.latitude.is_finite() {
            return Err(GeoError::NonFiniteCoordinate);
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err(GeoError::LongitudeOutOfRange(self.longitude));
        }
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err(GeoError::LatitudeOutOfRange(self.latitude));
        }
        Ok(self)
    }

    pub fn mercator(self) -> Result<MercatorPoint, GeoError> {
        self.validate()?;
        let latitude = self
            .latitude
            .clamp(-MAX_MERCATOR_LATITUDE, MAX_MERCATOR_LATITUDE);
        let x = self.longitude.to_radians() * EARTH_RADIUS_METERS;
        let y = ((std::f64::consts::FRAC_PI_4 + latitude.to_radians() / 2.0).tan()).ln()
            * EARTH_RADIUS_METERS;
        Ok(MercatorPoint { x, y })
    }

    pub fn tile(self, zoom: u8) -> Result<TileId, GeoError> {
        let (x, y) = self.tile_fraction(zoom)?;
        let world_tiles = f64::from(TileId::count(zoom)?);
        Ok(TileId {
            zoom,
            x: x.floor().clamp(0.0, world_tiles - 1.0) as u32,
            y: y.floor().clamp(0.0, world_tiles - 1.0) as u32,
        })
    }

    pub fn tile_fraction(self, zoom: u8) -> Result<(f64, f64), GeoError> {
        self.validate()?;
        let world_tiles = f64::from(TileId::count(zoom)?);
        let latitude = self
            .latitude
            .clamp(-MAX_MERCATOR_LATITUDE, MAX_MERCATOR_LATITUDE);
        let x = (self.longitude + 180.0) / 360.0 * world_tiles;
        let latitude_radians = latitude.to_radians();
        let y = (1.0
            - (latitude_radians.tan() + 1.0 / latitude_radians.cos()).ln() / std::f64::consts::PI)
            / 2.0
            * world_tiles;
        Ok((x, y))
    }

    pub fn from_tile_fraction(zoom: u8, x: f64, y: f64) -> Result<Self, GeoError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(GeoError::NonFiniteCoordinate);
        }
        let world_tiles = f64::from(TileId::count(zoom)?);
        let normalized_x = x.rem_euclid(world_tiles);
        let clamped_y = y.clamp(0.0, world_tiles);
        let longitude = normalized_x / world_tiles * 360.0 - 180.0;
        let latitude = (std::f64::consts::PI * (1.0 - 2.0 * clamped_y / world_tiles))
            .sinh()
            .atan()
            .to_degrees();
        Ok(Self::new(longitude, latitude))
    }

    pub fn local_offset(self, origin: Self) -> Result<LocalPoint, GeoError> {
        let point = self.mercator()?;
        let origin = origin.mercator()?;
        Ok(LocalPoint {
            east_meters: point.x - origin.x,
            north_meters: point.y - origin.y,
        })
    }

    pub fn from_local_offset(
        origin: Self,
        east_meters: f64,
        north_meters: f64,
    ) -> Result<Self, GeoError> {
        if !east_meters.is_finite() || !north_meters.is_finite() {
            return Err(GeoError::NonFiniteCoordinate);
        }
        let origin = origin.mercator()?;
        MercatorPoint {
            x: origin.x + east_meters,
            y: origin.y + north_meters,
        }
        .geo()
    }

    pub fn distance_meters(self, other: Self) -> Result<f64, GeoError> {
        self.validate()?;
        other.validate()?;
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let delta_lat = lat2 - lat1;
        let delta_lon = (other.longitude - self.longitude).to_radians();
        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        Ok(2.0 * EARTH_RADIUS_METERS * a.sqrt().asin())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MercatorPoint {
    pub x: f64,
    pub y: f64,
}

impl MercatorPoint {
    pub fn geo(self) -> Result<GeoPoint, GeoError> {
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(GeoError::NonFiniteCoordinate);
        }
        let longitude = (self.x / EARTH_RADIUS_METERS).to_degrees();
        let latitude = (2.0 * (self.y / EARTH_RADIUS_METERS).exp().atan()
            - std::f64::consts::FRAC_PI_2)
            .to_degrees();
        GeoPoint::new(longitude, latitude).validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileId {
    pub zoom: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub fn new(zoom: u8, x: u32, y: u32) -> Result<Self, GeoError> {
        let count = Self::count(zoom)?;
        if x >= count || y >= count {
            return Err(GeoError::TileOutOfRange { zoom, x, y });
        }
        Ok(Self { zoom, x, y })
    }

    pub fn count(zoom: u8) -> Result<u32, GeoError> {
        if zoom > 30 {
            return Err(GeoError::ZoomOutOfRange(zoom));
        }
        Ok(1u32 << zoom)
    }

    pub fn parent(self) -> Option<Self> {
        (self.zoom > 0).then_some(Self {
            zoom: self.zoom - 1,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    pub fn children(self) -> Result<[Self; 4], GeoError> {
        if self.zoom >= 30 {
            return Err(GeoError::ZoomOutOfRange(self.zoom));
        }
        let zoom = self.zoom + 1;
        let x = self.x * 2;
        let y = self.y * 2;
        Ok([
            Self { zoom, x, y },
            Self { zoom, x: x + 1, y },
            Self { zoom, x, y: y + 1 },
            Self {
                zoom,
                x: x + 1,
                y: y + 1,
            },
        ])
    }

    pub fn center(self) -> GeoPoint {
        GeoPoint::from_tile_fraction(self.zoom, f64::from(self.x) + 0.5, f64::from(self.y) + 0.5)
            .unwrap_or(GeoPoint::new(0.0, 0.0))
    }

    pub fn mercator_bounds(self) -> (MercatorPoint, MercatorPoint) {
        let count = f64::from(1u32 << self.zoom);
        let half_world = std::f64::consts::PI * EARTH_RADIUS_METERS;
        let north = (1.0 - 2.0 * f64::from(self.y) / count) * half_world;
        let south = (1.0 - 2.0 * f64::from(self.y + 1) / count) * half_world;
        let west = (2.0 * f64::from(self.x) / count - 1.0) * half_world;
        let east = (2.0 * f64::from(self.x + 1) / count - 1.0) * half_world;
        (
            MercatorPoint { x: west, y: north },
            MercatorPoint { x: east, y: south },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LocalPoint {
    pub east_meters: f64,
    pub north_meters: f64,
}

pub fn meters_per_tile(latitude: f64, zoom: u8) -> Result<f64, GeoError> {
    let world_tiles = f64::from(TileId::count(zoom)?);
    Ok(EQUATOR_CIRCUMFERENCE_METERS * latitude.to_radians().cos() / world_tiles)
}

#[derive(Debug, Error, PartialEq)]
pub enum GeoError {
    #[error("coordinate is not finite")]
    NonFiniteCoordinate,
    #[error("longitude out of range: {0}")]
    LongitudeOutOfRange(f64),
    #[error("latitude out of range: {0}")]
    LatitudeOutOfRange(f64),
    #[error("zoom out of range: {0}")]
    ZoomOutOfRange(u8),
    #[error("tile out of range: z={zoom} x={x} y={y}")]
    TileOutOfRange { zoom: u8, x: u32, y: u32 },
}

#[cfg(test)]
mod tests {
    use super::{GeoError, GeoPoint, TileId};

    #[test]
    fn origin_tile_at_zoom_zero() {
        assert_eq!(
            GeoPoint::new(0.0, 0.0).tile(0).unwrap(),
            TileId {
                zoom: 0,
                x: 0,
                y: 0,
            }
        );
    }

    #[test]
    fn origin_tile_at_zoom_one() {
        assert_eq!(
            GeoPoint::new(0.0, 0.0).tile(1).unwrap(),
            TileId {
                zoom: 1,
                x: 1,
                y: 1,
            }
        );
    }

    #[test]
    fn bali_tile_is_in_expected_range() {
        let tile = GeoPoint::new(115.225_287_207_233_22, -8.648_393_442_650_265)
            .tile(15)
            .unwrap();
        assert_eq!(tile.zoom, 15);
        assert!(tile.x < 1 << 15);
        assert!(tile.y < 1 << 15);
    }

    #[test]
    fn invalid_coordinates_are_rejected() {
        assert_eq!(
            GeoPoint::new(181.0, 0.0).validate(),
            Err(GeoError::LongitudeOutOfRange(181.0))
        );
        assert_eq!(
            GeoPoint::new(0.0, -91.0).validate(),
            Err(GeoError::LatitudeOutOfRange(-91.0))
        );
    }

    #[test]
    fn mercator_origin_is_zero() {
        let point = GeoPoint::new(0.0, 0.0).mercator().unwrap();
        assert!(point.x.abs() < 1.0e-9);
        assert!(point.y.abs() < 1.0e-9);
    }

    #[test]
    fn tile_fraction_round_trips() {
        let point = GeoPoint::new(115.225_287_207_233_22, -8.648_393_442_650_265);
        let (x, y) = point.tile_fraction(15).unwrap();
        let round_trip = GeoPoint::from_tile_fraction(15, x, y).unwrap();
        assert!((round_trip.longitude - point.longitude).abs() < 1.0e-9);
        assert!((round_trip.latitude - point.latitude).abs() < 1.0e-9);
    }

    #[test]
    fn tile_children_are_contiguous() {
        let tile = TileId::new(10, 100, 200).unwrap();
        let children = tile.children().unwrap();
        assert_eq!(children[0], TileId::new(11, 200, 400).unwrap());
        assert_eq!(children[3], TileId::new(11, 201, 401).unwrap());
        assert_eq!(children[0].parent(), Some(tile));
    }

    #[test]
    fn local_offset_is_zero_at_origin() {
        let point = GeoPoint::new(115.0, -8.0);
        let offset = point.local_offset(point).unwrap();
        assert!(offset.east_meters.abs() < 1.0e-6);
        assert!(offset.north_meters.abs() < 1.0e-6);
    }

    #[test]
    fn local_offset_round_trips() {
        let origin = GeoPoint::new(115.225, -8.648);
        let target = GeoPoint::new(115.226, -8.649);
        let offset = target.local_offset(origin).unwrap();
        let round_trip =
            GeoPoint::from_local_offset(origin, offset.east_meters, offset.north_meters).unwrap();
        assert!((round_trip.longitude - target.longitude).abs() < 1.0e-9);
        assert!((round_trip.latitude - target.latitude).abs() < 1.0e-9);
    }
}
