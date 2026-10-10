use anyhow::{Context as _, Result, bail};
use clap::{Parser, Subcommand};
use config::{Config, Environment, File};
use cords_protocol::ServerOrigin;
use cords_server_core::{AppState, ServerIdentity, ownership, router};
use cords_storage::PostgresStore;
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tokio::net::TcpListener;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "cords-server", version, about = "Cords messaging server")]
struct Cli {
    #[arg(long, default_value = "deploy/server.toml")]
    config: PathBuf,
    #[arg(long)]
    listen: Option<SocketAddr>,
    #[arg(long)]
    public_origin: Option<String>,
    #[arg(long)]
    server_name: Option<String>,
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long)]
    migrations_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start the HTTP server.
    Serve {
        /// Apply pending database migrations before serving.
        #[arg(long, conflicts_with = "require_current_schema")]
        migrate: bool,
        /// Refuse to start unless the schema is already current.
        #[arg(long)]
        require_current_schema: bool,
    },
    /// Apply pending database migrations and exit.
    Migrate,
    /// Manage the server-local ownership bootstrap secret.
    OwnershipBootstrap {
        #[command(subcommand)]
        command: OwnershipBootstrapCommand,
    },
    /// Check the local readiness endpoint and exit.
    Healthcheck {
        #[arg(long, default_value = "http://127.0.0.1:4848/health/ready")]
        url: String,
    },
}

#[derive(Debug, Subcommand)]
enum OwnershipBootstrapCommand {
    /// Initialize an unowned database, including an explicitly acknowledged populated legacy one.
    Initialize,
    /// Rotate the one-time code while the server remains unclaimed.
    Rotate,
}

#[derive(Clone, Debug, Deserialize)]
struct Settings {
    server: ServerSettings,
    database: DatabaseSettings,
    #[serde(default)]
    authentication: AuthenticationSettings,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AuthenticationSettings {
    challenge_seconds: u64,
    session_seconds: u64,
}
impl Default for AuthenticationSettings {
    fn default() -> Self {
        Self {
            challenge_seconds: 60,
            session_seconds: 900,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct ServerSettings {
    listen: SocketAddr,
    public_origin: String,
    name: String,
    data_dir: PathBuf,
    #[serde(default)]
    join_policy: cords_server_core::messaging::JoinPolicy,
}

#[derive(Clone, Debug, Deserialize)]
struct DatabaseSettings {
    url: String,
    migrations_dir: PathBuf,
}

impl Settings {
    fn load(cli: &Cli) -> Result<Self> {
        let config = Config::builder()
            .add_source(File::from(cli.config.clone()).required(true))
            .add_source(
                Environment::with_prefix("CORDS")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()
            .with_context(|| {
                format!("failed to load configuration from {}", cli.config.display())
            })?;
        let mut settings: Self = config
            .try_deserialize()
            .context("configuration is invalid")?;
        if let Some(value) = cli.listen {
            settings.server.listen = value;
        }
        if let Some(value) = &cli.public_origin {
            settings.server.public_origin.clone_from(value);
        }
        if let Some(value) = &cli.server_name {
            settings.server.name.clone_from(value);
        }
        if let Some(value) = &cli.data_dir {
            settings.server.data_dir.clone_from(value);
        }
        if let Some(value) = &cli.database_url {
            settings.database.url.clone_from(value);
        }
        if let Some(value) = &cli.migrations_dir {
            settings.database.migrations_dir.clone_from(value);
        }

        settings.validate()?;
        Ok(settings)
    }

    fn validate(&self) -> Result<()> {
        ServerOrigin::parse(&self.server.public_origin)
            .context("server.public_origin must be a bare HTTPS origin")?;
        let name_length = self.server.name.chars().count();
        if name_length == 0 || name_length > 100 {
            bail!("server.name must contain between 1 and 100 Unicode scalar values");
        }
        if self.database.url.trim().is_empty() {
            bail!("database.url must not be empty");
        }
        Ok(())
    }
}

async fn load_server_identity(
    store: &PostgresStore,
    data_dir: &std::path::Path,
) -> Result<ServerIdentity> {
    let identity_path = data_dir.join("server-signing.key");
    if store.server_identity().await?.is_some() && !identity_path.exists() {
        bail!(
            "server database has an established signing identity but its key is missing; restore the matching server-data volume"
        );
    }
    let (identity, created) = ServerIdentity::load_or_create(&identity_path)
        .context("failed to load persistent server identity")?;
    store.bind_server_identity(&identity.server_id()).await?;
    if created {
        warn!(
            server_id = %identity.server_id(),
            path = %identity_path.display(),
            "created a new persistent Cords server identity; verify that the data volume is durable"
        );
    } else {
        info!(server_id = %identity.server_id(), "loaded persistent Cords server identity");
    }
    Ok(identity)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Command::Healthcheck { url } = &cli.command {
        return healthcheck(url).await;
    }

    init_tracing();
    let settings = Settings::load(&cli)?;
    let store = PostgresStore::connect(&settings.database.url)
        .await
        .context("failed to connect to PostgreSQL")?;

    match cli.command {
        Command::Migrate => {
            store
                .migrate(&settings.database.migrations_dir)
                .await
                .context("failed to apply PostgreSQL migrations")?;
            info!("database migrations are current");
            Ok(())
        }
        Command::OwnershipBootstrap { command } => {
            store
                .require_current()
                .await
                .context("database schema is not current")?;
            let identity = load_server_identity(&store, &settings.server.data_dir).await?;
            let code = match command {
                OwnershipBootstrapCommand::Initialize => {
                    match ownership::initialize_legacy(&store, &identity.server_id()).await? {
                        ownership::BootstrapOutcome::Generated(code) => code,
                        ownership::BootstrapOutcome::Existing => {
                            bail!(
                                "ownership bootstrap is already initialized; use `rotate` only while it is unclaimed"
                            )
                        }
                    }
                }
                OwnershipBootstrapCommand::Rotate => {
                    ownership::rotate(&store, &identity.server_id()).await?
                }
            };
            warn!(claim_code = %code.as_str(), "Cords ownership bootstrap code; store it securely because it will not be shown again");
            Ok(())
        }
        Command::Serve {
            migrate,
            require_current_schema,
        } => {
            if migrate {
                store
                    .migrate(&settings.database.migrations_dir)
                    .await
                    .context("failed to apply PostgreSQL migrations")?;
            }
            if require_current_schema || !migrate {
                store
                    .require_current()
                    .await
                    .context("database schema is not current")?;
            }
            store
                .health()
                .await
                .context("PostgreSQL readiness check failed")?;

            let identity = load_server_identity(&store, &settings.server.data_dir).await?;
            if let ownership::BootstrapOutcome::Generated(code) =
                ownership::ensure(&store, &identity.server_id()).await?
            {
                warn!(claim_code = %code.as_str(), "Cords ownership bootstrap code; store it securely because it will not be shown again");
            }

            let metadata = identity
                .signed_metadata_with_policy(
                    &settings.server.name,
                    settings.server.join_policy.as_str(),
                )
                .context("failed to sign server metadata")?;
            let service = cords_server_core::messaging::Service::with_policy(
                store.clone(),
                identity,
                settings.authentication.challenge_seconds,
                settings.authentication.session_seconds,
                settings.server.join_policy,
            )
            .map_err(|_| anyhow::anyhow!("authentication lifetime configuration is invalid"))?;
            let app = router(AppState::new(metadata).with_store(store))
                .merge(cords_server_core::messaging::router(service));
            let listener = TcpListener::bind(settings.server.listen)
                .await
                .with_context(|| format!("failed to bind {}", settings.server.listen))?;
            info!(listen = %settings.server.listen, public_origin = %settings.server.public_origin, "Cords server is ready");
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .context("HTTP server failed")
        }
        Command::Healthcheck { .. } => unreachable!("healthcheck returned before configuration"),
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_current_span(false)
        .with_span_list(false)
        .init();
}

async fn healthcheck(url: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to initialize healthcheck client")?;
    let response = client
        .get(url)
        .send()
        .await
        .context("readiness endpoint is unavailable")?;
    if response.status() != reqwest::StatusCode::NO_CONTENT {
        bail!("readiness endpoint returned {}", response.status());
    }
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            warn!(%error, "failed to install Ctrl+C shutdown handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => warn!(%error, "failed to install termination shutdown handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    info!("shutdown requested");
}
