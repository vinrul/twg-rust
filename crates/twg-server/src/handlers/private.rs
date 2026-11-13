use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use serde::Deserialize;
use twg_auth::AuthContext;
use twg_contracts::ListResponse;
use twg_contracts::SingleResponse;
use twg_contracts::categories::Category;
use twg_contracts::operations::{HomeOperation, Operation};
use twg_contracts::satkers::Satker;
use twg_contracts::users::User;
use twg_domain::Role;
use twg_storage::{SatkerScope, UserListScope};
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::read_store;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(rename = "pageSize")]
    pub page_size: usize,
    #[serde(rename = "pageNumber")]
    pub page_number: usize,
    #[serde(default)]
    pub search: String,
}

impl PaginationQuery {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.page_size == 0 {
            return Err(ApiError::bad_request("pageSize must be greater than 0"));
        }
        if self.page_number == 0 {
            return Err(ApiError::bad_request("pageNumber must be greater than 0"));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ActiveQuery {
    active: Option<String>,
}

pub async fn operations(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<ListResponse<Operation>>, ApiError> {
    query.validate()?;
    let (data, total) = read_store(&state)?
        .list_operations(
            satker_scope(&context),
            &query.search,
            query.page_size,
            query.page_number,
        )
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, total)))
}

pub async fn operation_detail(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<SingleResponse<Operation>>, ApiError> {
    let operation = read_store(&state)?
        .get_operation(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("operasi not found"))?;
    if !context.can_access_satker(operation.satker_id) {
        return Err(match context.subject.role {
            Role::User => ApiError::forbidden("cannot access satker-assigned operasi"),
            _ => ApiError::forbidden("cannot access operasi from different satker"),
        });
    }
    Ok(Json(SingleResponse::success(operation)))
}

pub async fn home_operations(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
) -> Result<Json<ListResponse<HomeOperation>>, ApiError> {
    let data = read_store(&state)?
        .home_operations(satker_scope(&context))
        .await
        .map_err(ApiError::internal)?;
    let total = data.len();
    Ok(Json(ListResponse::new(data, total)))
}

pub async fn users(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
) -> Result<Json<ListResponse<User>>, ApiError> {
    let scope = match (context.subject.role, context.subject.satker_id) {
        (Role::AdminSatker, Some(satker_id)) => UserListScope::Satker(satker_id),
        _ => UserListScope::AllExceptAdmin,
    };
    let data = read_store(&state)?
        .list_users(scope)
        .await
        .map_err(ApiError::internal)?;
    let total = data.len();
    Ok(Json(ListResponse::new(data, total)))
}

pub async fn satkers(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    Query(query): Query<ActiveQuery>,
) -> Result<Json<ListResponse<Satker>>, ApiError> {
    if !matches!(
        context.subject.role,
        Role::Admin | Role::AdminMonitor | Role::AdminSatker | Role::UserSatker
    ) {
        return Err(ApiError::forbidden("forbidden"));
    }
    let scope = match (context.subject.role, context.subject.satker_id) {
        (Role::AdminSatker | Role::UserSatker, Some(satker_id)) => SatkerScope::Satker(satker_id),
        _ => SatkerScope::All,
    };
    let active = query.active.as_deref().map(|value| value == "true");
    let data = read_store(&state)?
        .list_satkers(scope, active)
        .await
        .map_err(ApiError::internal)?;
    let total = data.len();
    Ok(Json(ListResponse::new(data, total)))
}

pub async fn categories(
    State(state): State<AppState>,
) -> Result<Json<ListResponse<Category>>, ApiError> {
    let data = read_store(&state)?
        .list_categories()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(ListResponse::new(data, 0)))
}

fn satker_scope(context: &AuthContext) -> SatkerScope {
    match context.subject.role {
        Role::Admin | Role::AdminMonitor => SatkerScope::All,
        Role::User | Role::BigScreen => SatkerScope::Unscoped,
        Role::AdminSatker | Role::UserSatker => context
            .subject
            .satker_id
            .map(SatkerScope::Satker)
            .unwrap_or(SatkerScope::All),
    }
}
