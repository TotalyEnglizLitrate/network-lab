mod models;

use std::{collections::HashMap, env};

use sqlx::migrate::Migrator;
use thiserror::Error;
use tracing::{debug, error, info, instrument};

static MIGRATOR: Migrator = sqlx::migrate!();

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
        variables_map.insert(
            var.into(),
            match env::var(var) {
                Ok(value) => value,
                Err(_) => return Err(SetupError::EnvVarNotFound(var.into())),
            },
        );
        debug!("Loaded environment variable {}", &var);
    }

    Ok(variables_map)
}

#[tokio::main]
#[instrument]
async fn main() {
    tracing_subscriber::fmt::init();

    debug!("Loading environment variables.");

    let db_env = match load_env(
        ".env.database",
        &[
            "POSTGRES_USER",
            "POSTGRES_PASSWORD",
            "POSTGRES_DB",
            "POSTGRES_PORT",
            "POSTGRES_HOST",
        ],
    ) {
        Ok(env) => env,
        Err(e) => {
            error!("{e}");
            return;
        }
    };

    let qemu_env = match load_env(".env.qemu", &["IMAGE_DIR", "OVERLAY_DIR"]) {
        Ok(env) => env,
        Err(e) => {
            error!("{e}");
            return;
        }
    };

    debug!("Loaded environment variables.");

    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        db_env.get("POSTGRES_USER").unwrap(),
        db_env.get("POSTGRES_PASSWORD").unwrap(),
        db_env.get("POSTGRES_HOST").unwrap(),
        db_env.get("POSTGRES_PORT").unwrap(),
        db_env.get("POSTGRES_DB").unwrap()
    );

    debug!(
        "Connecting to the database at {}:{}",
        db_env.get("POSTGRES_HOST").unwrap(),
        db_env.get("POSTGRES_PORT").unwrap()
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
}
