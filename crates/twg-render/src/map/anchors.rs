use bevy::prelude::*;
use twg_map::{GeoPoint, MercatorPoint};

use super::MapCameraResource;

/// Keeps an entity at a fixed geographic position.
///
/// Tile quads are positioned relative to the camera center every frame. An
/// unanchored marker would stay at a fixed world coordinate and appear to float
/// away from the map when the user pans. Storing the projected Mercator point
/// makes the per-frame update a pair of subtractions instead of re-projecting
/// every marker.
#[derive(Component, Debug, Clone, Copy)]
pub struct GeoAnchor {
    pub mercator: MercatorPoint,
}

impl GeoAnchor {
    pub fn new(point: GeoPoint) -> Option<Self> {
        point.mercator().ok().map(|mercator| Self { mercator })
    }
}

pub fn update_geo_anchors(
    state: Res<MapCameraResource>,
    mut last_center: Local<Option<GeoPoint>>,
    anchors: Query<(&GeoAnchor, &mut Transform)>,
    added: Query<(), Added<GeoAnchor>>,
) {
    // Panning is the only operation that moves existing anchors. New markers
    // also need one pass in case they spawned after the input system moved the
    // camera in the same frame. Skipping idle frames keeps the 10.000-marker
    // benchmark from paying for a no-op query.
    let center_changed = last_center.as_ref() != Some(&state.0.center);
    if !center_changed && added.is_empty() {
        return;
    }
    *last_center = Some(state.0.center);

    let Ok(center) = state.0.center.mercator() else {
        return;
    };
    let scale = state.0.mercator_ground_scale();
    for (anchor, mut transform) in anchors {
        let x = ((anchor.mercator.x - center.x) * scale) as f32;
        let z = (-(anchor.mercator.y - center.y) * scale) as f32;
        if (transform.translation.x - x).abs() > 0.001
            || (transform.translation.z - z).abs() > 0.001
        {
            transform.translation.x = x;
            transform.translation.z = z;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GeoAnchor;
    use twg_map::{GeoPoint, MapCameraState};

    #[test]
    fn anchor_stays_at_the_same_geo_point_when_the_map_pans() {
        let mut camera = MapCameraState::default();
        camera.set_zoom(19.0);
        let point = GeoPoint::from_local_offset(camera.center, 120.0, -80.0).unwrap();
        let anchor = GeoAnchor::new(point).unwrap();

        let before = camera.geo_to_world(point).unwrap();
        camera.pan_by_tiles(0.25, 0.1);
        let after = camera.geo_to_world(point).unwrap();

        assert!((before.0 - after.0).abs() > 1.0);
        assert!((before.1 - after.1).abs() > 1.0);
        assert_eq!(anchor.mercator, point.mercator().unwrap());
    }

    #[test]
    fn anchor_center_is_world_origin() {
        let camera = MapCameraState::default();
        let anchor = GeoAnchor::new(camera.center).unwrap();
        let (x, z) = camera.geo_to_world(camera.center).unwrap();
        assert_eq!(anchor.mercator, camera.center.mercator().unwrap());
        assert!(x.abs() < 1.0e-4);
        assert!(z.abs() < 1.0e-4);
    }
}
