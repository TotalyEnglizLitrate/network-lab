use std::collections::HashMap;

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::models::{AppState, Node};

#[derive(Debug, Clone)]
pub struct GuacamoleBootstrap {
    base_http_url: String,
    websocket_url: String,
    tunnel_path: String,
    api_path: String,
    connection_prefix: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuacamoleUiDescriptor {
    pub connection_name: String,
    pub connection_key: String,
    pub client_identifier: String,
    pub api_url: String,
    pub client_url: String,
    pub websocket_url: String,
    pub tunnel_url: String,
    pub share_link: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GuacamoleError {
    #[error("missing expected environment variable `{0}`")]
    MissingEnv(&'static str),
    #[error("empty environment variable `{0}`")]
    EmptyEnv(&'static str),
    #[error("environment variable `{key}` had an invalid value: {reason}")]
    InvalidValue { key: &'static str, reason: String },
}

impl GuacamoleBootstrap {
    pub fn from_env(env: &HashMap<String, String>) -> Result<Self, GuacamoleError> {
        let base_http_url = trim_url(&get_required(env, "GUAC_URL")?);
        let tunnel_path = trim_path(&get_required(env, "GUAC_TUNNEL_PATH")?);
        let api_path = trim_path(&get_required(env, "GUAC_API_PATH")?);
        let raw_prefix = get_required(env, "GUAC_CONNECTION_PREFIX")?;
        let connection_prefix = sanitize_identifier(&raw_prefix);
        if connection_prefix.is_empty() {
            return Err(GuacamoleError::InvalidValue {
                key: "GUAC_CONNECTION_PREFIX",
                reason: "expected at least one alphanumeric character".into(),
            });
        }

        let websocket_url = env
            .get("GUAC_WEBSOCKET_URL")
            .map(|value| trim_url(value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| compute_websocket_url(&base_http_url, &tunnel_path));

        Ok(Self {
            base_http_url,
            websocket_url,
            tunnel_path,
            api_path,
            connection_prefix,
        })
    }

    pub fn from_state(state: &AppState) -> Result<Self, GuacamoleError> {
        Self::from_env(state.env.as_ref())
    }

    pub fn validate_database_prerequisites(
        env: &HashMap<String, String>,
    ) -> Result<(), GuacamoleError> {
        for key in ["GUAC_DB", "GUAC_DB_USER", "GUAC_DB_PASSWORD"] {
            match env.get(key) {
                Some(value) if !value.trim().is_empty() => continue,
                Some(_) => return Err(GuacamoleError::EmptyEnv(key)),
                None => return Err(GuacamoleError::MissingEnv(key)),
            }
        }
        Ok(())
    }

    pub fn base_http_url(&self) -> &str {
        self.base_http_url.as_str()
    }

    pub fn websocket_url(&self) -> &str {
        self.websocket_url.as_str()
    }

    pub fn connection_prefix(&self) -> &str {
        self.connection_prefix.as_str()
    }

    pub fn api_url(&self) -> String {
        format!(
            "{}/{}",
            self.base_http_url.trim_end_matches('/'),
            self.api_path
        )
    }

    pub fn tunnel_url(&self) -> String {
        format!(
            "{}/{}",
            self.base_http_url.trim_end_matches('/'),
            self.tunnel_path
        )
    }

    pub fn descriptor_for_connection<S: Into<String>>(
        &self,
        connection_name: S,
    ) -> GuacamoleUiDescriptor {
        let raw_name = connection_name.into();
        let mut connection_key = sanitize_identifier(&raw_name);
        if connection_key.is_empty() {
            connection_key = sanitize_identifier(&Uuid::now_v7().to_string());
        }
        let client_identifier = format!("{}-{}", self.connection_prefix, connection_key);
        let base = self.base_http_url.trim_end_matches('/');
        let client_url = format!("{base}/#/client/{client_identifier}");
        let share_link = format!("{client_url}?GUAC_ID={client_identifier}");
        GuacamoleUiDescriptor {
            connection_name: raw_name,
            connection_key,
            client_identifier: client_identifier.clone(),
            api_url: self.api_url(),
            client_url,
            websocket_url: self.websocket_url.clone(),
            tunnel_url: self.tunnel_url(),
            share_link,
        }
    }

    pub fn descriptor_for_node(&self, node: &Node) -> Option<GuacamoleUiDescriptor> {
        node.guacamole_connection_id
            .as_ref()
            .map(|connection_id| self.descriptor_for_connection(connection_id.clone()))
    }

    pub fn provision_ephemeral_descriptor(&self) -> GuacamoleUiDescriptor {
        self.descriptor_for_connection(Uuid::now_v7().to_string())
    }
}

fn get_required(
    env: &HashMap<String, String>,
    key: &'static str,
) -> Result<String, GuacamoleError> {
    match env.get(key) {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Err(GuacamoleError::EmptyEnv(key))
            } else {
                Ok(trimmed.to_string())
            }
        }
        None => Err(GuacamoleError::MissingEnv(key)),
    }
}

fn trim_path(value: &str) -> String {
    value.trim().trim_matches('/').to_string()
}

fn compute_websocket_url(base_http_url: &str, tunnel_path: &str) -> String {
    let base = base_http_url.trim();
    let (scheme, remainder) = if let Some(rest) = base.strip_prefix("https://") {
        ("wss://", rest)
    } else if let Some(rest) = base.strip_prefix("http://") {
        ("ws://", rest)
    } else {
        ("ws://", base)
    };
    let normalized_remainder = remainder.trim_start_matches('/');
    let normalized_path = tunnel_path.trim_start_matches('/');
    format!("{scheme}{normalized_remainder}/{normalized_path}")
}

fn trim_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn sanitize_identifier(input: &str) -> String {
    let mut intermediate = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            intermediate.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            intermediate.push('-');
        } else {
            intermediate.push('-');
        }
    }
    let mut collapsed = String::with_capacity(intermediate.len());
    let mut prev_hyphen = false;
    for ch in intermediate.chars() {
        if ch == '-' {
            if !prev_hyphen {
                collapsed.push('-');
            }
            prev_hyphen = true;
        } else {
            collapsed.push(ch);
            prev_hyphen = false;
        }
    }
    collapsed.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Node, NodeStatus};
    use uuid::Uuid;

    fn baseline_env() -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("GUAC_URL".into(), "https://example.com/guac/".into());
        env.insert("GUAC_TUNNEL_PATH".into(), "/websocket-tunnel/".into());
        env.insert("GUAC_API_PATH".into(), "/api".into());
        env.insert("GUAC_CONNECTION_PREFIX".into(), "network-lab".into());
        env
    }

    #[test]
    fn requires_presence_of_mandatory_env() {
        let env = HashMap::new();
        let err = GuacamoleBootstrap::from_env(&env).unwrap_err();
        assert_eq!(err, GuacamoleError::MissingEnv("GUAC_URL"));
    }

    #[test]
    fn reads_configuration_from_env() {
        let env = baseline_env();
        let bootstrap = GuacamoleBootstrap::from_env(&env).unwrap();

        assert_eq!(bootstrap.base_http_url(), "https://example.com/guac");
        assert_eq!(
            bootstrap.websocket_url(),
            "wss://example.com/guac/websocket-tunnel"
        );
        assert_eq!(bootstrap.api_url(), "https://example.com/guac/api");
        assert_eq!(
            bootstrap.tunnel_url(),
            "https://example.com/guac/websocket-tunnel"
        );
    }

    #[test]
    fn uses_explicit_websocket_url_when_provided() {
        let mut env = baseline_env();
        env.insert(
            "GUAC_WEBSOCKET_URL".into(),
            "wss://example.com/custom/socket/".into(),
        );

        let bootstrap = GuacamoleBootstrap::from_env(&env).unwrap();
        assert_eq!(bootstrap.websocket_url(), "wss://example.com/custom/socket");
    }

    #[test]
    fn descriptor_sanitizes_connection_name() {
        let env = baseline_env();
        let bootstrap = GuacamoleBootstrap::from_env(&env).unwrap();
        let descriptor = bootstrap.descriptor_for_connection("My Connection/01");

        assert_eq!(descriptor.connection_key, "my-connection-01");
        assert_eq!(descriptor.client_identifier, "network-lab-my-connection-01");
        assert_eq!(
            descriptor.client_url,
            "https://example.com/guac/#/client/network-lab-my-connection-01"
        );
        assert_eq!(
            descriptor.share_link,
            "https://example.com/guac/#/client/network-lab-my-connection-01?GUAC_ID=network-lab-my-connection-01"
        );
    }

    #[test]
    fn validates_database_prerequisites() {
        let mut env = HashMap::new();
        env.insert("GUAC_DB".into(), "guac_db".into());
        env.insert("GUAC_DB_USER".into(), "guac_user".into());
        env.insert("GUAC_DB_PASSWORD".into(), "secret".into());

        assert!(GuacamoleBootstrap::validate_database_prerequisites(&env).is_ok());

        env.insert("GUAC_DB_PASSWORD".into(), "   ".into());
        let err = GuacamoleBootstrap::validate_database_prerequisites(&env).unwrap_err();
        assert!(matches!(err, GuacamoleError::EmptyEnv("GUAC_DB_PASSWORD")));

        env.insert("GUAC_DB_PASSWORD".into(), "secret".into());
        env.remove("GUAC_DB_USER");
        let err = GuacamoleBootstrap::validate_database_prerequisites(&env).unwrap_err();
        assert!(matches!(err, GuacamoleError::MissingEnv("GUAC_DB_USER")));
    }

    #[test]
    fn descriptor_for_node_requires_connection_id() {
        let env = baseline_env();
        let bootstrap = GuacamoleBootstrap::from_env(&env).unwrap();
        let node = Node {
            id: Some(Uuid::now_v7()),
            name: "router".into(),
            status: NodeStatus::Stopped,
            image_path: "images/router.qcow2".into(),
            overlay_path: None,
            vnc_port: Some(5901),
            guacamole_connection_id: Some("router-console".into()),
        };

        assert!(bootstrap.descriptor_for_node(&node).is_some());

        let mut without_connection = node.clone();
        without_connection.guacamole_connection_id = None;
        assert!(bootstrap.descriptor_for_node(&without_connection).is_none());
    }
}
