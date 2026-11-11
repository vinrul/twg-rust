use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use twg_contracts::MessageResponse;
use twg_contracts::SingleResponse;
use twg_contracts::map::{
    Marker, MarkerAltitudeRequest, MarkerCreateRequest, MarkerDuplicateRequest, MarkerGeomRequest,
    MarkerNameRequest, MarkerObstacleRequest, MarkerRotateRequest, MarkerScaleRequest,
    MarkerUnitRequest,
};
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::{parse_uuid, write_store};
use crate::state::AppState;

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<MarkerCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Marker>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let unit_id = parse_uuid(&request.unit_id, "unit_id")?;
    let marker = write_store(&state)?
        .create_marker(
            operasi_id,
            skenario_id,
            unit_id,
            &request.name,
            &request.kategori,
            request.rot_x,
            request.rot_y,
            request.rot_z,
            request.pos_x,
            request.pos_y,
            request.jumlah,
            request.obstacle,
            &request.keterangan,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(marker))))
}

pub async fn update_geom(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MarkerGeomRequest>,
) -> Result<Json<SingleResponse<Marker>>, ApiError> {
    let marker = write_store(&state)?
        .update_marker_geom(
            id,
            request.rot_x,
            request.rot_y,
            request.rot_z,
            request.pos_x,
            request.pos_y,
        )
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn update_name(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MarkerNameRequest>,
) -> Result<Json<SingleResponse<Marker>>, ApiError> {
    let marker = write_store(&state)?
        .update_marker_name(id, &request.name, request.jumlah, &request.keterangan)
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn update_unit(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MarkerUnitRequest>,
) -> Result<Json<SingleResponse<Marker>>, ApiError> {
    let unit_id = parse_uuid(&request.unit_id, "unit_id")?;
    let marker = write_store(&state)?.update_marker_unit(id, unit_id).await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn update_scale(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MarkerScaleRequest>,
) -> Result<Json<SingleResponse<Marker>>, ApiError> {
    let marker = write_store(&state)?
        .update_marker_scale(id, request.scale, request.scale_y, request.scale_z)
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn update_altitude(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MarkerAltitudeRequest>,
) -> Result<Json<SingleResponse<Marker>>, ApiError> {
    let marker = write_store(&state)?
        .update_marker_altitude(id, request.altitude)
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn update_obstacle(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MarkerObstacleRequest>,
) -> Result<Json<SingleResponse<Marker>>, ApiError> {
    let marker = write_store(&state)?
        .update_marker_obstacle(id, request.obstacle)
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn update_rotation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MarkerRotateRequest>,
) -> Result<Json<SingleResponse<Marker>>, ApiError> {
    let marker = write_store(&state)?
        .update_marker_rotation(id, request.rotasi, request.rotasi_x, request.rotasi_y)
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn duplicate(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<MarkerDuplicateRequest>,
) -> Result<Json<SingleResponse<Marker>>, ApiError> {
    let marker = write_store(&state)?
        .duplicate_marker(id, request.pos_x, request.pos_y)
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_marker(id).await?;
    Ok(Json(MessageResponse {
        code: 200,
        message: "success".to_owned(),
    }))
}
