use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use serde_json::Value;
use twg_auth::{AuthContext, hash_password};
use twg_contracts::MessageResponse;
use twg_contracts::SingleResponse;
use twg_contracts::categories::{Category, CategoryRequest};
use twg_contracts::operations::{Operation, OperationRequest};
use twg_contracts::satkers::{Satker, SatkerCreateRequest, SatkerUpdateRequest};
use twg_contracts::scenarios::{
    CopyScenarioRequest, Scenario, ScenarioCreateRequest, ScenarioNameUpdateRequest,
    ScenarioUpdateRequest,
};
use twg_contracts::users::{UpdateProfileRequest, User, UserCreateRequest, UserUpdateRequest};
use twg_domain::{BaseLayer, MapMode, Role};
use twg_storage::{NewScenario, ScenarioUpdate};
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::{parse_uuid, read_store, write_store};
use crate::state::AppState;

fn success() -> Json<MessageResponse> {
    Json(MessageResponse {
        code: 200,
        message: "success".to_owned(),
    })
}

// ---------------------------------------------------------------- categories

pub async fn create_category(
    State(state): State<AppState>,
    Json(request): Json<CategoryRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Category>>), ApiError> {
    let category = write_store(&state)?
        .create_category(&request.name, &request.icon)
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(category))))
}

pub async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<CategoryRequest>,
) -> Result<Json<SingleResponse<Category>>, ApiError> {
    let category = write_store(&state)?
        .update_category(id, &request.name, &request.icon)
        .await?;
    Ok(Json(SingleResponse::success(category)))
}

pub async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_category(id).await?;
    Ok(success())
}

// ---------------------------------------------------------------- operations

pub async fn create_operation(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Json(request): Json<OperationRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Operation>>), ApiError> {
    let satker_id = if context.subject.role.is_satker_role() {
        context.subject.satker_id
    } else {
        None
    };
    let operation = write_store(&state)?
        .create_operation(&request.name, satker_id)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(SingleResponse::success(operation)),
    ))
}

pub async fn update_operation(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(request): Json<OperationRequest>,
) -> Result<Json<SingleResponse<Operation>>, ApiError> {
    let satker_id = match request
        .satker_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            if !matches!(context.subject.role, Role::Admin | Role::AdminMonitor) {
                return Err(ApiError::forbidden("cannot assign satker to operasi"));
            }
            Some(parse_uuid(value, "satker_id")?)
        }
        None => None,
    };
    let operation = write_store(&state)?
        .update_operation(id, &request.name, satker_id)
        .await?;
    Ok(Json(SingleResponse::success(operation)))
}

pub async fn delete_operation(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    if context.subject.role == Role::AdminSatker {
        let operation = read_store(&state)?
            .get_operation(id)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::not_found("operasi not found"))?;
        if operation.satker_id != context.subject.satker_id || context.subject.satker_id.is_none() {
            return Err(ApiError::forbidden(
                "cannot delete operasi from different satker",
            ));
        }
    }
    write_store(&state)?.delete_operation(id).await?;
    Ok(success())
}

// ---------------------------------------------------------------- scenarios

fn map_mode_str(mode: MapMode) -> &'static str {
    match mode {
        MapMode::Outdoor => "outdoor",
        MapMode::Indoor => "indoor",
    }
}

fn base_layer_str(layer: BaseLayer) -> &'static str {
    match layer {
        BaseLayer::Osm => "osm",
        BaseLayer::Satellite => "satellite",
        BaseLayer::GridDark => "grid_dark",
        BaseLayer::GridLight => "grid_light",
        BaseLayer::BlankDark => "blank_dark",
        BaseLayer::BlankLight => "blank_light",
        BaseLayer::FloorPlan => "floor_plan",
    }
}

fn normalize_map_mode(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("indoor") {
        "indoor".to_owned()
    } else {
        "outdoor".to_owned()
    }
}

fn normalize_min_zoom(value: f64, map_mode: &str) -> f64 {
    if value > 0.0 {
        return value;
    }
    if map_mode == "indoor" { 21.0 } else { 2.0 }
}

fn normalize_base_layer(value: &str, map_mode: &str) -> String {
    let normalized = value.trim().to_lowercase();
    if map_mode == "indoor" {
        return match normalized.as_str() {
            "blank_light" | "blank_dark" | "grid_light" | "grid_dark" | "floor_plan" => normalized,
            _ => "grid_dark".to_owned(),
        };
    }
    if normalized == "satellite" {
        "satellite".to_owned()
    } else {
        "osm".to_owned()
    }
}

fn normalize_indoor_config(value: Option<&Value>, fallback: Value) -> String {
    match value {
        Some(value) => serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned()),
        None => serde_json::to_string(&fallback).unwrap_or_else(|_| "{}".to_owned()),
    }
}

pub async fn create_scenario(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Json(request): Json<ScenarioCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Scenario>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let map_mode = normalize_map_mode(&request.map_mode);
    let min_zoom = normalize_min_zoom(request.min_zoom, &map_mode);
    let base_layer = normalize_base_layer(&request.base_layer, &map_mode);
    let indoor_config = normalize_indoor_config(
        request.indoor_config.as_ref(),
        Value::Object(serde_json::Map::new()),
    );
    let satker_id = if context.subject.role.is_satker_role() {
        context.subject.satker_id
    } else {
        None
    };
    let scenario = write_store(&state)?
        .create_scenario(&NewScenario {
            operasi_id,
            satker_id,
            name: request.name,
            center_x: request.center_x,
            center_y: request.center_y,
            zoom: request.zoom,
            min_zoom,
            max_zoom: request.max_zoom,
            pitch: request.pitch,
            bearing: request.bearing,
            map_mode,
            base_layer,
            indoor_config,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(scenario))))
}

pub async fn update_scenario(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ScenarioUpdateRequest>,
) -> Result<Json<SingleResponse<Scenario>>, ApiError> {
    let current = read_store(&state)?
        .get_scenario(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("skenario not found"))?;
    let map_mode = if request.map_mode.is_empty() {
        map_mode_str(current.map_mode).to_owned()
    } else {
        normalize_map_mode(&request.map_mode)
    };
    let min_zoom = if request.min_zoom > 0.0 {
        request.min_zoom
    } else {
        current.min_zoom
    };
    let max_zoom = if request.max_zoom > 0.0 {
        request.max_zoom
    } else {
        current.max_zoom
    };
    let base_layer = if request.base_layer.is_empty() {
        base_layer_str(current.base_layer).to_owned()
    } else {
        normalize_base_layer(&request.base_layer, &map_mode)
    };
    let indoor_config = normalize_indoor_config(
        request.indoor_config.as_ref(),
        current.indoor_config.clone(),
    );
    let scenario = write_store(&state)?
        .update_scenario(
            id,
            &ScenarioUpdate {
                center_x: request.center_x,
                center_y: request.center_y,
                zoom: request.zoom,
                min_zoom,
                max_zoom,
                pitch: request.pitch,
                bearing: request.bearing,
                map_mode,
                base_layer,
                indoor_config,
            },
        )
        .await?;
    Ok(Json(SingleResponse::success(scenario)))
}

pub async fn update_scenario_name(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ScenarioNameUpdateRequest>,
) -> Result<Json<SingleResponse<Scenario>>, ApiError> {
    let scenario = write_store(&state)?
        .update_scenario_name(id, &request.name)
        .await?;
    Ok(Json(SingleResponse::success(scenario)))
}

pub async fn copy_scenario(
    State(state): State<AppState>,
    Json(request): Json<CopyScenarioRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let source_id = parse_uuid(&request.src_skenario_id, "src_skenario_id")?;
    let target_operasi_id = parse_uuid(&request.tgr_operasi_id, "tgr_operasi_id")?;
    write_store(&state)?
        .copy_scenario(&request.name, source_id, target_operasi_id)
        .await?;
    Ok(success())
}

pub async fn delete_scenario(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    if context.subject.role == Role::AdminSatker {
        let scenario = read_store(&state)?
            .get_scenario(id)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::not_found("skenario not found"))?;
        if scenario.satker_id != context.subject.satker_id || context.subject.satker_id.is_none() {
            return Err(ApiError::forbidden(
                "cannot delete skenario from different satker",
            ));
        }
    }
    write_store(&state)?.delete_scenario(id).await?;
    Ok(success())
}

// ---------------------------------------------------------------- users

pub async fn create_user(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Json(request): Json<UserCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<User>>), ApiError> {
    let password_hash = hash_password(&request.password)?;
    let (role, satker_id) = if context.subject.role == Role::AdminSatker {
        ("user_satker".to_owned(), context.subject.satker_id)
    } else {
        let satker_id = request
            .satker_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| parse_uuid(value, "satker_id"))
            .transpose()?;
        (request.role, satker_id)
    };
    let user = write_store(&state)?
        .create_user(
            &request.name,
            &request.username,
            &password_hash,
            &role,
            &request.profile_img,
            &request.units,
            satker_id,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::created(user))))
}

pub async fn update_user(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(request): Json<UserUpdateRequest>,
) -> Result<Json<SingleResponse<User>>, ApiError> {
    let satker_id = match request
        .satker_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            let satker_id = parse_uuid(value, "satker_id")?;
            if context.subject.role == Role::AdminSatker
                && context.subject.satker_id != Some(satker_id)
            {
                return Err(ApiError::forbidden(
                    "cannot assign user to different satker",
                ));
            }
            Some(satker_id)
        }
        None => None,
    };
    let user = write_store(&state)?
        .update_user(
            id,
            &request.name,
            &request.units,
            &request.profile_img,
            satker_id,
        )
        .await?;
    Ok(Json(SingleResponse::success(user)))
}

pub async fn delete_user(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    if context.subject.role == Role::AdminSatker {
        let user = read_store(&state)?
            .get_user(id)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::not_found("user not found"))?;
        if user.satker_id != context.subject.satker_id || context.subject.satker_id.is_none() {
            return Err(ApiError::forbidden(
                "cannot delete user from different satker",
            ));
        }
    }
    write_store(&state)?.delete_user(id).await?;
    Ok(success())
}

pub async fn update_profile(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<SingleResponse<User>>, ApiError> {
    let user = write_store(&state)?
        .update_profile(context.subject.user_id, &request.name, &request.profile_img)
        .await?;
    Ok(Json(SingleResponse::success(user)))
}

pub async fn kick_user_sessions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.revoke_user_sessions(id).await?;
    Ok(success())
}

// ---------------------------------------------------------------- satkers

pub async fn create_satker(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Json(request): Json<SatkerCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Satker>>), ApiError> {
    if context.subject.role != Role::Admin {
        return Err(ApiError::forbidden("only admin can create satker"));
    }
    let satker = write_store(&state)?
        .create_satker(
            &request.code,
            &request.name,
            request.is_active,
            context.subject.user_id,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(satker))))
}

pub async fn update_satker(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(request): Json<SatkerUpdateRequest>,
) -> Result<Json<SingleResponse<Satker>>, ApiError> {
    if context.subject.role != Role::Admin {
        return Err(ApiError::forbidden("only admin can update satker"));
    }
    let satker = write_store(&state)?
        .update_satker(id, &request.code, &request.name, request.is_active)
        .await?;
    Ok(Json(SingleResponse::success(satker)))
}

pub async fn delete_satker(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    if context.subject.role != Role::Admin {
        return Err(ApiError::forbidden("only admin can delete satker"));
    }
    write_store(&state)?.delete_satker(id).await?;
    Ok(success())
}
