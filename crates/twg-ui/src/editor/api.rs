use serde_json::Value;
use twg_client::api::{ApiError, ApiHandle, ApiResponse, HttpMethod};
use twg_client::session::{NavItem, Screen};
use twg_contracts::geocode::GeocodeResult;
use twg_contracts::map::{
    Building, BuildingCreateRequest, BuildingGeomUpdateRequest, BuildingUpdateRequest, Line,
    LineCreateRequest, LineGeomUpdateRequest, LineUpdateRequest, Marker, MarkerCreateRequest,
    MarkerGeomRequest, MarkerNameRequest, MarkerScaleRequest, TextMarker, TextMarkerCreateRequest,
    TextMarkerGeomUpdateRequest, TextMarkerUpdateRequest,
};
use twg_contracts::scenarios::Scenario;
use twg_domain::{BaseLayer as DomainBaseLayer, MapMode};
use twg_map::{BaseLayer, GeoPoint, MapCameraState};
use uuid::Uuid;

use crate::state::{ClientUi, ToastKind};

use super::model::{
    BuildingObject, EditorKind, EditorObject, LineObject, MarkerObject, ObjectKey, ObjectType,
    ParticleObject, TextObject, unit_for_uuid,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorJobKind {
    Scenario,
    Markers,
    Lines,
    Buildings,
    Texts,
    Particles,
    CreateMarker,
    CreateLine,
    CreateBuilding,
    CreateText,
    Update,
    Delete,
    Geocode,
}

pub struct EditorJob {
    pub kind: EditorJobKind,
    pub key: Option<ObjectKey>,
    pub handle: ApiHandle,
}

struct EditorRequest {
    kind: EditorJobKind,
    key: Option<ObjectKey>,
    method: HttpMethod,
    path: String,
    body: Option<Value>,
}

impl ClientUi {
    pub fn open_editor(&mut self, scenario_id: Option<Uuid>) {
        let scenario_id = scenario_id
            .or(self.selected_scenario)
            .or_else(|| self.data.scenarios.first().map(|scenario| scenario.id));
        self.editor.reset();
        self.editor.scenario_id = scenario_id;
        self.state.nav = NavItem::Preview;
        self.state.screen = Screen::MapEditor;

        let Some(scenario_id) = scenario_id else {
            self.editor.loaded = true;
            self.editor.log("Belum ada skenario dipilih.");
            self.load_map_defaults();
            return;
        };
        self.selected_scenario = Some(scenario_id);
        self.ws.scenario = Some(scenario_id);
        self.editor.loading = true;
        self.editor.pending = 6;
        self.editor.log("Memuat skenario dan objek...");

        let requests = [
            (
                EditorJobKind::Scenario,
                format!("/public/skenarios/get/{scenario_id}"),
            ),
            (
                EditorJobKind::Markers,
                format!("/public/markers/list/{scenario_id}"),
            ),
            (
                EditorJobKind::Lines,
                format!("/public/lines/list/{scenario_id}"),
            ),
            (
                EditorJobKind::Buildings,
                format!("/public/buildings/list/{scenario_id}"),
            ),
            (
                EditorJobKind::Texts,
                format!("/public/text-markers/list/{scenario_id}"),
            ),
            (
                EditorJobKind::Particles,
                format!("/public/particles/list/{scenario_id}"),
            ),
        ];
        for (kind, path) in requests {
            let handle = self.api.request(HttpMethod::Get, path, None);
            self.editor.jobs.push(EditorJob {
                kind,
                key: None,
                handle,
            });
        }
    }

    fn load_map_defaults(&mut self) {
        self.editor.requested_flight = Some(MapCameraState::default());
        self.editor.requested_layer = Some(BaseLayer::OpenStreetMap);
    }

    pub fn close_editor(&mut self) {
        self.state.screen = Screen::Workspace;
        self.state.nav = NavItem::Scenarios;
        self.editor.preview = false;
        self.editor.touch();
    }

    pub fn reload_editor(&mut self) {
        let scenario_id = self.editor.scenario_id;
        self.open_editor(scenario_id);
    }

    pub fn editor_save(&mut self) {
        if self.editor.loading {
            self.toast("Tunggu pemuatan skenario selesai.", ToastKind::Info);
            return;
        }
        let Some(scenario_id) = self.editor.scenario_id else {
            self.toast("Pilih skenario sebelum menyimpan.", ToastKind::Error);
            return;
        };
        let Some(operation_id) = self.editor.operation_id else {
            self.toast("Operasi skenario belum dimuat.", ToastKind::Error);
            return;
        };

        let mut requests: Vec<EditorRequest> = Vec::new();
        for object in &self.editor.objects {
            if object.server_id.is_none() {
                if let Some(request) = create_request(object, operation_id, scenario_id) {
                    requests.push(request);
                }
            } else if object.dirty
                && let Some(request) = update_requests(object)
            {
                requests.extend(request);
            }
        }
        for (kind, id) in &self.editor.deleted {
            let path = delete_path(*kind, *id);
            if path.is_empty() {
                continue;
            }
            requests.push(EditorRequest {
                kind: EditorJobKind::Delete,
                key: None,
                method: HttpMethod::Delete,
                path,
                body: None,
            });
        }

        if requests.is_empty() {
            self.editor.dirty = false;
            self.editor.last_save = Some("tidak ada perubahan".to_owned());
            self.toast("Tidak ada perubahan untuk disimpan.", ToastKind::Info);
            return;
        }

        self.editor.save_pending = requests.len();
        self.editor.save_error = None;
        self.editor.last_save = None;
        self.editor
            .log(format!("Menyimpan {} perubahan...", requests.len()));
        for request in requests {
            let handle = self.api.request(request.method, request.path, request.body);
            self.editor.jobs.push(EditorJob {
                kind: request.kind,
                key: request.key,
                handle,
            });
        }
    }

    pub fn editor_geocode(&mut self) {
        let query = self.editor.geocode_query.trim().to_owned();
        if query.len() < 2 {
            return;
        }
        self.editor.geocode_pending = true;
        self.editor.geocode_results.clear();
        let path = format!("/public/geocode?q={}", percent_encode(&query));
        let handle = self.api.request(HttpMethod::Get, path, None);
        self.editor.jobs.push(EditorJob {
            kind: EditorJobKind::Geocode,
            key: None,
            handle,
        });
    }

    pub fn tick_editor_jobs(&mut self) {
        let jobs = std::mem::take(&mut self.editor.jobs);
        for job in jobs {
            let Some(result) = job.handle.try_take() else {
                self.editor.jobs.push(job);
                continue;
            };
            self.finish_editor_job(job.kind, job.key, result);
        }
    }

    fn finish_editor_job(
        &mut self,
        kind: EditorJobKind,
        key: Option<ObjectKey>,
        result: Result<ApiResponse, ApiError>,
    ) {
        match kind {
            EditorJobKind::Scenario => {
                match result.and_then(|response| response.data::<Scenario>()) {
                    Ok(scenario) => self.apply_scenario(scenario),
                    Err(error) => {
                        self.editor.load_error = Some(error.message.clone());
                        self.toast(
                            format!("Gagal memuat skenario: {}", error.message),
                            ToastKind::Error,
                        );
                    }
                }
                self.editor_load_done();
            }
            EditorJobKind::Markers => {
                match result.and_then(|response| response.list::<Marker>()) {
                    Ok((markers, _)) => {
                        let objects = markers.into_iter().map(marker_object).collect::<Vec<_>>();
                        self.editor.objects.extend(objects);
                    }
                    Err(error) => self.editor.load_error = Some(error.message),
                }
                self.editor_load_done();
            }
            EditorJobKind::Lines => {
                match result.and_then(|response| response.list::<Line>()) {
                    Ok((lines, _)) => {
                        let objects = lines.into_iter().map(line_object).collect::<Vec<_>>();
                        self.editor.objects.extend(objects);
                    }
                    Err(error) => self.editor.load_error = Some(error.message),
                }
                self.editor_load_done();
            }
            EditorJobKind::Buildings => {
                match result.and_then(|response| response.list::<Building>()) {
                    Ok((buildings, _)) => {
                        let objects = buildings
                            .into_iter()
                            .map(building_object)
                            .collect::<Vec<_>>();
                        self.editor.objects.extend(objects);
                    }
                    Err(error) => self.editor.load_error = Some(error.message),
                }
                self.editor_load_done();
            }
            EditorJobKind::Texts => {
                match result.and_then(|response| response.list::<TextMarker>()) {
                    Ok((texts, _)) => {
                        let objects = texts.into_iter().map(text_object).collect::<Vec<_>>();
                        self.editor.objects.extend(objects);
                    }
                    Err(error) => self.editor.load_error = Some(error.message),
                }
                self.editor_load_done();
            }
            EditorJobKind::Particles => {
                match result.and_then(|response| response.list::<twg_contracts::map::Particle>()) {
                    Ok((particles, _)) => {
                        let objects = particles
                            .into_iter()
                            .map(particle_object)
                            .collect::<Vec<_>>();
                        self.editor.objects.extend(objects);
                    }
                    Err(error) => self.editor.load_error = Some(error.message),
                }
                self.editor_load_done();
            }
            EditorJobKind::CreateMarker => {
                self.create_done(key, result, |response| {
                    response.data::<Marker>().map(|marker| marker.id)
                });
            }
            EditorJobKind::CreateLine => {
                self.create_done(key, result, |response| {
                    response.data::<Line>().map(|line| line.id)
                });
            }
            EditorJobKind::CreateBuilding => {
                self.create_done(key, result, |response| {
                    response.data::<Building>().map(|building| building.id)
                });
            }
            EditorJobKind::CreateText => {
                self.create_done(key, result, |response| {
                    response.data::<TextMarker>().map(|text| text.id)
                });
            }
            EditorJobKind::Update | EditorJobKind::Delete => {
                if let Err(error) = result {
                    self.editor.save_error = Some(error.message);
                }
                self.editor_save_done();
            }
            EditorJobKind::Geocode => {
                self.editor.geocode_pending = false;
                match result.and_then(|response| response.list::<GeocodeResult>()) {
                    Ok((results, _)) => {
                        self.editor.geocode_results = results
                            .into_iter()
                            .map(|result| super::model::GeocodeHit {
                                label: result.label,
                                latitude: result.latitude,
                                longitude: result.longitude,
                            })
                            .collect();
                    }
                    Err(error) => {
                        self.toast(
                            format!("Geocode gagal: {}", error.message),
                            ToastKind::Error,
                        );
                    }
                }
            }
        }
    }

    fn apply_scenario(&mut self, scenario: Scenario) {
        self.editor.operation_id = Some(scenario.operasi.id);
        self.editor.scenario_name = scenario.name.clone();
        self.editor.scenario_center = GeoPoint::new(scenario.center_x, scenario.center_y);
        self.editor.indoor = scenario.map_mode == MapMode::Indoor;
        self.editor.floor_plan = matches!(scenario.base_layer, DomainBaseLayer::FloorPlan)
            || scenario.indoor_config["floor_plan_url"]
                .as_str()
                .is_some_and(|url| !url.trim().is_empty());
        self.editor.grid_spacing = scenario.indoor_config["grid_size_m"]
            .as_f64()
            .unwrap_or(1.0)
            .clamp(0.1, 50.0) as f32;
        self.editor.floor_size = [
            scenario.indoor_config["floor_plan_width_m"]
                .as_f64()
                .unwrap_or(100.0)
                .clamp(1.0, 10_000.0) as f32,
            scenario.indoor_config["floor_plan_height_m"]
                .as_f64()
                .unwrap_or(60.0)
                .clamp(1.0, 10_000.0) as f32,
        ];
        self.editor.requested_layer = Some(match scenario.base_layer {
            DomainBaseLayer::Satellite => BaseLayer::ArcGisImagery,
            _ => BaseLayer::OpenStreetMap,
        });
        self.editor.requested_flight = Some(MapCameraState {
            center: GeoPoint::new(scenario.center_x, scenario.center_y),
            zoom: scenario.zoom as f32,
            pitch_degrees: scenario.pitch as f32,
            bearing_degrees: scenario.bearing as f32,
        });
        self.editor
            .log(format!("Skenario '{}' dimuat.", scenario.name));
    }

    fn editor_load_done(&mut self) {
        self.editor.pending = self.editor.pending.saturating_sub(1);
        if self.editor.pending == 0 {
            self.editor.loading = false;
            self.editor.loaded = true;
            self.editor.undo.clear();
            self.editor.redo.clear();
            self.editor.dirty = false;
            for object in &mut self.editor.objects {
                object.dirty = false;
            }
            assign_keys(&mut self.editor);
            self.editor.selected.clear();
            self.editor
                .log(format!("{} objek dimuat.", self.editor.objects.len()));
            self.editor.touch();
        }
    }

    fn create_done(
        &mut self,
        key: Option<ObjectKey>,
        result: Result<ApiResponse, ApiError>,
        extract: impl FnOnce(ApiResponse) -> Result<Uuid, ApiError>,
    ) {
        match result.and_then(extract) {
            Ok(id) => {
                if let Some(object) = key.and_then(|key| self.editor.object_mut(key)) {
                    object.server_id = Some(id);
                    object.dirty = false;
                }
                let extras = key
                    .and_then(|key| self.editor.object(key))
                    .and_then(marker_extra_requests);
                if let Some(extras) = extras {
                    self.editor.save_pending += extras.len();
                    for (path, body) in extras {
                        let handle = self.api.request(HttpMethod::Put, path, Some(body));
                        self.editor.jobs.push(EditorJob {
                            kind: EditorJobKind::Update,
                            key,
                            handle,
                        });
                    }
                }
            }
            Err(error) => self.editor.save_error = Some(error.message),
        }
        self.editor_save_done();
    }

    fn editor_save_done(&mut self) {
        self.editor.save_pending = self.editor.save_pending.saturating_sub(1);
        if self.editor.save_pending != 0 {
            return;
        }
        let result = self.editor.save_error.take();
        match result {
            Some(error) => {
                self.toast(format!("Simpan gagal: {error}"), ToastKind::Error);
            }
            None => {
                for object in &mut self.editor.objects {
                    object.dirty = false;
                }
                self.editor.deleted.clear();
                self.editor.dirty = false;
                self.editor.last_save = Some(chrono::Local::now().format("%H:%M:%S").to_string());
                self.editor.log("Semua perubahan tersimpan.");
                self.toast("Skenario tersimpan.", ToastKind::Success);
                self.editor.touch();
            }
        }
    }
}

fn marker_object(marker: Marker) -> EditorObject {
    let unit_id = marker.unit_id.clone();
    let unit = unit_for_uuid(&unit_id);
    EditorObject {
        key: 0,
        server_id: Some(marker.id),
        name: marker.name,
        visible: true,
        dirty: false,
        kind: EditorKind::Marker(MarkerObject {
            unit,
            unit_id,
            lon: marker.pos_x,
            lat: marker.pos_y,
            rot_x: marker.rot_x,
            rot_y: marker.rot_y,
            rot_z: marker.rot_z,
            scale_x: marker.scale,
            scale_y: marker.scale_y,
            scale_z: marker.scale_z,
            altitude: marker.altitude,
            jumlah: marker.jumlah,
            kategori: marker.kategori,
            keterangan: marker.keterangan,
        }),
    }
}

fn line_object(line: Line) -> EditorObject {
    EditorObject {
        key: 0,
        server_id: Some(line.id),
        name: line.name,
        visible: true,
        dirty: false,
        kind: EditorKind::Line(LineObject {
            points: parse_geom(&line.geom),
            color: line.color,
            width: line.width,
            alpha: line.alpha,
        }),
    }
}

fn building_object(building: Building) -> EditorObject {
    EditorObject {
        key: 0,
        server_id: Some(building.id),
        name: building.name,
        visible: true,
        dirty: false,
        kind: EditorKind::Building(BuildingObject {
            points: parse_geom(&building.geom),
            color: building.color,
            height: building.height,
            keterangan: building.keterangan,
        }),
    }
}

fn text_object(text: TextMarker) -> EditorObject {
    EditorObject {
        key: 0,
        server_id: Some(text.id),
        name: text.text.clone(),
        visible: true,
        dirty: false,
        kind: EditorKind::Text(TextObject {
            text: text.text,
            lon: text.pos_x,
            lat: text.pos_y,
            color: text.color,
            font_size: text.font_size,
            align: match text.align {
                twg_domain::TextAlign::Left => "left",
                twg_domain::TextAlign::Center => "center",
                twg_domain::TextAlign::Right => "right",
            }
            .to_owned(),
            wrap_text: text.wrap_text,
        }),
    }
}

fn particle_object(particle: twg_contracts::map::Particle) -> EditorObject {
    EditorObject {
        key: 0,
        server_id: Some(particle.id),
        name: particle.name,
        visible: true,
        dirty: false,
        kind: EditorKind::Particle(ParticleObject {
            lon: particle.pos_x,
            lat: particle.pos_y,
            tipe: particle.tipe,
            data: particle.data,
            is_spawn: particle.is_spawn,
        }),
    }
}

fn parse_geom(geom: &str) -> Vec<[f64; 2]> {
    serde_json::from_str(geom).unwrap_or_default()
}

fn geom_json(points: &[[f64; 2]]) -> String {
    serde_json::to_string(points).unwrap_or_else(|_| "[]".to_owned())
}

fn create_request(
    object: &EditorObject,
    operation_id: Uuid,
    scenario_id: Uuid,
) -> Option<EditorRequest> {
    let (kind, body) = match &object.kind {
        EditorKind::Marker(marker) => (
            EditorJobKind::CreateMarker,
            serde_json::to_value(MarkerCreateRequest {
                name: object.name.clone(),
                operasi_id: operation_id.to_string(),
                skenario_id: scenario_id.to_string(),
                unit_id: marker.unit_id.clone(),
                kategori: marker.kategori.clone(),
                rot_x: marker.rot_x,
                rot_y: marker.rot_y,
                rot_z: marker.rot_z,
                pos_x: marker.lon,
                pos_y: marker.lat,
                jumlah: marker.jumlah,
                keterangan: marker.keterangan.clone(),
            })
            .ok()?,
        ),
        EditorKind::Line(line) => (
            EditorJobKind::CreateLine,
            serde_json::to_value(LineCreateRequest {
                name: object.name.clone(),
                operasi_id: operation_id.to_string(),
                skenario_id: scenario_id.to_string(),
                color: line.color.clone(),
                width: line.width,
                alpha: line.alpha,
                geom: geom_json(&line.points),
            })
            .ok()?,
        ),
        EditorKind::Building(building) => (
            EditorJobKind::CreateBuilding,
            serde_json::to_value(BuildingCreateRequest {
                name: object.name.clone(),
                operasi_id: operation_id.to_string(),
                skenario_id: scenario_id.to_string(),
                color: building.color.clone(),
                geom: geom_json(&building.points),
                height: building.height,
                keterangan: building.keterangan.clone(),
            })
            .ok()?,
        ),
        EditorKind::Text(text) => (
            EditorJobKind::CreateText,
            serde_json::to_value(TextMarkerCreateRequest {
                text: text.text.clone(),
                operasi_id: operation_id.to_string(),
                skenario_id: scenario_id.to_string(),
                color: text.color.clone(),
                font_size: text.font_size,
                align: align_value(&text.align),
                wrap_text: text.wrap_text,
                pos_x: text.lon,
                pos_y: text.lat,
            })
            .ok()?,
        ),
        EditorKind::Particle(_) => return None,
    };
    let path = match kind {
        EditorJobKind::CreateMarker => "/private/markers",
        EditorJobKind::CreateLine => "/private/lines",
        EditorJobKind::CreateBuilding => "/private/buildings",
        EditorJobKind::CreateText => "/private/text-markers",
        _ => return None,
    };
    Some(EditorRequest {
        kind,
        key: Some(object.key),
        method: HttpMethod::Post,
        path: path.to_owned(),
        body: Some(body),
    })
}

fn marker_extra_requests(object: &EditorObject) -> Option<Vec<(String, Value)>> {
    let EditorKind::Marker(marker) = &object.kind else {
        return None;
    };
    let id = object.server_id?;
    let mut requests = Vec::new();
    if (marker.scale_x - 1.0).abs() > 1.0e-6
        || (marker.scale_y - 1.0).abs() > 1.0e-6
        || (marker.scale_z - 1.0).abs() > 1.0e-6
    {
        requests.push((
            format!("/private/markers/scale/{id}"),
            serde_json::json!({
                "scale": marker.scale_x,
                "scale_y": marker.scale_y,
                "scale_z": marker.scale_z,
            }),
        ));
    }
    if marker.altitude.abs() > 1.0e-6 {
        requests.push((
            format!("/private/markers/altitude/{id}"),
            serde_json::json!({ "altitude": marker.altitude }),
        ));
    }
    (!requests.is_empty()).then_some(requests)
}

fn update_requests(object: &EditorObject) -> Option<Vec<EditorRequest>> {
    let id = object.server_id?;
    let mut requests = Vec::new();
    match &object.kind {
        EditorKind::Marker(marker) => {
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/markers/geom/{id}"),
                body: serde_json::to_value(MarkerGeomRequest {
                    rot_x: marker.rot_x,
                    rot_y: marker.rot_y,
                    rot_z: marker.rot_z,
                    pos_x: marker.lon,
                    pos_y: marker.lat,
                })
                .ok(),
            });
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/markers/name/{id}"),
                body: serde_json::to_value(MarkerNameRequest {
                    name: object.name.clone(),
                    jumlah: marker.jumlah,
                    keterangan: marker.keterangan.clone(),
                })
                .ok(),
            });
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/markers/scale/{id}"),
                body: serde_json::to_value(MarkerScaleRequest {
                    scale: marker.scale_x,
                    scale_y: marker.scale_y,
                    scale_z: marker.scale_z,
                })
                .ok(),
            });
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/markers/altitude/{id}"),
                body: serde_json::to_value(twg_contracts::map::MarkerAltitudeRequest {
                    altitude: marker.altitude,
                })
                .ok(),
            });
        }
        EditorKind::Line(line) => {
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/lines/{id}"),
                body: serde_json::to_value(LineUpdateRequest {
                    name: object.name.clone(),
                    color: line.color.clone(),
                    width: line.width,
                    alpha: line.alpha,
                })
                .ok(),
            });
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/lines/geom/{id}"),
                body: serde_json::to_value(LineGeomUpdateRequest {
                    geom: geom_json(&line.points),
                })
                .ok(),
            });
        }
        EditorKind::Building(building) => {
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/buildings/{id}"),
                body: serde_json::to_value(BuildingUpdateRequest {
                    name: object.name.clone(),
                    color: building.color.clone(),
                    keterangan: building.keterangan.clone(),
                    height: building.height,
                })
                .ok(),
            });
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/buildings/geom/{id}"),
                body: serde_json::to_value(BuildingGeomUpdateRequest {
                    geom: geom_json(&building.points),
                })
                .ok(),
            });
        }
        EditorKind::Text(text) => {
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/text-markers/{id}"),
                body: serde_json::to_value(TextMarkerUpdateRequest {
                    text: text.text.clone(),
                    color: text.color.clone(),
                    font_size: text.font_size,
                    align: align_value(&text.align),
                    wrap_text: text.wrap_text,
                })
                .ok(),
            });
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/text-markers/geom/{id}"),
                body: serde_json::to_value(TextMarkerGeomUpdateRequest {
                    pos_x: text.lon,
                    pos_y: text.lat,
                })
                .ok(),
            });
        }
        EditorKind::Particle(_) => return None,
    }
    Some(requests)
}

fn align_value(align: &str) -> twg_domain::TextAlign {
    match align {
        "left" => twg_domain::TextAlign::Left,
        "right" => twg_domain::TextAlign::Right,
        _ => twg_domain::TextAlign::Center,
    }
}

fn delete_path(kind: ObjectType, id: Uuid) -> String {
    match kind {
        ObjectType::Marker => format!("/private/markers/{id}"),
        ObjectType::Line => format!("/private/lines/{id}"),
        ObjectType::Building => format!("/private/buildings/{id}"),
        ObjectType::Text => format!("/private/text-markers/{id}"),
        ObjectType::Particle => String::new(),
    }
}

fn percent_encode(input: &str) -> String {
    let mut output = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char);
            }
            b' ' => output.push('+'),
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

/// Assigns keys to objects loaded from the API. The editor keeps local keys
/// separate from server UUIDs so unsaved objects can be renumbered safely.
pub fn assign_keys(editor: &mut super::model::EditorState) {
    let mut next = 1;
    for object in &mut editor.objects {
        object.key = next;
        next += 1;
    }
    editor.next_key = next;
}
