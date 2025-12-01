use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;
use uuid::Uuid;

use crate::Environment;

#[derive(Debug, Error)]
pub enum NodePathError {
    #[error("Invalid path for node: {0}")]
    InvalidPath(#[from] io::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "PascalCase")]
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
    pub fn get_image_path(&self, env: &Environment) -> Result<PathBuf, NodePathError> {
        let full_path =
            validate_and_resolve_path(env.variables.get("IMAGE_DIR").unwrap(), &self.image_path)?;
        Ok(full_path)
    }

    pub fn get_overlay_path(&self, env: &Environment) -> Result<Option<PathBuf>, NodePathError> {
        if let Some(overlay_path) = &self.overlay_path {
            let full_path =
                validate_and_resolve_path(env.variables.get("OVERLAY_DIR").unwrap(), overlay_path)?;
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
    base_dir: &str,
    relative_path: &str,
) -> Result<PathBuf, NodePathError> {
    let base_dir = Path::new(base_dir).canonicalize()?;

    let full_path = base_dir.join(relative_path).canonicalize()?;

    // Ensure the resolved path is within the base directory (to prevent directory traversal attacks)
    if !full_path.starts_with(&base_dir) {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Path traversal detected: {} is outside the allowed directory",
                relative_path
            ),
        ))?;
    }

    Ok(full_path)
}
