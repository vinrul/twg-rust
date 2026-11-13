use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use bevy::prelude::Vec3;
use bevy_egui::egui;
use twg_contracts::assets::{AssetCategory, AssetModelKind, AssetMoveMode, AssetSourceSlot};
use twg_contracts::map::ParticleKind;
use twg_map::{BaseLayer, GeoPoint, MapCameraState};
use twg_render::{
    EditorGizmo, EditorGizmoAxis, EditorGizmoMode, EditorGrid, EditorModelShading,
    EditorParticleKind, EditorParticleSettings, EditorPlayModel, EditorScene, EditorSceneKind,
    EditorSceneObject, EditorShapeKind, SkyMode,
};
use uuid::Uuid;

use super::textures;

pub type ObjectKey = u64;

#[derive(Clone, Copy, Debug)]
pub struct UnitEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub model: &'static str,
    pub category: &'static str,
    pub base_scale: f32,
}

/// Local unit catalogue. The server stores only `unit_id`, so these UUIDs are
/// stable identifiers that also match the baseline fixture.
pub const UNIT_CATALOG: [UnitEntry; 4] = [
    UnitEntry {
        id: "abababab-abab-4bab-8bab-abababababab",
        name: "Polwan Airud",
        model: "models/polwan_airud_idle_lod2.glb",
        category: "personel",
        base_scale: 0.02,
    },
    UnitEntry {
        id: "cdcdcdcd-cdcd-4dcd-8dcd-cdcdcdcdcdcd",
        name: "Motor Patroli",
        model: "models/motor_patroli_brimob_anim_1.glb",
        category: "kendaraan",
        base_scale: 0.006,
    },
    UnitEntry {
        id: "efefefef-efef-4fef-8fef-efefefefefef",
        name: "Pos Komando",
        model: "models/polwan_airud_idle_lod2.glb",
        category: "markas",
        base_scale: 0.035,
    },
    UnitEntry {
        id: "12121212-1212-4212-8212-121212121212",
        name: "Personel",
        model: "models/polwan_airud_idle_lod2.glb",
        category: "personel",
        base_scale: 0.02,
    },
];

pub fn unit_for_uuid(id: &str) -> usize {
    UNIT_CATALOG
        .iter()
        .position(|unit| unit.id == id)
        .unwrap_or(0)
}

/// Model file of a server asset, resolved from the offline cache. The asset UUID
/// is what gets persisted in `markers.unit_id`, so a marker keeps its model
/// across save/reload without changing the server contract.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkerAssetModel {
    pub name: String,
    /// Bevy asset path, e.g. `twg-cache://<id>/static/original.glb`.
    pub path: String,
    pub base_scale: f32,
    /// Half diagonal of the model's XZ bounds, in model units before
    /// `base_scale`. Used for Play collision radius.
    pub footprint_radius: f32,
    /// The file uses Draco compression and must skip glTF validation.
    pub draco: bool,
    /// The asset was uploaded as an animated model (idle + animation slots).
    pub animated: bool,
    /// Play move behaviour configured on the asset.
    pub move_mode: AssetMoveMode,
    /// Cached path of the animated file, used in Play mode while moving.
    pub animation_path: Option<String>,
    pub animation_draco: bool,
}

/// One row of the editor's Assets tab (server catalogue, models only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorAssetRow {
    /// Asset UUID as a string; becomes `unit_id` when the marker is created.
    pub unit_id: String,
    pub name: String,
    pub kind_label: String,
    /// The model file exists in the offline cache and can be rendered.
    pub cached: bool,
    /// Semantic asset category shown as the tree root.
    pub group: String,
    pub move_mode_label: String,
    pub version: i64,
    pub size_label: String,
    /// True for assets uploaded from the scenario editor's Import tab.
    pub imported: bool,
    /// Models can be dropped on the map; images only feed shape materials.
    pub is_model: bool,
    /// The current user may delete this import (uploader or admin).
    pub can_delete: bool,
    /// Latest saved preview screenshot, when one exists.
    pub preview: Option<PathBuf>,
}

/// One image asset available as a shape texture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorTextureRow {
    pub id: String,
    pub name: String,
    /// A local cache path or server URL is available.
    pub available: bool,
    /// True for images uploaded through the scenario editor's Import tab.
    pub imported: bool,
}

/// Upload form of the Import tab. Files are stored in slot order: `[static]`
/// for static models and images, `[idle, animated]` for animated models.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportDraft {
    pub name: String,
    pub category: AssetCategory,
    pub model_kind: AssetModelKind,
    /// Only used by static models; animated models always move freely.
    pub move_mode: AssetMoveMode,
    pub files: Vec<PathBuf>,
}

impl Default for ImportDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            category: AssetCategory::Model,
            model_kind: AssetModelKind::Static,
            move_mode: AssetMoveMode::None,
            files: Vec::new(),
        }
    }
}

impl ImportDraft {
    pub fn slots(&self) -> &'static [AssetSourceSlot] {
        if self.category.is_model() {
            self.model_kind.slots()
        } else {
            &[AssetSourceSlot::Static]
        }
    }

    /// File chosen for one required slot.
    pub fn file(&self, index: usize) -> Option<&Path> {
        self.files
            .get(index)
            .filter(|path| !path.as_os_str().is_empty())
            .map(PathBuf::as_path)
    }

    /// Assigns a picked file to one slot, padding earlier slots when needed.
    pub fn set_file(&mut self, index: usize, path: PathBuf) {
        if self.name.trim().is_empty()
            && let Some(stem) = path.file_stem().and_then(|value| value.to_str())
        {
            self.name = stem.to_owned();
        }
        if self.files.len() <= index {
            self.files.resize(index + 1, PathBuf::new());
        }
        self.files[index] = path;
    }

    /// Drops files that were selected for a different category/model kind.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Asset dragged from the Assets tab onto the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetMarkerDraft {
    pub unit_id: String,
    pub name: String,
}

/// Height in meters server models are normalised to when added as markers.
pub const ASSET_MARKER_HEIGHT: f32 = 2.0;

/// Smallest marker scale the editor accepts (Inspector and gizmo).
pub const MIN_MARKER_SCALE: f64 = 0.001;

/// RTS play mode tuning.
const PLAY_ARRIVE_METERS: f64 = 0.6;
const PLAY_FORMATION_SPACING: f64 = 2.6;
const PLAY_LOOKAHEAD_METERS: f64 = 6.0;

/// Runtime state of one movable unit in Play mode. Positions live here (not in
/// the marker object) so playing never dirties or saves the scenario.
#[derive(Debug, Clone, Copy)]
pub struct PlayUnit {
    pub position: GeoPoint,
    pub target: Option<GeoPoint>,
    /// Bearing in degrees clockwise from north.
    pub heading_deg: f32,
    /// Metres per second.
    pub speed: f64,
    /// Radius used for the selection ring on the ground.
    pub radius: f64,
    /// When false, other units can pass through this unit.
    pub obstacle: bool,
    /// Height the unit keeps while moving.
    pub altitude: f32,
    /// Animated units use their walk clip. Static movable units only turn
    /// toward their movement direction.
    pub animated: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayObstacle {
    pub center: GeoPoint,
    pub radius: f64,
    /// Set when the obstacle is a movable `Limited` asset, so its obstacle
    /// circle can follow the gizmo.
    pub key: Option<ObjectKey>,
}

/// A static asset with `Limited` move mode. It does not receive move orders;
/// the player drags it with the 3D gizmo instead.
#[derive(Debug, Clone, Copy)]
pub struct PlayLimitedUnit {
    pub position: GeoPoint,
    pub radius: f64,
    /// Runtime height, adjustable with the Y handle of the Play gizmo.
    pub altitude: f32,
    /// Bearing in degrees clockwise from north.
    pub heading_deg: f32,
}

/// One authored route animation resolved for Play mode.
#[derive(Debug, Clone)]
pub struct PlayRouteAnimation {
    pub route_key: ObjectKey,
    pub name: String,
    /// Marker object that follows the route, when it still exists.
    pub marker_key: Option<ObjectKey>,
    pub points: Vec<GeoPoint>,
    pub altitudes: Vec<f64>,
    /// Metres per second along the polyline.
    pub speed: f64,
    pub length_m: f64,
    pub progress_m: f64,
    pub state: PlaybackState,
    pub repeat: bool,
    /// Persisted play order shared with the scenario editor.
    pub order: i32,
}

impl PlayRouteAnimation {
    pub fn is_finished(&self) -> bool {
        self.length_m > 0.0 && self.progress_m >= self.length_m - 1.0e-6
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// RTS play mode state: movable animated markers plus static obstacles.
#[derive(Debug, Clone, Default)]
pub struct PlayState {
    pub active: bool,
    /// Units moved with an order (animated assets and `Free` static assets).
    pub units: BTreeMap<ObjectKey, PlayUnit>,
    /// Static assets with `Limited` move mode: selected and moved with a gizmo.
    pub limited: BTreeMap<ObjectKey, PlayLimitedUnit>,
    pub obstacles: Vec<PlayObstacle>,
    /// Authored route animations available in the Play panel.
    pub animations: Vec<PlayRouteAnimation>,
    /// Markers controlled by a route animation; manual orders are ignored.
    pub controlled: BTreeSet<ObjectKey>,
    /// Screen-space marquee while dragging on empty ground.
    pub marquee: Option<(egui::Pos2, egui::Pos2)>,
    /// Right button press position, used to tell a move click from an orbit drag.
    pub right_press: Option<egui::Pos2>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    Move,
    Rotate,
    Scale,
    Shape,
    Line,
    Text,
    Particle,
    Measure,
}

impl EditorTool {
    pub const ALL: [Self; 9] = [
        Self::Select,
        Self::Move,
        Self::Rotate,
        Self::Scale,
        Self::Shape,
        Self::Line,
        Self::Text,
        Self::Particle,
        Self::Measure,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Move => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::Shape => "Bentuk",
            Self::Line => "Line",
            Self::Text => "Text",
            Self::Particle => "Particle",
            Self::Measure => "Measure",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Select => "S",
            Self::Move => "M",
            Self::Rotate => "R",
            Self::Scale => "K",
            Self::Shape => "B",
            Self::Line => "L",
            Self::Text => "T",
            Self::Particle => "P",
            Self::Measure => "D",
        }
    }

    pub const fn shortcut(self) -> &'static str {
        match self {
            Self::Select => "Q",
            Self::Move => "W",
            Self::Rotate => "E",
            Self::Scale => "R",
            Self::Shape => "1",
            Self::Line => "2",
            Self::Text => "3",
            Self::Particle => "5",
            Self::Measure => "4",
        }
    }

    pub const fn changes_gizmo(self) -> bool {
        matches!(self, Self::Move | Self::Rotate | Self::Scale)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectType {
    Marker,
    Line,
    Building,
    Text,
    Particle,
}

impl ObjectType {
    pub const ALL: [Self; 5] = [
        Self::Marker,
        Self::Line,
        Self::Building,
        Self::Text,
        Self::Particle,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Marker => "Marker",
            Self::Line => "Line",
            Self::Building => "Bentuk 3D",
            Self::Text => "Text",
            Self::Particle => "Particle",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Marker => "M",
            Self::Line => "L",
            Self::Building => "B",
            Self::Text => "T",
            Self::Particle => "P",
        }
    }
}

#[derive(Clone, Debug)]
pub struct MarkerObject {
    pub unit: usize,
    pub unit_id: String,
    pub lon: f64,
    pub lat: f64,
    pub rot_x: f64,
    pub rot_y: f64,
    pub rot_z: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub scale_z: f64,
    pub altitude: f64,
    pub jumlah: i32,
    pub kategori: String,
    pub keterangan: String,
    /// When false, Play units can pass through this marker.
    pub obstacle: bool,
}

impl MarkerObject {
    pub fn new(unit: usize, point: GeoPoint) -> Self {
        let unit = unit.min(UNIT_CATALOG.len() - 1);
        let entry = UNIT_CATALOG[unit];
        Self {
            unit,
            unit_id: entry.id.to_owned(),
            lon: point.longitude,
            lat: point.latitude,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            scale_z: 1.0,
            altitude: 0.0,
            jumlah: 1,
            kategori: entry.category.to_owned(),
            keterangan: String::new(),
            obstacle: true,
        }
    }

    pub fn point(&self) -> GeoPoint {
        GeoPoint::new(self.lon, self.lat)
    }

    pub fn set_point(&mut self, point: GeoPoint) {
        self.lon = point.longitude;
        self.lat = point.latitude;
    }
}

#[derive(Clone, Debug)]
pub struct RouteAnimation {
    /// Metres per second along the polyline.
    pub speed: f64,
    /// Server id of the marker that follows the route.
    pub marker_id: Option<Uuid>,
    /// Restart from the first point after reaching the end.
    pub repeat: bool,
    /// Position in the Play animation order, persisted in the route metadata.
    pub order: i32,
}

impl Default for RouteAnimation {
    fn default() -> Self {
        Self {
            speed: 5.0,
            marker_id: None,
            repeat: false,
            order: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LineObject {
    pub points: Vec<[f64; 2]>,
    /// Height in meters for each point, parallel to `points`.
    pub altitudes: Vec<f64>,
    pub color: String,
    pub width: f64,
    pub alpha: f64,
    /// Set for lines that animate a marker when Play starts.
    pub animation: Option<RouteAnimation>,
}

impl LineObject {
    pub fn new(points: Vec<[f64; 2]>) -> Self {
        let altitudes = vec![0.0; points.len()];
        Self {
            points,
            altitudes,
            color: "#00b8ff".to_owned(),
            width: 4.0,
            alpha: 0.9,
            animation: None,
        }
    }

    pub fn altitude(&self, index: usize) -> f64 {
        self.altitudes.get(index).copied().unwrap_or(0.0)
    }
}

#[derive(Clone, Debug)]
pub struct BuildingObject {
    pub shape: EditorShapeKind,
    /// Centre of the footprint.
    pub lon: f64,
    pub lat: f64,
    pub altitude: f64,
    pub rot_x: f64,
    pub rot_y: f64,
    pub rot_z: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub scale_z: f64,
    pub color: String,
    pub use_texture: bool,
    /// Asset UUID of the image texture.
    pub texture: Option<String>,
    /// Footprint for `Custom`; empty for primitive shapes.
    pub points: Vec<[f64; 2]>,
    /// Extrusion height for `Custom` (primitives use `scale_y`).
    pub height: f64,
    pub keterangan: String,
    /// When false, Play units can pass through this shape.
    pub obstacle: bool,
}

impl BuildingObject {
    pub fn primitive(shape: EditorShapeKind, point: GeoPoint) -> Self {
        let (scale_x, scale_y, scale_z) = match shape {
            EditorShapeKind::Ground => (20.0, 1.0, 20.0),
            EditorShapeKind::Cone | EditorShapeKind::Sphere => (4.0, 4.0, 4.0),
            _ => (4.0, 3.0, 4.0),
        };
        Self {
            shape,
            lon: point.longitude,
            lat: point.latitude,
            altitude: 0.0,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            scale_x,
            scale_y,
            scale_z,
            color: "#ff9f43".to_owned(),
            use_texture: false,
            texture: None,
            points: Vec::new(),
            height: scale_y,
            keterangan: String::new(),
            obstacle: true,
        }
    }

    pub fn custom(points: Vec<GeoPoint>) -> Self {
        let center = centroid_of(&points).unwrap_or(GeoPoint::new(0.0, 0.0));
        Self {
            shape: EditorShapeKind::Custom,
            lon: center.longitude,
            lat: center.latitude,
            altitude: 0.0,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            scale_z: 1.0,
            color: "#ff9f43".to_owned(),
            use_texture: false,
            texture: None,
            points: points
                .iter()
                .map(|point| [point.longitude, point.latitude])
                .collect(),
            height: 12.0,
            keterangan: String::new(),
            obstacle: true,
        }
    }

    pub fn point(&self) -> GeoPoint {
        GeoPoint::new(self.lon, self.lat)
    }

    pub fn translate(&mut self, east_meters: f64, north_meters: f64) {
        if let Ok(point) = GeoPoint::from_local_offset(self.point(), east_meters, north_meters) {
            self.lon = point.longitude;
            self.lat = point.latitude;
        }
        offset_points(&mut self.points, east_meters, north_meters);
    }

    /// Places the footprint centre on `point`, keeping the local outline
    /// aligned with it.
    pub fn set_position(&mut self, point: GeoPoint) {
        let Ok(offset) = point.local_offset(self.point()) else {
            return;
        };
        self.lon = point.longitude;
        self.lat = point.latitude;
        offset_points(&mut self.points, offset.east_meters, offset.north_meters);
    }

    /// Outline used for picking, selection and focus. Primitive shapes use a
    /// square footprint derived from their scale.
    pub fn outline(&self) -> Vec<GeoPoint> {
        if self.shape == EditorShapeKind::Custom && !self.points.is_empty() {
            return self
                .points
                .iter()
                .map(|point| GeoPoint::new(point[0], point[1]))
                .collect();
        }
        let half_east = (self.scale_x.abs() * 0.5).max(0.5);
        let half_north = (self.scale_z.abs() * 0.5).max(0.5);
        let mut points = Vec::with_capacity(4);
        for (east, north) in [
            (-half_east, -half_north),
            (half_east, -half_north),
            (half_east, half_north),
            (-half_east, half_north),
        ] {
            if let Ok(point) = GeoPoint::from_local_offset(self.point(), east, north) {
                points.push(point);
            }
        }
        points
    }

    /// Height sent to the legacy building record.
    pub fn render_height(&self) -> f64 {
        if self.shape == EditorShapeKind::Custom {
            self.height
        } else {
            self.scale_y.abs().max(0.1)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectTransform {
    pub lon: f64,
    pub lat: f64,
    pub altitude: f64,
    pub rot: [f64; 3],
    pub scale: [f64; 3],
}

impl Default for ObjectTransform {
    fn default() -> Self {
        Self {
            lon: 0.0,
            lat: 0.0,
            altitude: 0.0,
            rot: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextObject {
    pub text: String,
    pub lon: f64,
    pub lat: f64,
    pub altitude: f64,
    pub color: String,
    pub font_size: f64,
    pub align: String,
    pub wrap_text: bool,
}

impl TextObject {
    pub fn point(&self) -> GeoPoint {
        GeoPoint::new(self.lon, self.lat)
    }

    pub fn set_point(&mut self, point: GeoPoint) {
        self.lon = point.longitude;
        self.lat = point.latitude;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleSettings {
    /// Overall puff/droplet size multiplier.
    pub size: f64,
    /// Number of puffs/droplets multiplier.
    pub density: f64,
    /// Flame plume height, smoke rise, or fountain height multiplier.
    pub height: f64,
    /// Swirl radius, smoke wind drift, or fountain width multiplier.
    pub spread: f64,
    /// Animation speed multiplier.
    pub speed: f64,
    /// Brightness/thickness multiplier.
    pub opacity: f64,
}

impl Default for ParticleSettings {
    fn default() -> Self {
        Self {
            size: 1.0,
            density: 1.0,
            height: 1.0,
            spread: 1.0,
            speed: 1.0,
            opacity: 1.0,
        }
    }
}

impl ParticleSettings {
    pub const SIZE_RANGE: std::ops::RangeInclusive<f64> = 0.25..=4.0;
    pub const DENSITY_RANGE: std::ops::RangeInclusive<f64> = 0.25..=3.0;
    pub const HEIGHT_RANGE: std::ops::RangeInclusive<f64> = 0.25..=3.0;
    pub const SPREAD_RANGE: std::ops::RangeInclusive<f64> = 0.25..=3.0;
    pub const SPEED_RANGE: std::ops::RangeInclusive<f64> = 0.25..=3.0;
    pub const OPACITY_RANGE: std::ops::RangeInclusive<f64> = 0.1..=2.0;

    /// Reads the effect settings stored in the particle `data` column. Missing
    /// or legacy fields fall back to 1.0, so old rows keep the default effect.
    pub fn from_json(data: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            return Self::default();
        };
        let get = |key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1.0)
        };
        Self {
            size: get("size"),
            density: get("density"),
            height: get("height"),
            spread: get("spread"),
            speed: get("speed"),
            opacity: get("opacity"),
        }
        .clamped()
    }

    pub fn to_json(self) -> String {
        serde_json::json!({
            "size": self.size,
            "density": self.density,
            "height": self.height,
            "spread": self.spread,
            "speed": self.speed,
            "opacity": self.opacity,
        })
        .to_string()
    }

    pub fn clamped(self) -> Self {
        Self {
            size: self
                .size
                .clamp(*Self::SIZE_RANGE.start(), *Self::SIZE_RANGE.end()),
            density: self
                .density
                .clamp(*Self::DENSITY_RANGE.start(), *Self::DENSITY_RANGE.end()),
            height: self
                .height
                .clamp(*Self::HEIGHT_RANGE.start(), *Self::HEIGHT_RANGE.end()),
            spread: self
                .spread
                .clamp(*Self::SPREAD_RANGE.start(), *Self::SPREAD_RANGE.end()),
            speed: self
                .speed
                .clamp(*Self::SPEED_RANGE.start(), *Self::SPEED_RANGE.end()),
            opacity: self
                .opacity
                .clamp(*Self::OPACITY_RANGE.start(), *Self::OPACITY_RANGE.end()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParticleObject {
    pub lon: f64,
    pub lat: f64,
    pub altitude: f64,
    pub kind: ParticleKind,
    pub settings: ParticleSettings,
}

impl ParticleObject {
    pub fn point(&self) -> GeoPoint {
        GeoPoint::new(self.lon, self.lat)
    }

    pub fn set_point(&mut self, point: GeoPoint) {
        self.lon = point.longitude;
        self.lat = point.latitude;
    }
}

#[derive(Clone, Debug)]
pub enum EditorKind {
    Marker(MarkerObject),
    Line(LineObject),
    Building(BuildingObject),
    Text(TextObject),
    Particle(ParticleObject),
}

impl EditorKind {
    pub fn object_type(&self) -> ObjectType {
        match self {
            Self::Marker(_) => ObjectType::Marker,
            Self::Line(_) => ObjectType::Line,
            Self::Building(_) => ObjectType::Building,
            Self::Text(_) => ObjectType::Text,
            Self::Particle(_) => ObjectType::Particle,
        }
    }

    pub fn point(&self) -> Option<GeoPoint> {
        match self {
            Self::Marker(marker) => Some(marker.point()),
            Self::Line(line) => line
                .points
                .first()
                .map(|point| GeoPoint::new(point[0], point[1])),
            Self::Building(building) => Some(building.point()),
            Self::Text(text) => Some(text.point()),
            Self::Particle(particle) => Some(particle.point()),
        }
    }

    pub fn points(&self) -> Vec<GeoPoint> {
        match self {
            Self::Marker(marker) => vec![marker.point()],
            Self::Line(line) => line
                .points
                .iter()
                .map(|point| GeoPoint::new(point[0], point[1]))
                .collect(),
            Self::Building(building) => building.outline(),
            Self::Text(text) => vec![text.point()],
            Self::Particle(particle) => vec![particle.point()],
        }
    }

    pub fn color(&self) -> String {
        match self {
            Self::Marker(marker) => marker_color(&marker.kategori).to_owned(),
            Self::Line(line) => line.color.clone(),
            Self::Building(building) => building.color.clone(),
            Self::Text(text) => text.color.clone(),
            Self::Particle(particle) => particle_color(particle).to_owned(),
        }
    }

    /// Point the camera should centre on when this object is clicked in the
    /// Scene tree. Lines and buildings use the middle of their footprint.
    pub fn focus_point(&self) -> Option<GeoPoint> {
        match self {
            Self::Line(line) => points_centroid(&line.points),
            _ => self.point(),
        }
    }

    /// Approximate object size in meters, used to pick a focus zoom.
    pub fn focus_extent_meters(&self) -> f64 {
        match self {
            Self::Marker(marker) => {
                let scale = marker
                    .scale_x
                    .abs()
                    .max(marker.scale_y.abs())
                    .max(marker.scale_z.abs())
                    .clamp(0.1, 20.0);
                (2.0 * scale).max(1.5)
            }
            Self::Line(line) => points_extent(&line.points).max(6.0),
            Self::Building(building) => {
                let outline = building
                    .outline()
                    .iter()
                    .map(|point| [point.longitude, point.latitude])
                    .collect::<Vec<_>>();
                points_extent(&outline)
                    .max(building.scale_y.abs() * 2.0)
                    .max(4.0)
            }
            Self::Text(_) | Self::Particle(_) => 3.0,
        }
    }

    /// Position/rotation/scale used by the transform gizmo. Markers and shape
    /// objects are transformable; lines/text/particles are not.
    pub fn transform(&self) -> Option<ObjectTransform> {
        match self {
            Self::Marker(marker) => Some(ObjectTransform {
                lon: marker.lon,
                lat: marker.lat,
                altitude: marker.altitude,
                rot: [marker.rot_x, marker.rot_y, marker.rot_z],
                scale: [marker.scale_x, marker.scale_y, marker.scale_z],
            }),
            Self::Building(building) => Some(ObjectTransform {
                lon: building.lon,
                lat: building.lat,
                altitude: building.altitude,
                rot: [building.rot_x, building.rot_y, building.rot_z],
                scale: [building.scale_x, building.scale_y, building.scale_z],
            }),
            Self::Text(text) => Some(ObjectTransform {
                lon: text.lon,
                lat: text.lat,
                altitude: text.altitude,
                rot: [0.0; 3],
                scale: [1.0; 3],
            }),
            Self::Particle(particle) => Some(ObjectTransform {
                lon: particle.lon,
                lat: particle.lat,
                altitude: particle.altitude,
                rot: [0.0; 3],
                scale: [1.0; 3],
            }),
            _ => None,
        }
    }

    pub fn set_altitude(&mut self, altitude: f64) {
        match self {
            Self::Marker(marker) => marker.altitude = altitude,
            Self::Building(building) => building.altitude = altitude,
            Self::Text(text) => text.altitude = altitude,
            Self::Particle(particle) => particle.altitude = altitude,
            _ => {}
        }
    }

    /// Moves the object so its anchor sits on `point`. Used by drag interaction
    /// so repeated updates place the object instead of accumulating offsets.
    pub fn set_position(&mut self, point: GeoPoint) {
        match self {
            Self::Marker(marker) => marker.set_point(point),
            Self::Building(building) => building.set_position(point),
            Self::Text(text) => text.set_point(point),
            Self::Particle(particle) => {
                particle.lon = point.longitude;
                particle.lat = point.latitude;
            }
            Self::Line(line) => {
                let Some(first) = line.points.first().copied() else {
                    return;
                };
                let Ok(offset) = point.local_offset(GeoPoint::new(first[0], first[1])) else {
                    return;
                };
                offset_points(&mut line.points, offset.east_meters, offset.north_meters);
            }
        }
    }

    pub fn set_rotation_axis(&mut self, axis: usize, value: f64) {
        match self {
            Self::Marker(marker) => match axis {
                0 => marker.rot_x = value,
                1 => marker.rot_y = value,
                _ => marker.rot_z = value,
            },
            Self::Building(building) => match axis {
                0 => building.rot_x = value,
                1 => building.rot_y = value,
                _ => building.rot_z = value,
            },
            _ => {}
        }
    }

    pub fn set_scale(&mut self, scale: [f64; 3]) {
        let scale = scale.map(|value| value.max(MIN_MARKER_SCALE));
        match self {
            Self::Marker(marker) => {
                marker.scale_x = scale[0];
                marker.scale_y = scale[1];
                marker.scale_z = scale[2];
            }
            Self::Building(building) => {
                building.scale_x = scale[0];
                building.scale_y = scale[1];
                building.scale_z = scale[2];
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug)]
pub struct EditorObject {
    pub key: ObjectKey,
    pub server_id: Option<Uuid>,
    pub name: String,
    pub visible: bool,
    pub dirty: bool,
    pub kind: EditorKind,
}

impl EditorObject {
    pub fn point(&self) -> Option<GeoPoint> {
        self.kind.point()
    }

    pub fn object_type(&self) -> ObjectType {
        self.kind.object_type()
    }

    /// Route lines live in the Animasi tab instead of the Scene tree.
    pub fn is_animation_route(&self) -> bool {
        matches!(&self.kind, EditorKind::Line(line) if line.animation.is_some())
    }

    /// Position of this route in the persisted Play order.
    pub fn route_order(&self) -> i32 {
        match &self.kind {
            EditorKind::Line(line) => line
                .animation
                .as_ref()
                .map(|route| route.order)
                .unwrap_or(0),
            _ => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SceneSnapshot {
    objects: Vec<EditorObject>,
    deleted: Vec<(ObjectType, Uuid)>,
    selected: BTreeSet<ObjectKey>,
    next_key: ObjectKey,
}

#[derive(Clone, Debug)]
pub struct VisibleLayers {
    pub marker: bool,
    pub line: bool,
    pub building: bool,
    pub text: bool,
    pub particle: bool,
    pub raster: bool,
}

impl Default for VisibleLayers {
    fn default() -> Self {
        Self::all()
    }
}

impl VisibleLayers {
    pub fn all() -> Self {
        Self {
            marker: true,
            line: true,
            building: true,
            text: true,
            particle: true,
            raster: true,
        }
    }

    pub fn allows(&self, kind: ObjectType) -> bool {
        match kind {
            ObjectType::Marker => self.marker,
            ObjectType::Line => self.line,
            ObjectType::Building => self.building,
            ObjectType::Text => self.text,
            ObjectType::Particle => self.particle,
        }
    }

    pub fn toggle(&mut self, kind: ObjectType) {
        match kind {
            ObjectType::Marker => self.marker = !self.marker,
            ObjectType::Line => self.line = !self.line,
            ObjectType::Building => self.building = !self.building,
            ObjectType::Text => self.text = !self.text,
            ObjectType::Particle => self.particle = !self.particle,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Draft {
    #[default]
    None,
    Line(Vec<GeoPoint>),
    Route(Vec<GeoPoint>),
    Polygon(Vec<GeoPoint>),
}

impl Draft {
    pub fn points(&self) -> &[GeoPoint] {
        match self {
            Self::None => &[],
            Self::Line(points) | Self::Route(points) | Self::Polygon(points) => points,
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn push(&mut self, point: GeoPoint) {
        match self {
            Self::None => {}
            Self::Line(points) | Self::Route(points) | Self::Polygon(points) => points.push(point),
        }
    }

    pub fn clear(&mut self) {
        *self = Self::None;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MeasureState {
    pub start: Option<GeoPoint>,
    pub end: Option<GeoPoint>,
}

#[derive(Clone, Debug, Default)]
pub enum DragState {
    #[default]
    None,
    MoveObjects {
        start: GeoPoint,
        originals: Vec<(ObjectKey, GeoPoint)>,
    },
    Gizmo {
        axis: EditorGizmoAxis,
        original: Vec<(ObjectKey, [f64; 3])>,
        start_world: Vec3,
        start_screen: egui::Pos2,
    },
    /// Drags a `Limited` static asset during Play. The authored position is
    /// untouched; only the Play position/altitude moves.
    PlayGizmo {
        axis: EditorGizmoAxis,
        key: ObjectKey,
        original: GeoPoint,
        original_altitude: f32,
        start_world: Vec3,
        start_screen: egui::Pos2,
    },
    PointDrag {
        axis: EditorGizmoAxis,
        key: ObjectKey,
        index: usize,
        /// Original `[lon, lat, altitude]` of the edited point.
        original: [f64; 3],
        start_world: Vec3,
        start_screen: egui::Pos2,
    },
    Rotate {
        axis: EditorGizmoAxis,
        /// Original `[rot_x, rot_y, rot_z]` per selected marker.
        originals: Vec<(ObjectKey, [f64; 3])>,
        center_screen: egui::Pos2,
        start_angle: f32,
    },
    Scale {
        axis: EditorGizmoAxis,
        originals: Vec<(ObjectKey, [f64; 3])>,
        start_world: Vec3,
        start_screen: egui::Pos2,
    },
    Marquee {
        start: egui::Pos2,
        current: egui::Pos2,
    },
}

impl DragState {
    pub fn is_dragging(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Clone, Debug, Default)]
pub struct GeocodeHit {
    pub label: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// Point-editing state for a line selected in the Inspector.
#[derive(Clone, Copy, Debug)]
pub struct PointEditState {
    pub key: ObjectKey,
    pub point: Option<usize>,
}

pub struct EditorState {
    pub scenario_id: Option<Uuid>,
    pub operation_id: Option<Uuid>,
    pub scenario_name: String,
    pub loaded: bool,
    pub loading: bool,
    pub pending: usize,
    pub load_error: Option<String>,
    pub objects: Vec<EditorObject>,
    pub deleted: Vec<(ObjectType, Uuid)>,
    pub selected: BTreeSet<ObjectKey>,
    pub tool: EditorTool,
    /// Primitive created by the Shape tool.
    pub active_shape: EditorShapeKind,
    /// Particle type created by the Particle tool.
    pub active_particle: ParticleKind,
    pub next_key: ObjectKey,
    pub dirty: bool,
    pub undo: Vec<SceneSnapshot>,
    pub redo: Vec<SceneSnapshot>,
    pub drag: DragState,
    /// Active per-point line editing, if any.
    pub point_edit: Option<PointEditState>,
    pub draft: Draft,
    pub measure: MeasureState,
    pub snap_meters: f32,
    pub show_grid: bool,
    pub labels_visible: bool,
    /// Editor-only particle animation toggle. Play/simulation always animates.
    pub particle_preview: bool,
    pub show_console: bool,
    /// Left panel tab: 0 = Scene, 1 = Assets, 2 = Import.
    pub left_tab: usize,
    /// Sub-tab inside Scene: 0 = Layers, 1 = Animasi.
    pub scene_tab: usize,
    /// Right panel tab: 0 = Inspector, 1 = Urutan Anim.
    pub inspector_tab: usize,
    /// Left Play panel tab in the play-only screen: 0 = Animasi, 1 = Skenario.
    pub play_panel_tab: usize,
    /// Viewport shading for imported models (Texture/Solid/Wireframe).
    pub model_shading: EditorModelShading,
    pub confirm_close: bool,
    pub confirm_reload: bool,
    pub draft_screen: Vec<egui::Pos2>,
    pub measure_screen: Vec<egui::Pos2>,
    pub console: Vec<String>,
    pub preview: bool,
    pub visible: VisibleLayers,
    /// Server asset models resolved from the offline cache, keyed by asset UUID.
    pub marker_models: BTreeMap<String, MarkerAssetModel>,
    /// Image asset paths (offline cache or server URL), keyed by asset UUID.
    pub texture_paths: BTreeMap<String, String>,
    /// Filesystem paths of cached image assets, used by the Inspector preview.
    pub texture_files: BTreeMap<String, String>,
    /// Cached egui textures for shape previews, keyed by file path.
    pub texture_previews: BTreeMap<String, egui::TextureHandle>,
    /// Server base URL used to resolve `/uploads/...` texture references.
    pub base_url: String,
    /// `(asset id, version, move mode)` of the asset list the models were built
    /// from. The move mode is included because changing it must refresh Play
    /// behaviour without bumping the file version.
    pub marker_model_signature: Vec<(Uuid, i64, AssetMoveMode)>,
    /// Set while an asset card is being dragged onto the map.
    pub asset_drag: Option<AssetMarkerDraft>,
    /// Search filter in the editor's Assets tab.
    pub asset_search: String,
    /// Asset category groups collapsed in the editor tree.
    pub asset_groups_closed: BTreeSet<String>,
    /// Asks the shell to sync the offline cache with the server manifest.
    pub requested_asset_sync: bool,
    /// Upload form shown in the Import tab.
    pub import_draft: ImportDraft,
    pub import_error: Option<String>,
    pub import_busy: bool,
    /// The upload form is shown in a modal dialog.
    pub import_dialog_open: bool,
    /// Import asset waiting for delete confirmation (`id`, `name`).
    pub import_delete: Option<(Uuid, String)>,
    /// Selected map objects are waiting for delete confirmation.
    pub confirm_delete: bool,
    /// Asks the shell to upload the Import draft to the server.
    pub requested_import_upload: bool,
    /// Asks the shell to delete this import asset.
    pub requested_import_delete: Option<Uuid>,
    /// Asks the shell to reload import assets of the current operation.
    pub requested_import_refresh: bool,
    pub last_click: Option<(f64, egui::Pos2)>,
    pub scenario_center: GeoPoint,
    pub indoor: bool,
    /// `map_mode` of the loaded scenario, unaffected by the Indoor display toggle.
    pub scenario_indoor: bool,
    pub floor_plan: bool,
    pub grid_spacing: f32,
    pub grid_extent: f32,
    pub floor_size: [f32; 2],
    pub map_state: MapCameraState,
    pub cursor_geo: Option<GeoPoint>,
    pub fps: f64,
    pub render_revision: u64,
    pub projected: Vec<(ObjectKey, ObjectType, Vec<egui::Pos2>, bool)>,
    pub labels: Vec<(egui::Pos2, String, [f32; 4])>,
    /// Screen-space text objects (content), shown in the editor and in Play.
    pub text_overlays: Vec<(egui::Pos2, String, [f32; 4], f32)>,
    pub viewport_rect: egui::Rect,
    pub cursor_world: Option<Vec3>,
    pub hover_key: Option<ObjectKey>,
    pub hover_axis: Option<EditorGizmoAxis>,
    pub context_menu: Option<(egui::Pos2, ObjectKey)>,
    pub right_press: Option<egui::Pos2>,
    pub geocode_query: String,
    pub geocode_results: Vec<GeocodeHit>,
    pub geocode_pending: bool,
    pub requested_layer: Option<BaseLayer>,
    pub requested_flight: Option<MapCameraState>,
    pub requested_camera: Option<MapCameraState>,
    pub requested_zoom: Option<f32>,
    pub requested_north: bool,
    pub requested_geocode: bool,
    pub base_layer: BaseLayer,
    /// Background style shared by the editor viewport and Play mode.
    pub sky: SkyMode,
    /// Marker search text for the route inspector.
    pub route_marker_search: String,
    /// Route waiting for a marker picked directly on the map.
    pub pick_route_marker: Option<ObjectKey>,
    /// Daftar Anim filter: list only route animations hidden from the map.
    pub anim_filter_hidden: bool,
    /// Daftar Anim filter: list only locked route animations.
    pub anim_filter_locked: bool,
    /// Routes locked from map picking (editor session only).
    pub locked: BTreeSet<ObjectKey>,
    /// Set when local Play state changed; the play-only screen sends a
    /// realtime snapshot to other clients when this is true.
    pub play_sync_pending: bool,
    /// RTS play mode state, rebuilt every time Play starts.
    pub play: PlayState,
    /// Screen-space ground rings for the RTS selection overlay, rebuilt every
    /// frame in [`super::input`].
    pub play_rings: Vec<(ObjectKey, Vec<egui::Pos2>)>,
    pub save_pending: usize,
    pub save_error: Option<String>,
    pub last_save: Option<String>,
    /// True while the scenario camera view is being persisted.
    pub saving_view: bool,
    /// Time of the last successful camera-view save.
    pub view_saved_at: Option<String>,
    /// Camera view last persisted to (or loaded from) the server.
    pub saved_view: Option<MapCameraState>,
    /// Base layer that belongs to `saved_view`.
    pub saved_base_layer: Option<BaseLayer>,
    pub jobs: Vec<super::api::EditorJob>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            scenario_id: None,
            operation_id: None,
            scenario_name: String::new(),
            loaded: false,
            loading: false,
            pending: 0,
            load_error: None,
            objects: Vec::new(),
            deleted: Vec::new(),
            selected: BTreeSet::new(),
            tool: EditorTool::Select,
            active_shape: EditorShapeKind::Box,
            active_particle: ParticleKind::Fire,
            next_key: 1,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            drag: DragState::None,
            point_edit: None,
            draft: Draft::None,
            measure: MeasureState::default(),
            snap_meters: 0.0,
            show_grid: false,
            labels_visible: true,
            particle_preview: true,
            show_console: true,
            left_tab: 0,
            scene_tab: 0,
            inspector_tab: 0,
            play_panel_tab: 0,
            model_shading: EditorModelShading::Textured,
            confirm_close: false,
            confirm_reload: false,
            draft_screen: Vec::new(),
            measure_screen: Vec::new(),
            console: Vec::new(),
            preview: false,
            visible: VisibleLayers::all(),
            marker_models: BTreeMap::new(),
            texture_paths: BTreeMap::new(),
            texture_files: BTreeMap::new(),
            texture_previews: BTreeMap::new(),
            base_url: String::new(),
            marker_model_signature: Vec::new(),
            asset_drag: None,
            asset_search: String::new(),
            asset_groups_closed: BTreeSet::new(),
            requested_asset_sync: false,
            import_draft: ImportDraft::default(),
            import_error: None,
            import_busy: false,
            import_dialog_open: false,
            import_delete: None,
            confirm_delete: false,
            requested_import_upload: false,
            requested_import_delete: None,
            requested_import_refresh: false,
            last_click: None,
            scenario_center: MapCameraState::default().center,
            indoor: false,
            scenario_indoor: false,
            floor_plan: false,
            grid_spacing: 1.0,
            grid_extent: 400.0,
            floor_size: [100.0, 60.0],
            map_state: MapCameraState::default(),
            cursor_geo: None,
            fps: 0.0,
            render_revision: 1,
            projected: Vec::new(),
            labels: Vec::new(),
            text_overlays: Vec::new(),
            viewport_rect: egui::Rect::NOTHING,
            cursor_world: None,
            hover_key: None,
            hover_axis: None,
            context_menu: None,
            right_press: None,
            geocode_query: String::new(),
            geocode_results: Vec::new(),
            geocode_pending: false,
            requested_layer: None,
            requested_flight: None,
            requested_camera: None,
            requested_zoom: None,
            requested_north: false,
            requested_geocode: false,
            base_layer: BaseLayer::OpenStreetMap,
            sky: SkyMode::Black,
            route_marker_search: String::new(),
            pick_route_marker: None,
            anim_filter_hidden: false,
            anim_filter_locked: false,
            locked: BTreeSet::new(),
            play_sync_pending: false,
            play: PlayState::default(),
            play_rings: Vec::new(),
            save_pending: 0,
            save_error: None,
            last_save: None,
            saving_view: false,
            view_saved_at: None,
            saved_view: None,
            saved_base_layer: None,
            jobs: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        let scenario_id = self.scenario_id;
        let sky = self.sky;
        *self = Self::new();
        self.scenario_id = scenario_id;
        self.sky = sky;
    }

    pub fn clear_scene(&mut self) {
        self.objects.clear();
        self.deleted.clear();
        self.selected.clear();
        self.locked.clear();
        self.undo.clear();
        self.redo.clear();
        self.next_key = 1;
        self.dirty = false;
        self.touch();
    }

    /// True when the live camera and base layer still match the view stored
    /// on the server (or the one loaded with the scenario).
    pub fn view_is_saved(&self) -> bool {
        let (Some(saved), Some(layer)) = (self.saved_view, self.saved_base_layer) else {
            return false;
        };
        layer == self.base_layer && same_camera(saved, self.map_state)
    }

    pub fn set_tool(&mut self, tool: EditorTool) {
        self.pick_route_marker = None;
        if self.tool == tool {
            return;
        }
        self.tool = tool;
        // Line point editing only supports the Move gizmo; switching to any
        // other tool leaves the point-edit mode.
        if tool != EditorTool::Move {
            self.point_edit = None;
        }
        self.draft.clear();
        // The transform gizmo depends on the tool, so the scene must be rebuilt
        // immediately instead of on the next unrelated change.
        self.touch();
    }

    pub fn touch(&mut self) {
        self.render_revision = self.render_revision.wrapping_add(1);
    }

    pub fn log(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.console.push(message);
        if self.console.len() > 200 {
            self.console.remove(0);
        }
    }

    pub fn object(&self, key: ObjectKey) -> Option<&EditorObject> {
        self.objects.iter().find(|object| object.key == key)
    }

    pub fn object_mut(&mut self, key: ObjectKey) -> Option<&mut EditorObject> {
        self.objects.iter_mut().find(|object| object.key == key)
    }

    /// True when the object is locked from map selection in Daftar Anim.
    pub fn is_locked(&self, key: ObjectKey) -> bool {
        self.locked.contains(&key)
    }

    pub fn primary_selected(&self) -> Option<&EditorObject> {
        self.selected
            .iter()
            .next()
            .and_then(|key| self.object(*key))
    }

    pub fn primary_marker(&self) -> Option<&MarkerObject> {
        self.selected.iter().find_map(|key| {
            let object = self.object(*key)?;
            match &object.kind {
                EditorKind::Marker(marker) if object.visible => Some(marker),
                _ => None,
            }
        })
    }

    /// First selected object that supports the transform gizmo, with a copy of
    /// its transform values.
    pub fn primary_transform(&self) -> Option<(ObjectKey, ObjectTransform)> {
        self.selected.iter().find_map(|key| {
            let object = self.object(*key)?;
            if !object.visible || self.locked.contains(key) {
                return None;
            }
            object.kind.transform().map(|transform| (*key, transform))
        })
    }

    /// Markers driven by the selected route animations. The editor overlay
    /// rings them, so the target marker is visible even in scenarios with
    /// hundreds of markers.
    pub fn selected_route_markers(&self) -> BTreeSet<ObjectKey> {
        let mut markers = BTreeSet::new();
        for key in &self.selected {
            let Some(object) = self.object(*key) else {
                continue;
            };
            // A route hidden from the map should not ring its marker either.
            if !object.visible {
                continue;
            }
            let EditorKind::Line(line) = &object.kind else {
                continue;
            };
            let Some(marker_id) = line.animation.as_ref().and_then(|route| route.marker_id) else {
                continue;
            };
            if let Some(marker) = self.objects.iter().find(|candidate| {
                candidate.server_id == Some(marker_id)
                    && matches!(candidate.kind, EditorKind::Marker(_))
                    && candidate.visible
            }) {
                markers.insert(marker.key);
            }
        }
        markers
    }

    pub fn select_only(&mut self, key: ObjectKey) {
        if !self.selected.contains(&key) {
            self.pick_route_marker = None;
            self.route_marker_search.clear();
        }
        if self.point_edit.is_some_and(|edit| edit.key != key) {
            self.point_edit = None;
        }
        self.selected.clear();
        self.selected.insert(key);
        self.touch();
    }

    pub fn toggle_selected(&mut self, key: ObjectKey) {
        if self.point_edit.is_some_and(|edit| edit.key != key) {
            self.point_edit = None;
        }
        if !self.selected.insert(key) {
            self.selected.remove(&key);
            if self.point_edit.is_some_and(|edit| edit.key == key) {
                self.point_edit = None;
            }
        }
        self.touch();
    }

    pub fn add_to_selection(&mut self, key: ObjectKey) {
        if self.selected.insert(key) {
            self.touch();
        }
    }

    pub fn clear_selection(&mut self) {
        self.point_edit = None;
        if !self.selected.is_empty() {
            self.selected.clear();
            self.touch();
        }
    }

    /// Flies the camera to an object (pan + zoom) so clicking a row in the
    /// Scene tree always brings the 3D model into view.
    pub fn focus_object(&mut self, key: ObjectKey) {
        let Some(object) = self.object(key) else {
            return;
        };
        let Some(center) = object.kind.focus_point() else {
            return;
        };
        let extent = object.kind.focus_extent_meters().max(1.0);
        let viewport_height = if self.viewport_rect.height() > 100.0 {
            self.viewport_rect.height()
        } else {
            720.0
        };
        // Frame the object at roughly a fifth of the viewport height, clamped
        // so very small/huge windows still get a sensible distance.
        let target_pixels = (viewport_height * 0.22).clamp(90.0, 220.0);
        let desired_mpp = extent / f64::from(target_pixels);
        let current_mpp = self.map_state.meters_per_pixel().max(1.0e-9);
        let zoom_delta = (current_mpp / desired_mpp).log2();
        let zoom = (f64::from(self.map_state.zoom) + zoom_delta)
            .clamp(f64::from(twg_map::MIN_ZOOM), f64::from(twg_map::MAX_ZOOM))
            as f32;
        let mut target = self.map_state;
        target.center = center;
        target.zoom = zoom;
        self.requested_flight = Some(target);
    }

    // ------------------------------------------------------------- play mode

    /// Builds the RTS play state from the asset move mode:
    ///
    /// - animated assets become `PlayUnit`s that play their walk clip;
    /// - `Free` static assets become `PlayUnit`s without animation;
    /// - `Limited` static assets are kept apart and moved with the 3D gizmo;
    /// - `None` assets are non-pickable obstacles;
    /// - 3D shapes (except the flat ground plate) block movement as well.
    pub fn start_play(&mut self) {
        self.pick_route_marker = None;
        self.play = PlayState {
            active: true,
            ..PlayState::default()
        };
        self.drag = DragState::None;
        self.hover_axis = None;
        self.selected.clear();
        for object in &self.objects {
            if !object.visible || !self.visible.allows(object.object_type()) {
                continue;
            }
            let Some(point) = object.point() else {
                continue;
            };
            match &object.kind {
                EditorKind::Marker(marker) => {
                    let model = self.marker_models.get(&marker.unit_id);
                    let animated =
                        model.is_some_and(|model| model.animated && model.animation_path.is_some());
                    let move_mode = model.map(|model| model.move_mode).unwrap_or_default();
                    let scale = marker.scale_x.abs().max(marker.scale_z.abs()).max(0.5);
                    // Use the actual model footprint when it is known; a large
                    // vehicle must not overlap another unit just because the
                    // marker actor scale is 1.0.
                    let radius = model
                        .filter(|model| model.footprint_radius > 0.01)
                        .map(|model| {
                            (model.footprint_radius as f64
                                * model.base_scale as f64
                                * marker.scale_x.abs().max(marker.scale_z.abs()))
                            .clamp(0.8, 12.0)
                        })
                        .unwrap_or_else(|| (1.4 * scale).clamp(0.8, 12.0));
                    if animated || move_mode == AssetMoveMode::Free {
                        self.play.units.insert(
                            object.key,
                            PlayUnit {
                                position: point,
                                target: None,
                                // Bearing (clockwise from north) that matches the
                                // marker's authored Y rotation when Play starts.
                                heading_deg: 180.0 - marker.rot_y as f32,
                                speed: play_speed(&marker.kategori),
                                radius,
                                obstacle: marker.obstacle,
                                altitude: marker.altitude as f32,
                                animated,
                            },
                        );
                    } else if move_mode == AssetMoveMode::Limited {
                        self.play.limited.insert(
                            object.key,
                            PlayLimitedUnit {
                                position: point,
                                radius,
                                altitude: marker.altitude as f32,
                                heading_deg: 180.0 - marker.rot_y as f32,
                            },
                        );
                        // Limited assets still block free-moving units; the
                        // obstacle circle follows the asset when its gizmo is
                        // dragged.
                        if marker.obstacle {
                            self.play.obstacles.push(PlayObstacle {
                                center: point,
                                radius,
                                key: Some(object.key),
                            });
                        }
                    } else if marker.obstacle {
                        self.play.obstacles.push(PlayObstacle {
                            center: point,
                            radius,
                            key: None,
                        });
                    }
                }
                // Every 3D primitive and custom mesh blocks movement unless the
                // object is explicitly marked as passable. The flat ground
                // plate is walkable in any case.
                EditorKind::Building(building)
                    if building.shape != EditorShapeKind::Ground && building.obstacle =>
                {
                    self.play.obstacles.push(PlayObstacle {
                        center: point,
                        radius: building_obstacle_radius(building),
                        key: None,
                    });
                }
                _ => {}
            }
        }
        self.init_play_animations();
        self.play_sync_pending = true;
    }

    /// Resolves authored routes into Play state and records which markers they
    /// control, so the normal RTS order logic leaves those markers alone.
    fn init_play_animations(&mut self) {
        let mut animations = Vec::new();
        let mut controlled = BTreeSet::new();
        for object in &self.objects {
            // Hiding a route only declutters the editor map; the animation still
            // runs in Play, where route lines are never drawn.
            if !self.visible.allows(ObjectType::Line) {
                continue;
            }
            let EditorKind::Line(line) = &object.kind else {
                continue;
            };
            let Some(route) = &line.animation else {
                continue;
            };
            let points = line
                .points
                .iter()
                .map(|point| GeoPoint::new(point[0], point[1]))
                .collect::<Vec<_>>();
            if points.len() < 2 {
                continue;
            }
            let marker_key = route.marker_id.and_then(|id| {
                self.objects.iter().find_map(|candidate| {
                    let EditorKind::Marker(marker) = &candidate.kind else {
                        return None;
                    };
                    (candidate.server_id == Some(id) && self.marker_is_animatable(marker))
                        .then_some(candidate.key)
                })
            });
            if let Some(key) = marker_key {
                controlled.insert(key);
            }
            animations.push(PlayRouteAnimation {
                route_key: object.key,
                name: object.name.clone(),
                marker_key,
                points: points.clone(),
                altitudes: line.altitudes.clone(),
                speed: route.speed.clamp(0.1, 100.0),
                length_m: polyline_length(&points),
                progress_m: 0.0,
                state: PlaybackState::Stopped,
                repeat: route.repeat,
                order: route.order,
            });
        }
        // Play follows the order authored in the editor's Urutan tab.
        animations.sort_by_key(|animation| animation.order);
        self.play.animations = animations;
        self.play.controlled = controlled;
    }

    pub fn stop_play(&mut self) {
        self.play = PlayState::default();
        self.selected.clear();
    }

    /// Advances every moving unit, steering around static obstacles.
    pub fn play_tick(&mut self, delta_seconds: f64) {
        if !self.play.active {
            return;
        }
        let dt = delta_seconds.clamp(0.0, 0.1);
        if dt <= 0.0 {
            return;
        }
        // Units are obstacles too, so columns do not walk through each other.
        // Snapshotting once per tick keeps the borrow simple and avoids
        // allocating a list per unit.
        let mut obstacles = self.play.obstacles.clone();
        obstacles.extend(
            self.play
                .units
                .iter()
                .filter(|(_, unit)| unit.obstacle)
                .map(|(key, unit)| PlayObstacle {
                    center: unit.position,
                    radius: unit.radius,
                    key: Some(*key),
                }),
        );
        for (key, unit) in self.play.units.iter_mut() {
            if self.play.controlled.contains(key) {
                continue;
            }
            let Some(target) = unit.target else {
                continue;
            };
            let Ok(to_target) = target.local_offset(unit.position) else {
                unit.target = None;
                continue;
            };
            let distance = to_target.east_meters.hypot(to_target.north_meters);
            if distance <= PLAY_ARRIVE_METERS {
                unit.position = target;
                unit.target = None;
                continue;
            }
            let (east, north) = play_steer(to_target, unit.position, &obstacles, *key, unit.radius);
            unit.heading_deg = east.atan2(north).to_degrees() as f32;
            let step = unit.speed * dt;
            let Ok(mut candidate) =
                GeoPoint::from_local_offset(unit.position, east * step, north * step)
            else {
                continue;
            };
            // Push the candidate out of any obstacle it still intersects, so a
            // sharp turn cannot clip through a static model or another unit.
            for obstacle in &obstacles {
                if obstacle.key == Some(*key) {
                    continue;
                }
                let Ok(relative) = obstacle.center.local_offset(candidate) else {
                    continue;
                };
                let gap = relative.east_meters.hypot(relative.north_meters);
                let minimum = obstacle.radius + unit.radius;
                if gap >= minimum || gap <= 1.0e-6 {
                    continue;
                }
                let push = minimum - gap;
                if let Ok(pushed) = GeoPoint::from_local_offset(
                    candidate,
                    -relative.east_meters / gap * push,
                    -relative.north_meters / gap * push,
                ) {
                    candidate = pushed;
                }
            }
            unit.position = candidate;
        }

        let mut updated = Vec::new();
        for (index, animation) in self.play.animations.iter_mut().enumerate() {
            if animation.state != PlaybackState::Playing || animation.length_m <= 0.0 {
                continue;
            }
            if !animation.repeat && animation.is_finished() {
                continue;
            }
            animation.progress_m += animation.speed * dt;
            if animation.repeat {
                animation.progress_m %= animation.length_m;
            } else {
                animation.progress_m = animation.progress_m.min(animation.length_m);
            }
            updated.push(index);
        }
        for index in updated {
            self.apply_route_animation(index);
        }
    }

    /// Starts or resumes a route. A finished route restarts from the first point.
    pub fn play_route_animation(&mut self, index: usize) {
        let Some(animation) = self.play.animations.get_mut(index) else {
            return;
        };
        if animation.length_m <= 0.0 {
            return;
        }
        if animation.progress_m >= animation.length_m {
            animation.progress_m = 0.0;
        }
        animation.state = PlaybackState::Playing;
        self.apply_route_animation(index);
        self.play_sync_pending = true;
    }

    pub fn pause_route_animation(&mut self, index: usize) {
        if let Some(animation) = self.play.animations.get_mut(index)
            && animation.state == PlaybackState::Playing
        {
            animation.state = PlaybackState::Paused;
            self.play_sync_pending = true;
        }
    }

    /// Stops a route and puts its marker back at the first point.
    pub fn stop_route_animation(&mut self, index: usize) {
        if let Some(animation) = self.play.animations.get_mut(index) {
            animation.progress_m = 0.0;
            animation.state = PlaybackState::Stopped;
        }
        self.apply_route_animation(index);
        self.play_sync_pending = true;
    }

    pub(super) fn apply_route_animation(&mut self, index: usize) {
        let Some(animation) = self.play.animations.get(index) else {
            return;
        };
        let Some(marker_key) = animation.marker_key else {
            return;
        };
        let Some((position, altitude, heading)) = route_sample(
            &animation.points,
            &animation.altitudes,
            animation.progress_m,
        ) else {
            return;
        };
        if let Some(unit) = self.play.units.get_mut(&marker_key) {
            unit.position = position;
            unit.altitude = altitude as f32;
            unit.heading_deg = heading;
            unit.target = None;
        }
        if let Some(unit) = self.play.limited.get_mut(&marker_key) {
            unit.position = position;
            unit.altitude = altitude as f32;
            unit.heading_deg = heading;
        }
        for obstacle in &mut self.play.obstacles {
            if obstacle.key == Some(marker_key) {
                obstacle.center = position;
            }
        }
    }

    /// Sends the selected units to `target` in a grid formation (rows grow
    /// toward the camera). A single unit goes straight to the click point.
    pub fn play_move_selected(&mut self, target: GeoPoint) {
        if !self.play.active {
            return;
        }
        let keys = self
            .selected
            .iter()
            .copied()
            .filter(|key| self.play.units.contains_key(key) && !self.play.controlled.contains(key))
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return;
        }
        let count = keys.len();
        let columns = (count as f64).sqrt().ceil() as usize;
        let columns = columns.max(1);
        let rows = count.div_ceil(columns);
        for (index, key) in keys.into_iter().enumerate() {
            let row = index / columns;
            let column = index % columns;
            let east = (column as f64 - (columns as f64 - 1.0) / 2.0) * PLAY_FORMATION_SPACING;
            let north = ((rows as f64 - 1.0) / 2.0 - row as f64) * PLAY_FORMATION_SPACING;
            let slot = GeoPoint::from_local_offset(target, east, north).unwrap_or(target);
            if let Some(unit) = self.play.units.get_mut(&key) {
                unit.target = Some(slot);
            }
        }
        self.play_sync_pending = true;
    }

    /// Returns the pickable unit under a screen point, if any. Limited static
    /// assets are pickable too so their move gizmo can be shown.
    pub fn play_pick(&self, position: egui::Pos2) -> Option<ObjectKey> {
        let mut best: Option<(f32, ObjectKey)> = None;
        for (key, kind, points, _) in &self.projected {
            if !matches!(kind, ObjectType::Marker)
                || (!self.play.units.contains_key(key) && !self.play.limited.contains_key(key))
            {
                continue;
            }
            let Some(point) = points.first() else {
                continue;
            };
            let distance = point.distance(position);
            if distance <= 26.0 && best.is_none_or(|(nearest, _)| distance < nearest) {
                best = Some((distance, *key));
            }
        }
        best.map(|(_, key)| key)
    }

    /// Selection changes in Play never call `touch()`: the selection marker is
    /// drawn by egui, so the glTF scene must not be rebuilt.
    pub fn play_select_only(&mut self, key: ObjectKey) {
        self.selected.clear();
        self.selected.insert(key);
    }

    /// Selects a `Limited` static asset. The next scene build draws its move
    /// gizmo at the current Play position.
    pub fn play_select_limited(&mut self, key: ObjectKey) {
        self.selected.clear();
        self.selected.insert(key);
    }

    /// Key, ground position, and runtime altitude of the limited asset whose
    /// gizmo should be visible. Only a single selected limited asset shows a
    /// gizmo.
    pub fn play_gizmo_target(&self) -> Option<(ObjectKey, GeoPoint, f32)> {
        if !self.preview || !self.play.active {
            return None;
        }
        let key = *self
            .selected
            .iter()
            .find(|key| self.play.limited.contains_key(key))?;
        let unit = self.play.limited.get(&key)?;
        Some((key, unit.position, unit.altitude))
    }

    pub fn play_toggle_selected(&mut self, key: ObjectKey) {
        if !self.selected.insert(key) {
            self.selected.remove(&key);
        }
    }

    pub fn play_box_select(&mut self, rect: egui::Rect, additive: bool) {
        if !additive {
            self.selected.clear();
        }
        for (key, kind, points, _) in &self.projected {
            if !matches!(kind, ObjectType::Marker) || !self.play.units.contains_key(key) {
                continue;
            }
            if points.first().is_some_and(|point| rect.contains(*point)) {
                self.selected.insert(*key);
            }
        }
    }

    pub fn play_clear_selection(&mut self) {
        self.selected.clear();
    }

    pub fn push_undo(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            objects: self.objects.clone(),
            deleted: self.deleted.clone(),
            selected: self.selected.clone(),
            next_key: self.next_key,
        }
    }

    fn restore(&mut self, mut snapshot: SceneSnapshot) {
        // Objects created after the snapshot must be deleted on the server the
        // next time the scenario is saved. Objects restored by undo must no
        // longer be queued for deletion.
        let snapshot_keys = snapshot
            .objects
            .iter()
            .map(|object| object.key)
            .collect::<BTreeSet<_>>();
        for object in &self.objects {
            if !snapshot_keys.contains(&object.key)
                && let Some(id) = object.server_id
            {
                snapshot.deleted.push((object.object_type(), id));
            }
        }
        let current_keys = self
            .objects
            .iter()
            .map(|object| object.key)
            .collect::<BTreeSet<_>>();
        for object in &snapshot.objects {
            if !current_keys.contains(&object.key)
                && let Some(id) = object.server_id
            {
                snapshot.deleted.retain(|(_, deleted)| *deleted != id);
            }
        }

        self.objects = snapshot.objects;
        self.deleted = snapshot.deleted;
        self.selected = snapshot.selected;
        self.next_key = snapshot.next_key;
        self.touch();
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.redo.push(current);
        self.restore(snapshot);
        self.dirty = true;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.undo.push(current);
        self.restore(snapshot);
        self.dirty = true;
        true
    }

    pub fn add_marker(&mut self, point: GeoPoint, unit: usize) {
        self.push_undo();
        let key = self.alloc_key();
        let mut marker = MarkerObject::new(unit, point);
        marker.kategori = UNIT_CATALOG[marker.unit].category.to_owned();
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: format!("Marker {key}"),
            visible: true,
            dirty: true,
            kind: EditorKind::Marker(marker),
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
    }

    /// Adds a marker backed by a server asset. The asset UUID is stored in
    /// `unit_id`, so reloading the scenario re-resolves the cached model file.
    pub fn add_asset_marker(&mut self, draft: &AssetMarkerDraft, point: GeoPoint) {
        self.push_undo();
        let key = self.alloc_key();
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: draft.name.clone(),
            visible: true,
            dirty: true,
            kind: EditorKind::Marker(MarkerObject {
                unit: unit_for_uuid(&draft.unit_id),
                unit_id: draft.unit_id.clone(),
                lon: point.longitude,
                lat: point.latitude,
                // New asset markers start flat on the ground and unrotated;
                // the inspector/gizmo is used to orient them.
                rot_x: 0.0,
                rot_y: 0.0,
                rot_z: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                scale_z: 1.0,
                altitude: 0.0,
                jumlah: 1,
                kategori: "personel".to_owned(),
                keterangan: String::new(),
                obstacle: true,
            }),
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
    }

    pub fn add_text(&mut self, point: GeoPoint) {
        self.push_undo();
        let key = self.alloc_key();
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: format!("Teks {key}"),
            visible: true,
            dirty: true,
            kind: EditorKind::Text(TextObject {
                text: "Teks baru".to_owned(),
                lon: point.longitude,
                lat: point.latitude,
                altitude: 0.0,
                color: "#ffffff".to_owned(),
                font_size: 18.0,
                align: "center".to_owned(),
                wrap_text: false,
            }),
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
    }

    pub fn add_line(&mut self, points: Vec<GeoPoint>) {
        if points.len() < 2 {
            return;
        }
        self.push_undo();
        let key = self.alloc_key();
        let coordinates = points
            .iter()
            .map(|point| [point.longitude, point.latitude])
            .collect::<Vec<_>>();
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: format!("Garis {key}"),
            visible: true,
            dirty: true,
            kind: EditorKind::Line(LineObject::new(coordinates)),
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
    }

    /// Creates the line behind a route animation and selects it so the
    /// Inspector shows its speed and marker settings.
    pub fn add_route(&mut self, points: Vec<GeoPoint>) {
        if points.len() < 2 {
            return;
        }
        self.push_undo();
        let key = self.alloc_key();
        let coordinates = points
            .iter()
            .map(|point| [point.longitude, point.latitude])
            .collect::<Vec<_>>();
        let mut line = LineObject::new(coordinates);
        line.color = "#ffb020".to_owned();
        line.width = 0.5;
        line.animation = Some(RouteAnimation {
            order: self.next_route_order(),
            ..RouteAnimation::default()
        });
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: format!("Single marker rute {key}"),
            visible: true,
            dirty: true,
            kind: EditorKind::Line(line),
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
        self.log("Rute dibuat. Atur kecepatan dan marker di Inspector.");
    }

    /// Starts drawing a new route from the Animasi tab.
    pub fn begin_route_draw(&mut self) {
        if !self.loaded {
            return;
        }
        self.end_point_edit();
        self.selected.clear();
        self.set_tool(EditorTool::Line);
        self.draft = Draft::Route(Vec::new());
        self.last_click = None;
        self.log("Klik titik rute di peta. Enter atau klik dua kali untuk selesai.");
    }

    /// Marker assets that Play can move: animated models and static `Free` or
    /// `Limited` assets.
    pub fn marker_is_animatable(&self, marker: &MarkerObject) -> bool {
        self.marker_models
            .get(&marker.unit_id)
            .is_some_and(|model| {
                (model.animated && model.animation_path.is_some())
                    || matches!(
                        model.move_mode,
                        AssetMoveMode::Free | AssetMoveMode::Limited
                    )
            })
    }

    /// Markers shown in the route inspector. All saved markers are listed so a
    /// marker can still be picked even when its model metadata is incomplete;
    /// the label explains why a marker is not usable yet.
    pub fn animation_marker_options(&self, route_key: ObjectKey) -> Vec<(Option<Uuid>, String)> {
        let needle = self.route_marker_search.trim().to_lowercase();
        let mut used = BTreeSet::new();
        for object in &self.objects {
            if object.key == route_key {
                continue;
            }
            let EditorKind::Line(line) = &object.kind else {
                continue;
            };
            if let Some(marker_id) = line.animation.as_ref().and_then(|route| route.marker_id) {
                used.insert(marker_id);
            }
        }
        let mut options = self
            .objects
            .iter()
            .filter_map(|object| {
                let EditorKind::Marker(marker) = &object.kind else {
                    return None;
                };
                let id = object.server_id?;
                if !needle.is_empty() && !object.name.to_lowercase().contains(&needle) {
                    return None;
                }
                let mut label = object.name.clone();
                if !self.marker_is_animatable(marker) {
                    label.push_str(" · tidak bisa bergerak");
                }
                if used.contains(&id) {
                    label.push_str(" · dipakai rute lain");
                }
                Some((Some(id), label))
            })
            .collect::<Vec<_>>();
        options.sort_by_key(|left| left.1.to_lowercase());
        let mut result = vec![(None, "— Pilih marker —".to_owned())];
        result.extend(options);
        result
    }

    /// Assigns the marker picked on the map to a route and marks the line dirty.
    pub fn assign_route_marker(&mut self, route_key: ObjectKey, marker_id: Uuid) {
        let mut assigned = false;
        for object in &mut self.objects {
            if object.key != route_key {
                continue;
            }
            if let EditorKind::Line(line) = &mut object.kind
                && let Some(animation) = &mut line.animation
            {
                animation.marker_id = Some(marker_id);
                object.dirty = true;
                assigned = true;
            }
            break;
        }
        if assigned {
            self.pick_route_marker = None;
            self.dirty = true;
            self.touch();
            self.log("Marker rute dipilih dari peta.");
        }
    }

    /// Route keys ordered by the persisted Play order. Ties keep creation
    /// order so legacy scenarios without order metadata stay stable.
    pub fn ordered_route_keys(&self) -> Vec<ObjectKey> {
        let mut routes = self
            .objects
            .iter()
            .filter(|object| object.is_animation_route())
            .map(|object| (object.key, object.route_order()))
            .collect::<Vec<_>>();
        routes.sort_by_key(|(_, order)| *order);
        routes.into_iter().map(|(key, _)| key).collect()
    }

    /// Moves a route one slot up (`-1`) or down (`+1`) in the Play order and
    /// rewrites sequential order values, marking changed lines dirty so the
    /// order is saved with the scenario.
    pub fn move_route_order(&mut self, key: ObjectKey, delta: isize) -> bool {
        let mut keys = self.ordered_route_keys();
        let Some(index) = keys.iter().position(|candidate| *candidate == key) else {
            return false;
        };
        let target = index as isize + delta;
        if target < 0 || target as usize >= keys.len() {
            return false;
        }
        let target = target as usize;
        self.push_undo();
        keys.swap(index, target);
        for (position, route_key) in keys.iter().enumerate() {
            let Some(object) = self.object_mut(*route_key) else {
                continue;
            };
            if let EditorKind::Line(line) = &mut object.kind
                && let Some(animation) = &mut line.animation
            {
                let position = position as i32;
                if animation.order != position {
                    animation.order = position;
                    object.dirty = true;
                }
            }
        }
        self.dirty = true;
        true
    }

    /// Next Play order value for a new or duplicated route.
    fn next_route_order(&self) -> i32 {
        self.objects
            .iter()
            .filter_map(|object| match &object.kind {
                EditorKind::Line(line) => line.animation.as_ref().map(|route| route.order),
                _ => None,
            })
            .max()
            .map_or(0, |max| max.saturating_add(1))
    }

    pub fn add_polygon(&mut self, points: Vec<GeoPoint>) {
        if points.len() < 3 {
            return;
        }
        self.push_undo();
        let key = self.alloc_key();
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: format!("Custom mesh {key}"),
            visible: true,
            dirty: true,
            kind: EditorKind::Building(BuildingObject::custom(points)),
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
    }

    /// Creates a primitive shape (box, cylinder, sphere, cone, plane, custom).
    pub fn add_shape(&mut self, shape: EditorShapeKind, point: GeoPoint) {
        self.push_undo();
        let key = self.alloc_key();
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: format!("{} {key}", shape.label()),
            visible: true,
            dirty: true,
            kind: EditorKind::Building(BuildingObject::primitive(shape, point)),
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
    }

    /// Creates an animated particle effect (fire/smoke/water).
    pub fn add_particle(&mut self, point: GeoPoint) {
        self.push_undo();
        let key = self.alloc_key();
        let kind = self.active_particle;
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: format!("{} {key}", particle_kind_label(kind)),
            visible: true,
            dirty: true,
            kind: EditorKind::Particle(ParticleObject {
                lon: point.longitude,
                lat: point.latitude,
                altitude: 0.0,
                kind,
                settings: ParticleSettings::default(),
            }),
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
    }

    /// Asks for confirmation before removing the selected objects. Used by the
    /// context menu, inspector, menu bar, and Delete shortcut.
    pub fn request_delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.confirm_delete = true;
    }

    pub fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        if self
            .point_edit
            .is_some_and(|edit| self.selected.contains(&edit.key))
        {
            self.point_edit = None;
        }
        self.push_undo();
        let selected = self.selected.clone();
        for key in &selected {
            self.locked.remove(key);
            if let Some(index) = self.objects.iter().position(|object| object.key == *key) {
                let object = self.objects.remove(index);
                if let Some(id) = object.server_id {
                    self.deleted.push((object.object_type(), id));
                }
            }
        }
        self.selected.clear();
        self.dirty = true;
        self.touch();
    }

    pub fn duplicate_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let originals = self
            .objects
            .iter()
            .filter(|object| self.selected.contains(&object.key))
            .cloned()
            .collect::<Vec<_>>();
        let mut new_selection = BTreeSet::new();
        let mut next_order = self.next_route_order();
        for mut object in originals {
            let key = self.alloc_key();
            object.key = key;
            object.server_id = None;
            object.dirty = true;
            object.name = format!("{} copy", object.name);
            // A route copy must not fight the original over the same marker,
            // and it goes to the end of the Play order.
            if let EditorKind::Line(line) = &mut object.kind
                && let Some(animation) = &mut line.animation
            {
                animation.marker_id = None;
                animation.order = next_order;
                next_order = next_order.saturating_add(1);
            }
            offset_object(&mut object.kind, 8.0, -8.0);
            new_selection.insert(key);
            self.objects.push(object);
        }
        self.selected = new_selection;
        self.dirty = true;
        self.touch();
    }

    fn alloc_key(&mut self) -> ObjectKey {
        let key = self.next_key;
        self.next_key = self.next_key.wrapping_add(1).max(1);
        key
    }

    /// True when `object` supports on-map point editing.
    fn supports_point_edit(object: &EditorObject) -> bool {
        match &object.kind {
            EditorKind::Line(_) => true,
            EditorKind::Building(building) => building.shape == EditorShapeKind::Custom,
            _ => false,
        }
    }

    /// Starts on-map point editing for `key` (line or custom mesh).
    pub fn begin_point_edit(&mut self, key: ObjectKey) {
        let Some(object) = self.object(key) else {
            return;
        };
        if !Self::supports_point_edit(object) {
            return;
        }
        self.point_edit = Some(PointEditState { key, point: None });
        self.selected.clear();
        self.selected.insert(key);
        if self.tool != EditorTool::Move {
            self.set_tool(EditorTool::Move);
        }
        self.last_click = None;
        self.touch();
    }

    pub fn end_point_edit(&mut self) {
        if self.point_edit.take().is_some() {
            self.last_click = None;
            self.touch();
        }
    }

    pub fn point_edit_active(&self, key: ObjectKey) -> bool {
        self.point_edit.is_some_and(|edit| edit.key == key)
    }

    /// Custom mesh points are flat, so their move gizmo has no Y axis.
    pub fn point_edit_allows_altitude(&self) -> bool {
        self.point_edit.is_some_and(|edit| {
            self.object(edit.key)
                .is_some_and(|object| matches!(object.kind, EditorKind::Line(_)))
        })
    }

    /// Transform used by the gizmo. In point-edit mode this is the selected
    /// vertex instead of a whole object.
    pub fn gizmo_target(&self) -> Option<(ObjectKey, ObjectTransform)> {
        if let Some(edit) = self.point_edit {
            let object = self.object(edit.key)?;
            let index = edit.point?;
            return match &object.kind {
                EditorKind::Line(line) => {
                    let point = line.points.get(index)?;
                    Some((
                        edit.key,
                        ObjectTransform {
                            lon: point[0],
                            lat: point[1],
                            altitude: line.altitude(index),
                            rot: [0.0; 3],
                            scale: [1.0; 3],
                        },
                    ))
                }
                EditorKind::Building(building) if building.shape == EditorShapeKind::Custom => {
                    let point = building.points.get(index)?;
                    Some((
                        edit.key,
                        ObjectTransform {
                            lon: point[0],
                            lat: point[1],
                            altitude: building.altitude,
                            rot: [0.0; 3],
                            scale: [1.0; 3],
                        },
                    ))
                }
                _ => None,
            };
        }
        self.primary_transform()
    }

    pub fn set_point_edit_index(&mut self, index: usize) {
        let Some(edit) = self.point_edit else {
            return;
        };
        let Some(object) = self.object(edit.key) else {
            return;
        };
        let count = match &object.kind {
            EditorKind::Line(line) => line.points.len(),
            EditorKind::Building(building) => building.points.len(),
            _ => return,
        };
        if index >= count {
            return;
        }
        self.point_edit = Some(PointEditState {
            key: edit.key,
            point: Some(index),
        });
        self.last_click = None;
        self.touch();
    }

    /// Inserts a point on segment `segment` at fraction `t` (0..1).
    pub fn insert_edit_point(&mut self, key: ObjectKey, segment: usize, t: f64) -> bool {
        let Some(object) = self.object(key) else {
            return false;
        };
        match &object.kind {
            EditorKind::Line(_) => self.insert_line_point(key, segment, t),
            EditorKind::Building(building) if building.shape == EditorShapeKind::Custom => {
                self.insert_custom_point(key, segment, t)
            }
            _ => false,
        }
    }

    fn insert_line_point(&mut self, key: ObjectKey, segment: usize, t: f64) -> bool {
        let Some(object) = self.object(key) else {
            return false;
        };
        let EditorKind::Line(line) = &object.kind else {
            return false;
        };
        if segment + 1 >= line.points.len() {
            return false;
        }
        let start = GeoPoint::new(line.points[segment][0], line.points[segment][1]);
        let end = GeoPoint::new(line.points[segment + 1][0], line.points[segment + 1][1]);
        let Ok(offset) = end.local_offset(start) else {
            return false;
        };
        let t = t.clamp(0.0, 1.0);
        let Ok(point) =
            GeoPoint::from_local_offset(start, offset.east_meters * t, offset.north_meters * t)
        else {
            return false;
        };
        let altitude = line.altitude(segment) * (1.0 - t) + line.altitude(segment + 1) * t;

        self.push_undo();
        let Some(object) = self.object_mut(key) else {
            return false;
        };
        let EditorKind::Line(line) = &mut object.kind else {
            return false;
        };
        let index = segment + 1;
        line.points.insert(index, [point.longitude, point.latitude]);
        line.altitudes.insert(index, altitude);
        object.dirty = true;
        self.point_edit = Some(PointEditState {
            key,
            point: Some(index),
        });
        self.dirty = true;
        self.touch();
        true
    }

    fn insert_custom_point(&mut self, key: ObjectKey, segment: usize, t: f64) -> bool {
        let Some(object) = self.object(key) else {
            return false;
        };
        let EditorKind::Building(building) = &object.kind else {
            return false;
        };
        if building.shape != EditorShapeKind::Custom || segment + 1 >= building.points.len() {
            return false;
        }
        let start = GeoPoint::new(building.points[segment][0], building.points[segment][1]);
        let end = GeoPoint::new(
            building.points[segment + 1][0],
            building.points[segment + 1][1],
        );
        let Ok(offset) = end.local_offset(start) else {
            return false;
        };
        let t = t.clamp(0.0, 1.0);
        let Ok(point) =
            GeoPoint::from_local_offset(start, offset.east_meters * t, offset.north_meters * t)
        else {
            return false;
        };

        self.push_undo();
        let Some(object) = self.object_mut(key) else {
            return false;
        };
        let EditorKind::Building(building) = &mut object.kind else {
            return false;
        };
        let index = segment + 1;
        building
            .points
            .insert(index, [point.longitude, point.latitude]);
        object.dirty = true;
        self.point_edit = Some(PointEditState {
            key,
            point: Some(index),
        });
        self.dirty = true;
        self.touch();
        true
    }

    pub fn remove_point_edit_point(&mut self) -> bool {
        let Some(edit) = self.point_edit else {
            return false;
        };
        let Some(index) = edit.point else {
            return false;
        };
        let Some(object) = self.object(edit.key) else {
            return false;
        };
        let (count, minimum) = match &object.kind {
            EditorKind::Line(line) => {
                if index >= line.points.len() {
                    return false;
                }
                (line.points.len(), 2)
            }
            EditorKind::Building(building) if building.shape == EditorShapeKind::Custom => {
                if index >= building.points.len() {
                    return false;
                }
                (building.points.len(), 3)
            }
            _ => return false,
        };
        if count <= minimum {
            return false;
        }

        self.push_undo();
        let Some(object) = self.object_mut(edit.key) else {
            return false;
        };
        let next = match &mut object.kind {
            EditorKind::Line(line) => {
                line.points.remove(index);
                if index < line.altitudes.len() {
                    line.altitudes.remove(index);
                }
                index.min(line.points.len() - 1)
            }
            EditorKind::Building(building) => {
                building.points.remove(index);
                index.min(building.points.len() - 1)
            }
            _ => return false,
        };
        object.dirty = true;
        self.point_edit = Some(PointEditState {
            key: edit.key,
            point: Some(next),
        });
        self.dirty = true;
        self.touch();
        true
    }

    pub fn snap_point(&self, point: GeoPoint) -> GeoPoint {
        if self.snap_meters <= 0.0 {
            return point;
        }
        let Some((x, z)) = self.map_state.geo_to_world(point) else {
            return point;
        };
        let snapped = (
            (x / self.snap_meters).round() * self.snap_meters,
            (z / self.snap_meters).round() * self.snap_meters,
        );
        self.map_state
            .world_to_geo(snapped.0, snapped.1)
            .unwrap_or(point)
    }

    /// Resolves an asset id to the path the renderer should load for a shape
    /// texture. `/uploads/...` references from older scenarios are still served
    /// from the server URL.
    pub fn texture_source(&self, reference: &str) -> Option<String> {
        if reference.starts_with("/uploads/") {
            let base = self.base_url.trim_end_matches('/');
            return (!base.is_empty()).then(|| format!("{base}{reference}"));
        }
        self.texture_paths.get(reference).cloned()
    }

    /// Cached egui preview for a shape texture, or `None` when the image is
    /// not available locally (un-downloaded asset, legacy upload, web build).
    pub fn texture_preview(
        &mut self,
        ctx: &egui::Context,
        reference: &str,
    ) -> Option<egui::TextureHandle> {
        let path = self.texture_files.get(reference).map(PathBuf::from)?;
        let cache_key = path.display().to_string();
        if let Some(handle) = self.texture_previews.get(&cache_key) {
            return Some(handle.clone());
        }
        let handle = textures::load_preview(ctx, &path)?;
        self.texture_previews.insert(cache_key, handle.clone());
        Some(handle)
    }

    /// Cached egui preview for a saved screenshot file (asset tooltip).
    pub fn stored_preview(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
    ) -> Option<egui::TextureHandle> {
        let cache_key = path.display().to_string();
        if let Some(handle) = self.texture_previews.get(&cache_key) {
            return Some(handle.clone());
        }
        let handle = textures::load_preview(ctx, path)?;
        self.texture_previews.insert(cache_key, handle.clone());
        Some(handle)
    }

    pub fn build_scene(&self) -> EditorScene {
        let mut objects = Vec::new();
        for object in &self.objects {
            if !object.visible || !self.visible.allows(object.object_type()) {
                continue;
            }
            // Routes are authoring guides; hide them while the scenario plays.
            if self.preview && object.is_animation_route() {
                continue;
            }
            let Some(point) = object.point() else {
                continue;
            };
            // Play moves markers at runtime without touching the authored
            // position. Rebuilds during Play (selection changes) must therefore
            // use the current Play position instead of snapping back.
            let point = if self.preview {
                self.play
                    .units
                    .get(&object.key)
                    .map(|unit| unit.position)
                    .or_else(|| self.play.limited.get(&object.key).map(|unit| unit.position))
                    .unwrap_or(point)
            } else {
                point
            };
            let selected = self.selected.contains(&object.key);
            let kind = match &object.kind {
                EditorKind::Marker(marker) => {
                    let entry = UNIT_CATALOG[marker.unit.min(UNIT_CATALOG.len() - 1)];
                    let asset_model = self.marker_models.get(&marker.unit_id);
                    // In Play mode an animated asset is shown with its animated
                    // file so the walk clip can loop while the unit moves. The
                    // editor viewport keeps using the idle file.
                    let animated_model = if self.preview {
                        asset_model.filter(|model| model.animated && model.animation_path.is_some())
                    } else {
                        None
                    };
                    let play_idle = if animated_model.is_some() {
                        asset_model.map(|model| EditorPlayModel {
                            model: model.path.clone(),
                            draco: model.draco,
                        })
                    } else {
                        None
                    };
                    let model = animated_model
                        .and_then(|model| model.animation_path.clone())
                        .or_else(|| asset_model.map(|model| model.path.clone()))
                        .unwrap_or_else(|| entry.model.to_owned());
                    let draco = animated_model.map_or_else(
                        || asset_model.is_some_and(|model| model.draco),
                        |model| model.animation_draco,
                    );
                    EditorSceneKind::Marker {
                        model,
                        base_scale: asset_model.map_or(entry.base_scale, |model| model.base_scale),
                        normalize_height: None,
                        draco,
                        scene_asset: asset_model.is_some(),
                        play_idle,
                        rotation_deg: [
                            marker.rot_x as f32,
                            marker.rot_y as f32,
                            marker.rot_z as f32,
                        ],
                        scale: [
                            marker.scale_x as f32,
                            marker.scale_y as f32,
                            marker.scale_z as f32,
                        ],
                        altitude: marker.altitude as f32,
                        color: parse_hex_color(marker_color(&marker.kategori)),
                    }
                }
                EditorKind::Line(line) => EditorSceneKind::Line {
                    points: line.points.clone(),
                    altitudes: line.altitudes.iter().map(|value| *value as f32).collect(),
                    width: line.width as f32,
                    color: parse_hex_color(&line.color),
                    alpha: line.alpha as f32,
                },
                EditorKind::Building(building) => {
                    let texture = if building.use_texture {
                        building
                            .texture
                            .as_ref()
                            .and_then(|reference| self.texture_source(reference))
                    } else {
                        None
                    };
                    EditorSceneKind::Building {
                        shape: building.shape,
                        points: building.points.clone(),
                        height: building.render_height() as f32,
                        color: parse_hex_color(&building.color),
                        rotation_deg: [
                            building.rot_x as f32,
                            building.rot_y as f32,
                            building.rot_z as f32,
                        ],
                        scale: [
                            building.scale_x as f32,
                            building.scale_y as f32,
                            building.scale_z as f32,
                        ],
                        altitude: building.altitude as f32,
                        texture,
                    }
                }
                EditorKind::Text(text) => EditorSceneKind::Text {
                    color: parse_hex_color(&text.color),
                    font_size: text.font_size as f32,
                },
                EditorKind::Particle(particle) => EditorSceneKind::Particle {
                    kind: scene_particle_kind(particle.kind),
                    altitude: particle.altitude as f32,
                    settings: scene_particle_settings(particle.settings),
                },
            };
            objects.push(EditorSceneObject {
                key: object.key,
                label: object.name.clone(),
                lon: point.longitude,
                lat: point.latitude,
                selected,
                kind,
            });
        }

        let grid = if self.indoor || self.show_grid {
            Some(EditorGrid {
                center: [
                    self.scenario_center.longitude,
                    self.scenario_center.latitude,
                ],
                spacing_m: self.grid_spacing.max(0.25),
                extent_m: self.grid_extent.max(10.0),
                floor_plan: self.indoor && self.floor_plan,
                floor_size_m: self.floor_size,
            })
        } else {
            None
        };

        // Select mode shows only the selection outline (see the viewport
        // overlay); the transform gizmo belongs to Move/Rotate/Scale so the two
        // modes are no longer confusable.
        let gizmo = if self.preview {
            // Play mode shows a move gizmo for the selected limited asset,
            // including the Y handle so it can be lifted.
            self.play_gizmo_target()
                .map(|(key, point, altitude)| EditorGizmo {
                    lon: point.longitude,
                    lat: point.latitude,
                    altitude,
                    mode: EditorGizmoMode::Move,
                    active_axis: self.hover_axis,
                    show_y: true,
                    play_key: Some(key),
                })
        } else if !self.tool.changes_gizmo() {
            None
        } else {
            self.gizmo_target().and_then(|(key, transform)| {
                let mode = match self.tool {
                    EditorTool::Rotate => EditorGizmoMode::Rotate,
                    EditorTool::Scale => EditorGizmoMode::Scale,
                    _ => EditorGizmoMode::Move,
                };
                // Text and particles have no rotation/scale control: text size
                // comes from Font and a particle effect is billboarded. Only the
                // Move gizmo applies to them.
                let is_flat = matches!(
                    self.object(key).map(|object| &object.kind),
                    Some(EditorKind::Text(_) | EditorKind::Particle(_))
                );
                let is_point_edit = self.point_edit.is_some();
                if (is_flat || is_point_edit) && mode != EditorGizmoMode::Move {
                    return None;
                }
                Some(EditorGizmo {
                    lon: transform.lon,
                    lat: transform.lat,
                    altitude: transform.altitude as f32,
                    mode,
                    active_axis: self.hover_axis,
                    show_y: !is_point_edit || self.point_edit_allows_altitude(),
                    play_key: None,
                })
            })
        };

        EditorScene {
            revision: self.render_revision,
            indoor: self.indoor || !self.visible.raster,
            objects,
            grid,
            gizmo,
            // Playback always shows textures, even if the editor is in solid or
            // wireframe shading.
            model_shading: if self.preview {
                EditorModelShading::Textured
            } else {
                self.model_shading
            },
        }
    }
}

/// Compares camera views with tolerances small enough that float noise from
/// the camera flight does not flip the Inspector status back to unsaved.
fn same_camera(a: MapCameraState, b: MapCameraState) -> bool {
    const POSITION_EPSILON: f64 = 1e-7;
    const ZOOM_EPSILON: f32 = 0.005;
    const ANGLE_EPSILON: f32 = 0.05;
    (a.center.longitude - b.center.longitude).abs() <= POSITION_EPSILON
        && (a.center.latitude - b.center.latitude).abs() <= POSITION_EPSILON
        && (a.zoom - b.zoom).abs() <= ZOOM_EPSILON
        && (a.pitch_degrees - b.pitch_degrees).abs() <= ANGLE_EPSILON
        && (a.bearing_degrees - b.bearing_degrees).abs() <= ANGLE_EPSILON
}

fn points_centroid(points: &[[f64; 2]]) -> Option<GeoPoint> {
    if points.is_empty() {
        return None;
    }
    let count = points.len() as f64;
    let longitude = points.iter().map(|point| point[0]).sum::<f64>() / count;
    let latitude = points.iter().map(|point| point[1]).sum::<f64>() / count;
    Some(GeoPoint::new(longitude, latitude))
}

fn centroid_of(points: &[GeoPoint]) -> Option<GeoPoint> {
    if points.is_empty() {
        return None;
    }
    let count = points.len() as f64;
    let longitude = points.iter().map(|point| point.longitude).sum::<f64>() / count;
    let latitude = points.iter().map(|point| point.latitude).sum::<f64>() / count;
    Some(GeoPoint::new(longitude, latitude))
}

/// Full span (meters) of the footprint, used for the focus zoom.
fn points_extent(points: &[[f64; 2]]) -> f64 {
    let Some(first) = points.first() else {
        return 0.0;
    };
    let origin = GeoPoint::new(first[0], first[1]);
    let mut max_east = 0.0_f64;
    let mut max_north = 0.0_f64;
    for point in points {
        let Ok(offset) = GeoPoint::new(point[0], point[1]).local_offset(origin) else {
            continue;
        };
        max_east = max_east.max(offset.east_meters.abs());
        max_north = max_north.max(offset.north_meters.abs());
    }
    (max_east.max(max_north) * 2.0).max(0.0)
}

fn play_speed(kategori: &str) -> f64 {
    match kategori {
        "personel" => 3.5,
        "kendaraan" => 9.0,
        _ => 5.0,
    }
}

fn polyline_length(points: &[GeoPoint]) -> f64 {
    points
        .windows(2)
        .filter_map(|pair| pair[1].local_offset(pair[0]).ok())
        .map(|offset| offset.east_meters.hypot(offset.north_meters))
        .sum()
}

/// Position, altitude, and heading at `distance` metres along a polyline.
fn route_sample(
    points: &[GeoPoint],
    altitudes: &[f64],
    distance: f64,
) -> Option<(GeoPoint, f64, f32)> {
    if points.len() < 2 {
        return None;
    }
    let mut remaining = distance.max(0.0);
    let last = points.len() - 2;
    for index in 0..=last {
        let from = points[index];
        let to = points[index + 1];
        let Ok(offset) = to.local_offset(from) else {
            continue;
        };
        let segment = offset.east_meters.hypot(offset.north_meters);
        let heading = offset.east_meters.atan2(offset.north_meters).to_degrees() as f32;
        if remaining <= segment || index == last {
            let t = if segment > 1.0e-6 {
                (remaining / segment).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let position =
                GeoPoint::from_local_offset(from, offset.east_meters * t, offset.north_meters * t)
                    .unwrap_or(to);
            let start_altitude = altitudes.get(index).copied().unwrap_or(0.0);
            let end_altitude = altitudes.get(index + 1).copied().unwrap_or(start_altitude);
            return Some((
                position,
                start_altitude + (end_altitude - start_altitude) * t,
                heading,
            ));
        }
        remaining -= segment;
    }
    None
}

/// Ground-plane radius of a 3D shape used for Play obstacle avoidance. Custom
/// meshes use the farthest footprint vertex, so the circle encloses the whole
/// outline and units cannot clip through a corner.
fn building_obstacle_radius(building: &BuildingObject) -> f64 {
    let radius = if building.shape == EditorShapeKind::Custom && !building.points.is_empty() {
        building
            .points
            .iter()
            .filter_map(|point| {
                let offset = GeoPoint::new(point[0], point[1])
                    .local_offset(building.point())
                    .ok()?;
                Some(offset.east_meters.hypot(offset.north_meters))
            })
            .fold(0.0f64, f64::max)
    } else {
        let (x, z) = (building.scale_x.abs(), building.scale_z.abs());
        match building.shape {
            // Round cross-sections keep the scale as their diameter.
            EditorShapeKind::Cylinder | EditorShapeKind::Sphere | EditorShapeKind::Cone => {
                x.max(z) * 0.5
            }
            // A rotated box can expose its corners, so use the half diagonal.
            _ => (x.hypot(z)) * 0.5,
        }
    };
    radius.max(0.5)
}

/// Steering direction (east, north unit vector) toward the target, deflected
/// sideways when an obstacle is ahead. `self_key` skips the moving unit's own
/// entry in the combined static + unit obstacle list.
fn play_steer(
    to_target: twg_map::LocalPoint,
    position: GeoPoint,
    obstacles: &[PlayObstacle],
    self_key: ObjectKey,
    unit_radius: f64,
) -> (f64, f64) {
    let distance = to_target.east_meters.hypot(to_target.north_meters);
    if distance <= 1.0e-6 {
        return (0.0, 0.0);
    }
    let mut east = to_target.east_meters / distance;
    let mut north = to_target.north_meters / distance;
    for obstacle in obstacles {
        if obstacle.key == Some(self_key) {
            continue;
        }
        let Ok(relative) = obstacle.center.local_offset(position) else {
            continue;
        };
        let center_distance = relative
            .east_meters
            .hypot(relative.north_meters)
            .max(1.0e-6);
        let surface = center_distance - obstacle.radius - unit_radius;
        if surface > PLAY_LOOKAHEAD_METERS {
            continue;
        }
        // Only steer around obstacles that are in front of the unit.
        if relative.east_meters * east + relative.north_meters * north <= 0.0 {
            continue;
        }
        let strength = (1.0 - surface / PLAY_LOOKAHEAD_METERS).clamp(0.0, 1.0) * 1.8;
        let cross = east * relative.north_meters - north * relative.east_meters;
        let (side_east, side_north) = if cross >= 0.0 {
            (north, -east)
        } else {
            (-north, east)
        };
        east += side_east * strength;
        north += side_north * strength;
    }
    let length = east.hypot(north).max(1.0e-6);
    (east / length, north / length)
}

pub fn offset_object(kind: &mut EditorKind, east_meters: f64, north_meters: f64) {
    match kind {
        EditorKind::Marker(marker) => {
            if let Ok(point) =
                GeoPoint::from_local_offset(marker.point(), east_meters, north_meters)
            {
                marker.set_point(point);
            }
        }
        EditorKind::Line(line) => offset_points(&mut line.points, east_meters, north_meters),
        EditorKind::Building(building) => building.translate(east_meters, north_meters),
        EditorKind::Text(text) => {
            if let Ok(point) = GeoPoint::from_local_offset(text.point(), east_meters, north_meters)
            {
                text.set_point(point);
            }
        }
        EditorKind::Particle(particle) => {
            if let Ok(point) =
                GeoPoint::from_local_offset(particle.point(), east_meters, north_meters)
            {
                particle.set_point(point);
            }
        }
    }
}

fn offset_points(points: &mut [[f64; 2]], east_meters: f64, north_meters: f64) {
    for point in points {
        let geo = GeoPoint::new(point[0], point[1]);
        if let Ok(shifted) = GeoPoint::from_local_offset(geo, east_meters, north_meters) {
            point[0] = shifted.longitude;
            point[1] = shifted.latitude;
        }
    }
}

pub fn marker_color(category: &str) -> &'static str {
    match category {
        "kendaraan" => "#ffb74d",
        "markas" => "#81c784",
        "personel" => "#4fc3f7",
        _ => "#b0bec5",
    }
}

pub fn particle_kind_label(kind: ParticleKind) -> &'static str {
    match kind {
        ParticleKind::Fire => "Api",
        ParticleKind::Smoke => "Asap",
        ParticleKind::Water => "Air Mancur",
    }
}

pub fn scene_particle_kind(kind: ParticleKind) -> EditorParticleKind {
    match kind {
        ParticleKind::Fire => EditorParticleKind::Fire,
        ParticleKind::Smoke => EditorParticleKind::Smoke,
        ParticleKind::Water => EditorParticleKind::Water,
    }
}

pub fn scene_particle_settings(settings: ParticleSettings) -> EditorParticleSettings {
    EditorParticleSettings {
        size: settings.size as f32,
        density: settings.density as f32,
        height: settings.height as f32,
        spread: settings.spread as f32,
        speed: settings.speed as f32,
        opacity: settings.opacity as f32,
    }
}

pub fn particle_color(particle: &ParticleObject) -> &'static str {
    match particle.kind {
        ParticleKind::Fire => "#ff7043",
        ParticleKind::Smoke => "#b0bec5",
        ParticleKind::Water => "#4fc3f7",
    }
}

pub fn parse_hex_color(hex: &str) -> [f32; 4] {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return [0.85, 0.85, 0.85, 1.0];
    }
    let parse = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&hex[range], 16).unwrap_or(0) as f32 / 255.0
    };
    [parse(0..2), parse(2..4), parse(4..6), 1.0]
}

pub fn hex_from_color(color: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> EditorState {
        let mut editor = EditorState::new();
        editor.loaded = true;
        editor
    }

    #[test]
    fn delete_requests_confirmation_before_removing_objects() {
        let mut editor = editor();
        editor.add_shape(EditorShapeKind::Box, GeoPoint::new(115.225, -8.648));
        let key = *editor.selected.iter().next().expect("shape selected");

        editor.request_delete_selected();
        assert!(editor.confirm_delete);
        assert!(editor.object(key).is_some());

        editor.confirm_delete = false;
        editor.delete_selected();
        assert!(editor.object(key).is_none());
    }

    #[test]
    fn import_draft_keeps_files_in_slot_order() {
        let mut draft = ImportDraft::default();
        assert_eq!(draft.slots(), &[AssetSourceSlot::Static]);
        draft.category = AssetCategory::Texture;
        draft.model_kind = AssetModelKind::Animated;
        assert_eq!(draft.slots(), &[AssetSourceSlot::Static]);

        draft.category = AssetCategory::Model;
        assert_eq!(
            draft.slots(),
            &[AssetSourceSlot::Idle, AssetSourceSlot::Animated]
        );
        draft.set_file(1, std::path::PathBuf::from("walk.glb"));
        assert_eq!(draft.name, "walk");
        assert!(draft.file(0).is_none());
        assert!(draft.file(1).is_some());
    }

    #[test]
    fn play_mode_forces_textured_shading() {
        let mut editor = editor();
        assert_eq!(
            editor.build_scene().model_shading,
            EditorModelShading::Textured
        );
        editor.model_shading = EditorModelShading::Wireframe;
        assert_eq!(
            editor.build_scene().model_shading,
            EditorModelShading::Wireframe
        );
        editor.preview = true;
        assert_eq!(
            editor.build_scene().model_shading,
            EditorModelShading::Textured
        );
    }

    #[test]
    fn asset_markers_resolve_the_cached_model_by_unit_id() {
        let mut editor = editor();
        let unit_id = Uuid::new_v4().to_string();
        editor.marker_models.insert(
            unit_id.clone(),
            MarkerAssetModel {
                name: "Truk Logistik".to_owned(),
                path: format!("twg-cache://{unit_id}/static/original.glb"),
                base_scale: 1.0,
                footprint_radius: 0.0,
                draco: false,
                animated: false,
                move_mode: AssetMoveMode::None,
                animation_path: None,
                animation_draco: false,
            },
        );
        editor.add_asset_marker(
            &AssetMarkerDraft {
                unit_id: unit_id.clone(),
                name: "Truk Logistik".to_owned(),
            },
            GeoPoint::new(115.225, -8.648),
        );
        let scene = editor.build_scene();
        match &scene.objects[0].kind {
            EditorSceneKind::Marker {
                model,
                normalize_height,
                scene_asset,
                ..
            } => {
                assert_eq!(model, &format!("twg-cache://{unit_id}/static/original.glb"));
                assert_eq!(*normalize_height, None);
                assert!(*scene_asset);
            }
            other => panic!("expected marker, got {other:?}"),
        }
        let EditorKind::Marker(marker) = &editor.objects[0].kind else {
            panic!("expected marker");
        };
        assert_eq!(marker.unit_id, unit_id);
        assert_eq!(marker.rot_x, 0.0);
        assert_eq!(marker.altitude, 0.0);
    }

    #[test]
    fn asset_marker_without_cached_model_falls_back_to_the_catalog() {
        let mut editor = editor();
        editor.add_asset_marker(
            &AssetMarkerDraft {
                unit_id: "11111111-1111-4111-8111-111111111111".to_owned(),
                name: "Belum disinkron".to_owned(),
            },
            GeoPoint::new(115.225, -8.648),
        );
        let scene = editor.build_scene();
        match &scene.objects[0].kind {
            EditorSceneKind::Marker {
                model,
                normalize_height,
                scene_asset,
                ..
            } => {
                assert_eq!(model, UNIT_CATALOG[0].model);
                assert_eq!(*normalize_height, None);
                assert!(!*scene_asset);
            }
            other => panic!("expected marker, got {other:?}"),
        }
    }

    #[test]
    fn particle_uses_the_selected_kind_and_can_be_deleted() {
        let mut editor = editor();
        editor.active_particle = ParticleKind::Water;
        editor.add_particle(GeoPoint::new(115.225, -8.648));

        let scene = editor.build_scene();
        match &scene.objects[0].kind {
            EditorSceneKind::Particle {
                kind,
                altitude,
                settings,
            } => {
                assert_eq!(*kind, EditorParticleKind::Water);
                assert_eq!(*altitude, 0.0);
                assert_eq!(*settings, EditorParticleSettings::default());
            }
            other => panic!("expected particle, got {other:?}"),
        }
        let EditorKind::Particle(particle) = &editor.objects[0].kind else {
            panic!("expected particle");
        };
        assert_eq!(particle.kind, ParticleKind::Water);
        assert!(editor.objects[0].name.starts_with("Air Mancur"));

        editor.delete_selected();
        assert!(editor.objects.is_empty());
    }

    #[test]
    fn particle_settings_round_trip_and_clamp() {
        let settings = ParticleSettings {
            size: 1.75,
            density: 0.5,
            height: 2.25,
            spread: 1.2,
            speed: 1.8,
            opacity: 0.7,
        };
        assert_eq!(ParticleSettings::from_json(&settings.to_json()), settings);

        let clamped = ParticleSettings::from_json("{\"size\":99,\"density\":0}");
        assert_eq!(*ParticleSettings::SIZE_RANGE.end(), clamped.size);
        assert_eq!(*ParticleSettings::DENSITY_RANGE.start(), clamped.density);
        assert_eq!(
            ParticleSettings::from_json("not json"),
            ParticleSettings::default()
        );
    }

    #[test]
    fn add_marker_can_be_undone_and_redone() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        assert_eq!(editor.objects.len(), 1);
        assert!(editor.undo());
        assert!(editor.objects.is_empty());
        assert!(editor.redo());
        assert_eq!(editor.objects.len(), 1);
    }

    #[test]
    fn duplicate_offsets_the_copy() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        editor.duplicate_selected();
        assert_eq!(editor.objects.len(), 2);
        let points = editor
            .objects
            .iter()
            .filter_map(|object| object.point())
            .collect::<Vec<_>>();
        assert!(points[0].longitude != points[1].longitude);
        assert!(editor.objects[1].server_id.is_none());
    }

    #[test]
    fn deleting_a_saved_object_queues_a_server_delete() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        let id = Uuid::new_v4();
        editor.objects[0].server_id = Some(id);
        editor.selected.insert(editor.objects[0].key);
        editor.delete_selected();
        assert!(editor.objects.is_empty());
        assert_eq!(editor.deleted, vec![(ObjectType::Marker, id)]);
    }

    #[test]
    fn undo_restores_a_deleted_object_and_clears_the_pending_delete() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        let id = Uuid::new_v4();
        editor.objects[0].server_id = Some(id);
        editor.selected.insert(editor.objects[0].key);
        editor.delete_selected();
        editor.undo();
        assert_eq!(editor.objects.len(), 1);
        assert!(editor.deleted.is_empty());
    }

    #[test]
    fn selected_marker_emits_a_gizmo_only_for_transform_tools() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        // Select shows only the selection outline.
        assert!(editor.build_scene().gizmo.is_none());
        for (tool, mode) in [
            (EditorTool::Move, EditorGizmoMode::Move),
            (EditorTool::Rotate, EditorGizmoMode::Rotate),
            (EditorTool::Scale, EditorGizmoMode::Scale),
        ] {
            editor.set_tool(tool);
            let scene = editor.build_scene();
            assert_eq!(scene.objects.len(), 1);
            assert_eq!(scene.gizmo.as_ref().map(|gizmo| gizmo.mode), Some(mode));
        }
    }

    #[test]
    fn hidden_layers_are_removed_from_the_render_scene() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        editor.visible.marker = false;
        assert!(editor.build_scene().objects.is_empty());
    }

    #[test]
    fn locked_objects_do_not_show_a_gizmo() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        let key = *editor.selected.iter().next().expect("marker selected");
        editor.set_tool(EditorTool::Move);
        assert!(editor.primary_transform().is_some());

        editor.locked.insert(key);
        assert!(editor.primary_transform().is_none());
        assert!(editor.build_scene().gizmo.is_none());
    }

    #[test]
    fn snap_moves_the_point_at_most_half_the_grid() {
        let mut editor = editor();
        editor.snap_meters = 5.0;
        let point = GeoPoint::new(115.225_123, -8.648_456);
        let snapped = editor.snap_point(point);
        let offset = snapped.local_offset(point).unwrap();
        let distance = (offset.east_meters * offset.east_meters
            + offset.north_meters * offset.north_meters)
            .sqrt();
        assert!(distance <= 4.0, "snap moved {distance} m");
    }

    #[test]
    fn parse_hex_color_reads_rgb() {
        assert_eq!(parse_hex_color("#ff8000"), [1.0, 128.0 / 255.0, 0.0, 1.0]);
    }

    fn animated_asset(id: &str) -> MarkerAssetModel {
        MarkerAssetModel {
            name: "Unit Animasi".to_owned(),
            path: format!("twg-cache://{id}/idle/original.glb"),
            base_scale: 1.0,
            footprint_radius: 0.0,
            draco: false,
            animated: true,
            move_mode: AssetMoveMode::Free,
            animation_path: Some(format!("twg-cache://{id}/animated/original.glb")),
            animation_draco: false,
        }
    }

    fn static_asset(id: &str, move_mode: AssetMoveMode) -> MarkerAssetModel {
        MarkerAssetModel {
            name: "Model Statis".to_owned(),
            path: format!("twg-cache://{id}/static/original.glb"),
            base_scale: 1.0,
            footprint_radius: 0.0,
            draco: false,
            animated: false,
            move_mode,
            animation_path: None,
            animation_draco: false,
        }
    }

    fn add_static_marker(
        editor: &mut EditorState,
        id: &str,
        point: GeoPoint,
        move_mode: AssetMoveMode,
    ) -> ObjectKey {
        editor.add_marker(point, 0);
        let key = *editor.selected.iter().next().expect("new marker selected");
        if let Some(object) = editor.object_mut(key)
            && let EditorKind::Marker(marker) = &mut object.kind
        {
            marker.unit_id = id.to_owned();
        }
        editor
            .marker_models
            .insert(id.to_owned(), static_asset(id, move_mode));
        key
    }

    fn add_animated_marker(editor: &mut EditorState, id: &str, point: GeoPoint) -> ObjectKey {
        editor.add_marker(point, 0);
        let key = *editor.selected.iter().next().expect("new marker selected");
        if let Some(object) = editor.object_mut(key)
            && let EditorKind::Marker(marker) = &mut object.kind
        {
            marker.unit_id = id.to_owned();
        }
        editor
            .marker_models
            .insert(id.to_owned(), animated_asset(id));
        key
    }

    #[test]
    fn play_mode_uses_the_asset_move_mode() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        add_animated_marker(&mut editor, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", base);
        editor.add_marker(GeoPoint::new(115.226, -8.649), 0);

        editor.start_play();

        assert!(editor.play.active);
        assert_eq!(editor.play.units.len(), 1);
        assert_eq!(editor.play.obstacles.len(), 1);
        assert!(editor.play.limited.is_empty());
        let unit = editor.play.units.values().next().unwrap();
        assert_eq!(unit.position, base);
        assert!(unit.animated);
    }

    #[test]
    fn free_static_marker_becomes_a_play_unit() {
        let mut editor = editor();
        let point = GeoPoint::new(115.225, -8.648);
        let key = add_static_marker(
            &mut editor,
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            point,
            AssetMoveMode::Free,
        );
        editor.start_play();

        assert_eq!(editor.play.units.len(), 1);
        assert!(editor.play.obstacles.is_empty());
        let unit = editor.play.units.get(&key).expect("free static unit");
        assert!(!unit.animated);
    }

    #[test]
    fn route_sample_interpolates_distance_altitude_and_heading() {
        let start = GeoPoint::new(115.225, -8.648);
        let end = GeoPoint::from_local_offset(start, 100.0, 0.0).expect("east");
        let points = vec![start, end];
        let altitudes = vec![0.0, 10.0];

        assert!((polyline_length(&points) - 100.0).abs() < 0.5);
        let (mid, altitude, heading) = route_sample(&points, &altitudes, 50.0).expect("midpoint");
        let offset = mid.local_offset(start).expect("offset");
        assert!((offset.east_meters - 50.0).abs() < 0.5);
        assert!((altitude - 5.0).abs() < 0.5);
        assert!((heading - 90.0).abs() < 1.0);
    }

    #[test]
    fn route_animation_moves_the_selected_marker_at_constant_speed() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        let marker_key =
            add_animated_marker(&mut editor, "cccccccc-cccc-4ccc-8ccc-cccccccccccc", base);
        let marker_id = Uuid::new_v4();
        editor.object_mut(marker_key).expect("marker").server_id = Some(marker_id);
        let end = GeoPoint::from_local_offset(base, 10.0, 0.0).expect("east");
        editor.add_route(vec![base, end]);
        let route_key = *editor.selected.iter().next().expect("route selected");
        if let Some(object) = editor.object_mut(route_key)
            && let EditorKind::Line(line) = &mut object.kind
        {
            line.animation = Some(RouteAnimation {
                speed: 50.0,
                marker_id: Some(marker_id),
                repeat: false,
                order: 0,
            });
        }

        editor.start_play();
        assert_eq!(editor.play.animations.len(), 1);
        assert!(editor.play.controlled.contains(&marker_key));

        editor.play_route_animation(0);
        editor.play_tick(0.1);
        let unit = editor.play.units.get(&marker_key).expect("animated unit");
        let offset = unit.position.local_offset(base).expect("offset");
        assert!((offset.east_meters - 5.0).abs() < 0.8);
        assert!(unit.target.is_none());

        editor.stop_route_animation(0);
        let unit = editor.play.units.get(&marker_key).expect("animated unit");
        let offset = unit.position.local_offset(base).expect("offset");
        assert!(offset.east_meters.abs() < 0.1);
    }

    #[test]
    fn repeating_route_wraps_back_to_the_first_point() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        let marker_key =
            add_animated_marker(&mut editor, "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee", base);
        let marker_id = Uuid::new_v4();
        editor.object_mut(marker_key).expect("marker").server_id = Some(marker_id);
        let end = GeoPoint::from_local_offset(base, 10.0, 0.0).expect("east");
        editor.add_route(vec![base, end]);
        let route_key = *editor.selected.iter().next().expect("route selected");
        if let Some(object) = editor.object_mut(route_key)
            && let EditorKind::Line(line) = &mut object.kind
        {
            assert!((line.width - 0.5).abs() < 1.0e-9);
            line.animation = Some(RouteAnimation {
                speed: 50.0,
                marker_id: Some(marker_id),
                repeat: true,
                order: 0,
            });
        }

        editor.start_play();
        editor.play_route_animation(0);
        editor.play_tick(0.1);
        editor.play_tick(0.1);

        let animation = &editor.play.animations[0];
        assert!(
            animation.progress_m < 0.1,
            "progress {}",
            animation.progress_m
        );
        assert_eq!(animation.state, PlaybackState::Playing);
        let unit = editor.play.units.get(&marker_key).expect("animated unit");
        let offset = unit.position.local_offset(base).expect("offset");
        assert!(offset.east_meters.abs() < 0.1);
    }

    #[test]
    fn hidden_route_is_still_registered_for_play() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        let end = GeoPoint::from_local_offset(base, 10.0, 0.0).expect("east");
        editor.add_route(vec![base, end]);
        let route_key = *editor.selected.iter().next().expect("route selected");
        editor.object_mut(route_key).expect("route").visible = false;

        editor.start_play();
        assert_eq!(editor.play.animations.len(), 1);
    }

    #[test]
    fn route_order_moves_and_drives_play_order() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        let add = |editor: &mut EditorState, east: f64| {
            let end = GeoPoint::from_local_offset(base, east, 0.0).expect("east");
            editor.add_route(vec![base, end]);
            *editor.selected.iter().next().expect("route selected")
        };
        let first = add(&mut editor, 10.0);
        let second = add(&mut editor, 20.0);
        let third = add(&mut editor, 30.0);
        assert_eq!(editor.ordered_route_keys(), vec![first, second, third]);

        assert!(editor.move_route_order(third, -1));
        assert_eq!(editor.ordered_route_keys(), vec![first, third, second]);

        // The top row cannot move up and the last row cannot move down.
        assert!(!editor.move_route_order(first, -1));
        assert!(!editor.move_route_order(second, 1));

        editor.start_play();
        let play_order = editor
            .play
            .animations
            .iter()
            .map(|animation| animation.route_key)
            .collect::<Vec<_>>();
        assert_eq!(play_order, vec![first, third, second]);
    }

    #[test]
    fn route_marker_can_be_assigned_from_the_map() {
        let mut editor = editor();
        let point = GeoPoint::new(115.225, -8.648);
        let marker_key = add_static_marker(
            &mut editor,
            "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            point,
            AssetMoveMode::Free,
        );
        let marker_id = Uuid::new_v4();
        editor.object_mut(marker_key).expect("marker").server_id = Some(marker_id);
        let end = GeoPoint::from_local_offset(point, 10.0, 0.0).expect("east");
        editor.add_route(vec![point, end]);
        let route_key = *editor.selected.iter().next().expect("route selected");

        let options = editor.animation_marker_options(route_key);
        assert!(options.iter().any(|(id, _)| *id == Some(marker_id)));

        editor.pick_route_marker = Some(route_key);
        editor.assign_route_marker(route_key, marker_id);
        assert!(editor.pick_route_marker.is_none());
        let object = editor.object(route_key).expect("route");
        let EditorKind::Line(line) = &object.kind else {
            panic!("route should be a line");
        };
        assert_eq!(
            line.animation.as_ref().and_then(|route| route.marker_id),
            Some(marker_id)
        );
        assert!(object.dirty);
    }

    #[test]
    fn selecting_a_route_highlights_its_marker() {
        let mut editor = editor();
        let point = GeoPoint::new(115.225, -8.648);
        let marker_key = add_static_marker(
            &mut editor,
            "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            point,
            AssetMoveMode::Free,
        );
        let marker_id = Uuid::new_v4();
        editor.object_mut(marker_key).expect("marker").server_id = Some(marker_id);
        let end = GeoPoint::from_local_offset(point, 10.0, 0.0).expect("east");
        editor.add_route(vec![point, end]);
        let route_key = *editor.selected.iter().next().expect("route selected");
        assert!(editor.selected_route_markers().is_empty());

        editor.assign_route_marker(route_key, marker_id);
        assert_eq!(
            editor
                .selected_route_markers()
                .into_iter()
                .collect::<Vec<_>>(),
            vec![marker_key]
        );

        // Only the route selection links the marker; selecting the marker
        // itself does not highlight routes in return.
        editor.select_only(marker_key);
        assert!(editor.selected_route_markers().is_empty());
    }

    #[test]
    fn limited_static_marker_gets_a_play_gizmo_instead_of_an_order_unit() {
        let mut editor = editor();
        let point = GeoPoint::new(115.225, -8.648);
        let key = add_static_marker(
            &mut editor,
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            point,
            AssetMoveMode::Limited,
        );
        editor.preview = true;
        editor.start_play();

        assert!(editor.play.units.is_empty());
        assert_eq!(editor.play.obstacles.len(), 1);
        assert_eq!(
            editor.play.limited.get(&key).map(|unit| unit.position),
            Some(point)
        );
        // Not selected yet: no gizmo.
        assert!(editor.build_scene().gizmo.is_none());
        editor.play_select_limited(key);
        let scene = editor.build_scene();
        let gizmo = scene.gizmo.expect("limited asset shows a move gizmo");
        assert_eq!(gizmo.mode, EditorGizmoMode::Move);
        assert_eq!(gizmo.play_key, Some(key));
        assert!(gizmo.show_y);
    }

    #[test]
    fn play_mode_adds_3d_shapes_as_obstacles() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        editor.add_shape(twg_render::EditorShapeKind::Sphere, base);
        let cylinder = GeoPoint::from_local_offset(base, 10.0, 0.0).unwrap();
        editor.add_shape(twg_render::EditorShapeKind::Cylinder, cylinder);
        let cone = GeoPoint::from_local_offset(base, 20.0, 0.0).unwrap();
        editor.add_shape(twg_render::EditorShapeKind::Cone, cone);
        editor.add_polygon(vec![
            GeoPoint::from_local_offset(base, 30.0, 0.0).unwrap(),
            GeoPoint::from_local_offset(base, 34.0, 0.0).unwrap(),
            GeoPoint::from_local_offset(base, 34.0, 4.0).unwrap(),
            GeoPoint::from_local_offset(base, 30.0, 4.0).unwrap(),
        ]);
        let ground = GeoPoint::from_local_offset(base, 40.0, 0.0).unwrap();
        editor.add_shape(twg_render::EditorShapeKind::Ground, ground);

        editor.start_play();

        // Sphere, cylinder, cone, and custom mesh block movement; the ground
        // plate stays walkable.
        assert_eq!(editor.play.obstacles.len(), 4);
        assert!(
            editor
                .play
                .obstacles
                .iter()
                .all(|obstacle| obstacle.radius > 0.5)
        );
        assert!(
            editor
                .play
                .obstacles
                .iter()
                .any(|obstacle| (obstacle.radius - 2.0).abs() < 0.01),
            "primitive sphere radius comes from its scale"
        );
    }

    #[test]
    fn play_ignores_objects_marked_as_non_obstacle() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        let marker = add_static_marker(
            &mut editor,
            "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            base,
            AssetMoveMode::None,
        );
        let EditorKind::Marker(marker_object) = &mut editor.object_mut(marker).unwrap().kind else {
            panic!("expected marker");
        };
        marker_object.obstacle = false;

        editor.add_shape(
            twg_render::EditorShapeKind::Sphere,
            GeoPoint::from_local_offset(base, 10.0, 0.0).unwrap(),
        );
        let EditorKind::Building(building) = &mut editor.objects.last_mut().unwrap().kind else {
            panic!("expected building");
        };
        building.obstacle = false;

        editor.start_play();

        assert!(editor.play.obstacles.is_empty());
    }

    #[test]
    fn play_units_do_not_walk_through_each_other() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        let first = add_animated_marker(&mut editor, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", base);
        let second_point = GeoPoint::from_local_offset(base, 8.0, 0.0).unwrap();
        let second = add_animated_marker(
            &mut editor,
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            second_point,
        );
        editor.start_play();
        editor.selected.insert(first);
        editor.play_move_selected(second_point);

        let mut closest = f64::MAX;
        for _ in 0..900 {
            editor.play_tick(1.0 / 60.0);
            let offset = editor.play.units[&second]
                .position
                .local_offset(editor.play.units[&first].position)
                .unwrap();
            closest = closest.min(offset.east_meters.hypot(offset.north_meters));
        }
        assert!(
            closest > 2.0,
            "units overlapped while pathing: closest {closest} m"
        );
    }

    #[test]
    fn play_formation_gives_each_selected_unit_its_own_target() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        for index in 0..4 {
            let point = GeoPoint::from_local_offset(base, f64::from(index) * 4.0, 0.0).unwrap();
            add_animated_marker(
                &mut editor,
                &format!("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaa{index}"),
                point,
            );
        }
        editor.start_play();
        editor.selected = editor.play.units.keys().copied().collect();
        let target = GeoPoint::from_local_offset(base, 40.0, 25.0).unwrap();
        editor.play_move_selected(target);

        let targets = editor
            .play
            .units
            .values()
            .filter_map(|unit| unit.target)
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 4);
        for (index, first) in targets.iter().enumerate() {
            for second in &targets[index + 1..] {
                assert_ne!(first, second);
            }
        }
    }

    #[test]
    fn play_units_go_around_static_markers_and_reach_the_target() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        add_animated_marker(&mut editor, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", base);
        let obstacle = GeoPoint::from_local_offset(base, 25.0, 0.0).unwrap();
        editor.add_marker(obstacle, 0);
        let target = GeoPoint::from_local_offset(base, 50.0, 0.0).unwrap();

        editor.start_play();
        editor.selected = editor.play.units.keys().copied().collect();
        editor.play_move_selected(target);
        let blocked = editor.play.obstacles[0];
        let key = *editor.play.units.keys().next().unwrap();
        let mut closest = f64::MAX;
        for _ in 0..1200 {
            editor.play_tick(1.0 / 60.0);
            let unit = &editor.play.units[&key];
            let offset = blocked.center.local_offset(unit.position).unwrap();
            closest = closest.min(offset.east_meters.hypot(offset.north_meters));
        }

        assert!(
            closest >= blocked.radius,
            "unit clipped the obstacle: {closest} m"
        );
        let unit = &editor.play.units[&key];
        let remaining = target.local_offset(unit.position).unwrap();
        assert!(
            remaining.east_meters.hypot(remaining.north_meters) < 1.0,
            "unit did not reach the target"
        );
    }

    #[test]
    fn play_mode_shows_the_animated_scene_and_returns_to_idle() {
        let mut editor = editor();
        let id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        add_animated_marker(&mut editor, id, GeoPoint::new(115.225, -8.648));

        editor.preview = true;
        let scene = editor.build_scene();
        match &scene.objects[0].kind {
            EditorSceneKind::Marker {
                model,
                play_idle,
                scene_asset,
                ..
            } => {
                assert_eq!(model, &format!("twg-cache://{id}/animated/original.glb"));
                assert!(*scene_asset);
                let idle = play_idle.as_ref().expect("idle scene available");
                assert_eq!(idle.model, format!("twg-cache://{id}/idle/original.glb"));
            }
            _ => panic!("expected marker scene"),
        }

        editor.preview = false;
        let scene = editor.build_scene();
        match &scene.objects[0].kind {
            EditorSceneKind::Marker {
                model, play_idle, ..
            } => {
                assert_eq!(model, &format!("twg-cache://{id}/idle/original.glb"));
                assert!(play_idle.is_none());
            }
            _ => panic!("expected marker scene"),
        }
    }

    #[test]
    fn shape_objects_support_the_transform_gizmo() {
        let mut editor = editor();
        editor.add_shape(EditorShapeKind::Cylinder, GeoPoint::new(115.225, -8.648));
        let key = *editor.selected.iter().next().expect("shape selected");
        let object = editor.object_mut(key).expect("shape object");

        let transform = object.kind.transform().expect("shape transform");
        assert_eq!(transform.scale, [4.0, 3.0, 4.0]);
        object.kind.set_rotation_axis(1, 90.0);
        object.kind.set_scale([2.0, 2.0, 2.0]);
        let transform = object.kind.transform().expect("shape transform");
        assert_eq!(transform.rot, [0.0, 90.0, 0.0]);
        assert_eq!(transform.scale, [2.0, 2.0, 2.0]);
        // Primitive shapes expose a square outline for picking/selection.
        assert_eq!(object.kind.points().len(), 4);
    }

    #[test]
    fn line_point_editing_keeps_altitudes_in_sync() {
        let mut editor = editor();
        editor.add_line(vec![
            GeoPoint::new(115.225, -8.648),
            GeoPoint::new(115.226, -8.648),
            GeoPoint::new(115.226, -8.649),
        ]);
        let key = *editor.selected.iter().next().expect("line selected");
        editor.begin_point_edit(key);

        assert!(editor.insert_line_point(key, 0, 0.5));
        let index = editor
            .point_edit
            .and_then(|edit| edit.point)
            .expect("inserted point selected");
        if let Some(EditorKind::Line(line)) = editor.object_mut(key).map(|object| &mut object.kind)
        {
            line.altitudes[index] = 10.0;
        }

        let (_, transform) = editor.gizmo_target().expect("line point gizmo target");
        assert_eq!(transform.altitude, 10.0);
        assert!(editor.point_edit_allows_altitude());
        assert!(
            editor
                .build_scene()
                .gizmo
                .as_ref()
                .is_some_and(|gizmo| gizmo.show_y)
        );
        let EditorKind::Line(line) = &editor.object(key).expect("line object").kind else {
            panic!("expected line");
        };
        assert_eq!(line.points.len(), 4);
        assert_eq!(line.altitudes.len(), 4);

        assert!(editor.remove_point_edit_point());
        let EditorKind::Line(line) = &editor.object(key).expect("line object").kind else {
            panic!("expected line");
        };
        assert_eq!(line.points.len(), 3);
        assert_eq!(line.altitudes.len(), 3);
    }

    #[test]
    fn custom_mesh_points_are_flat_and_edited_on_the_map() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        editor.add_polygon(vec![
            base,
            GeoPoint::from_local_offset(base, 5.0, 0.0).expect("east"),
            GeoPoint::from_local_offset(base, 5.0, 5.0).expect("corner"),
            GeoPoint::from_local_offset(base, 0.0, 5.0).expect("north"),
        ]);
        let key = *editor.selected.iter().next().expect("shape selected");
        editor.begin_point_edit(key);

        assert!(editor.insert_edit_point(key, 0, 0.5));
        let EditorKind::Building(building) = &editor.object(key).expect("shape").kind else {
            panic!("expected building");
        };
        assert_eq!(building.points.len(), 5);
        assert!(!editor.point_edit_allows_altitude());
        assert!(
            !editor
                .build_scene()
                .gizmo
                .as_ref()
                .is_some_and(|gizmo| gizmo.show_y)
        );

        assert!(editor.remove_point_edit_point());
        let EditorKind::Building(building) = &editor.object(key).expect("shape").kind else {
            panic!("expected building");
        };
        assert_eq!(building.points.len(), 4);
    }

    #[test]
    fn text_supports_move_and_altitude_but_not_rotate_or_scale() {
        let mut editor = editor();
        editor.add_text(GeoPoint::new(115.225, -8.648));
        let key = *editor.selected.iter().next().expect("text selected");

        editor.set_tool(EditorTool::Move);
        let scene = editor.build_scene();
        assert_eq!(
            scene.gizmo.as_ref().map(|gizmo| gizmo.mode),
            Some(EditorGizmoMode::Move)
        );

        editor.set_tool(EditorTool::Rotate);
        assert!(editor.build_scene().gizmo.is_none());
        editor.set_tool(EditorTool::Scale);
        assert!(editor.build_scene().gizmo.is_none());

        editor
            .object_mut(key)
            .expect("text object")
            .kind
            .set_altitude(12.0);
        assert_eq!(
            editor
                .object(key)
                .and_then(|object| object.kind.transform())
                .map(|transform| transform.altitude),
            Some(12.0)
        );
    }

    #[test]
    fn set_position_is_absolute_not_accumulated() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        editor.add_marker(base, 0);
        let key = *editor.selected.iter().next().expect("marker selected");
        let target = GeoPoint::from_local_offset(base, 10.0, 0.0).expect("target point");

        // Drag updates run every frame with the same target; the object must
        // stay put instead of drifting further east each call.
        for _ in 0..5 {
            editor
                .object_mut(key)
                .expect("marker object")
                .kind
                .set_position(target);
        }

        let point = editor
            .object(key)
            .and_then(|object| object.point())
            .unwrap();
        let delta = point.local_offset(base).expect("move offset");
        assert!(
            (delta.east_meters - 10.0).abs() < 0.01,
            "east was {}",
            delta.east_meters
        );
    }

    #[test]
    fn set_position_keeps_a_custom_shape_footprint_aligned() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        let points = vec![
            base,
            GeoPoint::from_local_offset(base, 5.0, 0.0).expect("east point"),
            GeoPoint::from_local_offset(base, 5.0, 5.0).expect("corner point"),
            GeoPoint::from_local_offset(base, 0.0, 5.0).expect("north point"),
        ];
        editor.add_polygon(points);
        let key = *editor.selected.iter().next().expect("shape selected");
        let before_centre = editor
            .object(key)
            .and_then(|object| object.point())
            .expect("shape centre");
        let target = GeoPoint::from_local_offset(base, 10.0, 0.0).expect("target point");

        editor
            .object_mut(key)
            .expect("shape object")
            .kind
            .set_position(target);

        let object = editor.object(key).expect("shape object");
        let centre = object.point().expect("shape centre");
        let delta = centre.local_offset(base).expect("move offset");
        assert!(
            (delta.east_meters - 10.0).abs() < 0.01,
            "east was {}",
            delta.east_meters
        );
        let outline = object.kind.points();
        assert_eq!(outline.len(), 4);
        // The footprint keeps its shape: every vertex moves with the centre.
        let shift = target.local_offset(before_centre).expect("shape shift");
        let first = outline[0];
        let corner = first.local_offset(base).expect("corner offset");
        assert!((corner.east_meters - shift.east_meters).abs() < 0.01);
        assert!((corner.north_meters - shift.north_meters).abs() < 0.01);
    }

    #[test]
    fn uploaded_textures_resolve_against_the_server_base_url() {
        let mut editor = editor();
        editor.base_url = "http://127.0.0.1:8010/".to_owned();
        editor.add_shape(EditorShapeKind::Box, GeoPoint::new(115.225, -8.648));
        let key = *editor.selected.iter().next().expect("shape selected");
        let object = editor.object_mut(key).expect("shape object");
        let EditorKind::Building(building) = &mut object.kind else {
            panic!("expected building");
        };
        building.use_texture = true;
        building.texture = Some("/uploads/123-bukit.png".to_owned());

        assert_eq!(
            editor.texture_source("/uploads/123-bukit.png").as_deref(),
            Some("http://127.0.0.1:8010/uploads/123-bukit.png")
        );
        let scene = editor.build_scene();
        match &scene.objects[0].kind {
            EditorSceneKind::Building { texture, .. } => assert_eq!(
                texture.as_deref(),
                Some("http://127.0.0.1:8010/uploads/123-bukit.png")
            ),
            _ => panic!("expected building scene"),
        }
    }

    #[test]
    fn moving_a_custom_shape_moves_its_footprint() {
        let mut editor = editor();
        let base = GeoPoint::new(115.225, -8.648);
        let points = vec![
            base,
            GeoPoint::from_local_offset(base, 5.0, 0.0).expect("east point"),
            GeoPoint::from_local_offset(base, 5.0, 5.0).expect("corner point"),
            GeoPoint::from_local_offset(base, 0.0, 5.0).expect("north point"),
        ];
        editor.add_polygon(points);
        let key = *editor.selected.iter().next().expect("shape selected");
        let before = editor
            .object(key)
            .and_then(|object| object.point())
            .unwrap();

        offset_object(&mut editor.object_mut(key).unwrap().kind, 10.0, 0.0);

        let object = editor.object(key).expect("shape object");
        let after = object.point().expect("shape centre");
        let delta = after.local_offset(before).expect("move offset");
        assert!((delta.east_meters - 10.0).abs() < 0.01);
        assert_eq!(object.kind.points().len(), 4);
    }

    #[test]
    fn focusing_a_marker_pans_and_zooms_to_it() {
        let mut editor = editor();
        let point = GeoPoint::new(115.2265, -8.6495);
        editor.add_marker(point, 0);
        let key = *editor.selected.iter().next().expect("marker selected");
        editor.map_state.zoom = 13.0;
        editor.requested_flight = None;

        editor.focus_object(key);

        let target = editor.requested_flight.expect("focus flight requested");
        assert_eq!(target.center, point);
        assert!(target.zoom > 13.0, "camera should zoom in");
        assert!(target.zoom <= twg_map::MAX_ZOOM);
    }

    #[test]
    fn focusing_a_line_centres_on_its_middle() {
        let mut editor = editor();
        let start = GeoPoint::new(115.2250, -8.6480);
        let end = GeoPoint::new(115.2260, -8.6490);
        editor.add_line(vec![start, end]);
        let key = *editor.selected.iter().next().expect("line selected");

        editor.focus_object(key);

        let target = editor.requested_flight.expect("focus flight requested");
        let expected = points_centroid(&[
            [start.longitude, start.latitude],
            [end.longitude, end.latitude],
        ])
        .expect("centroid");
        assert!((target.center.longitude - expected.longitude).abs() < 1.0e-9);
        assert!((target.center.latitude - expected.latitude).abs() < 1.0e-9);
    }

    #[test]
    fn play_selection_does_not_rebuild_the_scene() {
        let mut editor = editor();
        let key = add_animated_marker(
            &mut editor,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            GeoPoint::new(115.225, -8.648),
        );
        editor.start_play();
        let revision = editor.render_revision;
        editor.play_select_only(key);
        editor.play_toggle_selected(key);
        assert_eq!(editor.render_revision, revision);
    }

    #[test]
    fn camera_view_stays_saved_until_the_view_or_layer_changes() {
        let mut editor = editor();
        let view = editor.map_state;
        editor.saved_view = Some(view);
        editor.saved_base_layer = Some(editor.base_layer);
        assert!(editor.view_is_saved());

        editor.map_state.zoom += 0.5;
        assert!(!editor.view_is_saved());

        editor.map_state = view;
        editor.base_layer = BaseLayer::ArcGisImagery;
        assert!(!editor.view_is_saved());
    }
}
