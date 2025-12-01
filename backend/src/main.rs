mod models;

use std::env;

use sqlx::migrate::Migrator;
use thiserror::Error;
use tracing::{debug, error, info, instrument};

static MIGRATOR: Migrator = sqlx::migrate!();

#[derive(Debug, Error)]
enum SetupError {
    #[error("Failed to load environment file: {0}")]
    EnvLoadError(#[from] dotenv::Error),

    #[error("Expected variable not found in environment file: {0}")]
    EnvVarNotFound(#[from] env::VarError),
}

struct Environment {
    path: String,
    variables: Vec<String>,
}

impl Environment {
    fn new(path: &str, variables: &[&str]) -> Self {
        Self {
            path: path.into(),
            variables: variables.iter().map(|&x| x.into()).collect(),
        }
    }

    fn load(&self) -> Result<(), SetupError> {
        debug!("Loading environment variables from file: {}", &self.path);
        dotenv::from_filename(&self.path)?;

        for var in &self.variables {
            env::var(var)?;
            debug!("Loaded environment variable {}", &var);
        }

        Ok(())
    }
}

#[tokio::main]
#[instrument]
async fn main() {
    tracing_subscriber::fmt::init();

    let envs = [
        Environment::new(
            ".env.database",
            &[
                "POSTGRES_USER",
                "POSTGRES_PASSWORD",
                "POSTGRES_DB",
                "DATABASE_URL",
            ],
        ),
        Environment::new(".env.qemu", &["IMAGE_DIR", "OVERLAY_DIR"]),
    ];

    debug!("Loading environment variables.");
    for env in envs {
        if let Err(e) = env.load() {
            error!("{e}");
            return;
        }
    }
    debug!("Loaded environment variables.");

    let database_url = std::env::var("DATABASE_URL").unwrap();

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
