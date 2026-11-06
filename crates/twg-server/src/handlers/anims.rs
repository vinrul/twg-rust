use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use twg_contracts::MessageResponse;
use twg_contracts::SingleResponse;
use twg_contracts::animations::{
    Animation, AnimationCombineRequest, AnimationCombineV2Request, AnimationCreateRequest,
    AnimationDataUpdateRequest, AnimationDataV2UpdateRequest, AnimationDeleteCombineDataRequest,
    AnimationGeomUpdateRequest, AnimationOrderRequest, AnimationRemoveRequest,
    RouteAnimationV2Request, ShootAnimationRequest, ShootAnimationV2Request, SpawnAnimationRequest,
    SpawnPickupRequest,
};
use twg_contracts::map::ParticleCreateRequest;
use twg_storage::{AnimOrder, NewAnimation};
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::{compact_json, parse_uuid, write_store};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct RemoveParticleRequest {
    pub operasi_id: String,
    pub skenario_id: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct SpawnDataUpdateRequest {
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    #[serde(default)]
    pub data: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ParticleDataUpdateRequest {
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub keterangan: String,
    pub tipe: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct RouteGeomV2Request {
    pub geom: Value,
}

#[derive(Debug, Deserialize)]
pub struct AnimOrderItem {
    pub id: Uuid,
    pub urut: i32,
}

fn success() -> Json<MessageResponse> {
    Json(MessageResponse {
        code: 200,
        message: "success".to_owned(),
    })
}

pub async fn create_route(
    State(state): State<AppState>,
    Json(request): Json<AnimationCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let anim = write_store(&state)?
        .create_anim(&NewAnimation {
            operasi_id,
            skenario_id,
            name: request.name,
            username: request.username,
            keterangan: request.keterangan,
            tipe: request.tipe,
            data: request.data,
            geom: request.geom,
            speed: request.speed,
            detik: request.detik,
            replay: Some(request.replay),
            autoplay: Some(request.autoplay),
        })
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_route_v2(
    State(state): State<AppState>,
    Json(request): Json<RouteAnimationV2Request>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let data = compact_json(&request.data)?;
    let geom = compact_json(&request.geom)?;
    let anim = write_store(&state)?
        .create_anim(&NewAnimation {
            operasi_id,
            skenario_id,
            name: request.name,
            username: request.username,
            keterangan: request.keterangan,
            tipe: request.tipe,
            data,
            geom,
            speed: request.speed,
            detik: request.detik,
            replay: Some(request.replay),
            autoplay: Some(request.autoplay),
        })
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_spawn(
    State(state): State<AppState>,
    Json(request): Json<SpawnAnimationRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let unit_id = parse_uuid(&request.unit_id, "unit_id")?;
    let anim = write_store(&state)?
        .create_spawn_anim(
            operasi_id,
            skenario_id,
            unit_id,
            &request.marker_name,
            &request.kategori,
            request.rot_x,
            request.rot_y,
            request.rot_z,
            request.pos_x,
            request.pos_y,
            request.scale,
            request.altitude,
            &request.name,
            &request.username,
            &request.keterangan,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_spawn_pickup(
    State(state): State<AppState>,
    Json(request): Json<SpawnPickupRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let mut marker_ids = request.marker_ids.clone();
    if marker_ids.is_empty() && !request.marker_id.is_empty() {
        marker_ids.push(request.marker_id.clone());
    }
    if marker_ids.is_empty() {
        return Err(ApiError::bad_request("marker ids must not be empty"));
    }
    let parsed = marker_ids
        .iter()
        .map(|value| parse_uuid(value, "marker_id"))
        .collect::<Result<Vec<_>, _>>()?;
    let anim = write_store(&state)?
        .create_spawn_pickup_anim(
            operasi_id,
            skenario_id,
            &parsed,
            &request.name,
            &request.username,
            &request.keterangan,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_remove(
    State(state): State<AppState>,
    Json(request): Json<AnimationRemoveRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let anim = write_store(&state)?
        .create_anim(&NewAnimation {
            operasi_id,
            skenario_id,
            name: request.name,
            username: request.username,
            keterangan: request.keterangan,
            tipe: "remove".to_owned(),
            data: request.data,
            geom: "[]".to_owned(),
            speed: 0,
            detik: 0,
            replay: None,
            autoplay: None,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_combine(
    State(state): State<AppState>,
    Json(request): Json<AnimationCombineRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let anim = write_store(&state)?
        .create_combine_anim(
            operasi_id,
            skenario_id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.data,
            &request.tipe,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_combine_v2(
    State(state): State<AppState>,
    Json(request): Json<AnimationCombineV2Request>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    if request.data.is_empty() {
        return Err(ApiError::bad_request("data must not be empty"));
    }
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let data = compact_json(&request.data)?;
    let anim = write_store(&state)?
        .create_combine_anim(
            operasi_id,
            skenario_id,
            &request.name,
            &request.username,
            &request.keterangan,
            &data,
            &request.tipe,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_shoot(
    State(state): State<AppState>,
    Json(request): Json<ShootAnimationRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let marker_id = parse_uuid(&request.marker_id, "marker_id")?;
    let anim = write_store(&state)?
        .create_shoot_anim(
            operasi_id,
            skenario_id,
            marker_id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.data,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_shoot_v2(
    State(state): State<AppState>,
    Json(request): Json<ShootAnimationV2Request>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let marker_id = parse_uuid(&request.data.marker_id, "marker_id")?;
    let data = compact_json(&request.data)?;
    let anim = write_store(&state)?
        .create_shoot_anim(
            operasi_id,
            skenario_id,
            marker_id,
            &request.name,
            &request.username,
            &request.keterangan,
            &data,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_spawn_particle(
    State(state): State<AppState>,
    Json(request): Json<ParticleCreateRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let is_spawn = if request.is_spawn == "false" { 0 } else { 1 };
    let anim = write_store(&state)?
        .create_particle_anim(
            operasi_id,
            skenario_id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.tipe,
            &request.data,
            request.pos_x,
            request.pos_y,
            is_spawn,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn create_remove_particle(
    State(state): State<AppState>,
    Json(request): Json<RemoveParticleRequest>,
) -> Result<(StatusCode, Json<SingleResponse<Animation>>), ApiError> {
    let operasi_id = parse_uuid(&request.operasi_id, "operasi_id")?;
    let skenario_id = parse_uuid(&request.skenario_id, "skenario_id")?;
    let anim = write_store(&state)?
        .create_anim(&NewAnimation {
            operasi_id,
            skenario_id,
            name: request.name,
            username: request.username,
            keterangan: request.keterangan,
            tipe: "remove_particle".to_owned(),
            data: request.data,
            geom: "[]".to_owned(),
            speed: 0,
            detik: 0,
            replay: None,
            autoplay: None,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(SingleResponse::success(anim))))
}

pub async fn update_route_data(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AnimationDataUpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let anim = write_store(&state)?
        .update_anim_route(
            id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.data,
            request.speed,
            request.detik,
            Some(request.replay),
            Some(request.autoplay),
        )
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_route_data_v2(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AnimationDataV2UpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let data = compact_json(&request.data)?;
    let anim = write_store(&state)?
        .update_anim_route(
            id,
            &request.name,
            &request.username,
            &request.keterangan,
            &data,
            request.speed,
            request.detik,
            Some(request.replay),
            Some(request.autoplay),
        )
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_spawn_data(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<SpawnDataUpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let anim = match request.data {
        Some(data) => {
            let data = compact_json(&data)?;
            write_store(&state)?
                .update_spawn_data(
                    id,
                    &request.name,
                    &request.username,
                    &request.keterangan,
                    &data,
                )
                .await?
        }
        None => {
            write_store(&state)?
                .update_anim_spawn_name(id, &request.name, &request.username, &request.keterangan)
                .await?
        }
    };
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_particle_data(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ParticleDataUpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let anim = write_store(&state)?
        .update_particle_data(
            id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.tipe,
            &request.data,
        )
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_remove_particle_data(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AnimationDataUpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let anim = write_store(&state)?
        .update_anim_route(
            id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.data,
            0,
            0,
            None,
            None,
        )
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_remove_data(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AnimationDataUpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let anim = write_store(&state)?
        .update_anim_route(
            id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.data,
            0,
            0,
            None,
            None,
        )
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_combine_data(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AnimationDataUpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let anim = write_store(&state)?
        .update_combine_data(
            id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.data,
        )
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_shoot_data(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AnimationDataUpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let anim = write_store(&state)?
        .update_shoot_data(
            id,
            &request.name,
            &request.username,
            &request.keterangan,
            &request.data,
        )
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_route_geom(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AnimationGeomUpdateRequest>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let anim = write_store(&state)?
        .update_anim_route_geom(id, &request.geom)
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn update_route_geom_v2(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<RouteGeomV2Request>,
) -> Result<Json<SingleResponse<Animation>>, ApiError> {
    let geom = compact_json(&request.geom)?;
    let anim = write_store(&state)?
        .update_anim_route_geom(id, &geom)
        .await?;
    Ok(Json(SingleResponse::success(anim)))
}

pub async fn delete_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_anim(id).await?;
    Ok(success())
}

pub async fn delete_spawn(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_spawn_anim(id).await?;
    Ok(success())
}

pub async fn delete_combine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_combine_anim(id).await?;
    Ok(success())
}

pub async fn delete_shoot(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_shoot_anim(id).await?;
    Ok(success())
}

pub async fn delete_particle(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?.delete_particle_anim(id).await?;
    Ok(success())
}

pub async fn update_combine_type(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<AnimationDeleteCombineDataRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    write_store(&state)?
        .update_anim_tipe(id, &request.tipe)
        .await?;
    Ok(success())
}

pub async fn update_order(
    State(state): State<AppState>,
    Json(request): Json<AnimationOrderRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let items: Vec<AnimOrderItem> = serde_json::from_str(&request.urut)
        .map_err(|_| ApiError::bad_request("invalid urut data"))?;
    let updates = items
        .into_iter()
        .map(|item| AnimOrder {
            id: item.id,
            urut: item.urut,
        })
        .collect::<Vec<_>>();
    write_store(&state)?.update_anim_order(&updates).await?;
    Ok(success())
}

pub async fn update_order_v2(
    State(state): State<AppState>,
    Json(items): Json<Vec<AnimOrderItem>>,
) -> Result<Json<MessageResponse>, ApiError> {
    if items.is_empty() {
        return Err(ApiError::bad_request("body must not be empty"));
    }
    let updates = items
        .into_iter()
        .map(|item| AnimOrder {
            id: item.id,
            urut: item.urut,
        })
        .collect::<Vec<_>>();
    write_store(&state)?.update_anim_order(&updates).await?;
    Ok(success())
}
