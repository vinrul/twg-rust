use axum::Json;
use axum::extract::{Multipart, State};
use twg_contracts::SingleResponse;
use twg_contracts::uploads::{ALLOWED_IMAGE_TYPES, ALLOWED_VIDEO_TYPES, IMAGE_FIELD, VIDEO_FIELD};

use crate::error::ApiError;
use crate::state::AppState;
use crate::uploads::generated_filename;

const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_VIDEO_BYTES: usize = 200 * 1024 * 1024;

pub async fn upload_image(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<SingleResponse<String>>, ApiError> {
    upload(
        &state,
        &mut multipart,
        IMAGE_FIELD,
        ALLOWED_IMAGE_TYPES,
        MAX_IMAGE_BYTES,
    )
    .await
}

pub async fn upload_video(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<SingleResponse<String>>, ApiError> {
    upload(
        &state,
        &mut multipart,
        VIDEO_FIELD,
        ALLOWED_VIDEO_TYPES,
        MAX_VIDEO_BYTES,
    )
    .await
}

async fn upload(
    state: &AppState,
    multipart: &mut Multipart,
    expected_field: &str,
    allowed_types: &[&str],
    max_bytes: usize,
) -> Result<Json<SingleResponse<String>>, ApiError> {
    let mut payload = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(format!("invalid multipart form: {error}")))?
    {
        if field.name() == Some(expected_field) {
            let original_name = field.file_name().unwrap_or("upload").to_owned();
            let bytes = field
                .bytes()
                .await
                .map_err(|error| ApiError::bad_request(format!("invalid upload: {error}")))?;
            payload = Some((original_name, bytes));
            break;
        }
    }
    let (original_name, bytes) =
        payload.ok_or_else(|| ApiError::bad_request(format!("missing {expected_field} file")))?;
    if bytes.is_empty() {
        return Err(ApiError::bad_request("uploaded file is empty"));
    }
    if bytes.len() > max_bytes {
        return Err(ApiError::bad_request(format!(
            "file too large: {} bytes (max {max_bytes})",
            bytes.len()
        )));
    }
    let mime = infer::get(&bytes)
        .map(|kind| kind.mime_type())
        .unwrap_or("application/octet-stream");
    if !allowed_types.contains(&mime) {
        return Err(ApiError::bad_request(format!("invalid file type: {mime}")));
    }

    let filename = generated_filename(&original_name);
    state
        .uploads
        .save(&filename, &bytes)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SingleResponse::success(format!(
        "/uploads/{filename}"
    ))))
}
