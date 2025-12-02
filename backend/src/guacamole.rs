use std::collections::HashMap;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::qemu::{self, QemuError, QemuInstance};

#[derive(Debug, thiserror::Error)]
pub enum GuacamoleError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Failed to create connection: {0}")]
    ConnectionFailed(String),
    #[error("QEMU error: {0}")]
    Qemu(#[from] QemuError),
    #[error("VNC is not enabled on the QEMU instance")]
    VncNotEnabled,
}

/// Represents a Guacamole connection with all URLs needed for UI integration
#[derive(Debug, Clone, Serialize)]
pub struct GuacamoleConnection {
    pub connection_name: String,
    pub connection_key: String,
    pub connection_id: String,
    pub client_identifier: String,
    pub api_url: String,
    pub client_url: String,
    pub websocket_url: String,
    pub tunnel_url: String,
    pub vnc_port: u16,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    #[serde(rename = "authToken")]
    auth_token: String,
    #[serde(rename = "dataSource")]
    data_source: String,
}

#[derive(Debug, Serialize)]
struct CreateConnectionRequest {
    name: String,
    #[serde(rename = "parentIdentifier")]
    parent_identifier: String,
    protocol: String,
    parameters: ConnectionParameters,
    attributes: ConnectionAttributes,
}

#[derive(Debug, Serialize)]
struct ConnectionParameters {
    hostname: String,
    port: String,
}

#[derive(Debug, Serialize)]
struct ConnectionAttributes {
    #[serde(rename = "max-connections")]
    max_connections: String,
    #[serde(rename = "max-connections-per-user")]
    max_connections_per_user: String,
}

#[derive(Debug, Deserialize)]
struct CreateConnectionResponse {
    identifier: String,
}

impl GuacamoleConnection {
    /// Create and register a new VNC connection with Guacamole from a running QEMU instance.
    ///
    /// This function will:
    /// 1. Enable VNC on the QEMU instance if not already enabled
    /// 2. Get the VNC connection info from QEMU
    /// 3. Register the VNC connection with Guacamole
    ///
    /// # Arguments
    /// * `env` - Environment variables containing Guacamole configuration
    /// * `connection_name` - Name for the Guacamole connection
    /// * `instance` - Mutable reference to the QEMU instance to bind
    /// * `vnc_display` - Optional VNC display number to use (if VNC needs to be enabled)
    ///
    /// # Returns
    /// A `GuacamoleConnection` with all URLs needed for UI integration
    pub async fn new(
        env: &HashMap<String, String>,
        connection_name: &str,
        instance: &mut QemuInstance,
        vnc_display: Option<u16>,
    ) -> Result<Self, GuacamoleError> {
        // Enable VNC on the QEMU instance if not already enabled
        if instance.vnc_port.is_none() {
            let display = vnc_display.unwrap_or(0);
            qemu::enable_vnc(instance, display).await?;
        }

        // Get VNC connection info from the QEMU instance
        let (vnc_host, vnc_port) = qemu::get_vnc_info(instance)?;

        // Load env and build URL/identifier data
        let env_cfg = Self::build_env_config(env, connection_name);

        let client = Client::new();

        // Authenticate with Guacamole
        let auth_response = Self::authenticate(
            &client,
            &env_cfg.api_url,
            &env_cfg.username,
            &env_cfg.password,
        )
        .await?;

        // Create VNC connection in Guacamole
        let create_response = Self::create_connection(
            &client,
            &env_cfg.api_url,
            &auth_response,
            connection_name,
            &vnc_host,
            vnc_port,
        )
        .await?;

        let client_url = format!(
            "{}/#/client/{}",
            env_cfg.base_http_url, env_cfg.client_identifier
        );

        Ok(Self {
            connection_name: connection_name.to_string(),
            connection_key: env_cfg.connection_key,
            connection_id: create_response.identifier,
            client_identifier: env_cfg.client_identifier,
            api_url: env_cfg.api_url,
            client_url,
            websocket_url: env_cfg.websocket_url,
            tunnel_url: env_cfg.tunnel_url,
            vnc_port,
        })
    }

    /// Create a Guacamole connection from explicit VNC host and port.
    ///
    /// Use this when you already have VNC running and just need to register it with Guacamole.
    ///
    /// # Arguments
    /// * `env` - Environment variables containing Guacamole configuration
    /// * `connection_name` - Name for the Guacamole connection
    /// * `vnc_host` - The VNC server hostname/IP
    /// * `vnc_port` - The VNC server port
    ///
    /// # Returns
    /// A `GuacamoleConnection` with all URLs needed for UI integration
    pub async fn from_vnc(
        env: &HashMap<String, String>,
        connection_name: &str,
        vnc_host: &str,
        vnc_port: u16,
    ) -> Result<Self, GuacamoleError> {
        // Load env and build URL/identifier data
        let env_cfg = Self::build_env_config(env, connection_name);

        let client = Client::new();

        // Authenticate with Guacamole
        let auth_response = Self::authenticate(
            &client,
            &env_cfg.api_url,
            &env_cfg.username,
            &env_cfg.password,
        )
        .await?;

        // Create VNC connection in Guacamole
        let create_response = Self::create_connection(
            &client,
            &env_cfg.api_url,
            &auth_response,
            connection_name,
            vnc_host,
            vnc_port,
        )
        .await?;

        let client_url = format!(
            "{}/#/client/{}",
            env_cfg.base_http_url, env_cfg.client_identifier
        );

        Ok(Self {
            connection_name: connection_name.to_string(),
            connection_key: env_cfg.connection_key,
            connection_id: create_response.identifier,
            client_identifier: env_cfg.client_identifier,
            api_url: env_cfg.api_url,
            client_url,
            websocket_url: env_cfg.websocket_url,
            tunnel_url: env_cfg.tunnel_url,
            vnc_port,
        })
    }

    /// Delete this connection from Guacamole
    pub async fn delete(&self, env: &HashMap<String, String>) -> Result<(), GuacamoleError> {
        let username = env.get("GUAC_USER").unwrap();
        let password = env.get("GUAC_PASS").unwrap();

        let client = Client::new();

        let auth_response: AuthResponse = client
            .post(format!("{}/tokens", self.api_url))
            .form(&[("username", username), ("password", password)])
            .send()
            .await?
            .error_for_status()
            .map_err(|_| GuacamoleError::AuthFailed)?
            .json()
            .await?;

        client
            .delete(format!(
                "{}/session/data/{}/connections/{}",
                self.api_url, auth_response.data_source, self.connection_id
            ))
            .header("Guacamole-Token", &auth_response.auth_token)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| GuacamoleError::ConnectionFailed(e.to_string()))?;

        Ok(())
    }

    /// Delete this connection from Guacamole and disable VNC on the QEMU instance
    pub async fn delete_with_vnc_disable(
        &self,
        env: &HashMap<String, String>,
        instance: &mut QemuInstance,
    ) -> Result<(), GuacamoleError> {
        // First delete the Guacamole connection
        self.delete(env).await?;

        // Then disable VNC on the QEMU instance
        qemu::disable_vnc(instance).await?;

        Ok(())
    }

    // Private helpers to reduce duplication between `new` and `from_vnc`.

    fn build_env_config(env: &HashMap<String, String>, connection_name: &str) -> EnvConfig {
        let base_http_url = env
            .get("GUAC_URL")
            .unwrap()
            .trim()
            .trim_end_matches('/')
            .to_string();
        let tunnel_path = env
            .get("GUAC_TUNNEL_PATH")
            .unwrap()
            .trim()
            .trim_matches('/')
            .to_string();
        let api_path = env
            .get("GUAC_API_PATH")
            .unwrap()
            .trim()
            .trim_matches('/')
            .to_string();
        let connection_prefix = sanitize_identifier(env.get("GUAC_CONNECTION_PREFIX").unwrap());
        let username = env.get("GUAC_ADMIN_USER").unwrap().to_string();
        let password = env.get("GUAC_ADMIN_PASS").unwrap().to_string();

        let connection_key = sanitize_identifier(connection_name);
        let client_identifier = format!("{}-{}", connection_prefix, connection_key);
        let api_url = format!("{}/{}", base_http_url, api_path);
        let tunnel_url = format!("{}/{}", base_http_url, tunnel_path);
        let websocket_url = compute_websocket_url(&base_http_url, &tunnel_path);

        EnvConfig {
            base_http_url,
            tunnel_path,
            api_path,
            connection_prefix,
            username,
            password,
            connection_key,
            client_identifier,
            api_url,
            tunnel_url,
            websocket_url,
        }
    }

    async fn authenticate(
        client: &Client,
        api_url: &str,
        username: &str,
        password: &str,
    ) -> Result<AuthResponse, GuacamoleError> {
        let auth_response: AuthResponse = client
            .post(format!("{}/tokens", api_url))
            .form(&[("username", username), ("password", password)])
            .send()
            .await?
            .error_for_status()
            .map_err(|_| GuacamoleError::AuthFailed)?
            .json()
            .await?;
        Ok(auth_response)
    }

    async fn create_connection(
        client: &Client,
        api_url: &str,
        auth_response: &AuthResponse,
        connection_name: &str,
        vnc_host: &str,
        vnc_port: u16,
    ) -> Result<CreateConnectionResponse, GuacamoleError> {
        let create_request = CreateConnectionRequest {
            name: connection_name.to_string(),
            parent_identifier: "ROOT".into(),
            protocol: "vnc".into(),
            parameters: ConnectionParameters {
                hostname: vnc_host.to_string(),
                port: vnc_port.to_string(),
            },
            attributes: ConnectionAttributes {
                max_connections: "".to_string(),
                max_connections_per_user: "".to_string(),
            },
        };

        let create_response: CreateConnectionResponse = client
            .post(format!(
                "{}/session/data/{}/connections",
                api_url, auth_response.data_source
            ))
            .header("Guacamole-Token", &auth_response.auth_token)
            .json(&create_request)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| GuacamoleError::ConnectionFailed(e.to_string()))?
            .json()
            .await?;

        Ok(create_response)
    }
}

/// Small struct returned by `build_env_config` to carry computed values.
struct EnvConfig {
    base_http_url: String,
    tunnel_path: String,
    api_path: String,
    connection_prefix: String,
    username: String,
    password: String,
    connection_key: String,
    client_identifier: String,
    api_url: String,
    tunnel_url: String,
    websocket_url: String,
}

fn compute_websocket_url(base_http_url: &str, tunnel_path: &str) -> String {
    let (scheme, remainder) = if let Some(rest) = base_http_url.strip_prefix("https://") {
        ("wss://", rest)
    } else if let Some(rest) = base_http_url.strip_prefix("http://") {
        ("ws://", rest)
    } else {
        ("ws://", base_http_url)
    };
    format!(
        "{}{}/{}",
        scheme,
        remainder.trim_matches('/'),
        tunnel_path.trim_matches('/')
    )
}

fn sanitize_identifier(input: &str) -> String {
    let intermediate: String = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let mut result = String::with_capacity(intermediate.len());
    let mut prev_hyphen = false;
    for ch in intermediate.chars() {
        if ch == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(ch);
            prev_hyphen = false;
        }
    }
    result.trim_matches('-').to_string()
}
