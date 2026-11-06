use std::io::Read;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use twg_contracts::ListResponse;
use twg_contracts::SingleResponse;
use twg_contracts::alurs::Alur;
use twg_contracts::animations::Animation;
use twg_contracts::geocode::GeocodeResult;
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

#[derive(Debug, Deserialize)]
pub struct GeocodeQuery {
    pub q: String,
}

/// Proxies OpenStreetMap Nominatim so the web client is not blocked by CORS
/// and the usage policy can be enforced in one place.
pub async fn geocode(
    Query(query): Query<GeocodeQuery>,
) -> Result<Json<ListResponse<GeocodeResult>>, ApiError> {
    let search = query.q.trim().to_owned();
    if search.len() < 2 {
        return Ok(Json(ListResponse::new(Vec::new(), 0)));
    }
    let results = tokio::task::spawn_blocking(move || fetch_geocode(&search))
        .await
        .unwrap_or_default();
    Ok(Json(ListResponse::new(results, 0)))
}

fn fetch_geocode(query: &str) -> Vec<GeocodeResult> {
    let url = format!(
        "https://nominatim.openstreetmap.org/search?format=jsonv2&limit=6&q={}",
        percent_encode(query)
    );
    let agent = ureq::Agent::config_builder().build().new_agent();
    let body = match agent
        .get(&url)
        .header(
            "User-Agent",
            "twg-rust-geocode/0.1 (+https://github.com/vinrul/twg-rust)",
        )
        .call()
    {
        Ok(mut response) => {
            let mut bytes = Vec::new();
            if response
                .body_mut()
                .with_config()
                .reader()
                .read_to_end(&mut bytes)
                .is_err()
            {
                return Vec::new();
            }
            bytes
        }
        Err(error) => {
            tracing::warn!(%query, %error, "geocode upstream failed");
            return Vec::new();
        }
    };
    let value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%query, %error, "geocode response was not JSON");
            return Vec::new();
        }
    };
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let label = item["display_name"].as_str()?.to_owned();
                    let latitude = item["lat"].as_str()?.parse().ok()?;
                    let longitude = item["lon"].as_str()?.parse().ok()?;
                    let id = item["place_id"]
                        .as_i64()
                        .map(|value| value.to_string())
                        .unwrap_or_default();
                    Some(GeocodeResult {
                        id,
                        label,
                        latitude,
                        longitude,
                        source: "nominatim".to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
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
