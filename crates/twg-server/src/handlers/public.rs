use axum::Json;
use axum::extract::{Path, State};
use twg_contracts::ListResponse;
use twg_contracts::SingleResponse;
use twg_contracts::alurs::Alur;
use twg_contracts::animations::Animation;
use twg_contracts::last_positions::LastPositionMap;
use twg_contracts::map::{Building, Line, Marker, Particle, TextMarker};
use twg_contracts::operations::HomeOperation;
use twg_contracts::scenarios::Scenario;
use twg_contracts::snapshots::Snapshot;
use twg_contracts::users::OnlineUser;
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::read_store;
use crate::state::AppState;

pub async fn scenarios(
    State(state): State<AppState>,
    Path(operasi_id): Path<Uuid>,
) -> Result<Json<ListResponse<Scenario>>, ApiError> {
    let data = read_store(&state)?
        .list_scenarios(operasi_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

pub async fn scenario_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SingleResponse<Scenario>>, ApiError> {
    let scenario = read_store(&state)?
        .get_scenario(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("skenario not found"))?;
    Ok(Json(SingleResponse::success(scenario)))
}

pub async fn markers(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<ListResponse<Marker>>, ApiError> {
    let data = read_store(&state)?
        .list_markers(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

pub async fn text_markers(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<ListResponse<TextMarker>>, ApiError> {
    let data = read_store(&state)?
        .list_text_markers(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

pub async fn particles(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<ListResponse<Particle>>, ApiError> {
    let data = read_store(&state)?
        .list_particles(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

pub async fn buildings(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<ListResponse<Building>>, ApiError> {
    let data = read_store(&state)?
        .list_buildings(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

pub async fn lines(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<ListResponse<Line>>, ApiError> {
    let data = read_store(&state)?
        .list_lines(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

pub async fn alurs(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<ListResponse<Alur>>, ApiError> {
    let data = read_store(&state)?
        .list_alurs(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

pub async fn anims(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<ListResponse<Animation>>, ApiError> {
    let data = read_store(&state)?
        .list_anims(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

pub async fn last_positions(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<SingleResponse<LastPositionMap>>, ApiError> {
    let data = read_store(&state)?
        .last_positions(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SingleResponse::success(data)))
}

pub async fn home_operations(
    State(state): State<AppState>,
) -> Result<Json<ListResponse<HomeOperation>>, ApiError> {
    let data = read_store(&state)?
        .home_operations(twg_storage::SatkerScope::All)
        .await
        .map_err(ApiError::internal)?;
    let total = data.len();
    Ok(Json(ListResponse::new(data, total)))
}

pub async fn home_operation_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SingleResponse<HomeOperation>>, ApiError> {
    let data = read_store(&state)?
        .get_home_operation(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("operasi not found"))?;
    Ok(Json(SingleResponse::success(data)))
}

pub async fn snapshot(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<SingleResponse<Option<Snapshot>>>, ApiError> {
    let data = read_store(&state)?
        .snapshot(skenario_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SingleResponse::success(data)))
}

pub async fn online_users(
    State(state): State<AppState>,
) -> Result<Json<ListResponse<OnlineUser>>, ApiError> {
    let data = read_store(&state)?
        .list_online_users()
        .await
        .map_err(ApiError::internal)?;
    let total = data.len();
    Ok(Json(ListResponse::new(data, total)))
}
