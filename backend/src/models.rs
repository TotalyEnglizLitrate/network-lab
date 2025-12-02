use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ImagePathError {
    #[error("Invalid path: {0}")]
    InvalidPath(#[from] io::Error),
    #[error("Path traversal detected: {0}")]
    PathTraversal(String),
}

/// Represents a disk image that can be a base image or an overlay.
/// Images form a hierarchy where overlays point to their parent (backing) image.
///
/// Example hierarchy:
/// ```text
/// ubuntu-base.qcow2 (parent_id: None)
///     ├── ubuntu-with-docker.qcow2 (parent_id: ubuntu-base)
///     │       └── [Node overlays created at runtime]
///     └── ubuntu-with-nginx.qcow2 (parent_id: ubuntu-base)
///             └── [Node overlays created at runtime]
/// ```
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Image {
    pub id: Uuid,
    pub name: String,
    /// Relative path to the image file within IMAGE_DIR
    pub path: String,
    /// Parent image ID if this is an overlay, None if this is a base image
    pub parent_id: Option<Uuid>,
    /// Description of what this image contains
    pub description: Option<String>,
}

impl Image {
    /// Get the full filesystem path for this image
    pub fn get_full_path(&self, app_state: &AppState) -> Result<PathBuf, ImagePathError> {
        validate_and_resolve_path(app_state.env.get("IMAGE_DIR").unwrap(), &self.path)
    }

    /// Check if this is a base image (has no parent)
    pub fn is_base_image(&self) -> bool {
        self.parent_id.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "text", rename_all = "PascalCase")]
pub enum NodeStatus {
    Running,
    Stopped,
}

/// Represents a virtual machine instance.
/// Each node is based on an Image and has its own runtime overlay for instance-specific changes.
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Node {
    pub id: Uuid,
    pub name: String,
    pub status: NodeStatus,
    /// The image this node is based on (can be a base image or a software layer)
    pub image_id: Uuid,
    /// Path to the per-instance runtime overlay (relative to OVERLAY_DIR)
    /// This captures all changes made while the VM is running
    pub instance_overlay_path: String,
    /// VNC port if VNC is enabled
    pub vnc_port: Option<u16>,
    /// Guacamole connection ID if connected
    pub guacamole_connection_id: Option<String>,
}

impl Node {
    /// Get the full filesystem path for this node's instance overlay
    pub fn get_instance_overlay_path(
        &self,
        app_state: &AppState,
    ) -> Result<PathBuf, ImagePathError> {
        validate_and_resolve_path(
            app_state.env.get("OVERLAY_DIR").unwrap(),
            &self.instance_overlay_path,
        )
    }
}

fn validate_and_resolve_path(
    base_dir: &str,
    relative_path: &str,
) -> Result<PathBuf, ImagePathError> {
    let base_dir = Path::new(base_dir).canonicalize()?;
    let full_path = base_dir.join(relative_path);

    // For new files that don't exist yet, we validate the parent directory
    let path_to_check =
        if full_path.exists() {
            full_path.canonicalize()?
        } else {
            // Canonicalize the parent and append the filename
            let parent = full_path
                .parent()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Path has no parent directory")
                })?
                .canonicalize()?;
            parent.join(full_path.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Path has no filename")
            })?)
        };

    // Ensure the resolved path is within the base directory (to prevent directory traversal attacks)
    if !path_to_check.starts_with(&base_dir) {
        return Err(ImagePathError::PathTraversal(format!(
            "{} is outside the allowed directory",
            relative_path
        )));
    }

    Ok(path_to_check)
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub env: Arc<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

// ============================================================================
// Request/Response DTOs
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateImageRequest {
    pub name: String,
    pub path: String,
    pub parent_id: Option<Uuid>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNodeRequest {
    pub name: String,
    /// ID of the image to base this node on
    pub image_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateVncConnectionRequest {
    pub connection_name: Option<String>,
    pub vnc_host: String,
    pub vnc_port: u16,
}

#[derive(Debug, Serialize)]
pub struct CreateVncConnectionResponse {
    pub connection_name: String,
    pub connection_id: String,
    pub client_url: String,
    pub websocket_url: String,
    pub tunnel_url: String,
}

#[derive(Debug, Serialize)]
pub struct ImageWithAncestors {
    pub image: Image,
    /// Full chain of ancestor images from immediate parent to root base image
    pub ancestors: Vec<Image>,
}

#[derive(Debug, Serialize)]
pub struct NodeWithImage {
    pub node: Node,
    /// The image this node is based on, with its full ancestry chain
    pub image: ImageWithAncestors,
}
