mod guacamole;
mod models;
mod qemu;
mod routes;

use std::{collections::HashMap, env, sync::Arc};

use sqlx::migrate::Migrator;
use thiserror::Error;
use tracing::{debug, error, info, instrument, trace};
use tracing_subscriber::filter::LevelFilter;

use models::AppState;
use routes::create_router;

static MIGRATOR: Migrator = sqlx::migrate!();

const ENV_SPECS: &'static [&'static str; 17] = &[
    "POSTGRES_USER",
    "POSTGRES_PASSWORD",
    "POSTGRES_HOST",
    "POSTGRES_PORT",
    "IMAGE_DIR",
    "OVERLAY_DIR",
    "BACKEND_DB",
    "BACKEND_HOST",
    "BACKEND_PORT",
    "GUAC_HTTPS",
    "GUAC_HOST",
    "GUAC_PORT",
    "GUAC_TUNNEL_PATH",
    "GUAC_API_PATH",
    "GUAC_CONNECTION_PREFIX",
    "GUAC_USER",
    "GUAC_PASS",
];

#[derive(Debug, Error)]
enum SetupError {
    #[error("Failed to load environment file {file}: {source}")]
    EnvLoadError { file: String, source: dotenv::Error },

    #[error("Expected variable `{0}` not found")]
    EnvVarNotFound(String),
}

fn read_env(name: &str) -> Option<String> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim().to_owned();
            if trimmed.is_empty() {
                None
            } else {
                trace!("Loaded environment variable: {trimmed}");
                Some(trimmed)
            }
        }
        Err(_) => None,
    }
}

fn load_env(
    file: &str,
    specs: &'static [&'static str],
) -> Result<HashMap<String, String>, SetupError> {
    debug!("Loading environment variables from file: {}", file);
    dotenv::from_filename(file).map_err(|err| SetupError::EnvLoadError {
        file: file.into(),
        source: err,
    })?;

    let mut variables = HashMap::new();
    for spec in specs {
        if let Some(val) = read_env(spec) {
            variables.insert(spec.to_string(), val);
        } else {
            Err(SetupError::EnvVarNotFound(spec.to_string()))?;
        }
    }

    Ok(variables)
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

    let mut env = match load_env(".env", ENV_SPECS) {
        Ok(env) => env,
        Err(err) => {
            error!("{err}");
            return;
        }
    };

    let database_url = build_postgres_url(
        env.get("POSTGRES_USER").unwrap(),
        env.get("POSTGRES_PASSWORD").unwrap(),
        env.get("POSTGRES_HOST").unwrap(),
        env.get("POSTGRES_PORT").unwrap(),
        env.get("BACKEND_DB").unwrap(),
    );
    env.insert("DATABASE_URL".into(), database_url.clone());

    env.insert(
        "GUAC_URL".into(),
        format!(
            "http{}://{}:{}/guacamole/",
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

    debug!("Migrations applied successfully.");
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
