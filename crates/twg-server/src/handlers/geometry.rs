use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use twg_contracts::MessageResponse;
use twg_contracts::SingleResponse;
use twg_contracts::alurs::{Alur, AlurCreateRequest, AlurUpdateRequest};
use twg_contracts::map::{
    Building, BuildingCreateRequest, BuildingGeomUpdateRequest, BuildingUpdateRequest, Line,
    LineCreateRequest, LineGeomUpdateRequest, LineUpdateRequest, Particle, ParticleCreateRequest,
    ParticleGeomUpdateRequest, ParticleKind, ParticleUpdateRequest, TextMarker,
    TextMarkerCreateRequest, TextMarkerGeomUpdateRequest, TextMarkerUpdateRequest,
};
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::{parse_uuid, write_store};
use crate::state::AppState;

fn success() -> Json<MessageResponse> {
    Json(MessageResponse {
        code: 200,
        message: "success".to_owned(),
    })
}

// ---------------------------------------------------------------- buildings

pub async fn create_building(
    State(state): State<AppState>,
    Json(request): Json<BuildingCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Building>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let building = write_store(&state)?
        .create_building(
            operasi_id,
            skenario_id,
            &request.name,
            &request.color,
            &request.geom,
            request.height,
            &request.keterangan,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(building))))
}

pub async fn update_building(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<BuildingUpdateRequest>,
) -> Result<Json<SingleResponse<Building>>, ApiError> {
    let building = write_store(&state)?
        .update_building(
            id,
            &request.name,
            &request.color,
            request.height,
            &request.keterangan,
        )
        .await?;
    Ok(Json(SingleResponse::success(building)))
}

pub async fn update_building_geom(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<BuildingGeomUpdateRequest>,
) -> Result<Json<SingleResponse<Building>>, ApiError> {
    let building = write_store(&state)?
        .update_building_geom(id, &request.geom)
        .await?;
    Ok(Json(SingleResponse::success(building)))
}

pub async fn delete_building(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_building(id).await?;
    Ok(success())
}

// ---------------------------------------------------------------- lines

pub async fn create_line(
    State(state): State<AppState>,
    Json(request): Json<LineCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Line>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let line = write_store(&state)?
        .create_line(
            operasi_id,
            skenario_id,
            &request.name,
            &request.color,
            request.width,
            request.alpha,
            &request.geom,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(line))))
}

pub async fn update_line(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<LineUpdateRequest>,
) -> Result<Json<SingleResponse<Line>>, ApiError> {
    let line = write_store(&state)?
        .update_line(
            id,
            &request.name,
            &request.color,
            request.width,
            request.alpha,
        )
        .await?;
    Ok(Json(SingleResponse::success(line)))
}

pub async fn update_line_geom(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<LineGeomUpdateRequest>,
) -> Result<Json<SingleResponse<Line>>, ApiError> {
    let line = write_store(&state)?
        .update_line_geom(id, &request.geom)
        .await?;
    Ok(Json(SingleResponse::success(line)))
}

pub async fn delete_line(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_line(id).await?;
    Ok(success())
}

// ---------------------------------------------------------------- text markers

pub async fn create_text_marker(
    State(state): State<AppState>,
    Json(request): Json<TextMarkerCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<TextMarker>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let marker = write_store(&state)?
        .create_text_marker(
            operasi_id,
            skenario_id,
            &request.text,
            &request.color,
            request.font_size,
            request.align,
            request.wrap_text,
            request.pos_x,
            request.pos_y,
            request.altitude,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(marker))))
}

pub async fn update_text_marker(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<TextMarkerUpdateRequest>,
) -> Result<Json<SingleResponse<TextMarker>>, ApiError> {
    let marker = write_store(&state)?
        .update_text_marker(
            id,
            &request.text,
            &request.color,
            request.font_size,
            request.align,
            request.wrap_text,
        )
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn update_text_marker_geom(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<TextMarkerGeomUpdateRequest>,
) -> Result<Json<SingleResponse<TextMarker>>, ApiError> {
    let marker = write_store(&state)?
        .update_text_marker_geom(id, request.pos_x, request.pos_y, request.altitude)
        .await?;
    Ok(Json(SingleResponse::success(marker)))
}

pub async fn delete_text_marker(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_text_marker(id).await?;
    Ok(success())
}

// ---------------------------------------------------------------- particles

pub async fn create_particle(
    State(state): State<AppState>,
    Json(request): Json<ParticleCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Particle>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let tipe = ParticleKind::from_key(&request.tipe).key();
    let particle = write_store(&state)?
        .create_particle(
            operasi_id,
            skenario_id,
            &request.name,
            tipe,
            &request.data,
            request.pos_x,
            request.pos_y,
            request.altitude,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(particle))))
}

pub async fn update_particle(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ParticleUpdateRequest>,
) -> Result<Json<SingleResponse<Particle>>, ApiError> {
    let tipe = ParticleKind::from_key(&request.tipe).key();
    let particle = write_store(&state)?
        .update_particle(id, &request.name, tipe, &request.data)
        .await?;
    Ok(Json(SingleResponse::success(particle)))
}

pub async fn update_particle_geom(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ParticleGeomUpdateRequest>,
) -> Result<Json<SingleResponse<Particle>>, ApiError> {
    let particle = write_store(&state)?
        .update_particle_geom(id, request.pos_x, request.pos_y, request.altitude)
        .await?;
    Ok(Json(SingleResponse::success(particle)))
}

pub async fn delete_particle(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_particle(id).await?;
    Ok(success())
}

// ---------------------------------------------------------------- alurs

pub async fn create_alur(
    State(state): State<AppState>,
    Json(request): Json<AlurCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Alur>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let alur = write_store(&state)?
        .create_alur(operasi_id, skenario_id, &request.alur)
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(alur))))
}

pub async fn update_alur(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AlurUpdateRequest>,
) -> Result<Json<SingleResponse<Alur>>, ApiError> {
    let alur = write_store(&state)?.update_alur(id, &request.alur).await?;
    Ok(Json(SingleResponse::success(alur)))
}

pub async fn delete_alur(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_alur(id).await?;
    Ok(success())
}
