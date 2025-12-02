use axum::{
    Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::post,
};
use uuid::Uuid;

use crate::guacamole::GuacamoleConnection;
use crate::models::{
    ApiResponse, AppState, CreateNodeRequest, CreateVncConnectionRequest,
    CreateVncConnectionResponse, NodeStatus,
};

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

/// POST /node/{id}/run - Start a node
pub async fn run_node(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    Json(ApiResponse::<()>::error("Not yet implemented".into()))
}

/// POST /node/{id}/stop - Stop a node
pub async fn stop_node(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    Json(ApiResponse::<()>::error("Not yet implemented".into()))
}

/// POST /node/{id}/wipe - Wipe a node
pub async fn wipe_node(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    Json(ApiResponse::<()>::error("Not yet implemented".into()))
}

/// POST /vnc - Create a VNC connection and bind it to Guacamole
pub async fn create_vnc_connection(
    State(state): State<AppState>,
    Json(payload): Json<CreateVncConnectionRequest>,
) -> impl IntoResponse {
    let connection_name = payload
        .connection_name
        .as_deref()
        .unwrap_or("vnc-connection");

    match GuacamoleConnection::from_vnc(
        &state.env,
        connection_name,
        &payload.vnc_host,
        payload.vnc_port,
    )
    .await
    {
        Ok(connection) => Json(ApiResponse::ok(CreateVncConnectionResponse {
            connection_name: connection.connection_name,
            connection_id: connection.connection_id,
            client_url: connection.client_url,
            websocket_url: connection.websocket_url,
            tunnel_url: connection.tunnel_url,
        }))
        .into_response(),
        Err(e) => Json(ApiResponse::<()>::error(format!(
            "Failed to create VNC connection: {}",
            e
        )))
        .into_response(),
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/node", post(create_node).get(list_nodes))
        .route("/node/{id}/run", post(run_node))
        .route("/node/{id}/stop", post(stop_node))
        .route("/node/{id}/wipe", post(wipe_node))
        .route("/vnc", post(create_vnc_connection))
        .with_state(state)
}
