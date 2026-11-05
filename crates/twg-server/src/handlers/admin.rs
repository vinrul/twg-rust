use axum::Json;
use axum::extract::{Path, Query, State};
use twg_contracts::ListResponse;
use twg_contracts::SingleResponse;
use twg_contracts::categories::Category;
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::private::PaginationQuery;
use crate::handlers::read_store;
use crate::state::AppState;

pub async fn categories(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<ListResponse<Category>>, ApiError> {
    query.validate()?;
    let rows = read_store(&state)?
        .list_categories()
        .await
        .map_err(ApiError::internal)?;
    let needle = query.search.trim().to_lowercase();
    let filtered = rows
        .into_iter()
        .filter(|category| needle.is_empty() || category.name.to_lowercase().contains(&needle))
        .collect::<Vec<_>>();
    let total = filtered.len();
    let offset = query
        .page_number
        .saturating_sub(1)
        .saturating_mul(query.page_size);
    let data = filtered
        .into_iter()
        .skip(offset)
        .take(query.page_size)
        .collect();
    Ok(Json(ListResponse::new(data, total)))
}

pub async fn category_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SingleResponse<Category>>, ApiError> {
    let category = read_store(&state)?
        .get_category(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("category not found"))?;
    Ok(Json(SingleResponse::success(category)))
}
