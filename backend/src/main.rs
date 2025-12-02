mod guacamole;
mod models;
mod routes;

use std::{collections::HashMap, env, sync::Arc};

use guacamole::GuacamoleBootstrap;
use sqlx::migrate::Migrator;
use thiserror::Error;
use tracing::{debug, error, info, instrument, trace};
use tracing_subscriber::filter::LevelFilter;

use models::AppState;
use routes::create_router;

static MIGRATOR: Migrator = sqlx::migrate!();

const ENV_FILES: [(&str, &[&str]); 4] = [
    (
        ".env.database",
        &[
            "POSTGRES_USER",
            "POSTGRES_PASSWORD",
            "POSTGRES_DB",
            "POSTGRES_PORT",
            "POSTGRES_HOST",
        ],
    ),
    (".env.qemu", &["IMAGE_DIR", "OVERLAY_DIR"]),
    (".env", &["BACKEND_HOST", "BACKEND_PORT"]),
    (
        ".env.guacamole",
        &[
            "GUAC_DB",
            "GUAC_DB_USER",
            "GUAC_DB_PASSWORD",
            "GUAC_HTTPS",
            "GUAC_HOST",
            "GUAC_PORT",
            "GUAC_TUNNEL_PATH",
            "GUAC_API_PATH",
            "GUAC_CONNECTION_PREFIX",
        ],
    ),
];

#[derive(Debug, Error)]
enum SetupError {
    #[error("Failed to load environment file {0}: {1}")]
    EnvLoadError(String, String),

    #[error("Expected variable not found in environment file: {0}")]
    EnvVarNotFound(String),
}

fn load_env(file: &str, variables: &[&str]) -> Result<HashMap<String, String>, SetupError> {
    let mut variables_map = HashMap::new();

    debug!("Loading environment variables from file: {}", file);
    match dotenv::from_filename(file) {
        Ok(_) => debug!("Successfully loaded environment file: {}", file),
        Err(e) => return Err(SetupError::EnvLoadError(file.into(), e.to_string())),
    }

    for &var in variables {
        let value = env::var(var).map_err(|_| SetupError::EnvVarNotFound(var.into()))?;
        trace!("Loaded environment variable {}", &var);
        variables_map.insert(var.into(), value);
    }

    Ok(variables_map)
}

fn load_envs(envs: &[(&str, &[&str])]) -> Result<HashMap<String, String>, SetupError> {
    let mut result = HashMap::new();
    for (file, variables) in envs {
        result.extend(load_env(file, variables)?);
    }
    Ok(result)
}

fn build_postgres_url(
    user: &str,
    password: &str,
    host: &str,
    port: &str,
    database: &str,
) -> String {
    format!(
        "postgres://{}:{}@{}:{}/{}",
        user, password, host, port, database
    )
}

fn parse_log_level(args: &mut env::Args) -> LevelFilter {
    while let Some(arg) = args.next() {
        if arg == "--log-level" {
            return if let Some(level) = args.next() {
                match level.to_lowercase().as_str() {
                    "debug" => LevelFilter::DEBUG,
                    "info" => LevelFilter::INFO,
                    "warn" | "warning" => LevelFilter::WARN,
                    "trace" => LevelFilter::TRACE,
                    "error" => LevelFilter::ERROR,
                    _ => LevelFilter::INFO,
                }
            } else {
                LevelFilter::INFO
            };
        }
    }

    LevelFilter::INFO
}

#[tokio::main]
#[instrument]
async fn main() {
    let log_level = parse_log_level(&mut env::args());
    tracing_subscriber::fmt().with_max_level(log_level).init();

    debug!("Loading environment variables.");

    let mut env = match load_envs(&ENV_FILES) {
        Ok(env) => env,
        Err(e) => {
            error!("{e}");
            return;
        }
    };

    let database_url = build_postgres_url(
        env.get("POSTGRES_USER").unwrap(),
        env.get("POSTGRES_PASSWORD").unwrap(),
        env.get("POSTGRES_HOST").unwrap(),
        env.get("POSTGRES_PORT").unwrap(),
        env.get("POSTGRES_DB").unwrap(),
    );
    env.insert("DATABASE_URL".into(), database_url.clone());

    let guacamole_database_url = build_postgres_url(
        env.get("GUAC_DB_USER").unwrap(),
        env.get("GUAC_DB_PASSWORD").unwrap(),
        env.get("POSTGRES_HOST").unwrap(),
        env.get("POSTGRES_PORT").unwrap(),
        env.get("GUAC_DB").unwrap(),
    );

    env.insert("GUAC_DATABASE_URL".into(), guacamole_database_url);
    env.insert(
        "GUAC_URL".into(),
        format!(
            "http{}://{}:{}/",
            if env.get("GUAC_HTTPS").unwrap() == "1" {
                "s"
            } else {
                ""
            },
            env.get("GUAC_HOST").unwrap(),
            env.get("GUAC_PORT").unwrap(),
        ),
    );

    debug!("Loaded environment variables.");

    if let Err(err) = GuacamoleBootstrap::from_env(&env) {
        error!("Failed to validate Guacamole configuration: {}", err);
        return;
    }

    info!(
        "Configured Guacamole database {} with user {} using shared Postgres instance.",
        env.get("GUAC_DB").unwrap(),
        env.get("GUAC_DB_USER").unwrap()
    );

    debug!(
        "Connecting to the database at {}:{}",
        env.get("POSTGRES_HOST").unwrap(),
        env.get("POSTGRES_PORT").unwrap()
    );

    let pool = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            info!("Successfully connected to the database.");
            pool
        }
        Err(err) => {
            error!("Failed to connect to the database: {}", err);
            return;
        }
    };

    if let Err(err) = MIGRATOR.run(&pool).await {
        error!("Failed to run migrations: {}", err);
        return;
    }

    info!("Migrations applied successfully.");
    info!("Database setup complete.");

    let address = format!(
        "{}:{}",
        env.get("BACKEND_HOST").unwrap(),
        env.get("BACKEND_PORT").unwrap()
    );

    let listener = match tokio::net::TcpListener::bind(&address).await {
        Ok(listener) => {
            info!("Listening on {address}");
            listener
        }
        Err(err) => {
            error!("Failed to bind listener to {address}: {err}");
            return;
        }
    };

    let app = create_router(AppState {
        db: pool,
        env: Arc::new(env),
    });

    if let Err(err) = axum::serve(listener, app).await {
        error!("Server error: {err}");
    }
}
