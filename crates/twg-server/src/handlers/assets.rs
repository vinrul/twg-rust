use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use chrono::{FixedOffset, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;
use twg_contracts::assets::{
    Asset, AssetCategory, AssetManifest, AssetModelKind, AssetSourceSlot, AssetUpdateRequest,
    AssetVariantKind, ConvertRequest, ConvertResult, ConvertTarget,
};
use twg_contracts::uploads::ALLOWED_IMAGE_TYPES;
use twg_contracts::{ListResponse, MessageResponse, SingleResponse};
use twg_storage::{NewAsset, NewAssetSource, NewAssetVariant};
use uuid::Uuid;

use crate::assets::{
    original_filename, probe_gltf, resize_image, run_convert, sha256_hex, stamp_glb_metadata,
    variant_filename,
};
use crate::error::ApiError;
use crate::handlers::asset_store;
use crate::state::AppState;

const MAX_NAME_CHARS: usize = 120;

#[derive(Debug, Default, Deserialize)]
pub struct ListAssetsQuery {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    search: Option<String>,
}

fn timestamp() -> twg_contracts::Timestamp {
    Utc::now().with_timezone(&FixedOffset::east_opt(8 * 60 * 60).expect("valid WITA offset"))
}

fn valid_name(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("name is required"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(ApiError::bad_request(format!(
            "name is longer than {MAX_NAME_CHARS} characters"
        )));
    }
    Ok(name.to_owned())
}

fn file_extension(filename: &str) -> Option<String> {
    std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
}

fn parse_category(raw: Option<&str>) -> Result<AssetCategory, ApiError> {
    let value = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("category is required"))?;
    AssetCategory::from_str(value)
        .map_err(|_| ApiError::bad_request("category must be one of: model, texture, icon"))
}

/// Validates one uploaded source file and fills the metadata stored in
/// `asset_sources`.
fn prepare_source(
    category: AssetCategory,
    slot: AssetSourceSlot,
    original_name: &str,
    bytes: &[u8],
) -> Result<NewAssetSource, ApiError> {
    if bytes.is_empty() {
        return Err(ApiError::bad_request(format!(
            "uploaded {} file is empty",
            slot
        )));
    }
    let extension = file_extension(original_name)
        .filter(|extension| category.allows_extension(extension))
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "{} uploads must use one of: {}",
                category,
                category.upload_extensions().join(", ")
            ))
        })?;

    let (mime, has_draco, lod) = if category.is_model() {
        let probe = probe_gltf(bytes, original_name).map_err(|error| {
            ApiError::bad_request(format!("invalid glTF/GLB in {slot}: {error}"))
        })?;
        if probe.external_resources {
            return Err(ApiError::bad_request(format!(
                "glTF in {slot} has external .bin/texture files; upload a self-contained .glb"
            )));
        }
        if slot.is_animated_source() && probe.animation_count == 0 {
            return Err(ApiError::bad_request(
                "the `animated` file has no animations; upload the GLB/glTF that contains the animation",
            ));
        }
        let mime = if extension == "glb" {
            "model/gltf-binary"
        } else {
            "model/gltf+json"
        };
        (mime.to_owned(), probe.has_draco, probe.lod)
    } else {
        let mime = infer::get(bytes)
            .map(|kind| kind.mime_type())
            .unwrap_or("application/octet-stream");
        if !ALLOWED_IMAGE_TYPES.contains(&mime) {
            return Err(ApiError::bad_request(format!(
                "unsupported image type in {slot}: {mime}"
            )));
        }
        (mime.to_owned(), false, None)
    };

    Ok(NewAssetSource {
        slot,
        filename: original_filename(&extension),
        mime,
        size_bytes: bytes.len() as i64,
        sha256: sha256_hex(bytes),
        has_draco,
        lod,
    })
}

// ---------------------------------------------------------------- upload

pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SingleResponse<Asset>>), ApiError> {
    let store = asset_store(&state)?;

    let mut name = None;
    let mut category = None;
    let mut model_kind = None;
    let mut files: HashMap<String, (String, Bytes)> = HashMap::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(format!("invalid multipart form: {error}")))?
    {
        let field_name = field.name().unwrap_or_default().to_owned();
        match field_name.as_str() {
            "name" => {
                name =
                    Some(field.text().await.map_err(|error| {
                        ApiError::bad_request(format!("invalid name: {error}"))
                    })?);
            }
            "category" => {
                category = Some(field.text().await.map_err(|error| {
                    ApiError::bad_request(format!("invalid category: {error}"))
                })?);
            }
            "model_kind" => {
                model_kind = Some(field.text().await.map_err(|error| {
                    ApiError::bad_request(format!("invalid model_kind: {error}"))
                })?);
            }
            "file" | "idle" | "animated" => {
                let original_name = field.file_name().unwrap_or("asset").to_owned();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|error| ApiError::bad_request(format!("invalid upload: {error}")))?;
                files.insert(field_name, (original_name, bytes));
            }
            _ => {}
        }
    }

    let name = valid_name(&name.unwrap_or_default())?;
    let category = parse_category(category.as_deref())?;

    // Models must say whether they are static (one file) or animated (idle +
    // animated). Textures/icons always use the `static` slot.
    let model_kind =
        if category.is_model() {
            let raw = model_kind
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request("model_kind is required for models: static or animated")
                })?;
            Some(AssetModelKind::from_str(raw).map_err(|_| {
                ApiError::bad_request("model_kind must be one of: static, animated")
            })?)
        } else {
            None
        };

    let required: &[AssetSourceSlot] = match model_kind {
        Some(kind) => kind.slots(),
        None => &[AssetSourceSlot::Static],
    };
    let required_fields = required
        .iter()
        .map(|slot| {
            model_kind
                .unwrap_or(AssetModelKind::Static)
                .upload_field(*slot)
        })
        .collect::<Vec<_>>();
    for field in files.keys() {
        if !required_fields.contains(&field.as_str()) {
            return Err(ApiError::bad_request(format!(
                "field `{field}` is not used by this upload (expected {})",
                required_fields
                    .iter()
                    .map(|field| format!("`{field}`"))
                    .collect::<Vec<_>>()
                    .join(" + ")
            )));
        }
    }

    let mut sources = Vec::with_capacity(required.len());
    let mut payloads = Vec::with_capacity(required.len());
    for (slot, field) in required.iter().zip(required_fields.iter()) {
        let (original_name, bytes) = files.remove(*field).ok_or_else(|| {
            ApiError::bad_request(format!("missing `{field}` file for slot {slot}"))
        })?;
        sources.push(prepare_source(category, *slot, &original_name, &bytes)?);
        payloads.push(bytes);
    }

    let asset_id = Uuid::new_v4();
    for (slot, bytes) in required.iter().zip(payloads.iter()) {
        let filename = sources
            .iter()
            .find(|source| source.slot == *slot)
            .map(|source| source.filename.clone())
            .unwrap_or_else(|| "original".to_owned());
        state
            .asset_files
            .write(asset_id, *slot, &filename, bytes)
            .await
            .map_err(ApiError::internal)?;
    }

    let created = store
        .create(NewAsset {
            id: asset_id,
            name,
            category,
            model_kind,
            sources,
        })
        .await;
    match created {
        Ok(asset) => Ok((StatusCode::CREATED, Json(SingleResponse::success(asset)))),
        Err(error) => {
            let _ = state.asset_files.remove_asset_dir(asset_id).await;
            Err(ApiError::internal(error))
        }
    }
}

// ---------------------------------------------------------------- reads

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListAssetsQuery>,
) -> Result<Json<ListResponse<Asset>>, ApiError> {
    let category = match query.category.as_deref().map(str::trim) {
        None | Some("") | Some("all") => None,
        Some(value) => Some(
            AssetCategory::from_str(value)
                .map_err(|_| ApiError::bad_request("invalid asset category"))?,
        ),
    };
    let assets = asset_store(&state)?
        .list(category, query.search.as_deref().unwrap_or(""))
        .await
        .map_err(ApiError::internal)?;
    let total = assets.len();
    Ok(Json(ListResponse::new(assets, total)))
}

pub async fn manifest(
    State(state): State<AppState>,
) -> Result<Json<SingleResponse<AssetManifest>>, ApiError> {
    let assets = asset_store(&state)?
        .list(None, "")
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SingleResponse::success(AssetManifest {
        generated_at: timestamp(),
        assets,
    })))
}

pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SingleResponse<Asset>>, ApiError> {
    let asset = asset_store(&state)?
        .get(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("asset not found"))?;
    Ok(Json(SingleResponse::success(asset)))
}

// ---------------------------------------------------------------- writes

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AssetUpdateRequest>,
) -> Result<Json<SingleResponse<Asset>>, ApiError> {
    let store = asset_store(&state)?;
    let current = store
        .get(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("asset not found"))?;
    let name = request.name.as_deref().map(valid_name).transpose()?;
    if let Some(category) = request.category
        && category != current.category
        && !(category.is_image() && current.category.is_image())
    {
        return Err(ApiError::bad_request(
            "category can only change between texture and icon; upload a new asset instead",
        ));
    }
    let asset = store
        .update(id, name.as_deref(), request.category)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("asset not found"))?;
    Ok(Json(SingleResponse::success(asset)))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    let removed = asset_store(&state)?
        .delete(id)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(ApiError::not_found("asset not found"));
    }
    state
        .asset_files
        .remove_asset_dir(id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(MessageResponse {
        code: 200,
        message: "success".to_owned(),
    }))
}

// ---------------------------------------------------------------- convert

pub async fn convert(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ConvertRequest>,
) -> Result<Json<SingleResponse<ConvertResult>>, ApiError> {
    let store = asset_store(&state)?;
    let asset = store
        .get(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("asset not found"))?;
    let slot = resolve_slot(&asset, &request)?;
    let kind = requested_kind(&asset, &request)?;
    let source = asset
        .source(slot)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("source slot {slot} not found")))?;

    if !request.force {
        if let Some(existing) = source.variant(kind) {
            return Ok(Json(SingleResponse::success(ConvertResult {
                asset,
                slot,
                kind: Some(existing.kind),
                converted: false,
                message: "variant already exists".to_owned(),
            })));
        }
        // The uploaded file already satisfies the target: link the source
        // instead of re-encoding an identical result.
        if source_satisfies(&source, kind) {
            let updated = store
                .upsert_variant(
                    id,
                    slot,
                    NewAssetVariant {
                        kind,
                        filename: source.file.filename.clone(),
                        mime: source.file.mime.clone(),
                        size_bytes: source.file.size_bytes,
                        sha256: source.file.sha256.clone(),
                        width: None,
                        height: None,
                        reused: true,
                    },
                )
                .await
                .map_err(ApiError::internal)?
                .ok_or_else(|| ApiError::not_found("asset not found"))?;
            return Ok(Json(SingleResponse::success(ConvertResult {
                asset: updated,
                slot,
                kind: Some(kind),
                converted: false,
                message: format!(
                    "{} upload is already {}; variant linked to the source file",
                    slot,
                    describe(&source)
                ),
            })));
        }
    }

    let bytes = state
        .asset_files
        .read(id, slot, &source.file.filename)
        .await
        .map_err(ApiError::internal)?;

    let (variant_bytes, mime, extension, width, height) = if let Some(side) = kind.resize_side() {
        let mime = source.file.mime.clone();
        let resized = tokio::task::spawn_blocking(move || resize_image(&bytes, side, &mime))
            .await
            .map_err(ApiError::internal)?
            .map_err(ApiError::conversion_failed)?;
        (
            resized.bytes,
            resized.mime,
            resized.extension,
            Some(resized.width),
            Some(resized.height),
        )
    } else {
        let config = (*state.convert).clone();
        let input = state.asset_files.path(id, slot, &source.file.filename);
        let output = state
            .asset_files
            .path(id, slot, &variant_filename(kind, "glb"));
        let target = match kind {
            AssetVariantKind::Draco => ConvertTarget::Draco,
            AssetVariantKind::Lod2 => ConvertTarget::Lod2,
            _ => ConvertTarget::DracoLod2,
        };
        let (input, convert_output) = (input.clone(), output.clone());
        tokio::task::spawn_blocking(move || run_convert(&config, &input, &convert_output, target))
            .await
            .map_err(ApiError::internal)?
            .map_err(ApiError::conversion_failed)?;

        let raw = tokio::fs::read(&output).await.map_err(ApiError::internal)?;
        let draco = kind.is_draco();
        let lod = if kind.is_lod2() { Some(2) } else { source.lod };
        let stamped = tokio::task::spawn_blocking(move || stamp_glb_metadata(&raw, lod, draco))
            .await
            .map_err(ApiError::internal)?
            .map_err(ApiError::conversion_failed)?;
        tokio::fs::write(&output, &stamped)
            .await
            .map_err(ApiError::internal)?;
        (stamped, "model/gltf-binary".to_owned(), "glb", None, None)
    };

    let filename = variant_filename(kind, extension);
    state
        .asset_files
        .write(id, slot, &filename, &variant_bytes)
        .await
        .map_err(ApiError::internal)?;

    let updated = store
        .upsert_variant(
            id,
            slot,
            NewAssetVariant {
                kind,
                filename,
                mime,
                size_bytes: variant_bytes.len() as i64,
                sha256: sha256_hex(&variant_bytes),
                width,
                height,
                reused: false,
            },
        )
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("asset not found"))?;

    Ok(Json(SingleResponse::success(ConvertResult {
        asset: updated,
        slot,
        kind: Some(kind),
        converted: true,
        message: format!("{} {slot} converted", kind.file_stem()),
    })))
}

fn resolve_slot(asset: &Asset, request: &ConvertRequest) -> Result<AssetSourceSlot, ApiError> {
    let allowed: &[AssetSourceSlot] = if asset.category.is_image() {
        &[AssetSourceSlot::Static]
    } else {
        asset.model_kind_or_static().slots()
    };
    let slot = match request.slot {
        Some(slot) => slot,
        None if allowed.len() == 1 => allowed[0],
        None => {
            return Err(ApiError::bad_request(
                "slot is required for animated models: idle or animated",
            ));
        }
    };
    if !allowed.contains(&slot) {
        return Err(ApiError::bad_request(format!(
            "slot {slot} is not valid for this asset"
        )));
    }
    Ok(slot)
}

fn requested_kind(asset: &Asset, request: &ConvertRequest) -> Result<AssetVariantKind, ApiError> {
    match request.target {
        ConvertTarget::Resize => {
            if !asset.category.is_image() {
                return Err(ApiError::bad_request(
                    "resize only applies to texture and icon assets",
                ));
            }
            let size = request.size.ok_or_else(|| {
                ApiError::bad_request("size is required for resize (512, 256, 128)")
            })?;
            AssetVariantKind::from_resize_side(size)
                .ok_or_else(|| ApiError::bad_request("size must be 512, 256, or 128"))
        }
        ConvertTarget::Draco => {
            require_model(asset)?;
            Ok(AssetVariantKind::Draco)
        }
        ConvertTarget::Lod2 => {
            require_model(asset)?;
            Ok(AssetVariantKind::Lod2)
        }
        ConvertTarget::DracoLod2 => {
            require_model(asset)?;
            Ok(AssetVariantKind::DracoLod2)
        }
    }
}

fn require_model(asset: &Asset) -> Result<(), ApiError> {
    if asset.category.is_model() {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "draco/lod2 conversion only applies to model assets",
        ))
    }
}

/// True when the uploaded source already meets the requested variant.
fn source_satisfies(source: &twg_contracts::assets::AssetSource, kind: AssetVariantKind) -> bool {
    if kind.is_draco() && !source.has_draco {
        return false;
    }
    if kind.is_lod2() && source.lod != Some(2) {
        return false;
    }
    kind.is_draco() || kind.is_lod2()
}

fn describe(source: &twg_contracts::assets::AssetSource) -> String {
    match (source.has_draco, source.lod) {
        (true, Some(level)) => format!("draco + lod{level}"),
        (true, None) => "draco".to_owned(),
        (false, Some(level)) => format!("lod{level}"),
        (false, None) => "original".to_owned(),
    }
}
