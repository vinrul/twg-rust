#[cfg(not(target_arch = "wasm32"))]
use bevy::prelude::{Mat4, Quat, Vec3};
use serde_json::Value;
use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::path::PathBuf;
use twg_client::api::{ApiError, ApiHandle, ApiResponse, HttpMethod};
use twg_client::session::{NavItem, Screen};
use twg_contracts::assets::{AssetCategory, AssetModelKind, AssetMoveMode, AssetSourceSlot};
use twg_contracts::geocode::GeocodeResult;
use twg_contracts::map::{
    Building, BuildingCreateRequest, BuildingGeomUpdateRequest, BuildingUpdateRequest, Line,
    LineCreateRequest, LineGeomUpdateRequest, LineUpdateRequest, Marker, MarkerCreateRequest,
    MarkerGeomRequest, MarkerNameRequest, MarkerScaleRequest, MarkerUnitRequest, Particle,
    ParticleCreateRequest, ParticleGeomUpdateRequest, ParticleKind, ParticleUpdateRequest,
    TextMarker, TextMarkerCreateRequest, TextMarkerGeomUpdateRequest, TextMarkerUpdateRequest,
};
use twg_contracts::scenarios::Scenario;
use twg_domain::{BaseLayer as DomainBaseLayer, MapMode, Role};
use twg_map::{BaseLayer, GeoPoint, MapCameraState};
#[cfg(not(target_arch = "wasm32"))]
use twg_render::cache_asset_path;
use twg_render::{EditorShapeKind, SkyMode};
use uuid::Uuid;

use crate::state::{ClientUi, JobKind, ToastKind};

#[cfg(not(target_arch = "wasm32"))]
use super::model::MarkerAssetModel;
use super::model::{
    BuildingObject, EditorAssetRow, EditorKind, EditorObject, EditorTextureRow, LineObject,
    MarkerObject, ObjectKey, ObjectType, ParticleObject, ParticleSettings, TextObject,
    unit_for_uuid,
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
    CreateParticle,
    Update,
    Delete,
    Geocode,
    SaveView,
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
        self.imports.clear();
        self.state.nav = NavItem::Preview;
        self.state.screen = Screen::MapEditor;
        self.refresh_assets();
        self.sync_editor_marker_models();

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

    /// Reloads the import assets of the operation shown in the editor.
    pub fn refresh_imports(&mut self) {
        let Some(operation_id) = self.editor.operation_id else {
            self.imports.clear();
            return;
        };
        let path = format!("/private/assets?origin=import&operasi_id={operation_id}");
        let handle = self.api.request(HttpMethod::Get, path, None);
        self.track(JobKind::Imports, handle);
    }

    /// Uploads the Import tab draft to the current scenario's operation.
    pub fn begin_import_upload(&mut self) {
        self.editor.import_error = None;
        if self.editor.import_busy {
            return;
        }
        let Some(operation_id) = self.editor.operation_id else {
            self.editor.import_error =
                Some("Operasi skenario belum dimuat; tunggu sebentar.".to_owned());
            return;
        };
        let payload = {
            let draft = self.editor.import_draft.clone();
            let name = draft.name.trim().to_owned();
            if name.is_empty() {
                self.editor.import_error = Some("Nama aset wajib diisi.".to_owned());
                return;
            }
            let slots = draft.slots();
            if slots
                .iter()
                .enumerate()
                .any(|(index, _)| draft.file(index).is_none())
            {
                self.editor.import_error =
                    Some(format!("Pilih {} file untuk kategori ini.", slots.len()));
                return;
            }
            let model_kind = draft.category.is_model().then_some(draft.model_kind);
            crate::state::multipart_asset_body(
                &name,
                draft.category,
                model_kind,
                draft.move_mode,
                None,
                Some(operation_id),
                &draft.files[..slots.len()],
            )
        };
        match payload {
            Ok((content_type, body)) => {
                self.editor.import_busy = true;
                let handle = self.api.upload("/private/imports", content_type, body);
                self.track(JobKind::ImportUpload, handle);
            }
            Err(error) => self.editor.import_error = Some(error),
        }
    }

    /// Deletes one import asset; the server only accepts the uploader or an
    /// admin.
    pub fn begin_import_delete(&mut self, asset_id: Uuid) {
        let handle = self.api.request(
            HttpMethod::Delete,
            format!("/private/imports/{asset_id}"),
            None,
        );
        self.track(JobKind::ImportDelete, handle);
    }

    fn load_map_defaults(&mut self) {
        self.editor.requested_flight = Some(MapCameraState::default());
        self.editor.requested_layer = Some(BaseLayer::OpenStreetMap);
    }

    /// Rebuilds the editor's `unit_id -> cached model file` map from the server
    /// asset list. Called when the list changes and once per editor frame; the
    /// signature check keeps it cheap.
    pub fn sync_editor_marker_models(&mut self) {
        let signature = self
            .assets
            .iter()
            .chain(self.imports.iter())
            .map(|asset| (asset.id, asset.version, asset.move_mode))
            .collect::<Vec<_>>();
        if signature == self.editor.marker_model_signature {
            return;
        }
        self.editor.marker_model_signature = signature;
        #[cfg(not(target_arch = "wasm32"))]
        let (models, textures, texture_files) = {
            let mut models = BTreeMap::new();
            let mut textures = BTreeMap::new();
            let mut texture_files = BTreeMap::new();
            if let (Some(cache), Some(root)) = (&self.asset_cache, self.asset_cache_root()) {
                let cache = cache.lock().expect("asset cache poisoned");
                let root = Path::new(&root);
                let base = self.base_url.trim_end_matches('/');
                for asset in self.assets.iter().chain(self.imports.iter()) {
                    if asset.category.is_image() {
                        let Some(source) = asset.sources.first() else {
                            continue;
                        };
                        let resolved = cache.resolve(asset, source.slot, false);
                        let path = resolved
                            .as_ref()
                            .and_then(|(_, path)| cache_asset_path(root, path))
                            .unwrap_or_else(|| format!("{base}{}", source.file.url));
                        if let Some((_, file)) = resolved {
                            texture_files.insert(asset.id.to_string(), file.display().to_string());
                        }
                        textures.insert(asset.id.to_string(), path);
                        continue;
                    }
                    if asset.category != AssetCategory::Model {
                        continue;
                    }
                    // Markers use the unanimated file (or the idle pose of an
                    // animated model) and load it as a full glTF scene so its
                    // own materials and textures are kept.
                    let Some(source) = asset.sources.iter().find(|source| {
                        matches!(source.slot, AssetSourceSlot::Static | AssetSourceSlot::Idle)
                    }) else {
                        continue;
                    };
                    let Some((_, path)) = cache.resolve(asset, source.slot, false) else {
                        continue;
                    };
                    let resolved_path = path.clone();
                    let Some(path) = cache_asset_path(root, &path) else {
                        continue;
                    };
                    let animated = asset.model_kind == Some(AssetModelKind::Animated)
                        || asset
                            .sources
                            .iter()
                            .any(|source| source.slot == AssetSourceSlot::Animated);
                    // Play mode swaps to the animated file so the walk clip can
                    // loop while the unit moves.
                    let animation = if animated {
                        asset
                            .sources
                            .iter()
                            .find(|source| source.slot == AssetSourceSlot::Animated)
                            .and_then(|source| {
                                let (_, path) = cache.resolve(asset, source.slot, false)?;
                                let path = cache_asset_path(root, &path)?;
                                Some((path, source.has_draco))
                            })
                    } else {
                        None
                    };
                    // Normalise the model to a usable size from the glTF
                    // bounds, so markers do not come out microscopic or huge
                    // depending on the exporter's units. The footprint radius
                    // follows the same transform so movement collision matches
                    // the rendered size.
                    let bounds = model_bounds(&resolved_path);
                    let base_scale = match bounds {
                        Some(bounds) if bounds.height > 1.0e-3 => {
                            super::model::ASSET_MARKER_HEIGHT / bounds.height
                        }
                        _ => 1.0,
                    };
                    let footprint_radius =
                        bounds.map(|bounds| bounds.footprint_radius).unwrap_or(0.0);
                    models.insert(
                        asset.id.to_string(),
                        MarkerAssetModel {
                            name: asset.name.clone(),
                            path,
                            base_scale,
                            footprint_radius,
                            draco: source.has_draco,
                            animated,
                            move_mode: asset.move_mode,
                            animation_path: animation.as_ref().map(|(path, _)| path.clone()),
                            animation_draco: animation.is_some_and(|(_, draco)| draco),
                        },
                    );
                }
            }
            (models, textures, texture_files)
        };
        #[cfg(target_arch = "wasm32")]
        let models = BTreeMap::new();
        #[cfg(target_arch = "wasm32")]
        let texture_files = BTreeMap::new();
        #[cfg(target_arch = "wasm32")]
        let textures = self
            .assets
            .iter()
            .chain(self.imports.iter())
            .filter(|asset| asset.category.is_image())
            .filter_map(|asset| {
                let source = asset.sources.first()?;
                Some((
                    asset.id.to_string(),
                    format!("{}{}", self.base_url.trim_end_matches('/'), source.file.url),
                ))
            })
            .collect::<BTreeMap<_, _>>();
        self.editor.marker_models = models;
        self.editor.texture_paths = textures;
        self.editor.texture_files = texture_files;
        self.editor.texture_previews.clear();
        self.editor.touch();
    }

    /// Server models shown in the editor's Assets tab: admin-library models
    /// plus the imports of the current operation.
    pub fn editor_asset_rows(&self) -> Vec<EditorAssetRow> {
        let previews = self.asset_preview_map();
        self.assets
            .iter()
            .chain(self.imports.iter())
            .filter(|asset| asset.category == AssetCategory::Model)
            .map(|asset| self.asset_row(asset, &previews))
            .collect()
    }

    /// Everything imported into the current operation, including textures and
    /// icons, for the Import tab list.
    pub fn editor_import_rows(&self) -> Vec<EditorAssetRow> {
        let previews = self.asset_preview_map();
        let mut rows = self
            .imports
            .iter()
            .map(|asset| self.asset_row(asset, &previews))
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.name.to_lowercase());
        rows
    }

    fn asset_row(
        &self,
        asset: &twg_contracts::assets::Asset,
        previews: &BTreeMap<String, PathBuf>,
    ) -> EditorAssetRow {
        let animated = asset.model_kind == Some(AssetModelKind::Animated)
            || asset
                .sources
                .iter()
                .any(|source| source.slot == AssetSourceSlot::Animated);
        let move_mode_label = match asset.move_mode {
            AssetMoveMode::None => "tidak bisa pindah".to_owned(),
            AssetMoveMode::Limited => "pindah via gizmo".to_owned(),
            AssetMoveMode::Free if animated => "pindah bebas (animasi)".to_owned(),
            AssetMoveMode::Free => "pindah bebas".to_owned(),
        };
        let size = asset
            .sources
            .iter()
            .map(|source| source.file.size_bytes)
            .sum::<i64>();
        let preview = self
            .cached_asset_preview(asset.id)
            .or_else(|| previews.get(&asset.id.to_string()).cloned());
        let imported = asset.origin.is_import();
        let is_model = asset.category == AssetCategory::Model;
        let user_id = self.session().map(|session| session.user_id);
        let can_delete = imported
            && (self.role() == Role::Admin
                || (asset.owner_id.is_some() && asset.owner_id == user_id));
        let id = asset.id.to_string();
        let cached = if is_model {
            self.editor.marker_models.contains_key(&id)
        } else {
            self.editor.texture_paths.contains_key(&id)
        };
        let kind_label = match asset.category {
            AssetCategory::Model if animated => "Animasi",
            AssetCategory::Model => "Static",
            AssetCategory::Texture => "Tekstur",
            AssetCategory::Icon => "Ikon",
        };
        EditorAssetRow {
            unit_id: id,
            name: asset.name.clone(),
            kind_label: kind_label.to_owned(),
            cached,
            group: if imported {
                "Import".to_owned()
            } else {
                asset
                    .group_name
                    .clone()
                    .unwrap_or_else(|| "Tanpa kategori".to_owned())
            },
            move_mode_label,
            version: asset.version,
            size_label: format_size(size),
            imported,
            is_model,
            can_delete,
            preview,
        }
    }

    /// Image assets available as shape textures in the Inspector.
    pub fn editor_texture_rows(&self) -> Vec<EditorTextureRow> {
        let mut rows = self
            .assets
            .iter()
            .chain(self.imports.iter())
            .filter(|asset| asset.category.is_image())
            .map(|asset| EditorTextureRow {
                id: asset.id.to_string(),
                name: asset.name.clone(),
                available: self
                    .editor
                    .texture_paths
                    .contains_key(&asset.id.to_string()),
                imported: asset.origin.is_import(),
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    pub fn close_editor(&mut self) {
        self.state.screen = Screen::Workspace;
        self.state.nav = NavItem::Scenarios;
        self.editor.preview = false;
        self.editor.stop_play();
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

    /// Persists the current camera (center, zoom, bearing, pitch) and base layer
    /// as the scenario default, so a reload, Play session, or exercise reopens
    /// the same view.
    pub fn editor_save_view(&mut self) {
        if self.editor.saving_view {
            return;
        }
        if !self.role().is_admin_like() {
            self.toast(
                "Hanya admin yang dapat menyimpan tampilan kamera.",
                ToastKind::Error,
            );
            return;
        }
        if self.editor.loading {
            self.toast("Tunggu pemuatan skenario selesai.", ToastKind::Info);
            return;
        }
        let Some(scenario_id) = self.editor.scenario_id else {
            self.toast(
                "Pilih skenario sebelum menyimpan tampilan.",
                ToastKind::Error,
            );
            return;
        };
        let camera = self.editor.map_state;
        // Indoor/floor-plan scenarios keep their server-side base layer: the
        // editor only has OSM/satellite tiles and must not overwrite it.
        let base_layer = if self.editor.scenario_indoor || self.editor.floor_plan {
            String::new()
        } else {
            match self.editor.base_layer {
                BaseLayer::ArcGisImagery => "satellite".to_owned(),
                BaseLayer::OpenStreetMap => "osm".to_owned(),
            }
        };
        let body = serde_json::json!({
            "center_x": camera.center.longitude,
            "center_y": camera.center.latitude,
            "zoom": f64::from(camera.zoom),
            "pitch": f64::from(camera.pitch_degrees),
            "bearing": f64::from(camera.bearing_degrees),
            "map_mode": "",
            "base_layer": base_layer,
            "sky_mode": self.editor.sky.slug(),
        });
        self.editor.saving_view = true;
        self.editor.save_error = None;
        let handle = self.api.request(
            HttpMethod::Put,
            format!("/admin/skenarios/{scenario_id}"),
            Some(body),
        );
        self.editor.log("Menyimpan tampilan kamera...");
        self.editor.jobs.push(EditorJob {
            kind: EditorJobKind::SaveView,
            key: None,
            handle,
        });
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
            EditorJobKind::CreateParticle => {
                self.create_done(key, result, |response| {
                    response.data::<Particle>().map(|particle| particle.id)
                });
            }
            EditorJobKind::Update | EditorJobKind::Delete => {
                if let Err(error) = result {
                    self.editor.save_error = Some(error.message);
                }
                self.editor_save_done();
            }
            EditorJobKind::SaveView => {
                self.editor.saving_view = false;
                match result.and_then(|response| response.data::<Scenario>()) {
                    Ok(scenario) => {
                        self.editor.scenario_center =
                            GeoPoint::new(scenario.center_x, scenario.center_y);
                        self.editor.saved_view = Some(MapCameraState {
                            center: self.editor.scenario_center,
                            zoom: scenario.zoom as f32,
                            pitch_degrees: scenario.pitch as f32,
                            bearing_degrees: scenario.bearing as f32,
                        });
                        self.editor.saved_base_layer = Some(self.editor.base_layer);
                        self.editor.sky = SkyMode::from_slug(&scenario.sky_mode);
                        let time = chrono::Local::now().format("%H:%M:%S").to_string();
                        self.editor.view_saved_at = Some(time.clone());
                        self.editor.last_save = Some(time);
                        self.editor.log("Tampilan kamera dan langit tersimpan.");
                        self.toast("Tampilan skenario tersimpan.", ToastKind::Success);
                        self.editor.touch();
                    }
                    Err(error) => {
                        self.editor.save_error = Some(error.message.clone());
                        self.toast(
                            format!("Simpan tampilan gagal: {}", error.message),
                            ToastKind::Error,
                        );
                    }
                }
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
        self.refresh_imports();
        self.editor.scenario_name = scenario.name.clone();
        self.editor.scenario_center = GeoPoint::new(scenario.center_x, scenario.center_y);
        self.editor.indoor = scenario.map_mode == MapMode::Indoor;
        self.editor.scenario_indoor = self.editor.indoor;
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
        self.editor.sky = SkyMode::from_slug(&scenario.sky_mode);
        let layer = match scenario.base_layer {
            DomainBaseLayer::Satellite => BaseLayer::ArcGisImagery,
            _ => BaseLayer::OpenStreetMap,
        };
        let view = MapCameraState {
            center: GeoPoint::new(scenario.center_x, scenario.center_y),
            zoom: scenario.zoom as f32,
            pitch_degrees: scenario.pitch as f32,
            bearing_degrees: scenario.bearing as f32,
        };
        self.editor.requested_layer = Some(layer);
        self.editor.requested_flight = Some(view);
        // The camera just loaded is what the server currently stores, so the
        // Inspector reports "Tersimpan" until the user moves the view.
        self.editor.saved_view = Some(view);
        self.editor.saved_base_layer = Some(layer);
        self.editor.view_saved_at = None;
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

/// Bounds of the meshes in a glTF/GLB file, measured after applying the node
/// hierarchy. Height is used to normalise marker size; the footprint radius is
/// the half diagonal of the XZ bounds and feeds Play obstacle avoidance. Only
/// the JSON chunk of a GLB is read.
#[cfg(not(target_arch = "wasm32"))]
fn model_bounds(path: &Path) -> Option<ModelBounds> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header).ok()?;
    let json = if &header[0..4] == b"glTF" {
        let mut chunk = [0u8; 8];
        file.read_exact(&mut chunk).ok()?;
        let length = u32::from_le_bytes(chunk[0..4].try_into().ok()?) as usize;
        let kind = u32::from_le_bytes(chunk[4..8].try_into().ok()?);
        // "JSON" chunk type.
        if kind != 0x4E4F_534A || length == 0 {
            return None;
        }
        let mut buffer = vec![0u8; length.min(16 * 1024 * 1024)];
        file.read_exact(&mut buffer).ok()?;
        buffer
    } else {
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok()?;
        buffer
    };
    let value: serde_json::Value = serde_json::from_slice(&json).ok()?;
    let accessors = value.get("accessors")?.as_array()?;
    let meshes = value.get("meshes")?.as_array()?;

    // Accessor bounds are in mesh-local space. Exported models often carry a
    // node scale (for example 100x) and a rotation, so the real-world height
    // must be measured after applying the node hierarchy.
    if let Some(nodes) = value.get("nodes").and_then(Value::as_array) {
        let mut parents = vec![None; nodes.len()];
        for (index, node) in nodes.iter().enumerate() {
            let Some(children) = node.get("children").and_then(Value::as_array) else {
                continue;
            };
            for child in children {
                if let Some(child) = child.as_u64().map(|value| value as usize)
                    && child < parents.len()
                {
                    parents[child] = Some(index);
                }
            }
        }
        let mut cache = vec![None; nodes.len()];
        let mut visiting = vec![false; nodes.len()];
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for (index, node) in nodes.iter().enumerate() {
            let Some(mesh) = node
                .get("mesh")
                .and_then(Value::as_u64)
                .and_then(|mesh| meshes.get(mesh as usize))
            else {
                continue;
            };
            let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) else {
                continue;
            };
            let world = node_world(index, nodes, &parents, &mut cache, &mut visiting);
            for primitive in primitives {
                let Some(accessor) = primitive
                    .pointer("/attributes/POSITION")
                    .and_then(Value::as_u64)
                    .and_then(|index| accessors.get(index as usize))
                else {
                    continue;
                };
                let Some(low) = accessor.get("min").and_then(Value::as_array) else {
                    continue;
                };
                let Some(high) = accessor.get("max").and_then(Value::as_array) else {
                    continue;
                };
                let (Some(lx), Some(ly), Some(lz)) = (coord(low, 0), coord(low, 1), coord(low, 2))
                else {
                    continue;
                };
                let (Some(hx), Some(hy), Some(hz)) =
                    (coord(high, 0), coord(high, 1), coord(high, 2))
                else {
                    continue;
                };
                for x in [lx, hx] {
                    for y in [ly, hy] {
                        for z in [lz, hz] {
                            let point =
                                world.transform_point3(Vec3::new(x as f32, y as f32, z as f32));
                            min = min.min(point);
                            max = max.max(point);
                        }
                    }
                }
            }
        }
        if min.is_finite() && max.is_finite() && max.y > min.y {
            return Some(ModelBounds::from_extents(min, max));
        }
    }

    // Fallback for files without node transforms.
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for mesh in meshes {
        let Some(primitives) = mesh.get("primitives").and_then(|value| value.as_array()) else {
            continue;
        };
        for primitive in primitives {
            let Some(index) = primitive
                .pointer("/attributes/POSITION")
                .and_then(|value| value.as_u64())
            else {
                continue;
            };
            let Some(accessor) = accessors.get(index as usize) else {
                continue;
            };
            let Some(low) = accessor.get("min").and_then(Value::as_array) else {
                continue;
            };
            let Some(high) = accessor.get("max").and_then(Value::as_array) else {
                continue;
            };
            let (Some(lx), Some(ly), Some(lz)) = (coord(low, 0), coord(low, 1), coord(low, 2))
            else {
                continue;
            };
            let (Some(hx), Some(hy), Some(hz)) = (coord(high, 0), coord(high, 1), coord(high, 2))
            else {
                continue;
            };
            min = min.min(Vec3::new(lx as f32, ly as f32, lz as f32));
            max = max.max(Vec3::new(hx as f32, hy as f32, hz as f32));
        }
    }
    (min.is_finite() && max.is_finite() && max.y > min.y)
        .then(|| ModelBounds::from_extents(min, max))
}

/// Size of a model in its own (transformed) units.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
struct ModelBounds {
    height: f32,
    /// Half of the XZ diagonal, used as a conservative ground footprint.
    footprint_radius: f32,
}

#[cfg(not(target_arch = "wasm32"))]
impl ModelBounds {
    fn from_extents(min: Vec3, max: Vec3) -> Self {
        Self {
            height: max.y - min.y,
            footprint_radius: 0.5 * (max.x - min.x).hypot(max.z - min.z),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn coord(values: &[Value], index: usize) -> Option<f64> {
    values.get(index).and_then(Value::as_f64)
}

#[cfg(not(target_arch = "wasm32"))]
fn node_local_matrix(node: &Value) -> Mat4 {
    if let Some(matrix) = node.get("matrix").and_then(Value::as_array)
        && matrix.len() == 16
    {
        let mut values = [0.0f32; 16];
        for (slot, value) in values.iter_mut().zip(matrix) {
            *slot = value.as_f64().unwrap_or(0.0) as f32;
        }
        return Mat4::from_cols_array(&values);
    }
    let translation = node
        .get("translation")
        .and_then(Value::as_array)
        .and_then(|values| vector3(values.as_slice()))
        .map(Vec3::from_array)
        .unwrap_or(Vec3::ZERO);
    let rotation = node
        .get("rotation")
        .and_then(Value::as_array)
        .and_then(|values| {
            Some([
                coord(values, 0)? as f32,
                coord(values, 1)? as f32,
                coord(values, 2)? as f32,
                coord(values, 3)? as f32,
            ])
        })
        .map(|[x, y, z, w]| Quat::from_xyzw(x, y, z, w))
        .unwrap_or(Quat::IDENTITY);
    let scale = node
        .get("scale")
        .and_then(Value::as_array)
        .and_then(|values| vector3(values.as_slice()))
        .map(Vec3::from_array)
        .unwrap_or(Vec3::ONE);
    Mat4::from_scale_rotation_translation(scale, rotation, translation)
}

#[cfg(not(target_arch = "wasm32"))]
fn vector3(values: &[Value]) -> Option<[f32; 3]> {
    Some([
        coord(values, 0)? as f32,
        coord(values, 1)? as f32,
        coord(values, 2)? as f32,
    ])
}

#[cfg(not(target_arch = "wasm32"))]
fn node_world(
    index: usize,
    nodes: &[Value],
    parents: &[Option<usize>],
    cache: &mut [Option<Mat4>],
    visiting: &mut [bool],
) -> Mat4 {
    if let Some(matrix) = cache[index] {
        return matrix;
    }
    // Malformed files could contain a cycle; refuse to recurse forever.
    if visiting[index] {
        return Mat4::IDENTITY;
    }
    visiting[index] = true;
    let local = node_local_matrix(&nodes[index]);
    let world = match parents[index] {
        Some(parent) => node_world(parent, nodes, parents, cache, visiting) * local,
        None => local,
    };
    visiting[index] = false;
    cache[index] = Some(world);
    world
}

fn format_size(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let bytes = bytes.max(0) as f64;
    if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes / KB)
    } else {
        format!("{} B", bytes as i64)
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
            obstacle: marker.obstacle,
        }),
    }
}

fn line_object(line: Line) -> EditorObject {
    let (points, altitudes) = parse_line_geom(&line.geom);
    EditorObject {
        key: 0,
        server_id: Some(line.id),
        name: line.name,
        visible: true,
        dirty: false,
        kind: EditorKind::Line(LineObject {
            points,
            altitudes,
            color: line.color,
            width: line.width,
            alpha: line.alpha,
        }),
    }
}

fn building_object(building: Building) -> EditorObject {
    let points = parse_geom(&building.geom);
    let meta = shape_meta(&building.keterangan);
    let (shape, center, rotation, scale, texture, use_texture, keterangan, obstacle) = match meta {
        Some(meta) => {
            let center = meta
                .lon
                .zip(meta.lat)
                .map(|(lon, lat)| GeoPoint::new(lon, lat))
                .or_else(|| points_centroid_geo(&points))
                .unwrap_or(GeoPoint::new(0.0, 0.0));
            (
                EditorShapeKind::from_key(&meta.shape),
                center,
                meta.rot,
                meta.scale,
                meta.texture,
                meta.use_texture,
                meta.note,
                meta.obstacle,
            )
        }
        None => (
            EditorShapeKind::Custom,
            points_centroid_geo(&points).unwrap_or(GeoPoint::new(0.0, 0.0)),
            [0.0; 3],
            [1.0; 3],
            None,
            false,
            building.keterangan,
            true,
        ),
    };
    EditorObject {
        key: 0,
        server_id: Some(building.id),
        name: building.name,
        visible: true,
        dirty: false,
        kind: EditorKind::Building(BuildingObject {
            shape,
            lon: center.longitude,
            lat: center.latitude,
            altitude: 0.0,
            rot_x: rotation[0],
            rot_y: rotation[1],
            rot_z: rotation[2],
            scale_x: scale[0],
            scale_y: scale[1],
            scale_z: scale[2],
            color: building.color,
            use_texture,
            texture,
            points,
            height: building.height,
            keterangan,
            obstacle,
        }),
    }
}

/// Shape metadata stored in the legacy `buildings.keterangan` column.
struct ShapeMeta {
    shape: String,
    lon: Option<f64>,
    lat: Option<f64>,
    rot: [f64; 3],
    scale: [f64; 3],
    texture: Option<String>,
    use_texture: bool,
    note: String,
    obstacle: bool,
}

fn shape_meta(keterangan: &str) -> Option<ShapeMeta> {
    let value: Value = serde_json::from_str(keterangan).ok()?;
    let shape = value.get("shape")?.as_str()?.trim().to_owned();
    if shape.is_empty() {
        return None;
    }
    Some(ShapeMeta {
        shape,
        lon: value.get("lon").and_then(Value::as_f64),
        lat: value.get("lat").and_then(Value::as_f64),
        rot: value_array3(value.get("rot"), [0.0; 3]),
        scale: value_array3(value.get("scale"), [1.0; 3]),
        texture: value
            .get("texture")
            .and_then(Value::as_str)
            .map(str::to_owned),
        use_texture: value
            .get("use_texture")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        note: value
            .get("note")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        obstacle: value
            .get("obstacle")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

fn value_array3(value: Option<&Value>, default: [f64; 3]) -> [f64; 3] {
    let Some(values) = value.and_then(Value::as_array) else {
        return default;
    };
    let mut result = default;
    for (index, value) in values.iter().take(3).enumerate() {
        if let Some(number) = value.as_f64() {
            result[index] = number;
        }
    }
    result
}

fn points_centroid_geo(points: &[[f64; 2]]) -> Option<GeoPoint> {
    if points.is_empty() {
        return None;
    }
    let count = points.len() as f64;
    Some(GeoPoint::new(
        points.iter().map(|point| point[0]).sum::<f64>() / count,
        points.iter().map(|point| point[1]).sum::<f64>() / count,
    ))
}

fn building_meta_json(building: &BuildingObject) -> String {
    serde_json::json!({
        "shape": building.shape.key(),
        "lon": building.lon,
        "lat": building.lat,
        "rot": [building.rot_x, building.rot_y, building.rot_z],
        "scale": [building.scale_x, building.scale_y, building.scale_z],
        "texture": building.texture,
        "use_texture": building.use_texture,
        "note": building.keterangan,
        "obstacle": building.obstacle,
    })
    .to_string()
}

fn building_geom_json(building: &BuildingObject) -> String {
    let points = building
        .outline()
        .iter()
        .map(|point| [point.longitude, point.latitude])
        .collect::<Vec<_>>();
    geom_json(&points)
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
            altitude: text.altitude,
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
            altitude: particle.altitude,
            kind: ParticleKind::from_key(&particle.tipe),
            settings: ParticleSettings::from_json(&particle.data),
        }),
    }
}

fn parse_geom(geom: &str) -> Vec<[f64; 2]> {
    serde_json::from_str(geom).unwrap_or_default()
}

fn geom_json(points: &[[f64; 2]]) -> String {
    serde_json::to_string(points).unwrap_or_else(|_| "[]".to_owned())
}

/// Lines store `[lon, lat]` or `[lon, lat, altitude]` rows. Older scenarios
/// only have two values per point; those load with altitude 0.
fn parse_line_geom(geom: &str) -> (Vec<[f64; 2]>, Vec<f64>) {
    let Ok(rows) = serde_json::from_str::<Vec<Value>>(geom) else {
        return (Vec::new(), Vec::new());
    };
    let mut points = Vec::with_capacity(rows.len());
    let mut altitudes = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(items) = row.as_array() else {
            continue;
        };
        let (Some(lon), Some(lat)) = (
            items.first().and_then(Value::as_f64),
            items.get(1).and_then(Value::as_f64),
        ) else {
            continue;
        };
        points.push([lon, lat]);
        altitudes.push(items.get(2).and_then(Value::as_f64).unwrap_or(0.0));
    }
    (points, altitudes)
}

fn line_geom_json(line: &LineObject) -> String {
    let with_altitudes = line
        .altitudes
        .iter()
        .any(|altitude| altitude.abs() > 1.0e-4);
    let rows = line
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            if with_altitudes {
                serde_json::json!([
                    point[0],
                    point[1],
                    line.altitudes.get(index).copied().unwrap_or(0.0)
                ])
            } else {
                serde_json::json!([point[0], point[1]])
            }
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned())
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
                obstacle: marker.obstacle,
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
                geom: line_geom_json(line),
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
                geom: building_geom_json(building),
                height: building.render_height(),
                keterangan: building_meta_json(building),
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
                altitude: text.altitude,
            })
            .ok()?,
        ),
        EditorKind::Particle(particle) => (
            EditorJobKind::CreateParticle,
            serde_json::to_value(ParticleCreateRequest {
                operasi_id: operation_id.to_string(),
                skenario_id: scenario_id.to_string(),
                name: object.name.clone(),
                tipe: particle.kind.key().to_owned(),
                data: particle.settings.to_json(),
                pos_x: particle.lon,
                pos_y: particle.lat,
                altitude: particle.altitude,
            })
            .ok()?,
        ),
    };
    let path = match kind {
        EditorJobKind::CreateMarker => "/private/markers",
        EditorJobKind::CreateLine => "/private/lines",
        EditorJobKind::CreateBuilding => "/private/buildings",
        EditorJobKind::CreateText => "/private/text-markers",
        EditorJobKind::CreateParticle => "/private/particles",
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
                path: format!("/private/markers/unit/{id}"),
                body: serde_json::to_value(MarkerUnitRequest {
                    unit_id: marker.unit_id.clone(),
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
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/markers/obstacle/{id}"),
                body: serde_json::to_value(twg_contracts::map::MarkerObstacleRequest {
                    obstacle: marker.obstacle,
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
                    geom: line_geom_json(line),
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
                    keterangan: building_meta_json(building),
                    height: building.render_height(),
                })
                .ok(),
            });
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/buildings/geom/{id}"),
                body: serde_json::to_value(BuildingGeomUpdateRequest {
                    geom: building_geom_json(building),
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
                    altitude: text.altitude,
                })
                .ok(),
            });
        }
        EditorKind::Particle(particle) => {
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/particles/{id}"),
                body: serde_json::to_value(ParticleUpdateRequest {
                    name: object.name.clone(),
                    tipe: particle.kind.key().to_owned(),
                    data: particle.settings.to_json(),
                })
                .ok(),
            });
            requests.push(EditorRequest {
                kind: EditorJobKind::Update,
                key: Some(object.key),
                method: HttpMethod::Put,
                path: format!("/private/particles/geom/{id}"),
                body: serde_json::to_value(ParticleGeomUpdateRequest {
                    pos_x: particle.lon,
                    pos_y: particle.lat,
                    altitude: particle.altitude,
                })
                .ok(),
            });
        }
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
        ObjectType::Particle => format!("/private/particles/{id}"),
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn reads_the_height_of_a_cached_glb() {
        let path = Path::new("../../assets/models/polwan_airud_idle_lod2.glb");
        let bounds = model_bounds(path).expect("GLB bounds");
        assert!(
            bounds.height > 0.01 && bounds.height < 100_000.0,
            "height {}",
            bounds.height
        );
        assert!(bounds.footprint_radius > 0.0);
    }

    #[test]
    fn node_scale_and_rotation_are_applied_to_the_height() {
        let json = br#"{"asset":{"version":"2.0"},"accessors":[{"componentType":5126,"count":8,"type":"VEC3","min":[-0.5,-0.5,0],"max":[0.5,0.5,0.25]}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"nodes":[{"mesh":0,"rotation":[-0.70710678,0,0,0.70710678],"scale":[100,100,100]}],"scenes":[{"nodes":[0]}]}"#;
        let mut padded = json.to_vec();
        while !padded.len().is_multiple_of(4) {
            padded.push(b' ');
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&((12 + 8 + padded.len()) as u32).to_le_bytes());
        bytes.extend_from_slice(&(padded.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        bytes.extend_from_slice(&padded);

        let path = std::env::temp_dir().join("twg-model-height-test.glb");
        std::fs::write(&path, bytes).expect("write test GLB");
        let bounds = model_bounds(&path).expect("GLB bounds");
        let _ = std::fs::remove_file(&path);
        // Local Z (0.25) maps to world Y after the -90 degree X rotation, then
        // the node scale of 100 applies. X and Z each span one unit at 100x.
        assert!(
            (bounds.height - 25.0).abs() < 0.01,
            "height {}",
            bounds.height
        );
        assert!(
            (bounds.footprint_radius - 70.710_68).abs() < 0.01,
            "footprint {}",
            bounds.footprint_radius
        );
    }

    #[test]
    fn line_geometry_round_trips_point_altitudes() {
        let mut line = LineObject::new(vec![[115.2, -8.6], [115.21, -8.61]]);
        line.altitudes = vec![0.0, 12.5];

        let (points, altitudes) = parse_line_geom(&line_geom_json(&line));

        assert_eq!(points, line.points);
        assert_eq!(altitudes, vec![0.0, 12.5]);
        // Legacy two-value rows still load with altitude 0.
        let (points, altitudes) = parse_line_geom("[[115.2,-8.6],[115.21,-8.61]]");
        assert_eq!(points.len(), 2);
        assert_eq!(altitudes, vec![0.0, 0.0]);
    }

    #[test]
    fn shape_metadata_round_trips_through_keterangan() {
        let mut building =
            BuildingObject::primitive(EditorShapeKind::Cylinder, GeoPoint::new(115.2, -8.6));
        building.rot_y = 45.0;
        building.scale_x = 3.0;
        building.use_texture = true;
        building.texture = Some("asset-id".to_owned());
        building.keterangan = "catatan".to_owned();
        building.obstacle = false;

        let meta = shape_meta(&building_meta_json(&building)).expect("shape metadata");

        assert_eq!(meta.shape, "cylinder");
        assert_eq!(meta.rot[1], 45.0);
        assert_eq!(meta.scale[0], 3.0);
        assert_eq!(meta.texture.as_deref(), Some("asset-id"));
        assert!(meta.use_texture);
        assert_eq!(meta.note, "catatan");
        assert!(!meta.obstacle);
        // Legacy metadata without the field stays an obstacle.
        let legacy = shape_meta(r#"{"shape":"box","note":"lama"}"#).expect("legacy meta");
        assert!(legacy.obstacle);
        let geom = building_geom_json(&building);
        assert!(geom.starts_with("[[") && geom.ends_with("]]"));
    }
}
