use serde::{Deserialize, Serialize};

pub const API_SUCCESS_CODE: u16 = 200;
pub const API_CREATED_CODE: u16 = 201;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SingleResponse<T> {
    pub code: u16,
    pub message: String,
    pub data: T,
}

impl<T> SingleResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: API_SUCCESS_CODE,
            message: "success".to_owned(),
            data,
        }
    }

    pub fn created(data: T) -> Self {
        Self {
            code: API_CREATED_CODE,
            message: "success".to_owned(),
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub code: u16,
    pub message: String,
    pub data: Vec<T>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub total: usize,
}

impl<T> ListResponse<T> {
    pub fn new(data: Vec<T>, total: usize) -> Self {
        Self {
            code: API_SUCCESS_CODE,
            message: "success".to_owned(),
            data,
            total,
        }
    }
}

const fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: u16,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageResponse {
    pub code: u16,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

impl HealthResponse {
    pub fn ok() -> Self {
        Self {
            status: "ok".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorResponse, HealthResponse, ListResponse, MessageResponse, SingleResponse};
    use serde_json::json;

    #[test]
    fn single_response_matches_baseline_envelope() {
        let response = SingleResponse::success(42_u32);
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({"code": 200, "message": "success", "data": 42})
        );
    }

    #[test]
    fn list_response_omits_zero_total() {
        let response: ListResponse<u32> = ListResponse::new(Vec::new(), 0);
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({"code": 200, "message": "success", "data": []})
        );
    }

    #[test]
    fn error_response_matches_baseline_shape() {
        let response = ErrorResponse {
            code: 403,
            message: "forbidden role".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({"code": 403, "message": "forbidden role"})
        );
    }

    #[test]
    fn message_response_has_no_data_field() {
        let response = MessageResponse {
            code: 200,
            message: "success".to_owned(),
        };
        let value = serde_json::to_value(response).unwrap();
        assert!(value.get("data").is_none());
    }

    #[test]
    fn health_response_matches_server() {
        assert_eq!(
            serde_json::to_value(HealthResponse::ok()).unwrap(),
            json!({"status": "ok"})
        );
    }
}
