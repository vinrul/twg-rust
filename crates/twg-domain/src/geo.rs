use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoCoordinate {
    pub longitude: f64,
    pub latitude: f64,
    pub altitude: f64,
}

impl GeoCoordinate {
    pub const fn new(longitude: f64, latitude: f64, altitude: f64) -> Self {
        Self {
            longitude,
            latitude,
            altitude,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rotation3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Rotation3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scale3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Scale3 {
    pub const fn uniform(value: f64) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorHex(String);

impl ColorHex {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
