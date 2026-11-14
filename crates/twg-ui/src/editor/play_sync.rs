//! Realtime snapshots for the play-only screen (Simulasi → play).
//!
//! Editor Play sessions never build or apply these snapshots; only the
//! play-only screen (opened from Simulasi) syncs its live state with other
//! clients through the `twg.ws.v1` `play` messages.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use twg_map::GeoPoint;
use uuid::Uuid;

use super::model::{EditorKind, EditorState, ObjectKey, PlaybackState};

impl EditorState {
    /// Compact snapshot of the live Play state. Units are keyed by their
    /// server marker UUID so every client can resolve them locally.
    pub fn play_snapshot(&self) -> Value {
        let mut units = Vec::new();
        for (key, unit) in &self.play.units {
            let Some(id) = self.marker_server_id(*key) else {
                continue;
            };
            units.push(json!({
                "marker_id": id.to_string(),
                "lon": unit.position.longitude,
                "lat": unit.position.latitude,
                "heading": unit.heading_deg,
                "altitude": unit.altitude,
                "target": unit.target.map(|target| [target.longitude, target.latitude]),
                "limited": false,
            }));
        }
        for (key, unit) in &self.play.limited {
            let Some(id) = self.marker_server_id(*key) else {
                continue;
            };
            units.push(json!({
                "marker_id": id.to_string(),
                "lon": unit.position.longitude,
                "lat": unit.position.latitude,
                "heading": unit.heading_deg,
                "altitude": unit.altitude,
                "target": Value::Null,
                "limited": true,
            }));
        }
        let routes = self
            .play
            .animations
            .iter()
            .filter_map(|animation| {
                let id = self.object(animation.route_key)?.server_id?;
                Some(json!({
                    "line_id": id.to_string(),
                    "state": playback_key(animation.state),
                    "progress_m": animation.progress_m,
                }))
            })
            .collect::<Vec<_>>();
        let groups = self
            .play
            .groups
            .iter()
            .filter_map(|group| {
                let id = self.object(group.group_key)?.server_id?;
                Some(json!({
                    "line_id": id.to_string(),
                    "state": playback_key(group.state),
                    "step": group.step,
                    "elapsed": group.elapsed,
                    "step_elapsed": group.step_elapsed,
                    "next_at": group.next_at,
                }))
            })
            .collect::<Vec<_>>();
        // Hidden markers/particles travel as server UUIDs too. The receiver
        // resolves them against its own scene, so spawn/hide animations look
        // identical on every client and on the big screen.
        let hidden = self
            .play
            .hidden
            .iter()
            .filter_map(|key| self.object(*key).and_then(|object| object.server_id))
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        json!({
            "units": units,
            "routes": routes,
            "groups": groups,
            "hidden": hidden,
        })
    }

    /// Applies a snapshot produced by [`Self::play_snapshot`] on another
    /// client. Unknown markers or routes are ignored.
    pub fn apply_play_snapshot(&mut self, snapshot: &Value) {
        if !self.play.active {
            return;
        }
        if let Some(routes) = snapshot.get("routes").and_then(Value::as_array) {
            for route in routes {
                let Some(line_id) = route
                    .get("line_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                else {
                    continue;
                };
                let Some(index) = self.play.animations.iter().position(|animation| {
                    self.object(animation.route_key)
                        .is_some_and(|object| object.server_id == Some(line_id))
                }) else {
                    continue;
                };
                let progress = route
                    .get("progress_m")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let state = match route.get("state").and_then(Value::as_str) {
                    Some("playing") => PlaybackState::Playing,
                    Some("paused") => PlaybackState::Paused,
                    _ => PlaybackState::Stopped,
                };
                if let Some(animation) = self.play.animations.get_mut(index) {
                    animation.progress_m = progress.clamp(0.0, animation.length_m.max(0.0));
                    animation.state = state;
                }
                self.apply_route_animation(index);
            }
            self.sync_play_controlled();
        }
        if let Some(groups) = snapshot.get("groups").and_then(Value::as_array) {
            for group in groups {
                let Some(line_id) = group
                    .get("line_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                else {
                    continue;
                };
                let Some(index) = self.play.groups.iter().position(|animation| {
                    self.object(animation.group_key)
                        .is_some_and(|object| object.server_id == Some(line_id))
                }) else {
                    continue;
                };
                let state = match group.get("state").and_then(Value::as_str) {
                    Some("playing") => PlaybackState::Playing,
                    Some("paused") => PlaybackState::Paused,
                    _ => PlaybackState::Stopped,
                };
                let step = group.get("step").and_then(Value::as_u64).unwrap_or(0) as usize;
                let elapsed = group
                    .get("elapsed")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
                    .max(0.0);
                let step_elapsed = group
                    .get("step_elapsed")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
                    .max(0.0);
                let next_at = group
                    .get("next_at")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
                    .max(0.0);
                if let Some(local) = self.play.groups.get_mut(index) {
                    local.state = state;
                    local.step = step.min(local.members.len());
                    local.elapsed = elapsed;
                    local.step_elapsed = step_elapsed;
                    local.next_at = next_at;
                }
            }
        }
        if let Some(hidden) = snapshot.get("hidden").and_then(Value::as_array) {
            let targets = self.object_keys_by_server_id();
            let mut keys = BTreeSet::new();
            for id in hidden {
                let Some(id) = id.as_str().and_then(|value| Uuid::parse_str(value).ok()) else {
                    continue;
                };
                if let Some(key) = targets.get(&id) {
                    keys.insert(*key);
                }
            }
            if keys != self.play.hidden {
                self.play.hidden = keys;
                self.selected.retain(|key| !self.play.hidden.contains(key));
                // Visibility is part of the render scene; rebuild once when
                // the remote state actually changes.
                self.touch();
            }
        }
        let Some(units) = snapshot.get("units").and_then(Value::as_array) else {
            return;
        };
        let markers = self.marker_keys_by_server_id();
        for unit in units {
            let Some(marker_id) = unit
                .get("marker_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
            else {
                continue;
            };
            let Some(key) = markers.get(&marker_id).copied() else {
                continue;
            };
            let Some(lon) = unit.get("lon").and_then(Value::as_f64) else {
                continue;
            };
            let Some(lat) = unit.get("lat").and_then(Value::as_f64) else {
                continue;
            };
            let position = GeoPoint::new(lon, lat);
            let heading = unit.get("heading").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let altitude = unit.get("altitude").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let target = unit
                .get("target")
                .and_then(Value::as_array)
                .and_then(|values| {
                    Some(GeoPoint::new(
                        values.first()?.as_f64()?,
                        values.get(1)?.as_f64()?,
                    ))
                });
            let limited = unit
                .get("limited")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if limited {
                if let Some(local) = self.play.limited.get_mut(&key) {
                    local.position = position;
                    local.heading_deg = heading;
                    local.altitude = altitude;
                }
            } else if let Some(local) = self.play.units.get_mut(&key) {
                local.position = position;
                local.heading_deg = heading;
                local.altitude = altitude;
                local.target = target;
            }
            for obstacle in &mut self.play.obstacles {
                if obstacle.key == Some(key) {
                    obstacle.center = position;
                }
            }
        }
    }

    fn marker_server_id(&self, key: ObjectKey) -> Option<Uuid> {
        self.object(key).and_then(|object| object.server_id)
    }

    fn marker_keys_by_server_id(&self) -> BTreeMap<Uuid, ObjectKey> {
        self.objects
            .iter()
            .filter_map(|object| match &object.kind {
                EditorKind::Marker(_) => object.server_id.map(|id| (id, object.key)),
                _ => None,
            })
            .collect()
    }

    /// Markers and particles share one lookup for visibility snapshots.
    fn object_keys_by_server_id(&self) -> BTreeMap<Uuid, ObjectKey> {
        self.objects
            .iter()
            .filter_map(|object| match &object.kind {
                EditorKind::Marker(_) | EditorKind::Particle(_) => {
                    object.server_id.map(|id| (id, object.key))
                }
                _ => None,
            })
            .collect()
    }
}

fn playback_key(state: PlaybackState) -> &'static str {
    match state {
        PlaybackState::Stopped => "stopped",
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
    }
}

#[cfg(test)]
mod tests {
    use twg_map::GeoPoint;
    use uuid::Uuid;

    use super::super::model::{
        EditorKind, EditorObject, EditorState, GroupAnimationKind, LineObject, MarkerObject,
        PLAY_FORMATION_SPACING, PlayGroupAnimation, PlayGroupMember, PlayRouteAnimation, PlayUnit,
        PlaybackState, SequenceAdvance,
    };

    fn marker(id: Uuid, point: GeoPoint) -> EditorObject {
        EditorObject {
            key: 0,
            server_id: Some(id),
            name: "Marker".to_owned(),
            visible: true,
            dirty: false,
            kind: EditorKind::Marker(MarkerObject::new(0, point)),
        }
    }

    fn route(id: Uuid, key: u64) -> EditorObject {
        EditorObject {
            key,
            server_id: Some(id),
            name: "Rute".to_owned(),
            visible: true,
            dirty: false,
            kind: EditorKind::Line(LineObject::new(vec![[115.225, -8.648], [115.226, -8.648]])),
        }
    }

    fn unit(position: GeoPoint, target: Option<GeoPoint>) -> PlayUnit {
        PlayUnit {
            position,
            target,
            heading_deg: 90.0,
            speed: 5.0,
            radius: 1.0,
            obstacle: false,
            altitude: 0.0,
            animated: true,
        }
    }

    fn group(group_key: u64) -> PlayGroupAnimation {
        PlayGroupAnimation {
            group_key,
            name: "Sequence".to_owned(),
            kind: GroupAnimationKind::Sequence,
            members: vec![PlayGroupMember::Visibility(0)],
            missing: 0,
            advance: SequenceAdvance::Delay,
            delay_seconds: 2.0,
            state: PlaybackState::Stopped,
            step: 0,
            elapsed: 0.0,
            step_elapsed: 0.0,
            next_at: 0.0,
            order: 0,
        }
    }

    fn animation(route_key: u64) -> PlayRouteAnimation {
        PlayRouteAnimation {
            route_key,
            name: "Rute".to_owned(),
            marker_keys: Vec::new(),
            formation: super::super::model::RouteFormation::Grid,
            columns: 4,
            spacing: PLAY_FORMATION_SPACING,
            points: vec![
                GeoPoint::new(115.225, -8.648),
                GeoPoint::new(115.226, -8.648),
            ],
            altitudes: vec![0.0, 0.0],
            speed: 5.0,
            length_m: 100.0,
            progress_m: 0.0,
            state: PlaybackState::Stopped,
            repeat: false,
            order: 0,
        }
    }

    #[test]
    fn play_snapshot_round_trips_units_and_routes() {
        let marker_id = Uuid::new_v4();
        let line_id = Uuid::new_v4();
        let start = GeoPoint::new(115.225, -8.648);
        let moved = GeoPoint::new(115.2255, -8.6481);
        let target = GeoPoint::new(115.226, -8.649);

        let mut source = EditorState::new();
        source.play.active = true;
        source.objects.push(marker(marker_id, start));
        source.objects.push(route(line_id, 1));
        source.play.units.insert(0, unit(moved, Some(target)));
        source.play.animations.push(animation(1));
        source.play.animations[0].state = PlaybackState::Playing;
        source.play.animations[0].progress_m = 12.5;

        let snapshot = source.play_snapshot();

        let mut receiver = EditorState::new();
        receiver.play.active = true;
        receiver.objects.push(marker(marker_id, start));
        receiver.objects.push(route(line_id, 1));
        receiver.play.units.insert(0, unit(start, None));
        receiver.play.animations.push(animation(1));

        receiver.apply_play_snapshot(&snapshot);

        let unit = &receiver.play.units[&0];
        assert!((unit.position.longitude - moved.longitude).abs() < 1.0e-9);
        assert!((unit.position.latitude - moved.latitude).abs() < 1.0e-9);
        assert_eq!(unit.target, Some(target));
        let animation = &receiver.play.animations[0];
        assert_eq!(animation.state, PlaybackState::Playing);
        assert!((animation.progress_m - 12.5).abs() < 1.0e-9);
    }

    #[test]
    fn play_snapshot_round_trips_group_progress() {
        let group_id = Uuid::new_v4();

        let mut source = EditorState::new();
        source.play.active = true;
        source.objects.push(route(group_id, 2));
        source.play.groups.push(group(2));
        source.play.groups[0].state = PlaybackState::Playing;
        source.play.groups[0].step = 1;
        source.play.groups[0].elapsed = 2.5;
        source.play.groups[0].step_elapsed = 0.5;
        source.play.groups[0].next_at = 4.0;

        let snapshot = source.play_snapshot();

        let mut receiver = EditorState::new();
        receiver.play.active = true;
        receiver.objects.push(route(group_id, 2));
        receiver.play.groups.push(group(2));
        receiver.apply_play_snapshot(&snapshot);

        let group = &receiver.play.groups[0];
        assert_eq!(group.state, PlaybackState::Playing);
        assert_eq!(group.step, 1);
        assert!((group.elapsed - 2.5).abs() < 1.0e-9);
        assert!((group.step_elapsed - 0.5).abs() < 1.0e-9);
        assert!((group.next_at - 4.0).abs() < 1.0e-9);
    }

    #[test]
    fn play_snapshot_round_trips_hidden_targets() {
        let marker_id = Uuid::new_v4();
        let start = GeoPoint::new(115.225, -8.648);

        let mut source = EditorState::new();
        source.play.active = true;
        source.objects.push(marker(marker_id, start));
        source.play.hidden.insert(0);

        let snapshot = source.play_snapshot();

        let mut receiver = EditorState::new();
        receiver.play.active = true;
        receiver.objects.push(marker(marker_id, start));
        receiver.apply_play_snapshot(&snapshot);
        assert!(receiver.play.hidden.contains(&0));

        // A later snapshot that reveals the marker clears the flag again.
        source.play.hidden.clear();
        receiver.apply_play_snapshot(&source.play_snapshot());
        assert!(receiver.play.hidden.is_empty());
    }
}
