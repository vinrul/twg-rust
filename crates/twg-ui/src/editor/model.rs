use std::collections::BTreeSet;

use bevy::prelude::Vec3;
use bevy_egui::egui;
use twg_map::{BaseLayer, GeoPoint, MapCameraState};
use twg_render::{
    EditorGizmo, EditorGizmoAxis, EditorGrid, EditorScene, EditorSceneKind, EditorSceneObject,
};
use uuid::Uuid;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    Move,
    Rotate,
    Scale,
    Marker,
    Line,
    Polygon,
    Text,
    Measure,
}

impl EditorTool {
    pub const ALL: [Self; 9] = [
        Self::Select,
        Self::Move,
        Self::Rotate,
        Self::Scale,
        Self::Marker,
        Self::Line,
        Self::Polygon,
        Self::Text,
        Self::Measure,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Move => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::Marker => "Marker",
            Self::Line => "Line",
            Self::Polygon => "Polygon",
            Self::Text => "Text",
            Self::Measure => "Measure",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Select => "S",
            Self::Move => "M",
            Self::Rotate => "R",
            Self::Scale => "K",
            Self::Marker => "P",
            Self::Line => "L",
            Self::Polygon => "G",
            Self::Text => "T",
            Self::Measure => "D",
        }
    }

    pub const fn shortcut(self) -> &'static str {
        match self {
            Self::Select => "Q",
            Self::Move => "W",
            Self::Rotate => "E",
            Self::Scale => "R",
            Self::Marker => "1",
            Self::Line => "2",
            Self::Polygon => "3",
            Self::Text => "4",
            Self::Measure => "5",
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
            Self::Building => "Building",
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
pub struct LineObject {
    pub points: Vec<[f64; 2]>,
    pub color: String,
    pub width: f64,
    pub alpha: f64,
}

#[derive(Clone, Debug)]
pub struct BuildingObject {
    pub points: Vec<[f64; 2]>,
    pub color: String,
    pub height: f64,
    pub keterangan: String,
}

#[derive(Clone, Debug)]
pub struct TextObject {
    pub text: String,
    pub lon: f64,
    pub lat: f64,
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

#[derive(Clone, Debug)]
pub struct ParticleObject {
    pub lon: f64,
    pub lat: f64,
    pub tipe: String,
    pub data: String,
    pub is_spawn: i32,
}

impl ParticleObject {
    pub fn point(&self) -> GeoPoint {
        GeoPoint::new(self.lon, self.lat)
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
            Self::Building(building) => building
                .points
                .first()
                .map(|point| GeoPoint::new(point[0], point[1])),
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
            Self::Building(building) => building
                .points
                .iter()
                .map(|point| GeoPoint::new(point[0], point[1]))
                .collect(),
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
    Polygon(Vec<GeoPoint>),
}

impl Draft {
    pub fn points(&self) -> &[GeoPoint] {
        match self {
            Self::None => &[],
            Self::Line(points) | Self::Polygon(points) => points,
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn push(&mut self, point: GeoPoint) {
        match self {
            Self::None => {}
            Self::Line(points) | Self::Polygon(points) => points.push(point),
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
    Rotate {
        originals: Vec<(ObjectKey, f64)>,
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
    pub next_key: ObjectKey,
    pub dirty: bool,
    pub undo: Vec<SceneSnapshot>,
    pub redo: Vec<SceneSnapshot>,
    pub drag: DragState,
    pub draft: Draft,
    pub measure: MeasureState,
    pub snap_meters: f32,
    pub show_grid: bool,
    pub labels_visible: bool,
    pub show_assets: bool,
    pub show_console: bool,
    pub bottom_tab: usize,
    pub confirm_close: bool,
    pub confirm_reload: bool,
    pub draft_screen: Vec<egui::Pos2>,
    pub measure_screen: Vec<egui::Pos2>,
    pub console: Vec<String>,
    pub preview: bool,
    pub visible: VisibleLayers,
    pub active_unit: usize,
    pub last_click: Option<(f64, egui::Pos2)>,
    pub scenario_center: GeoPoint,
    pub indoor: bool,
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
    pub save_pending: usize,
    pub save_error: Option<String>,
    pub last_save: Option<String>,
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
            next_key: 1,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            drag: DragState::None,
            draft: Draft::None,
            measure: MeasureState::default(),
            snap_meters: 0.0,
            show_grid: false,
            labels_visible: true,
            show_assets: true,
            show_console: true,
            bottom_tab: 0,
            confirm_close: false,
            confirm_reload: false,
            draft_screen: Vec::new(),
            measure_screen: Vec::new(),
            console: Vec::new(),
            preview: false,
            visible: VisibleLayers::all(),
            active_unit: 0,
            last_click: None,
            scenario_center: MapCameraState::default().center,
            indoor: false,
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
            save_pending: 0,
            save_error: None,
            last_save: None,
            jobs: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        let scenario_id = self.scenario_id;
        *self = Self::new();
        self.scenario_id = scenario_id;
    }

    pub fn clear_scene(&mut self) {
        self.objects.clear();
        self.deleted.clear();
        self.selected.clear();
        self.undo.clear();
        self.redo.clear();
        self.next_key = 1;
        self.dirty = false;
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

    pub fn select_only(&mut self, key: ObjectKey) {
        self.selected.clear();
        self.selected.insert(key);
        self.touch();
    }

    pub fn toggle_selected(&mut self, key: ObjectKey) {
        if !self.selected.insert(key) {
            self.selected.remove(&key);
        }
        self.touch();
    }

    pub fn add_to_selection(&mut self, key: ObjectKey) {
        if self.selected.insert(key) {
            self.touch();
        }
    }

    pub fn clear_selection(&mut self) {
        if !self.selected.is_empty() {
            self.selected.clear();
            self.touch();
        }
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
        self.add_polyline(points, false);
    }

    pub fn add_polygon(&mut self, points: Vec<GeoPoint>) {
        self.add_polyline(points, true);
    }

    fn add_polyline(&mut self, points: Vec<GeoPoint>, polygon: bool) {
        if points.len() < 2 {
            return;
        }
        self.push_undo();
        let key = self.alloc_key();
        let coordinates = points
            .iter()
            .map(|point| [point.longitude, point.latitude])
            .collect::<Vec<_>>();
        let kind = if polygon {
            EditorKind::Building(BuildingObject {
                points: coordinates,
                color: "#ff9f43".to_owned(),
                height: 12.0,
                keterangan: String::new(),
            })
        } else {
            EditorKind::Line(LineObject {
                points: coordinates,
                color: "#00b8ff".to_owned(),
                width: 4.0,
                alpha: 0.9,
            })
        };
        self.objects.push(EditorObject {
            key,
            server_id: None,
            name: if polygon {
                format!("Bangunan {key}")
            } else {
                format!("Garis {key}")
            },
            visible: true,
            dirty: true,
            kind,
        });
        self.selected.clear();
        self.selected.insert(key);
        self.dirty = true;
        self.touch();
    }

    pub fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let selected = self.selected.clone();
        for key in &selected {
            if let Some(index) = self.objects.iter().position(|object| object.key == *key) {
                if self.objects[index].object_type() == ObjectType::Particle {
                    continue;
                }
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
            .filter(|object| {
                self.selected.contains(&object.key) && object.object_type() != ObjectType::Particle
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut new_selection = BTreeSet::new();
        for mut object in originals {
            let key = self.alloc_key();
            object.key = key;
            object.server_id = None;
            object.dirty = true;
            object.name = format!("{} copy", object.name);
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

    pub fn build_scene(&self) -> EditorScene {
        let mut objects = Vec::new();
        for object in &self.objects {
            if !object.visible || !self.visible.allows(object.object_type()) {
                continue;
            }
            let Some(point) = object.point() else {
                continue;
            };
            let selected = self.selected.contains(&object.key);
            let kind = match &object.kind {
                EditorKind::Marker(marker) => {
                    let entry = UNIT_CATALOG[marker.unit.min(UNIT_CATALOG.len() - 1)];
                    EditorSceneKind::Marker {
                        model: entry.model.to_owned(),
                        base_scale: entry.base_scale,
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
                    width: line.width as f32,
                    color: parse_hex_color(&line.color),
                    alpha: line.alpha as f32,
                },
                EditorKind::Building(building) => EditorSceneKind::Building {
                    points: building.points.clone(),
                    height: building.height as f32,
                    color: parse_hex_color(&building.color),
                },
                EditorKind::Text(text) => EditorSceneKind::Text {
                    color: parse_hex_color(&text.color),
                    font_size: text.font_size as f32,
                },
                EditorKind::Particle(particle) => EditorSceneKind::Particle {
                    color: parse_hex_color(particle_color(particle)),
                    radius: 1.2,
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

        let gizmo = if self.preview {
            None
        } else {
            self.primary_marker().map(|marker| EditorGizmo {
                lon: marker.lon,
                lat: marker.lat,
                altitude: marker.altitude as f32,
                active_axis: self.hover_axis,
                show_rotate: self.tool == EditorTool::Rotate,
                show_scale: self.tool == EditorTool::Scale,
            })
        };

        EditorScene {
            revision: self.render_revision,
            indoor: self.indoor || !self.visible.raster,
            objects,
            grid,
            gizmo,
        }
    }
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
        EditorKind::Building(building) => {
            offset_points(&mut building.points, east_meters, north_meters)
        }
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
                particle.lon = point.longitude;
                particle.lat = point.latitude;
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

pub fn particle_color(particle: &ParticleObject) -> &'static str {
    if particle.data.contains("fire") || particle.tipe.contains("fire") {
        "#ff7043"
    } else {
        "#ce93d8"
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
    fn selected_marker_emits_a_gizmo() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        let scene = editor.build_scene();
        assert_eq!(scene.objects.len(), 1);
        assert!(scene.gizmo.is_some());
    }

    #[test]
    fn hidden_layers_are_removed_from_the_render_scene() {
        let mut editor = editor();
        editor.add_marker(GeoPoint::new(115.225, -8.648), 0);
        editor.visible.marker = false;
        assert!(editor.build_scene().objects.is_empty());
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
}
