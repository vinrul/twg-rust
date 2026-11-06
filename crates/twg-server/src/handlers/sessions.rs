use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use serde_json::json;
use twg_auth::AuthContext;
use twg_contracts::MessageResponse;
use twg_contracts::SingleResponse;
use twg_contracts::snapshots::{Snapshot, SnapshotToggleRequest, SnapshotUpsertRequest};
use twg_contracts::websocket::PublishCommandRequest;
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::{parse_uuid, read_store, write_store};
use crate::state::AppState;

pub async fn upsert_snapshot(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(skenario_id): Path<Uuid>,
    Json(request): Json<SnapshotUpsertRequest>,
) -> Result<Json<SingleResponse<Snapshot>>, ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let body_skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let _ = skenario_id;
    let snapshot = write_store(&state)?
        .upsert_snapshot(
            operasi_id,
            body_skenario_id,
            request.snapshot_enabled,
            request.map_state,
            request.markers_state,
            request.anims_state,
            context.subject.user_id,
        )
        .await?;
    Ok(Json(SingleResponse::success(snapshot)))
}

pub async fn toggle_snapshot(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
    Json(request): Json<SnapshotToggleRequest>,
) -> Result<Json<SingleResponse<Snapshot>>, ApiError> {
    let snapshot = write_store(&state)?
        .toggle_snapshot(skenario_id, request.snapshot_enabled)
        .await?;
    Ok(Json(SingleResponse::success(snapshot)))
}

pub async fn delete_snapshot(
    State(state): State<AppState>,
    Path(skenario_id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_snapshot(skenario_id).await?;
    Ok(Json(MessageResponse {
        code: 200,
        message: "success".to_owned(),
    }))
}

/// Saves command state (`MOVE`/`ROT`/`ZOOM`) and acknowledges the publish.
///
/// The real-time fan-out is implemented in Phase 7. Until then the endpoint
/// persists state and returns the Centrifugo-shaped acknowledgement so legacy
/// clients keep working.
pub async fn publish_commands(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Json(mut request): Json<PublishCommandRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let command = request.command.trim().to_owned();
    request.command = command.clone();

    if !command.eq_ignore_ascii_case("reload") {
        let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
        let scenario = read_store(&state)?
            .get_scenario(skenario_id)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::not_found("skenario not found"))?;
        if context.subject.role.is_satker_role()
            && let Some(resource_satker) = scenario.satker_id
            && context.subject.satker_id != Some(resource_satker)
        {
            return Err(ApiError::forbidden("cross-satker command not allowed"));
        }

        let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
        let marker_id = parse_uuid(&request.marker_id, "marker_id")?;
        write_store(&state)?
            .save_command(
                request.is_save,
                operasi_id,
                skenario_id,
                marker_id,
                &command,
                &request.data,
            )
            .await?;
    } else {
        request.is_save = 0;
    }

    Ok((StatusCode::CREATED, Json(json!({"result": {}}))))
}
