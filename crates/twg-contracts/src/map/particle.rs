use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operations::Operation;
use crate::scenarios::Scenario;
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Particle {
    pub id: Uuid,
    pub name: String,
    pub operasi: Operation,
    pub skenario: Scenario,
    pub tipe: String,
    pub data: String,
    pub pos_x: f64,
    pub pos_y: f64,
    #[serde(default)]
    pub altitude: f64,
    pub is_spawn: i32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// The particle types the scenario editor can create. Stored as the `tipe`
/// string so rows written by the legacy server stay readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParticleKind {
    Fire,
    Smoke,
    Water,
}

impl ParticleKind {
    pub const ALL: [Self; 3] = [Self::Fire, Self::Smoke, Self::Water];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Fire => "fire",
            Self::Smoke => "smoke",
            Self::Water => "water",
        }
    }

    /// Maps a stored `tipe` to one of the supported kinds. Unknown or legacy
    /// spellings fall back to `fire`, which is also the column default.
    pub fn from_key(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "smoke" | "asap" => Self::Smoke,
            "water" | "air" | "air_mancur" | "air mancur" => Self::Water,
            _ => Self::Fire,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleCreateRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub tipe: String,
    #[serde(default)]
    pub data: String,
    pub pos_x: f64,
    pub pos_y: f64,
    #[serde(default)]
    pub altitude: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleUpdateRequest {
    pub name: String,
    pub tipe: String,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleGeomUpdateRequest {
    pub pos_x: f64,
    pub pos_y: f64,
    #[serde(default)]
    pub altitude: f64,
}

/// Payload of the legacy `/private/anims/spawn_particle` endpoint, which
/// creates an animation alongside the particle row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnParticleRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    #[serde(default)]
    pub data: String,
    pub tipe: String,
    pub is_spawn: String,
    pub pos_x: f64,
    pub pos_y: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_kind_accepts_indonesian_aliases() {
        assert_eq!(ParticleKind::from_key("api"), ParticleKind::Fire);
        assert_eq!(ParticleKind::from_key("asap"), ParticleKind::Smoke);
        assert_eq!(ParticleKind::from_key("Air Mancur"), ParticleKind::Water);
        assert_eq!(ParticleKind::from_key("unknown"), ParticleKind::Fire);
    }

    #[test]
    fn particle_kind_keys_are_stable() {
        assert_eq!(ParticleKind::Fire.key(), "fire");
        assert_eq!(ParticleKind::Smoke.key(), "smoke");
        assert_eq!(ParticleKind::Water.key(), "water");
    }
}
