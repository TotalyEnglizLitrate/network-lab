use axum::{
    Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::post,
};
use uuid::Uuid;

use crate::models::{ApiResponse, AppState, CreateNodeRequest, NodeStatus};

/// POST /node - Create a new node

pub async fn create_node(
    State(state): State<AppState>,
    Json(payload): Json<CreateNodeRequest>,
) -> impl IntoResponse {
    Json(ApiResponse::<()>::error("Not yet implemented".into()))
}

/// GET /node - List all nodes

pub async fn list_nodes(State(state): State<AppState>) -> impl IntoResponse {
    Json(ApiResponse::<()>::error("Not yet implemented".into()))
}

/// POST /node/:id/run - Start a node

pub async fn run_node(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    Json(ApiResponse::<()>::error("Not yet implemented".into()))
}

/// POST /node/:id/stop - Stop a node

pub async fn stop_node(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    Json(ApiResponse::<()>::error("Not yet implemented".into()))
}

/// POST /node/:id/wipe - Wipe a node

pub async fn wipe_node(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    Json(ApiResponse::<()>::error("Not yet implemented".into()))
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/node", post(create_node).get(list_nodes))
        .route("/node/:id/run", post(run_node))
        .route("/node/:id/stop", post(stop_node))
        .route("/node/:id/wipe", post(wipe_node))
        .with_state(state)
}
