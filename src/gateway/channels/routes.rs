//! HTTP routes for channel management.

use super::auth::AuthRecord;
use super::manager::ChannelManager;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Request to approve a pairing code
#[derive(Debug, Deserialize)]
pub struct PairRequest {
    pub code: String,
}

/// Response for successful pairing
#[derive(Debug, Serialize)]
pub struct PairResponse {
    pub success: bool,
    pub channel: String,
    pub chat_id: String,
    pub message: String,
}

/// Channel status response
#[derive(Debug, Serialize)]
pub struct ChannelStatusResponse {
    pub plugins: Vec<String>,
    pub active_sessions: usize,
    pub pending_pairings: usize,
    pub authorizations: usize,
}

/// Authorization list response
#[derive(Debug, Serialize)]
pub struct AuthorizationsResponse {
    pub authorizations: Vec<AuthRecord>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

/// Create the channel routes
pub fn routes() -> Router<Arc<ChannelManager>> {
    Router::new()
        .route("/", get(status_handler))
        .route("/pair", post(pair_handler))
        .route("/authorizations", get(list_authorizations_handler))
        .route("/authorizations/:id", delete(revoke_authorization_handler))
}

/// GET /channels - Get channel status
async fn status_handler(State(manager): State<Arc<ChannelManager>>) -> impl IntoResponse {
    let status = manager.status().await;
    Json(ChannelStatusResponse {
        plugins: status.plugins,
        active_sessions: status.active_sessions,
        pending_pairings: status.pending_pairings,
        authorizations: status.authorizations,
    })
}

/// POST /channels/pair - Approve a pairing request
async fn pair_handler(
    State(manager): State<Arc<ChannelManager>>,
    Json(request): Json<PairRequest>,
) -> impl IntoResponse {
    match manager.auth().approve_pairing(&request.code) {
        Ok(target) => (
            StatusCode::OK,
            Json(PairResponse {
                success: true,
                channel: target.channel,
                chat_id: target.chat_id,
                message: "Pairing successful".to_string(),
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(PairResponse {
                success: false,
                channel: String::new(),
                chat_id: String::new(),
                message: e.to_string(),
            }),
        ),
    }
}

/// GET /channels/authorizations - List all authorizations
async fn list_authorizations_handler(
    State(manager): State<Arc<ChannelManager>>,
) -> impl IntoResponse {
    let authorizations = manager.auth().list_authorizations();
    Json(AuthorizationsResponse { authorizations })
}

/// DELETE /channels/authorizations/:id - Revoke an authorization
async fn revoke_authorization_handler(
    State(manager): State<Arc<ChannelManager>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if manager.auth().revoke_by_id(&id) {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "message": "Authorization revoked"
            })),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "error": "not_found",
                "message": "Authorization not found"
            })),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_pair_request_deserialize() {
        let json = r#"{"code": "ABC123"}"#;
        let req: super::PairRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.code, "ABC123");
    }
}
