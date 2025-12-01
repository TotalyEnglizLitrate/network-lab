use std::{
    env, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum NodePathError {
    #[error("Invalid path for node: {0}")]
    InvalidPath(#[from] io::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NodeStatus {
    Running,
    Stopped,
}
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Node {
    pub id: Option<Uuid>,
    pub name: String,
    pub status: NodeStatus,
    pub image_path: String,
    pub overlay_path: Option<String>,
    pub vnc_port: Option<u16>,
    pub guacamole_connection_id: Option<String>,
}

impl Node {
    fn get_image_path(&self) -> Result<PathBuf, NodePathError> {
        let full_path = validate_and_resolve_path("IMAGE_BASE_DIR", &self.image_path)?;
        Ok(full_path)
    }

    fn get_overlay_path(&self) -> Result<Option<PathBuf>, NodePathError> {
        if let Some(overlay_path) = &self.overlay_path {
            let full_path = validate_and_resolve_path("OVERLAY_BASE_DIR", overlay_path)?;
            Ok(Some(full_path))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct User {
    pub id: Option<Uuid>,
    pub username: String,
    pub email: String,
    pub password_hash: String,
}

fn validate_and_resolve_path(
    base_dir_env: &str,
    relative_path: &str,
) -> Result<PathBuf, NodePathError> {
    let base_dir = env::var(base_dir_env)
        .map_err(|_| "Unreachable: We checked this at startup")
        .unwrap();

    let full_path = Path::new(&base_dir).join(relative_path).canonicalize()?;

    // Ensure the resolved path is within the base directory (to prevent directory traversal attacks)
    if !full_path.starts_with(Path::new(&base_dir)) || !full_path.exists() {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Path does not exist: {}", relative_path),
        ))?;
    }

    Ok(full_path)
}
