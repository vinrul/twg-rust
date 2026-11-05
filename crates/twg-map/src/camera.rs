use crate::geo::{EQUATOR_CIRCUMFERENCE_METERS, GeoPoint, TileId, meters_per_tile};

pub const DEFAULT_MAP_CENTER: GeoPoint =
    GeoPoint::new(115.225_287_207_233_22, -8.648_393_442_650_265);
pub const MIN_ZOOM: f32 = 2.0;
pub const MAX_ZOOM: f32 = 26.0;
pub const DEFAULT_TILE_MAX_ZOOM: u8 = 19;
pub const MIN_PITCH_DEGREES: f32 = 0.0;
pub const MAX_PITCH_DEGREES: f32 = 85.0;
pub const WEB_MERCATOR_TILE_PIXELS: f64 = 256.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapCameraState {
    pub center: GeoPoint,
    pub zoom: f32,
    pub pitch_degrees: f32,
    pub bearing_degrees: f32,
}

impl Default for MapCameraState {
    fn default() -> Self {
        Self {
            center: DEFAULT_MAP_CENTER,
            zoom: 13.0,
            pitch_degrees: 35.0,
            bearing_degrees: 0.0,
        }
    }
}

impl MapCameraState {
    pub fn new(center: GeoPoint) -> Self {
        Self {
            center,
            ..Self::default()
        }
    }

    pub fn tile_zoom(&self) -> u8 {
        self.tile_zoom_for(DEFAULT_TILE_MAX_ZOOM)
    }

    pub fn tile_zoom_for(&self, max_zoom: u8) -> u8 {
        let rounded = self.zoom.round().clamp(MIN_ZOOM, MAX_ZOOM);
        rounded.min(f32::from(max_zoom.max(1))) as u8
    }

    pub fn tile_fraction(&self) -> (f64, f64) {
        self.tile_fraction_for(DEFAULT_TILE_MAX_ZOOM)
    }

    pub fn tile_fraction_for(&self, max_zoom: u8) -> (f64, f64) {
        self.center
            .tile_fraction(self.tile_zoom_for(max_zoom))
            .unwrap_or((0.0, 0.0))
    }

    pub fn tile_world_size(&self) -> f32 {
        self.tile_world_size_for(DEFAULT_TILE_MAX_ZOOM)
    }

    pub fn tile_world_size_for(&self, max_zoom: u8) -> f32 {
        let zoom = self.tile_zoom_for(max_zoom);
        meters_per_tile(self.center.latitude, zoom).unwrap_or(1.0) as f32
    }

    pub fn meters_per_pixel(&self) -> f64 {
        let latitude_scale = self.center.latitude.to_radians().cos().abs().max(1.0e-6);
        EQUATOR_CIRCUMFERENCE_METERS * latitude_scale
            / (WEB_MERCATOR_TILE_PIXELS * 2.0f64.powf(f64::from(self.zoom)))
    }

    pub fn ground_span_meters(&self, viewport_height: f32) -> f32 {
        if viewport_height <= 0.0 {
            return 0.0;
        }
        (f64::from(viewport_height) * self.meters_per_pixel()) as f32
    }

    pub fn distance_for_viewport(&self, viewport_height: f32, fov_y_radians: f32) -> f32 {
        let span = f64::from(self.ground_span_meters(viewport_height));
        let half_fov = f64::from(fov_y_radians) * 0.5;
        (span / (2.0 * half_fov.tan())).max(f64::from(MIN_CAMERA_DISTANCE)) as f32
    }

    pub fn cover_radius_for(
        &self,
        max_zoom: u8,
        viewport_height: f32,
        viewport_aspect: f32,
    ) -> u32 {
        let tile_size = f64::from(self.tile_world_size_for(max_zoom));
        if tile_size <= 0.0 {
            return 5;
        }
        let span = f64::from(self.ground_span_meters(viewport_height))
            * f64::from(viewport_aspect.max(1.0));
        let radius = (span / (2.0 * tile_size)).ceil() as u32 + 1;
        radius.clamp(1, 8)
    }

    pub fn pan_by_tiles(&mut self, east_tiles: f64, north_tiles: f64) {
        let zoom = self.tile_zoom();
        let Ok(count) = TileId::count(zoom) else {
            return;
        };
        let count = f64::from(count);
        let (x, y) = self.tile_fraction();
        let next_x = (x + east_tiles).rem_euclid(count);
        let next_y = (y - north_tiles).clamp(0.0, count);
        if let Ok(center) = GeoPoint::from_tile_fraction(zoom, next_x, next_y) {
            self.center = center;
        }
    }

    /// Pan by a screen-space delta measured in map tiles.
    ///
    /// `screen_east` points right on screen and `screen_north` points up. The
    /// map rotates around the camera, so screen axes must be rotated by the
    /// negative bearing to obtain world east/north. Using the positive bearing
    /// here reverses the diagonal directions at bearings other than 0/180.
    pub fn pan_by_screen_tiles(&mut self, screen_east: f64, screen_north: f64) {
        let bearing = f64::from(self.bearing_degrees.to_radians());
        let east = screen_east * bearing.cos() + screen_north * bearing.sin();
        let north = -screen_east * bearing.sin() + screen_north * bearing.cos();
        self.pan_by_tiles(east, north);
    }

    pub fn pan_from_screen_delta(
        &mut self,
        delta_x: f32,
        delta_y: f32,
        _viewport_height: f32,
        _fov_y_radians: f32,
    ) {
        let tile_zoom = self.tile_zoom();
        let tiles_per_pixel =
            2.0f64.powf(f64::from(tile_zoom) - f64::from(self.zoom)) / WEB_MERCATOR_TILE_PIXELS;
        let pitch_scale = 1.0 / f64::from(self.pitch_degrees.to_radians().cos()).max(0.2);
        let screen_east = -f64::from(delta_x) * tiles_per_pixel;
        let screen_north = f64::from(delta_y) * tiles_per_pixel * pitch_scale;
        self.pan_by_screen_tiles(screen_east, screen_north);
    }

    pub fn zoom_by(&mut self, delta: f32) {
        self.zoom = (self.zoom + delta).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    pub fn rotate_by(&mut self, delta_degrees: f32) {
        self.bearing_degrees = (self.bearing_degrees + delta_degrees).rem_euclid(360.0);
    }

    pub fn pitch_by(&mut self, delta_degrees: f32) {
        self.pitch_degrees =
            (self.pitch_degrees + delta_degrees).clamp(MIN_PITCH_DEGREES, MAX_PITCH_DEGREES);
    }

    /// Scale from Web Mercator meters to ground meters at the map center.
    ///
    /// Tile positions already use ground meters (`meters_per_tile` includes
    /// the cosine factor), so geo/world conversion must apply the same scale
    /// or markers drift against the raster when the map is panned.
    pub fn mercator_ground_scale(&self) -> f64 {
        self.center.latitude.to_radians().cos().abs().max(1.0e-6)
    }

    pub fn world_to_geo(&self, world_x: f32, world_z: f32) -> Option<GeoPoint> {
        let scale = self.mercator_ground_scale();
        GeoPoint::from_local_offset(
            self.center,
            f64::from(world_x) / scale,
            -f64::from(world_z) / scale,
        )
        .ok()
    }

    pub fn geo_to_world(&self, point: GeoPoint) -> Option<(f32, f32)> {
        let offset = point.local_offset(self.center).ok()?;
        let scale = self.mercator_ground_scale();
        Some((
            (offset.east_meters * scale) as f32,
            (-offset.north_meters * scale) as f32,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraFlight {
    pub from: MapCameraState,
    pub to: MapCameraState,
    pub elapsed: f32,
    pub duration: f32,
}

impl CameraFlight {
    pub fn new(from: MapCameraState, to: MapCameraState, duration: f32) -> Self {
        Self {
            from,
            to,
            elapsed: 0.0,
            duration: duration.max(0.0),
        }
    }

    pub fn tick(&mut self, delta_secs: f32) -> (MapCameraState, bool) {
        self.elapsed = (self.elapsed + delta_secs.max(0.0)).min(self.duration);
        let t = if self.duration > 0.0 {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let eased = t * t * (3.0 - 2.0 * t);
        let from = self.from;
        let to = self.to;

        let mut delta_lon = to.center.longitude - from.center.longitude;
        if delta_lon > 180.0 {
            delta_lon -= 360.0;
        } else if delta_lon < -180.0 {
            delta_lon += 360.0;
        }
        let longitude = (from.center.longitude + delta_lon * f64::from(eased) + 180.0)
            .rem_euclid(360.0)
            - 180.0;
        let bearing_delta = ((to.bearing_degrees - from.bearing_degrees + 540.0) % 360.0) - 180.0;

        let state = MapCameraState {
            center: GeoPoint::new(
                longitude,
                from.center.latitude
                    + (to.center.latitude - from.center.latitude) * f64::from(eased),
            ),
            zoom: from.zoom + (to.zoom - from.zoom) * eased,
            pitch_degrees: from.pitch_degrees + (to.pitch_degrees - from.pitch_degrees) * eased,
            bearing_degrees: (from.bearing_degrees + bearing_delta * eased).rem_euclid(360.0),
        };
        (state, t >= 1.0)
    }
}

const MIN_CAMERA_DISTANCE: f32 = 0.5;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MapCursor {
    pub position: Option<GeoPoint>,
}

#[cfg(test)]
mod tests {
    use super::{MAX_ZOOM, MIN_ZOOM, MapCameraState};
    use crate::geo::GeoPoint;

    #[test]
    fn world_center_maps_to_camera_center() {
        let camera = MapCameraState::default();
        let point = camera.world_to_geo(0.0, 0.0).unwrap();
        assert!((point.longitude - camera.center.longitude).abs() < 1.0e-9);
        assert!((point.latitude - camera.center.latitude).abs() < 1.0e-9);
    }

    #[test]
    fn geo_to_world_round_trips() {
        let camera = MapCameraState::default();
        let point = GeoPoint::new(115.23, -8.64);
        let (x, z) = camera.geo_to_world(point).unwrap();
        let round_trip = camera.world_to_geo(x, z).unwrap();
        assert!((round_trip.longitude - point.longitude).abs() < 1.0e-7);
        assert!((round_trip.latitude - point.latitude).abs() < 1.0e-7);
    }

    #[test]
    fn a_degree_of_longitude_is_about_110_km_ground_east_at_bali() {
        let camera = MapCameraState::default();
        let point = GeoPoint::new(camera.center.longitude + 1.0, camera.center.latitude);
        let (x, _) = camera.geo_to_world(point).unwrap();
        assert!((x - 110_050.0).abs() < 500.0, "world x was {x}");
    }

    #[test]
    fn geo_to_world_matches_tile_grid_scale() {
        let mut camera = MapCameraState::default();
        camera.set_zoom(19.0);
        let tile = camera.center.tile(19).unwrap();
        let center =
            GeoPoint::from_tile_fraction(19, f64::from(tile.x) + 0.5, f64::from(tile.y) + 0.5)
                .unwrap();
        let east =
            GeoPoint::from_tile_fraction(19, f64::from(tile.x) + 1.5, f64::from(tile.y) + 0.5)
                .unwrap();
        let (center_x, center_z) = camera.geo_to_world(center).unwrap();
        let (east_x, east_z) = camera.geo_to_world(east).unwrap();
        let tile_size = camera.tile_world_size_for(19);
        assert!((east_x - center_x - tile_size).abs() < 0.5);
        assert!((east_z - center_z).abs() < 0.01);
    }

    #[test]
    fn panning_north_decreases_tile_y() {
        let mut camera = MapCameraState::default();
        let before = camera.tile_fraction();
        camera.pan_by_tiles(0.0, 1.0);
        let after = camera.tile_fraction();
        assert!(after.1 < before.1);
    }

    #[test]
    fn pan_at_bearing_90_follows_screen_axes() {
        let mut camera = MapCameraState {
            bearing_degrees: 90.0,
            ..Default::default()
        };

        // Dragging right moves the camera north (screen right is south).
        let before = camera.center;
        camera.pan_from_screen_delta(100.0, 0.0, 720.0, 60.0f32.to_radians());
        assert!(camera.center.latitude > before.latitude);

        // Dragging down moves the camera east (screen down is west).
        let before = camera.center;
        camera.pan_from_screen_delta(0.0, 100.0, 720.0, 60.0f32.to_radians());
        assert!(camera.center.longitude > before.longitude);
    }

    #[test]
    fn pan_at_bearing_45_can_move_forward_and_backward() {
        let mut camera = MapCameraState {
            bearing_degrees: 45.0,
            ..Default::default()
        };

        let before = camera.center;
        camera.pan_from_screen_delta(0.0, 100.0, 720.0, 60.0f32.to_radians());
        assert!(camera.center.latitude > before.latitude);
        assert!(camera.center.longitude > before.longitude);

        let forward = camera.center;
        camera.pan_from_screen_delta(0.0, -100.0, 720.0, 60.0f32.to_radians());
        assert!(camera.center.latitude < forward.latitude);
        assert!(camera.center.longitude < forward.longitude);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut camera = MapCameraState::default();
        camera.zoom_by(-100.0);
        assert_eq!(camera.zoom, MIN_ZOOM);
        camera.zoom_by(100.0);
        assert_eq!(camera.zoom, MAX_ZOOM);
    }

    #[test]
    fn overzoom_keeps_provider_tile_zoom() {
        let mut camera = MapCameraState::default();
        camera.set_zoom(23.0);
        assert_eq!(camera.tile_zoom_for(19), 19);
        assert_eq!(camera.tile_zoom_for(26), 23);
    }

    #[test]
    fn camera_zoom_26_never_requests_above_provider_zoom_19() {
        let mut camera = MapCameraState::default();
        for zoom in [19.0, 20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0] {
            camera.set_zoom(zoom);
            assert_eq!(camera.tile_zoom_for(19), 19);
            let native_size =
                crate::geo::meters_per_tile(camera.center.latitude, 19).unwrap() as f32;
            assert!((camera.tile_world_size_for(19) - native_size).abs() < 0.01);
        }
    }

    #[test]
    fn tile_size_at_zoom_19_is_about_76_meters() {
        let mut camera = MapCameraState::default();
        camera.set_zoom(19.0);
        let size = camera.tile_world_size_for(19);
        assert!((size - 76.4).abs() < 2.0, "tile size was {size}");
    }

    #[test]
    fn camera_distance_grows_as_we_zoom_out() {
        let mut camera = MapCameraState::default();
        camera.set_zoom(19.0);
        let near = camera.distance_for_viewport(1080.0, 60.0f32.to_radians());
        camera.set_zoom(13.0);
        let far = camera.distance_for_viewport(1080.0, 60.0f32.to_radians());
        assert!(far > near * 60.0);
    }

    #[test]
    fn camera_flight_starts_and_ends_at_the_requested_states() {
        let from = MapCameraState::default();
        let mut to = from;
        to.center = GeoPoint::new(115.25, -8.70);
        to.zoom = 21.0;
        to.bearing_degrees = 90.0;
        let mut flight = super::CameraFlight::new(from, to, 1.0);

        let (first, done) = flight.tick(0.0);
        assert!(!done);
        assert!((first.center.longitude - from.center.longitude).abs() < 1.0e-9);

        let (last, done) = flight.tick(1.0);
        assert!(done);
        assert!((last.center.longitude - to.center.longitude).abs() < 1.0e-9);
        assert!((last.center.latitude - to.center.latitude).abs() < 1.0e-9);
        assert!((last.zoom - to.zoom).abs() < 1.0e-6);
        assert!((last.bearing_degrees - 90.0).abs() < 1.0e-6);
    }

    #[test]
    fn camera_flight_takes_the_short_bearing_path() {
        let from = MapCameraState::default();
        let mut to = from;
        to.bearing_degrees = 350.0;
        let mut flight = super::CameraFlight::new(from, to, 1.0);
        let (mid, _) = flight.tick(0.5);
        assert!(mid.bearing_degrees > 300.0 || mid.bearing_degrees < 10.0);
        let (last, _) = flight.tick(0.5);
        assert!((last.bearing_degrees - 350.0).abs() < 1.0e-4);
    }

    #[test]
    fn pitch_is_clamped() {
        let mut camera = MapCameraState::default();
        camera.pitch_by(-100.0);
        assert_eq!(camera.pitch_degrees, super::MIN_PITCH_DEGREES);
        camera.pitch_by(100.0);
        assert_eq!(camera.pitch_degrees, super::MAX_PITCH_DEGREES);
    }
}
