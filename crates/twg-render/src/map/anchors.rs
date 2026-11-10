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

#[allow(clippy::type_complexity)]
pub fn update_geo_anchors(
    state: Res<MapCameraResource>,
    mut last_center: Local<Option<GeoPoint>>,
    mut anchors: ParamSet<(
        Query<(&GeoAnchor, &mut Transform)>,
        Query<(&GeoAnchor, &mut Transform), Changed<GeoAnchor>>,
    )>,
) {
    // Panning is the only camera operation that moves every anchor. When the
    // camera is idle only anchors that changed themselves (RTS units moving or
    // newly spawned markers) are recomputed, so a scenario with thousands of
    // static markers does not pay a full pass every frame.
    let center_changed = last_center.as_ref() != Some(&state.0.center);
    let Ok(center) = state.0.center.mercator() else {
        return;
    };
    let scale = state.0.mercator_ground_scale();
    if center_changed {
        *last_center = Some(state.0.center);
        for (anchor, mut transform) in &mut anchors.p0() {
            place_anchor(anchor, &mut transform, center, scale);
        }
        return;
    }
    let mut changed = anchors.p1();
    if changed.is_empty() {
        return;
    }
    for (anchor, mut transform) in &mut changed {
        place_anchor(anchor, &mut transform, center, scale);
    }
}

fn place_anchor(anchor: &GeoAnchor, transform: &mut Transform, center: MercatorPoint, scale: f64) {
    let x = ((anchor.mercator.x - center.x) * scale) as f32;
    let z = (-(anchor.mercator.y - center.y) * scale) as f32;
    if (transform.translation.x - x).abs() > 0.001 || (transform.translation.z - z).abs() > 0.001 {
        transform.translation.x = x;
        transform.translation.z = z;
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
